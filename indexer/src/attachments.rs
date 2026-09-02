//! Download the PDFs attached to a decision (communiqué, rapport du conseiller,
//! avis de l'avocat général, note explicative…) and extract their text with
//! `pdf-inspector`, so their content becomes searchable alongside the decision.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use pdf_inspector::{PdfType, process_pdf_mem};
use tracing::{debug, warn};

use crate::transform::{Document, FileLink};

/// Extracted text kept per attachment. Judilibre PDFs are short (a few pages),
/// but a cap keeps Meilisearch documents bounded.
const MAX_CONTENT_CHARS: usize = 40_000;

/// Refuse oversized downloads: attachments are normally well under 1 MB.
const DEFAULT_MAX_BYTES: u64 = 25 * 1024 * 1024;

pub struct AttachmentFetcher {
    http: reqwest::Client,
    max_bytes: u64,
}

/// Outcome of processing one PDF.
#[derive(Debug, PartialEq)]
pub struct Extracted {
    pub content: String,
    pub pages: u32,
    /// `text_based`, `scanned`, `image_based` or `mixed`.
    pub pdf_type: String,
}

#[derive(Debug, Default)]
pub struct AttachmentStats {
    pub extracted: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl AttachmentFetcher {
    pub fn new(max_bytes: Option<u64>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("judilibre-meilisearch-demo/0.1")
            .build()?;
        Ok(Self { http, max_bytes: max_bytes.unwrap_or(DEFAULT_MAX_BYTES) })
    }

    /// Fetch one PDF and extract its markdown. Parsing runs on a blocking thread
    /// because `pdf-inspector` is CPU-bound.
    pub async fn extract(&self, url: &str) -> Result<Extracted> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("downloading {url}"))?;
        if !resp.status().is_success() {
            bail!("attachment {url} returned {}", resp.status());
        }
        if let Some(len) = resp.content_length()
            && len > self.max_bytes
        {
            bail!("attachment {url} is {len} bytes, over the {} byte limit", self.max_bytes);
        }
        let bytes = resp.bytes().await.with_context(|| format!("reading {url}"))?;
        if bytes.len() as u64 > self.max_bytes {
            bail!("attachment {url} is {} bytes, over the limit", bytes.len());
        }
        if !bytes.starts_with(b"%PDF") {
            bail!("attachment {url} is not a PDF");
        }

        let buffer = bytes.to_vec();
        let result = tokio::task::spawn_blocking(move || process_pdf_mem(&buffer))
            .await
            .context("PDF extraction task panicked")?
            .map_err(|e| anyhow::anyhow!("pdf-inspector failed on {url}: {e}"))?;

        Ok(Extracted {
            content: clean_markdown(result.markdown.as_deref().unwrap_or_default()),
            pages: result.page_count,
            pdf_type: pdf_type_label(&result.pdf_type).to_string(),
        })
    }

    /// Fill `content` for every attachment of `doc`, logging (not failing) on errors.
    /// A scanned PDF yields no text without OCR, which we do not run.
    pub async fn enrich(&self, doc: &mut Document, stats: &mut AttachmentStats) {
        for file in &mut doc.files {
            if file.url.is_empty() {
                stats.skipped += 1;
                continue;
            }
            match self.extract(&file.url).await {
                Ok(extracted) if extracted.content.is_empty() => {
                    debug!(url = %file.url, pdf_type = %extracted.pdf_type, "no text in attachment");
                    file.pages = extracted.pages;
                    file.pdf_type = extracted.pdf_type;
                    stats.skipped += 1;
                }
                Ok(extracted) => {
                    debug!(url = %file.url, pages = extracted.pages, chars = extracted.content.chars().count(), "extracted attachment");
                    file.content = extracted.content;
                    file.pages = extracted.pages;
                    file.pdf_type = extracted.pdf_type;
                    stats.extracted += 1;
                }
                Err(e) => {
                    warn!(url = %file.url, error = %e, "could not extract attachment");
                    stats.failed += 1;
                }
            }
        }
    }
}

fn pdf_type_label(pdf_type: &PdfType) -> &'static str {
    match pdf_type {
        PdfType::TextBased => "text_based",
        PdfType::Scanned => "scanned",
        PdfType::ImageBased => "image_based",
        PdfType::Mixed => "mixed",
    }
}


