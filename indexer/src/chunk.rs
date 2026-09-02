//! Split a decision (its full text and the text extracted from its PDFs) into
//! overlapping passages stored in a second index. Chunk-level embeddings give the
//! assistant precise passages to cite instead of one vector per whole decision.

use serde::Serialize;

use crate::transform::Document;

/// Target passage size in characters. Legal reasoning needs room, and
/// voyage-law-2 accepts 16K tokens, so ~2 000 characters is comfortable.
pub const TARGET_CHARS: usize = 2_000;

/// Overlap between consecutive passages, so a rule split across a boundary still
/// appears whole in one of them.
pub const OVERLAP_CHARS: usize = 200;

/// Passages shorter than this are merged into the previous one rather than kept alone.
const MIN_CHARS: usize = 200;

/// Where a passage comes from.
pub const SOURCE_DECISION: &str = "decision";
pub const SOURCE_ATTACHMENT: &str = "attachment";

/// One passage, denormalised with the metadata needed to cite and filter it.
#[derive(Debug, Serialize, PartialEq)]
pub struct Chunk {
    /// `<decision id>_<n>`. Meilisearch document ids allow only letters, digits,
    /// `-` and `_`, so the separator cannot be `#`.
    pub id: String,
    pub decision_id: String,
    pub chunk_index: usize,
    pub chunk_count: usize,
    /// `decision` or `attachment`.
    pub source: String,
    /// Attachment name, type and public URL when `source` is `attachment`.
    pub attachment_name: String,
    pub attachment_type: String,
    pub attachment_url: String,
    pub content: String,
    pub content_chars: usize,

    // Denormalised decision metadata (citation + facets).
    pub jurisdiction: String,
    pub chamber: String,
    pub formation: String,
    pub number: String,
    pub ecli: String,
    pub publication: Vec<String>,
    pub decision_date: String,
    pub decision_timestamp: i64,
    pub year: i32,
    #[serde(rename = "type")]
    pub kind: String,
    pub solution: String,
    pub titles: Vec<String>,
    pub themes: Vec<String>,
    pub summary: String,
    pub url: String,
}

impl Chunk {
    pub fn approx_size(&self) -> usize {
        self.content.len() + self.summary.len() + 1_024
    }
}

/// Split `text` into overlapping windows that prefer paragraph, then sentence,
/// then whitespace boundaries.
pub fn split_text(text: &str, target: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    if chars.len() <= target {
        return vec![text.trim().to_string()];
    }
    let overlap = overlap.min(target / 2);

    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let hard_end = (start + target).min(chars.len());
        let end = if hard_end == chars.len() {
            hard_end
        } else {
            // Look for a boundary in the last third of the window.
            let window_start = start + (target * 2 / 3);
            find_boundary(&chars, window_start, hard_end).unwrap_or(hard_end)
        };
        let piece: String = chars[start..end].iter().collect();
        let piece = piece.trim().to_string();
        if !piece.is_empty() {
            // Fold a too-short tail into the previous passage.
            if piece.chars().count() < MIN_CHARS && !chunks.is_empty() {
                let last = chunks.last_mut().expect("non-empty");
                last.push('\n');
                last.push_str(&piece);
            } else {
                chunks.push(piece);
            }
        }
        if end >= chars.len() {
            break;
        }
        start = end.saturating_sub(overlap).max(start + 1);
    }
    chunks
}

/// Prefer a paragraph break, then a sentence end, then any whitespace.
fn find_boundary(chars: &[char], from: usize, to: usize) -> Option<usize> {
    let mut sentence = None;
    let mut whitespace = None;
    let mut i = to;
    while i > from {
        i -= 1;
        let c = chars[i];
        if c == '\n' {
            // A blank line is the strongest boundary.
            if i + 1 < chars.len() && chars.get(i + 1) == Some(&'\n') {
                return Some(i + 1);
            }
            if sentence.is_none() {
                sentence = Some(i + 1);
            }
        } else if (c == '.' || c == ';' || c == '!' || c == '?')
            && chars.get(i + 1).is_some_and(|n| n.is_whitespace())
            && sentence.is_none()
        {
            sentence = Some(i + 1);
        } else if c.is_whitespace() && whitespace.is_none() {
            whitespace = Some(i + 1);
        }
    }
    sentence.or(whitespace)
}

