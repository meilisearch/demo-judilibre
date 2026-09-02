//! Convert a raw Judilibre decision (JSON) into the Meilisearch document shape.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum characters kept for the "motivations" excerpt used by the chat.
const EXCERPT_MAX_CHARS: usize = 6_000;

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Document {
    pub id: String,
    pub jurisdiction: String,
    pub chamber: String,
    pub formation: String,
    pub number: String,
    pub numbers: Vec<String>,
    pub ecli: String,
    pub publication: Vec<String>,
    pub decision_date: String,
    /// Unix timestamp (seconds) of the decision date, for filtering/sorting.
    pub decision_timestamp: i64,
    pub year: i32,
    #[serde(rename = "type")]
    pub kind: String,
    pub solution: String,
    pub summary: String,
    pub titles: Vec<String>,
    pub themes: Vec<String>,
    /// Applied legal texts (titles only).
    pub visa: Vec<String>,
    pub files: Vec<FileLink>,
    pub rapprochements: Vec<DecisionLink>,
    pub particular_interest: bool,
    pub location: String,
    pub bulletin: String,
    /// Motivations + dispositif excerpt (what the court decided and why).
    pub excerpt: String,
    pub text: String,
    pub text_length: usize,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct FileLink {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    /// Text extracted from the PDF by `pdf-inspector` (empty unless `--with-files`).
    pub content: String,
    /// Page count reported by `pdf-inspector`.
    pub pages: u32,
    /// `text_based`, `scanned`, `image_based`, `mixed`, or empty when not processed.
    pub pdf_type: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct DecisionLink {
    pub title: String,
    pub number: String,
    pub url: String,
}

impl Document {
    /// Rough serialized size, used to bound Meilisearch payloads.
    pub fn approx_size(&self) -> usize {
        self.text.len() + self.excerpt.len() + self.summary.len() + 2_048
    }
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn str_array(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Collect every string found under `value` (recursively), deduplicated in order.
fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            let s = s.trim();
            let s = strip_html(s);
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        }
        Value::Array(a) => a.iter().for_each(|v| collect_strings(v, out)),
        Value::Object(o) => o.values().for_each(|v| collect_strings(v, out)),
        _ => {}
    }
}

/// Titles come from `titlesAndSummaries` (Cour de cassation only). Its shape is
/// loosely specified, so gather all strings under `titles` / `secondaryTitles`.
fn titles(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(tas) = v.get("titlesAndSummaries") {
        let entries: Vec<&Value> = match tas {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for e in entries {
            if let Some(t) = e.get("titles") {
                collect_strings(t, &mut out);
            }
            if let Some(t) = e.get("secondaryTitles") {
                collect_strings(t, &mut out);
            }
        }
    }
    out
}

/// Slice `text` on character boundaries using the `zones` byte-or-char offsets
/// reported by Judilibre (offsets are character indices into the pseudonymised text).
fn zone_text(text: &str, zones: Option<&Value>, zone: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    if let Some(segments) = zones.and_then(|z| z.get(zone)).and_then(Value::as_array) {
        for seg in segments {
            let start = seg.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
            let end = seg.get("end").and_then(Value::as_u64).unwrap_or(0) as usize;
            let end = end.min(chars.len());
            if start < end {
                out.extend(&chars[start..end]);
                out.push('\n');
            }
        }
    }
    out
}

/// Judilibre embeds links in some text fields (notably `visa`, where articles point to
/// Legifrance). Strip tags and decode the few entities that appear, so indexed and
/// displayed text stays plain.
fn strip_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0usize;
    for c in raw.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    // Collapse the whitespace left behind by removed markup.
    let mut collapsed = String::with_capacity(out.len());
    let mut last_space = false;
    for c in out.chars() {
        if c.is_whitespace() && c != '\n' {
            if !last_space {
                collapsed.push(' ');
            }
            last_space = true;
        } else {
            last_space = false;
            collapsed.push(c);
        }
    }
    collapsed.trim().to_string()
}

fn normalise_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank_run = 0;
    for line in raw.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim().to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

pub fn to_document(raw: &Value) -> Option<Document> {
    let id = raw.get("id").and_then(Value::as_str)?.to_string();
    let decision_date = str_field(raw, "decision_date");
    let date = NaiveDate::parse_from_str(&decision_date, "%Y-%m-%d").ok();
    let decision_timestamp = date
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0);
    let year = date.map(|d| chrono::Datelike::year(&d)).unwrap_or(0);

    let raw_text = str_field(raw, "text");
    let zones = raw.get("zones");
    let mut excerpt = zone_text(&raw_text, zones, "motivations");
    excerpt.push_str(&zone_text(&raw_text, zones, "dispositif"));
    let excerpt = normalise_text(&excerpt);
    let text = normalise_text(&raw_text);
    let excerpt = if excerpt.is_empty() {
        truncate_chars(&text, EXCERPT_MAX_CHARS)
    } else {
        truncate_chars(&excerpt, EXCERPT_MAX_CHARS)
    };

    let files = raw
        .get("files")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|f| FileLink {
                    name: strip_html(&str_field(f, "name")),
                    kind: strip_html(&str_field(f, "type")),
                    url: {
                        let raw_url = str_field(f, "rawUrl");
                        if raw_url.is_empty() { str_field(f, "url") } else { raw_url }
                    },
                    content: String::new(),
                    pages: 0,
                    pdf_type: String::new(),
                })
                .collect()
        })
        .unwrap_or_default();

    let rapprochements = raw
        .get("rapprochements")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|r| DecisionLink {
                    title: str_field(r, "title"),
                    number: str_field(r, "number"),
                    url: str_field(r, "url"),
                })
                .collect()
        })
        .unwrap_or_default();

    let visa = raw
        .get("visa")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|t| strip_html(&str_field(t, "title")))
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Some(Document {
        text_length: text.chars().count(),
        url: format!("https://www.courdecassation.fr/decision/{id}"),
        id,
        jurisdiction: str_field(raw, "jurisdiction"),
        chamber: str_field(raw, "chamber"),
        formation: str_field(raw, "formation"),
        number: str_field(raw, "number"),
        numbers: str_array(raw, "numbers"),
        ecli: str_field(raw, "ecli"),
        publication: str_array(raw, "publication"),
        decision_date,
        decision_timestamp,
        year,
        kind: str_field(raw, "type"),
        solution: str_field(raw, "solution"),
        summary: strip_html(&str_field(raw, "summary")),
        titles: titles(raw),
        themes: str_array(raw, "themes").iter().map(|t| strip_html(t)).filter(|t| !t.is_empty()).collect(),
        visa,
        files,
        rapprochements,
        particular_interest: raw.get("particularInterest").and_then(Value::as_bool).unwrap_or(false),
        location: str_field(raw, "location"),
        bulletin: str_field(raw, "bulletin"),
        excerpt,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "id": "5fca7d162a251e6bf9c78514",
            "jurisdiction": "Cour de cassation",
            "chamber": "Troisième chambre civile",
            "number": "17-18.194",
            "numbers": ["17-18.194", "16-21.165"],
            "ecli": "ECLI:FR:CCASS:2018:C301117",
            "formation": "Formation de section",
            "publication": ["Publié au Bulletin", "Communiqué"],
            "decision_date": "2018-12-20",
            "type": "Arrêt",
            "solution": "Rejet",
            "summary": "  Le titulaire d'une AOT est en droit d'obtenir l'indemnisation.  ",
            "themes": ["Expropriation", "Indemnité"],
            "text": "INTRO\r\n\r\n\r\n\r\nMOTIFS ici\r\nDISPOSITIF là\r\n",
            "zones": {
                "introduction": [{"start": 0, "end": 5}],
                "motivations": [{"start": 13, "end": 23}],
                "dispositif": [{"start": 25, "end": 38}]
            },
            "files": [{"name": "Rapport", "type": "Rapport du conseiller", "url": "u", "rawUrl": "https://raw"}],
            "visa": [{"title": "Code civil, article 1240", "url": "x"}],
            "rapprochements": [{"title": "Civ. 1re, 2 mars 2010", "number": "09-12.345", "url": "https://r"}],
            "particularInterest": true,
            "titlesAndSummaries": [{"titles": ["EXPROPRIATION", "Indemnité"], "summary": "s", "secondaryTitles": ["EXPROPRIATION"]}]
        })
    }

    #[test]
    fn maps_core_fields() {
        let doc = to_document(&sample()).unwrap();
        assert_eq!(doc.id, "5fca7d162a251e6bf9c78514");
        assert_eq!(doc.kind, "Arrêt");
        assert_eq!(doc.year, 2018);
        assert_eq!(doc.decision_timestamp, 1_545_264_000);
        assert_eq!(doc.summary, "Le titulaire d'une AOT est en droit d'obtenir l'indemnisation.");
        assert_eq!(doc.titles, vec!["EXPROPRIATION", "Indemnité"]);
        assert_eq!(doc.visa, vec!["Code civil, article 1240"]);
        assert_eq!(doc.files[0].url, "https://raw");
        assert_eq!(doc.files[0].content, "");
        assert_eq!(doc.rapprochements[0].number, "09-12.345");
        assert!(doc.particular_interest);
        assert_eq!(doc.url, "https://www.courdecassation.fr/decision/5fca7d162a251e6bf9c78514");
    }

    #[test]
    fn normalises_text_and_builds_excerpt_from_zones() {
        let doc = to_document(&sample()).unwrap();
        assert_eq!(doc.text, "INTRO\n\nMOTIFS ici\nDISPOSITIF là");
        assert_eq!(doc.excerpt, "MOTIFS ici\nDISPOSITIF là");
        assert_eq!(doc.text_length, doc.text.chars().count());
    }

    #[test]
    fn excerpt_falls_back_to_text_without_zones() {
        let mut v = sample();
        v.as_object_mut().unwrap().remove("zones");
        let doc = to_document(&v).unwrap();
        assert_eq!(doc.excerpt, doc.text);
    }

    #[test]
    fn serialises_type_field_name() {
        let doc = to_document(&sample()).unwrap();
        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["type"], "Arrêt");
        assert!(json.get("kind").is_none());
    }

    #[test]
    fn strips_html_from_visa_and_summary() {
        let mut v = sample();
        v["visa"] = json!([{ "title": "Article <a href=\"https://legifrance\" target=\"_blank\">145</a> du code de procedure penale.", "url": "x" }]);
        v["summary"] = json!("Le <em>titulaire</em>&nbsp;d&#39;une AOT.");
        let doc = to_document(&v).unwrap();
        assert_eq!(doc.visa, vec!["Article 145 du code de procedure penale."]);
        assert_eq!(doc.summary, "Le titulaire d'une AOT.");
    }

    #[test]
    fn strip_html_handles_plain_text_and_entities() {
        assert_eq!(strip_html("texte simple"), "texte simple");
        assert_eq!(strip_html("a &amp; b"), "a & b");
        assert_eq!(strip_html("<p>a</p>  <p>b</p>"), "a b");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn document_round_trips_through_jsonl() {
        let doc = to_document(&sample()).unwrap();
        let line = serde_json::to_string(&doc).unwrap();
        assert!(!line.contains('\n'), "a document must fit on one JSONL line");
        let back: Document = serde_json::from_str(&line).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn requires_an_id() {
        assert!(to_document(&json!({"number": "1"})).is_none());
    }
}
