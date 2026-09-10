//! Parse the LEGI bulk archive (DILA) into indexable code articles.
//!
//! LEGI ships as one tar.gz holding ~5.25 million small XML files. Extracting
//! them would create as many inodes for no benefit, so the archive is streamed
//! once and only the files we want are parsed.
//!
//! Only `code_en_vigueur/**/article/**.xml` is of interest: that tree holds the
//! articles of the 144 codes currently in force. It still contains *every*
//! version of each article, so a version whose `ETAT` is not `VIGUEUR` is
//! skipped — otherwise the index would show superseded law.

use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::refs::reference_key;
use crate::transform::strip_html;

/// Only articles of in-force codes live under this prefix.
const CODE_PREFIX: &str = "legi/global/code_et_TNC_en_vigueur/code_en_vigueur/";

/// LEGI marks "no end date" with this sentinel.
const NO_END: &str = "2999-01-01";

/// One article of a French code, as indexed.
#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Article {
    /// `LEGIARTI…`
    pub id: String,
    /// Code title, e.g. "Code de commerce".
    pub code: String,
    /// `LEGITEXT…` of the code.
    pub code_id: String,
    /// Article number as LEGI states it, e.g. `L110-1`.
    pub number: String,
    /// Human reference, e.g. "Article L110-1 du Code de commerce".
    pub reference: String,
    /// Join key shared with a decision's `visa_refs`, e.g. `code-de-commerce:L110-1`.
    pub reference_key: String,
    /// Section path within the code, outermost first.
    pub hierarchy: Vec<String>,
    /// Innermost section, handy as a facet.
    pub section: String,
    pub text: String,
    pub text_length: usize,
    /// Date this version came into force.
    pub date_debut: String,
    pub date_debut_timestamp: i64,
    pub year: i32,
    pub url: String,
}

impl Article {
    pub fn approx_size(&self) -> usize {
        self.text.len() + self.reference.len() + 512
    }
}

/// Fields collected while walking one article's XML.
#[derive(Default)]
struct Raw {
    id: String,
    num: String,
    etat: String,
    date_debut: String,
    code: String,
    code_id: String,
    nature: String,
    hierarchy: Vec<String>,
    text: String,
}

/// Parse one article XML. Returns `None` unless it is an in-force article of a
/// code with actual content.
pub fn parse_article(xml: &str) -> Option<Article> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);

    let mut raw = Raw::default();
    // Which element's text we are currently accumulating.
    let mut field: Option<&'static str> = None;
    // Depth inside BLOC_TEXTUEL/CONTENU, whose content is an HTML fragment.
    let mut in_content = false;
    let mut in_meta_article = false;
    let mut buf = String::new();

    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name();
                match name.as_ref() {
                    "META_ARTICLE" => in_meta_article = true,
                    "ID" if raw.id.is_empty() => field = Some("id"),
                    "NUM" if in_meta_article => field = Some("num"),
                    "ETAT" if in_meta_article => field = Some("etat"),
                    "DATE_DEBUT" if in_meta_article => field = Some("date_debut"),
                    "TEXTE" => {
                        for a in e.attributes().flatten() {
                            match a.key.as_ref() {
                                "cid" => raw.code_id = a.value.to_string(),
                                "nature" => raw.nature = a.value.to_string(),
                                _ => {}
                            }
                        }
                    }
                    "TITRE_TXT" if raw.code.is_empty() => field = Some("code"),
                    "TITRE_TM" => field = Some("tm"),
                    "BLOC_TEXTUEL" => in_content = true,
                    _ => {}
                }
                buf.clear();
            }
            Ok(Event::Text(t)) => {
                let text = t.xml10_content();
                if field.is_some() {
                    buf.push_str(&text);
                } else if in_content {
                    raw.text.push_str(&text);
                }
            }
            // `<br/>` inside the content marks a line break.
            Ok(Event::Empty(e)) if in_content && e.name().as_ref() == "br" => raw.text.push('\n'),
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    "META_ARTICLE" => in_meta_article = false,
                    "BLOC_TEXTUEL" => in_content = false,
                    _ => {}
                }
                if let Some(f) = field.take() {
                    let value = buf.trim().to_string();
                    match f {
                        "id" => raw.id = value,
                        "num" => raw.num = value,
                        "etat" => raw.etat = value,
                        "date_debut" => raw.date_debut = value,
                        "code" => raw.code = value,
                        "tm" if !value.is_empty() => raw.hierarchy.push(value),
                        _ => {}
                    }
                    buf.clear();
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    // Only in-force articles of a code, with a number and real text.
    if raw.etat != "VIGUEUR" || raw.nature != "CODE" || raw.id.is_empty() || raw.num.is_empty() {
        return None;
    }
    let text = strip_html(&raw.text);
    if text.is_empty() {
        return None;
    }
    // Amending provisions carry no substantive rule.
    let lowered = text.to_lowercase();
    if lowered.starts_with("a modifié les dispositions suivantes")
        || lowered.starts_with("a abrogé les dispositions suivantes")
        || lowered.starts_with("a créé les dispositions suivantes")
        || lowered.starts_with("a transféré les dispositions suivantes")
    {
        return None;
    }

    let date = NaiveDate::parse_from_str(&raw.date_debut, "%Y-%m-%d").ok();
    let section = raw.hierarchy.last().cloned().unwrap_or_default();
    Some(Article {
        reference: format!("Article {} du {}", raw.num, raw.code),
        reference_key: reference_key(&raw.code, &raw.num),
        url: format!("https://www.legifrance.gouv.fr/codes/article_lc/{}", raw.id),
        text_length: text.chars().count(),
        text,
        date_debut_timestamp: date
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|d| d.and_utc().timestamp())
            .unwrap_or(0),
        year: date.map(|d| chrono::Datelike::year(&d)).unwrap_or(0),
        date_debut: if raw.date_debut == NO_END { String::new() } else { raw.date_debut },
        id: raw.id,
        number: raw.num,
        code: raw.code,
        code_id: raw.code_id,
        section,
        hierarchy: raw.hierarchy,
    })
}

