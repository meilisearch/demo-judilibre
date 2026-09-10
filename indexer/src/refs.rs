//! Normalised references to legislative articles, shared by both corpora.
//!
//! Judilibre states applied texts as prose ("Articles L. 461-1 et L. 461-3 du
//! code de l'urbanisme.") and links them to a Légifrance *search* URL, never to a
//! `LEGIARTI` id. LEGI, on the other hand, gives a code title and an article
//! number. Reducing both sides to the same key is therefore what lets a decision
//! and an article find each other.

use std::sync::LazyLock;

use regex::Regex;

/// Article numbers as they appear in prose: `L. 461-1`, `R. 123-4`, `1240`,
/// `L. 137-2`, `L110-1`. The letter prefix is optional and may carry a dot and
/// spaces; the number itself may be dash-separated.
static ARTICLE_NUM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:(?<prefix>[LRDA])\.?\s*)?(?<num>\d+(?:-\d+)*)\b").expect("valid regex")
});

/// Subdivisions whose number belongs to an article, not to a new article:
/// "Article 641, alinéa 1, du code de procédure civile" cites one article.
static SUBDIVISION: LazyLock<Regex> = LazyLock::new(|| {
    // `\b` applies only to the word forms: it never matches before "§".
    Regex::new(r"(?i)(?:\b(?:alin[eé]as?|al\.|paragraphes?|points?|n°|n[ou]s?\.?)|§)\s*\d+(?:-\d+)*")
        .expect("valid regex")
});

/// The code a visa refers to, e.g. "du code de l'urbanisme", "du code civil".
static CODE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bcode\s+(?:de\s+la\s+|de\s+l'|des\s+|du\s+|de\s+|d')?([^,;.]+)").expect("valid regex"));

/// A visa usually qualifies the article's version — "du code civil, dans sa
/// rédaction antérieure à l'ordonnance n° 2016-131 du 10 février 2016". None of
/// these words occur in a code's title, so the title ends at the first of them,
/// while genuine multi-word titles ("code de la construction et de
/// l'habitation") survive intact.
const TITLE_STOPS: &[&str] = &[
    " dans ",
    " dans sa",
    " dans leur",
    " applicable",
    " r\u{e9}daction",
    " redaction",
    " issu",
    " modifi\u{e9}",
    " modifie",
    " en vigueur",
    " pr\u{e9}cit\u{e9}",
    " precite",
    " ancien",
    " tel que",
    " version",
    " alors ",
    " interpr\u{e9}t\u{e9}",
];

/// Dates carry numbers that are not article numbers — "du 26 août 1789". They
/// must go before scanning, and a whole date has to match at once: Code civil
/// article numbers are themselves four digits, so bare years cannot be dropped.
static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b\d{1,2}(?:er)?\s+(?:janvier|f[eé]vrier|mars|avril|mai|juin|juillet|ao[uû]t|septembre|octobre|novembre|d[eé]cembre)\s+\d{4}\b",
    )
    .expect("valid regex")
});

/// A visa often cites non-codified sources alongside a code — "Article 11 de la
/// Déclaration des droits de l'homme … et L. 1234-1 du code du travail". Numbers
/// stated before one of these belong to it, not to the code that follows, so the
/// search for article numbers restarts after the last such marker.
static OTHER_SOURCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(d[eé]claration|loi|ordonnance|d[eé]cret|trait[eé]|convention|r[eè]glement|directive|constitution|protocole|arr[eê]t[eé]|charte|pacte)\b",
    )
    .expect("valid regex")
});

/// Words that continue a code's title after "et": "code rural **et de la** pêche
/// maritime", "code des postes **et des** communications électroniques". Anything
/// else after "et" starts a new reference — "code civil **et 480** du code de
/// procédure civile".
/// Only these actually occur: "et de la pêche maritime", "et de l'habitation",
/// "et des communications électroniques", "et du séjour". "et les …" always
/// introduces something else, so it is not a continuation.
const TITLE_CONTINUATIONS: &[&str] = &["de ", "du ", "des ", "d'", "d\u{2019}", "l'", "l\u{2019}"];

