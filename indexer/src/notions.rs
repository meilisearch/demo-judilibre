//! What the case law calls each code article.
//!
//! A decision's titrage names the rule it applies in the doctrine's words
//! ("VENTE - Garantie - Vices cachés"), and its visa says which article that is
//! (`visa_refs`, see `refs.rs`). Joining the two gives an article the names people
//! search it by, which its own text rarely uses: article 1641 says "défauts
//! cachés", article 122-7 never says "état de nécessité".

use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// A segment found on more than this share of cited articles ("portée",
/// "conditions", "cassation") names nothing in particular.
const MAX_SHARE: f64 = 0.01;
/// Floor for that cut-off, so a small corpus does not discard everything.
const MIN_SPREAD: usize = 5;
/// Notions kept per article, most frequent first.
const PER_ARTICLE: usize = 10;

#[derive(Debug, Default)]
pub struct CaseLaw {
    /// Decisions whose visa applies each article, by `reference_key`.
    pub cited_by: HashMap<String, u64>,
    /// Doctrinal names of each article, by `reference_key`.
    pub notions: HashMap<String, Vec<String>>,
}

/// Title and theme segments of one decision, lower-cased and deduplicated.
fn segments(decision: &Value) -> HashSet<String> {
    let strings = |field: &str| -> Vec<String> {
        decision[field]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default()
    };
    let mut out = HashSet::new();
    for title in strings("titles").into_iter().chain(strings("themes")) {
        for seg in title.split(" - ").flat_map(|s| s.split(';')) {
            let seg = seg.split_whitespace().collect::<Vec<_>>().join(" ");
            let seg = seg.trim_matches(|c: char| c == '.' || c == ',' || c == ':' || c == ' ').to_lowercase();
            let len = seg.chars().count();
            if len > 3 && len <= 60 && !seg.chars().any(|c| c.is_ascii_digit()) {
                out.insert(seg);
            }
        }
    }
    out
}

/// Build both maps from decisions carrying `visa_refs`, `titles` and `themes`.
pub fn from_decisions<'a>(decisions: impl IntoIterator<Item = &'a Value>) -> CaseLaw {
    let mut cited_by: HashMap<String, u64> = HashMap::new();
    let mut counts: HashMap<String, HashMap<String, u64>> = HashMap::new();
    for decision in decisions {
        let segs = segments(decision);
        let refs: HashSet<&str> = decision["visa_refs"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for r in refs {
            *cited_by.entry(r.to_string()).or_default() += 1;
            let per = counts.entry(r.to_string()).or_default();
            for s in &segs {
                *per.entry(s.clone()).or_default() += 1;
            }
        }
    }

    let mut spread: HashMap<&str, usize> = HashMap::new();
    for per in counts.values() {
        for s in per.keys() {
            *spread.entry(s).or_default() += 1;
        }
    }
    let limit = MIN_SPREAD.max((MAX_SHARE * counts.len() as f64) as usize);

    let mut notions = HashMap::new();
    for (reference, per) in &counts {
        let citing = cited_by[reference];
        let mut kept: Vec<(&String, u64)> = per
            .iter()
            .filter(|(s, c)| spread[s.as_str()] <= limit && (**c >= 2 || citing <= 2))
            .map(|(s, c)| (s, *c))
            .collect();
        kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        if !kept.is_empty() {
            notions.insert(reference.clone(), kept.into_iter().take(PER_ARTICLE).map(|(s, _)| s.clone()).collect());
        }
    }
    CaseLaw { cited_by, notions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn decision(refs: &[&str], titles: &[&str]) -> Value {
        json!({ "visa_refs": refs, "titles": titles, "themes": [] })
    }

    #[test]
    fn article_takes_the_names_of_the_decisions_applying_it() {
        let decisions = vec![
            decision(&["code-civil:1641"], &["VENTE - Garantie - Vices cachés - Action"]),
            decision(&["code-civil:1641"], &["VENTE - Garantie - Vices cachés"]),
            decision(&["code-civil:1641"], &["VENTE - Vices cachés - Délai"]),
            decision(&["code-penal:122-5"], &["RESPONSABILITE PENALE - Légitime défense"]),
        ];
        let law = from_decisions(&decisions);
        assert_eq!(law.cited_by["code-civil:1641"], 3);
        let n = &law.notions["code-civil:1641"];
        assert!(n.contains(&"vices cachés".to_string()), "{n:?}");
        // Seen once among three citing decisions: not kept.
        assert!(!n.contains(&"action".to_string()), "{n:?}");
        // An article cited by one or two decisions keeps what they say.
        assert!(law.notions["code-penal:122-5"].contains(&"légitime défense".to_string()));
    }

    #[test]
    fn segments_found_on_many_articles_are_dropped() {
        // "portée" on 10 articles is above the floor of 5: generic.
        let decisions: Vec<Value> = (0..10)
            .flat_map(|i| {
                let r = format!("code-civil:{i}");
                vec![decision(&[&r], &["Portée - Clause pénale"]), decision(&[&r], &["Portée - Clause pénale"])]
            })
            .collect();
        let law = from_decisions(&decisions);
        assert!(law.notions.values().all(|n| !n.contains(&"portée".to_string())));
        // "clause pénale" is on 10 articles too, so it goes as well — the rule is blind to meaning.
        assert!(law.notions.is_empty());
    }

    #[test]
    fn numbers_and_short_segments_are_not_notions() {
        let d = decision(&["code-civil:9"], &["Article 9 - Vie - Respect de la vie privée"]);
        let segs = segments(&d);
        assert!(segs.contains("respect de la vie privée"));
        assert!(!segs.iter().any(|s| s.contains('9')));
        assert!(!segs.contains("vie"));
    }
}
