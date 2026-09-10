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
    let Some(m) = CODE_NAME.find(visa) else {
        return Vec::new();
    };
    // "code de l'urbanisme." → the full code title, used for the slug.
    let code = visa[m.start()..m.end()].trim_end_matches(['.', ' ']);

    // Only numbers stated before the code name belong to it, and a subdivision
    // number ("alinéa 1") is part of the preceding article rather than a new one.
    let head = SUBDIVISION.replace_all(&visa[..m.start()], " ");
    let mut keys: Vec<String> = Vec::new();
    for caps in ARTICLE_NUM.captures_iter(&head) {
        let prefix = caps.name("prefix").map(|p| p.as_str()).unwrap_or("");
        let num = &caps["num"];
        let key = reference_key(code, &format!("{prefix}{num}"));
        if !keys.contains(&key) {
            keys.push(key);
        }
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