/// Cut a captured code title at the first version qualifier or new reference.
fn trim_title(raw: &str) -> &str {
    let lower = raw.to_lowercase();
    let mut end = raw.len();
    for stop in TITLE_STOPS {
        if let Some(i) = lower.find(stop) {
            end = end.min(i);
        }
    }
    // Cut at an "et" that introduces another reference rather than more title.
    let mut from = 0usize;
    while let Some(i) = lower[from..].find(" et ") {
        let at = from + i;
        let rest = &lower[at + 4..];
        if !TITLE_CONTINUATIONS.iter().any(|c| rest.starts_with(c)) {
            end = end.min(at);
            break;
        }
        from = at + 4;
    }
    raw[..end].trim_end_matches(['.', ',', ' '])
}

/// Strip accents and punctuation so "Code de l'urbanisme" and
/// "code de l’urbanisme" (curly apostrophe) collapse to the same slug.
fn slugify(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_dash = true; // avoids a leading dash
    for c in raw.trim().chars() {
        let c = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' | 'ý' => 'y',
            'ç' => 'c',
            'ñ' => 'n',
            'œ' => 'o',
            'æ' => 'a',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Normalise an article number to LEGI's own form: `L. 461-1` → `L461-1`.
pub fn normalise_number(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            out.push(c.to_ascii_uppercase());
        }
    }
    out
}

/// The join key for one article of one code, e.g. `code-de-commerce:L110-1`.
pub fn reference_key(code: &str, number: &str) -> String {
    format!("{}:{}", slugify(code), normalise_number(number))
}