#[derive(Debug, Default)]
pub struct LegiStats {
    /// Article XML files seen under the code tree.
    pub seen: usize,
    /// Articles kept (in force, with content).
    pub kept: usize,
}

/// Stream the archive and hand every in-force code article to `sink`.
pub fn read_archive<F>(archive: &Path, limit: Option<usize>, mut sink: F) -> Result<LegiStats>
where
    F: FnMut(Article) -> Result<()>,
{
    let file = std::fs::File::open(archive).with_context(|| format!("opening {}", archive.display()))?;
    let decoder = flate2::read::GzDecoder::new(std::io::BufReader::with_capacity(1 << 20, file));
    let mut tar = tar::Archive::new(decoder);
    let mut stats = LegiStats::default();
    let mut xml = String::new();

    info!(archive = %archive.display(), "streaming LEGI archive");
    for entry in tar.entries().context("reading the archive")? {
        let mut entry = entry.context("reading an archive entry")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().context("archive entry path")?.to_string_lossy().into_owned();
        if !path.starts_with(CODE_PREFIX) || !path.contains("/article/") || !path.ends_with(".xml") {
            continue;
        }
        stats.seen += 1;

        xml.clear();
        entry.read_to_string(&mut xml).with_context(|| format!("reading {path}"))?;
        if let Some(article) = parse_article(&xml) {
            stats.kept += 1;
            sink(article)?;
            if stats.kept % 25_000 == 0 {
                info!(seen = stats.seen, kept = stats.kept, "parsing LEGI");
            }
            if limit.is_some_and(|l| stats.kept >= l) {
                debug!(limit = stats.kept, "reached --limit");
                break;
            }
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN_FORCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ARTICLE><META><META_COMMUN><ID>LEGIARTI000044072567</ID><NATURE>Article</NATURE></META_COMMUN>
<META_SPEC><META_ARTICLE><NUM>L110-1</NUM><ETAT>VIGUEUR</ETAT>
<DATE_DEBUT>2022-01-01</DATE_DEBUT><DATE_FIN>2999-01-01</DATE_FIN></META_ARTICLE></META_SPEC></META>
<CONTEXTE><TEXTE cid="LEGITEXT000005634379" nature="CODE">
<TITRE_TXT id_txt="LEGITEXT000005634379">Code de commerce</TITRE_TXT>
<TM><TITRE_TM id="A">Partie législative</TITRE_TM>
<TM><TITRE_TM id="B">LIVRE Ier : Du commerce en général.</TITRE_TM>
<TM><TITRE_TM id="C">TITRE Ier : De l'acte de commerce.</TITRE_TM></TM></TM></TM></TEXTE></CONTEXTE>
<BLOC_TEXTUEL><CONTENU><br/>La loi répute actes de commerce :<br/><br/>1° Tout achat de biens meubles.<br/></CONTENU></BLOC_TEXTUEL>
</ARTICLE>"#;

    #[test]
    fn parses_an_in_force_code_article() {
        let a = parse_article(IN_FORCE).expect("kept");
        assert_eq!(a.id, "LEGIARTI000044072567");
        assert_eq!(a.number, "L110-1");
        assert_eq!(a.code, "Code de commerce");
        assert_eq!(a.code_id, "LEGITEXT000005634379");
        assert_eq!(a.reference, "Article L110-1 du Code de commerce");
        assert_eq!(a.reference_key, "code-de-commerce:L110-1");
        assert_eq!(a.hierarchy, vec![
            "Partie législative",
            "LIVRE Ier : Du commerce en général.",
            "TITRE Ier : De l'acte de commerce."
        ]);
        assert_eq!(a.section, "TITRE Ier : De l'acte de commerce.");
        assert!(a.text.starts_with("La loi répute actes de commerce :"), "text was {:?}", a.text);
        assert!(a.text.contains("1° Tout achat de biens meubles."));
        assert_eq!(a.date_debut, "2022-01-01");
        assert_eq!(a.year, 2022);
        assert_eq!(a.url, "https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000044072567");
    }

    #[test]
    fn skips_superseded_versions() {
        let xml = IN_FORCE.replace("<ETAT>VIGUEUR</ETAT>", "<ETAT>MODIFIE</ETAT>");
        assert!(parse_article(&xml).is_none());
        let xml = IN_FORCE.replace("<ETAT>VIGUEUR</ETAT>", "<ETAT>ABROGE</ETAT>");
        assert!(parse_article(&xml).is_none());
    }

    #[test]
    fn skips_uncodified_texts() {
        let xml = IN_FORCE.replace(r#"nature="CODE""#, r#"nature="DECRET""#);
        assert!(parse_article(&xml).is_none());
    }

    #[test]
    fn skips_amending_provisions_and_empty_content() {
        let xml = IN_FORCE.replace(
            "<br/>La loi répute actes de commerce :<br/><br/>1° Tout achat de biens meubles.<br/>",
            "a modifié les dispositions suivantes<br/>",
        );
        assert!(parse_article(&xml).is_none());

        let xml = IN_FORCE.replace(
            "<br/>La loi répute actes de commerce :<br/><br/>1° Tout achat de biens meubles.<br/>",
            "",
        );
        assert!(parse_article(&xml).is_none());
    }

    #[test]
    fn article_round_trips_through_jsonl() {
        let a = parse_article(IN_FORCE).unwrap();
        let line = serde_json::to_string(&a).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<Article>(&line).unwrap(), a);
    }
}

/// Index settings for the code-article index.
pub async fn apply_settings(meili: &crate::meili::MeiliClient, index: &str) -> Result<()> {
    use serde_json::json;
    meili.create_index(index, "id").await?;
    let settings = json!({
        "searchableAttributes": ["reference", "number", "code", "section", "hierarchy", "text"],
        "filterableAttributes": ["code", "code_id", "number", "reference_key", "section", "year", "date_debut_timestamp"],
        "sortableAttributes": ["date_debut_timestamp", "code", "number"],
        "rankingRules": ["words", "typo", "proximity", "attribute", "sort", "exactness"],
        // An article number must match exactly: "L110-1" is not "L110-11".
        "typoTolerance": { "disableOnAttributes": ["number", "reference_key"] },
        "faceting": { "maxValuesPerFacet": 200 },
        "pagination": { "maxTotalHits": 10000 },
        "stopWords": ["le", "la", "les", "de", "des", "du", "un", "une", "et", "en", "au", "aux",
                      "que", "qui", "dans", "par", "pour", "sur", "ce", "cette", "ces", "est", "a"]
    });
    info!(index, "applying LEGI index settings");
    meili.update_settings(index, &settings).await
}

/// Parse the archive and either index the articles or write them to disk.
pub async fn run(
    meili: &crate::meili::MeiliClient,
    index: &str,
    archive: &Path,
    out: Option<&Path>,
    limit: Option<usize>,
) -> Result<LegiStats> {
    /// Approximate JSON payload size per Meilisearch batch.
    const PAYLOAD_BUDGET: usize = 20 * 1024 * 1024;

    // Parsing is CPU-bound and synchronous; collect batches, then push them.
    let mut writer = match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
            }
            info!(path = %path.display(), "writing articles to disk");
            Some(std::io::BufWriter::new(
                std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?,
            ))
        }
        None => {
            apply_settings(meili, index).await?;
            None
        }
    };

    let mut batches: Vec<Vec<Article>> = Vec::new();
    let mut batch: Vec<Article> = Vec::new();
    let mut bytes = 0usize;

    let stats = read_archive(archive, limit, |article| {
        if let Some(w) = writer.as_mut() {
            use std::io::Write;
            serde_json::to_writer(&mut *w, &article).context("writing an article")?;
            w.write_all(b"\n").context("writing an article")?;
            return Ok(());
        }
        bytes += article.approx_size();
        batch.push(article);
        if bytes >= PAYLOAD_BUDGET {
            batches.push(std::mem::take(&mut batch));
            bytes = 0;
        }
        Ok(())
    })?;

    if let Some(mut w) = writer {
        use std::io::Write;
        w.flush().context("flushing the article dump")?;
        return Ok(stats);
    }

    if !batch.is_empty() {
        batches.push(batch);
    }
    let mut last_task = None;
    for (i, b) in batches.iter().enumerate() {
        last_task = Some(meili.add_documents(index, b).await?);
        info!(batch = i + 1, of = batches.len(), articles = b.len(), "articles queued in Meilisearch");
    }
    if let Some(task) = last_task {
        info!(task, "waiting for Meilisearch to finish indexing");
        meili.wait_for_task(task).await?;
    }
    Ok(stats)
}