/// PDF extraction returns Markdown. The index and the LLM want prose, so drop the
/// syntax markers while keeping the line structure.
fn strip_markdown(line: &str) -> String {
    let without_heading = line.trim_start().trim_start_matches('#').trim_start();
    let mut out = String::with_capacity(without_heading.len());
    let mut chars = without_heading.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Emphasis markers: **bold**, *italic*, __bold__, _italic_.
            '*' | '_' => {
                while chars.peek() == Some(&c) {
                    chars.next();
                }
            }
            '`' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Letter-spaced PDF headings extract as doubled glyphs, sometimes with stray
/// spaces: "C COOMMMMUUNNIIQQUUÉÉ" for "COMMUNIQUÉ". When a whole line is nothing
/// but doubled characters, halve it. Prose never satisfies that, and the guards on
/// length and distinct characters keep runs like "aaaa" or "ll" untouched.
fn undouble_line(line: &str) -> String {
    let compact: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 8 || !compact.len().is_multiple_of(2) {
        return line.to_string();
    }
    let mut halved = String::with_capacity(compact.len() / 2);
    for pair in compact.chunks(2) {
        if pair[0] != pair[1] {
            return line.to_string();
        }
        halved.push(pair[0]);
    }
    let distinct = halved.chars().collect::<std::collections::BTreeSet<_>>().len();
    if distinct < 3 {
        return line.to_string();
    }
    halved
}

/// Collapse the blank lines and trailing spaces PDF extraction leaves behind, and
/// cap the result so one attachment cannot dominate a Meilisearch document.
pub fn clean_markdown(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_CONTENT_CHARS * 2));
    let mut blank_run = 0;
    for line in raw.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let cleaned = undouble_line(&strip_markdown(line));
        let line = cleaned.trim_end();
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
    let out = out.trim();
    if out.chars().count() <= MAX_CONTENT_CHARS {
        return out.to_string();
    }
    let mut truncated: String = out.chars().take(MAX_CONTENT_CHARS).collect();
    truncated.push('…');
    truncated
}

/// Total characters of extracted attachment text on a document.
pub fn extracted_chars(files: &[FileLink]) -> usize {
    files.iter().map(|f| f.content.chars().count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_markdown_collapses_blank_lines_and_trims() {
        assert_eq!(clean_markdown("Titre  \n\n\n\ntexte \r\n\r\n"), "Titre\n\ntexte");
        assert_eq!(clean_markdown(""), "");
        assert_eq!(clean_markdown("   \n  \n"), "");
    }

    #[test]
    fn clean_markdown_caps_long_content() {
        let long = "a".repeat(MAX_CONTENT_CHARS + 500);
        let cleaned = clean_markdown(&long);
        assert_eq!(cleaned.chars().count(), MAX_CONTENT_CHARS + 1);
        assert!(cleaned.ends_with('…'));
    }

    #[test]
    fn clean_markdown_keeps_multibyte_text_intact() {
        let cleaned = clean_markdown("Détention provisoire — arrêt n° 145");
        assert_eq!(cleaned, "Détention provisoire — arrêt n° 145");
    }

    #[test]
    fn clean_markdown_strips_syntax_markers() {
        assert_eq!(clean_markdown("# Titre principal"), "Titre principal");
        assert_eq!(clean_markdown("**Régularité** de la *détention*"), "Régularité de la détention");
        assert_eq!(clean_markdown("un `code` inline"), "un code inline");
        // A lone underscore inside a word must not swallow the rest of the line.
        assert_eq!(clean_markdown("article L. 434_9 du code"), "article L. 4349 du code");
    }

    #[test]
    fn clean_markdown_undoubles_letterspaced_headings() {
        assert_eq!(clean_markdown("# C COOMMMMUUNNIIQQUUÉÉ"), "COMMUNIQUÉ");
        // Prose with legitimate double letters is untouched.
        assert_eq!(clean_markdown("La cour d'appel a retenu"), "La cour d'appel a retenu");
        assert_eq!(clean_markdown("attendu que"), "attendu que");
        assert_eq!(clean_markdown("ll"), "ll");
        // A long run of one character is not a letter-spaced heading.
        assert_eq!(clean_markdown(&"a".repeat(20)), "a".repeat(20));
    }

    #[test]
    fn extracted_chars_sums_attachment_content() {
        let files = vec![
            FileLink { name: "a".into(), kind: "Communiqué".into(), url: "u".into(), content: "abc".into(), pages: 1, pdf_type: "text_based".into() },
            FileLink { name: "b".into(), kind: "Rapport".into(), url: "v".into(), content: "dé".into(), pages: 2, pdf_type: "text_based".into() },
        ];
        assert_eq!(extracted_chars(&files), 5);
    }
}