/// Extract every `(code, number)` reference stated by one visa line.
///
/// A visa often lists several articles of a single code, so each number found
/// before the code name is paired with it. Lines that name no code (treaties,
/// the Déclaration des droits de l'homme, a dated decree) yield nothing: those
/// are not codified articles and have no counterpart in LEGI.
pub fn visa_references(visa: &str) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    // A visa may cite several codes — "Articles 1355 du code civil et 480 du code
    // de procédure civile" — so each code takes the numbers stated since the
    // previous one. The regex match runs past the title (it stops only at
    // punctuation), so scanning resumes at the end of the *trimmed* title,
    // otherwise the second code would sit inside the first match and be missed.
    let mut pos = 0usize;
    while pos < visa.len() {
        let Some(m) = CODE_NAME.find(&visa[pos..]) else { break };
        let start = pos + m.start();
        let code = trim_title(m.as_str().trim_end_matches(['.', ' ']));
        if code.is_empty() {
            pos += m.end();
            continue;
        }
        // Subdivision numbers ("alinéa 1") and dates are not article numbers.
        let head = SUBDIVISION.replace_all(&visa[pos..start], " ");
        let head = DATE.replace_all(&head, " ");
        // Drop anything up to the last non-codified source mentioned before this code.
        let head = match OTHER_SOURCE.find_iter(&head).last() {
            Some(m) => head[m.end()..].to_string(),
            None => head.to_string(),
        };
        for caps in ARTICLE_NUM.captures_iter(&head) {
            let prefix = caps.name("prefix").map(|p| p.as_str()).unwrap_or("");
            let key = reference_key(code, &format!("{prefix}{}", &caps["num"]));
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        // Advance past the title only, so a following code is still seen.
        pos = start + code.len();
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_article_numbers_to_legi_form() {
        assert_eq!(normalise_number("L. 461-1"), "L461-1");
        assert_eq!(normalise_number("R. 123-4"), "R123-4");
        assert_eq!(normalise_number("641"), "641");
        assert_eq!(normalise_number("L110-1"), "L110-1");
    }

    #[test]
    fn slugifies_code_titles_consistently() {
        assert_eq!(reference_key("Code de commerce", "L110-1"), "code-de-commerce:L110-1");
        // Accents, curly apostrophes and case must not create distinct keys.
        assert_eq!(
            reference_key("code de l'urbanisme", "L. 461-1"),
            reference_key("Code de l’urbanisme", "L461-1")
        );
        assert_eq!(reference_key("code de procédure civile", "641"), "code-de-procedure-civile:641");
    }

    #[test]
    fn extracts_several_articles_of_one_code() {
        assert_eq!(
            visa_references("Articles L. 461-1 et L. 461-3 du code de l'urbanisme."),
            vec!["code-de-l-urbanisme:L461-1", "code-de-l-urbanisme:L461-3"]
        );
    }

    #[test]
    fn extracts_a_single_article() {
        assert_eq!(
            visa_references("Article 641, alinéa 1, du code de procédure civile."),
            vec!["code-de-procedure-civile:641"]
        );
        assert_eq!(visa_references("Articles 2219 et 2224 du code civil."), vec![
            "code-civil:2219",
            "code-civil:2224"
        ]);
    }

    #[test]
    fn strips_version_qualifiers_from_the_code_title() {
        // Judilibre states which version of the article applied; that is not part
        // of the code's name and must not end up in the key.
        assert_eq!(
            visa_references("Article 1134 du code civil, dans sa rédaction antérieure à l'ordonnance n° 2016-131 du 10 février 2016."),
            vec!["code-civil:1134"]
        );
        assert_eq!(
            visa_references("Articles 2044 et 2052 du code civil, dans leur rédaction antérieure à celle de la loi n° 2016-1547 du 18 novembre 2016."),
            vec!["code-civil:2044", "code-civil:2052"]
        );
        assert_eq!(
            visa_references("Article 1134 du code civil dans sa rédaction applicable en la cause."),
            vec!["code-civil:1134"]
        );
        assert_eq!(
            visa_references("Article L. 137-2 du code de la consommation, devenu L. 218-2, en vigueur au moment des faits."),
            vec!["code-de-la-consommation:L137-2"]
        );
    }

    #[test]
    fn pairs_each_code_with_its_own_articles() {
        assert_eq!(
            visa_references("Articles 1355 du code civil et 480 du code de procédure civile."),
            vec!["code-civil:1355", "code-de-procedure-civile:480"]
        );
        assert_eq!(
            visa_references("Articles 2224 du code civil et L. 110-4 du code de commerce."),
            vec!["code-civil:2224", "code-de-commerce:L110-4"]
        );
        assert_eq!(
            visa_references("Article 1240 du code civil et 593 du code de procédure pénale."),
            vec!["code-civil:1240", "code-de-procedure-penale:593"]
        );
    }

    #[test]
    fn keeps_genuine_multi_word_titles() {
        assert_eq!(
            visa_references("Article L. 271-1 du code de la construction et de l'habitation."),
            vec!["code-de-la-construction-et-de-l-habitation:L271-1"]
        );
        assert_eq!(
            visa_references("Article L. 411-1 du code rural et de la pêche maritime."),
            vec!["code-rural-et-de-la-peche-maritime:L411-1"]
        );
        assert_eq!(visa_references("Article 39 du code général des impôts."), vec![
            "code-general-des-impots:39"
        ]);
    }

    #[test]
    fn ignores_subdivision_numbers() {
        assert_eq!(visa_references("Article 641, alinéa 1, du code de procédure civile."), vec![
            "code-de-procedure-civile:641"
        ]);
        assert_eq!(visa_references("Article L. 1152-1, alinéa 2, du code du travail."), vec![
            "code-du-travail:L1152-1"
        ]);
        assert_eq!(visa_references("Article 9, § 3, du code civil."), vec!["code-civil:9"]);
    }

    #[test]
    fn does_not_attach_another_source_numbers_to_a_code() {
        // The 11 and the 1789 belong to the Déclaration, not to the code du travail.
        assert_eq!(
            visa_references(
                "Articles 11 de la Déclaration des droits de l'homme et du citoyen du 26 août 1789 et L. 1234-1 du code du travail."
            ),
            vec!["code-du-travail:L1234-1"]
        );
        // "et les principes …" is not a title continuation.
        let keys = visa_references(
            "Article 1520 du code de procédure civile et les principes du droit international coutumier."
        );
        assert_eq!(keys, vec!["code-de-procedure-civile:1520"]);
    }

    #[test]
    fn ignores_visas_that_name_no_code() {
        assert!(visa_references("Article 2 de la Déclaration des droits de l'homme et du citoyen.").is_empty());
        assert!(visa_references("Article 178 du décret n° 91-1197 du 27 novembre 1991.").is_empty());
        assert!(visa_references("").is_empty());
    }

    #[test]
    fn keeps_the_renumbered_article_and_its_successor() {
        // "L. 137-2, devenu L. 218-2 du code de la consommation" states both.
        let keys = visa_references("Article L. 137-2, devenu L. 218-2 du code de la consommation.");
        assert_eq!(keys, vec![
            "code-de-la-consommation:L137-2",
            "code-de-la-consommation:L218-2"
        ]);
    }
}