/// Build the passages for one decision: its full text, then each attachment's text.
pub fn chunk_document(doc: &Document) -> Vec<Chunk> {
    // (content, source, attachment name, attachment type, attachment url)
    let mut pieces: Vec<(String, String, String, String, String)> = Vec::new();

    for piece in split_text(&doc.text, TARGET_CHARS, OVERLAP_CHARS) {
        pieces.push((piece, SOURCE_DECISION.to_string(), String::new(), String::new(), String::new()));
    }
    for file in &doc.files {
        if file.content.trim().is_empty() {
            continue;
        }
        for piece in split_text(&file.content, TARGET_CHARS, OVERLAP_CHARS) {
            pieces.push((
                piece,
                SOURCE_ATTACHMENT.to_string(),
                file.name.clone(),
                file.kind.clone(),
                file.url.clone(),
            ));
        }
    }

    let total = pieces.len();
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, (content, source, attachment_name, attachment_type, attachment_url))| Chunk {
            id: format!("{}_{i}", doc.id),
            decision_id: doc.id.clone(),
            chunk_index: i,
            chunk_count: total,
            source,
            attachment_name,
            attachment_type,
            attachment_url,
            content_chars: content.chars().count(),
            content,
            jurisdiction: doc.jurisdiction.clone(),
            chamber: doc.chamber.clone(),
            formation: doc.formation.clone(),
            number: doc.number.clone(),
            ecli: doc.ecli.clone(),
            publication: doc.publication.clone(),
            decision_date: doc.decision_date.clone(),
            decision_timestamp: doc.decision_timestamp,
            year: doc.year,
            kind: doc.kind.clone(),
            solution: doc.solution.clone(),
            titles: doc.titles.clone(),
            themes: doc.themes.clone(),
            summary: doc.summary.clone(),
            url: doc.url.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::{FileLink, to_document};
    use serde_json::json;

    fn doc_with(text: &str, files: Vec<FileLink>) -> Document {
        let mut doc = to_document(&json!({
            "id": "abc123",
            "jurisdiction": "Cour de cassation",
            "chamber": "Chambre sociale",
            "number": "22-10.123",
            "numbers": ["22-10.123"],
            "decision_date": "2024-01-17",
            "type": "Arrêt",
            "solution": "Rejet",
            "text": text,
        }))
        .unwrap();
        doc.files = files;
        doc
    }

    #[test]
    fn short_text_is_one_chunk() {
        let chunks = chunk_document(&doc_with("Texte court.", vec![]));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, "abc123_0");
        assert_eq!(chunks[0].content, "Texte court.");
        assert_eq!(chunks[0].source, SOURCE_DECISION);
        assert_eq!(chunks[0].chunk_count, 1);
        assert_eq!(chunks[0].chamber, "Chambre sociale");
    }

    #[test]
    fn empty_text_yields_no_chunks() {
        assert!(chunk_document(&doc_with("", vec![])).is_empty());
    }

    #[test]
    fn long_text_splits_with_overlap_and_covers_everything() {
        // Paragraphs of ~300 chars so boundaries exist.
        let para: String = "Attendu que la cour d'appel a retenu. ".repeat(9);
        let text = vec![para.clone(); 12].join("\n\n");
        let chunks = split_text(&text, TARGET_CHARS, OVERLAP_CHARS);
        assert!(chunks.len() > 1, "expected several chunks, got {}", chunks.len());
        for c in &chunks {
            assert!(c.chars().count() <= TARGET_CHARS, "chunk too long: {}", c.chars().count());
            assert!(!c.is_empty());
        }
        // Every chunk's text must come from the source, and the start/end must be covered.
        assert!(text.contains(chunks[0].split('\n').next().unwrap()));
        assert!(text.trim_end().ends_with(chunks.last().unwrap().trim_end().split('\n').last().unwrap()));
    }

    #[test]
    fn attachment_text_becomes_its_own_chunks() {
        let files = vec![FileLink {
            name: "CP_26-83.562.pdf".into(),
            kind: "Communiqué".into(),
            url: "https://example/x.pdf".into(),
            content: "Le communiqué explique la portée de l'arrêt.".into(),
            pages: 2,
            pdf_type: "text_based".into(),
        }];
        let chunks = chunk_document(&doc_with("Texte de la décision.", files));
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].source, SOURCE_DECISION);
        assert_eq!(chunks[1].source, SOURCE_ATTACHMENT);
        assert_eq!(chunks[1].attachment_type, "Communiqué");
        assert_eq!(chunks[1].attachment_url, "https://example/x.pdf");
        assert_eq!(chunks[0].attachment_url, "");
        assert_eq!(chunks[1].id, "abc123_1");
        assert!(chunks.iter().all(|c| c.chunk_count == 2));
    }

    #[test]
    fn attachments_without_text_are_skipped() {
        let files = vec![FileLink {
            name: "scan.pdf".into(),
            kind: "Rapport".into(),
            url: "u".into(),
            content: "   ".into(),
            pages: 1,
            pdf_type: "scanned".into(),
        }];
        let chunks = chunk_document(&doc_with("Texte.", files));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].source, SOURCE_DECISION);
    }

    #[test]
    fn chunk_ids_are_valid_meilisearch_ids() {
        let files = vec![FileLink {
            name: "cp.pdf".into(),
            kind: "Communiqué".into(),
            url: "u".into(),
            content: "extrait".into(),
            pages: 1,
            pdf_type: "text_based".into(),
        }];
        let chunks = chunk_document(&doc_with("Texte de la décision.", files));
        assert!(!chunks.is_empty());
        for c in &chunks {
            assert!(
                c.id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'),
                "invalid Meilisearch document id: {}",
                c.id
            );
            assert!(c.id.len() <= 511);
        }
    }

    #[test]
    fn boundary_prefers_paragraph_then_sentence() {
        let text = format!("{}\n\n{}", "a".repeat(1500), "b".repeat(1500));
        let chunks = split_text(&text, TARGET_CHARS, OVERLAP_CHARS);
        assert!(chunks[0].ends_with(&"a".repeat(10)), "first chunk should stop at the paragraph break");
    }

    #[test]
    fn split_text_handles_multibyte_without_panicking() {
        let text = "Décision — arrêt n° 145. ".repeat(400);
        let chunks = split_text(&text, TARGET_CHARS, OVERLAP_CHARS);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| !c.is_empty()));
    }
}
