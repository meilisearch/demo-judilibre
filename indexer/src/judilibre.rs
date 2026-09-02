//! Client for the Judilibre `/export` endpoint (PISTE).
//!
//! The export endpoint is paginated by `batch` / `batch_size` and, like the
//! underlying Elasticsearch index, cannot page past roughly 10 000 results for
//! a single query. We therefore slice the requested period into date windows
//! and split any window whose total exceeds that cap.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::{Days, Months, NaiveDate};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::attachments::{AttachmentFetcher, AttachmentStats, extracted_chars};
use crate::chunk::{Chunk, chunk_document};
use crate::meili::MeiliClient;
use crate::transform::{Document, to_document};

/// Elasticsearch-style deep pagination cap on the Judilibre side.
pub const MAX_WINDOW_TOTAL: u64 = 10_000;

/// Approximate JSON payload size per Meilisearch document batch.
const MEILI_PAYLOAD_BUDGET: usize = 20 * 1024 * 1024;

/// How to authenticate against PISTE.
#[derive(Debug, Clone)]
pub enum PisteAuth {
    /// API-key application: `KeyId: <key>` header.
    KeyId(String),
    /// OAuth2 application: client-credentials token sent as `Authorization: Bearer`.
    OAuth { client_id: String, client_secret: String, token_url: String },
}

#[derive(Debug, Clone)]
struct Token {
    value: String,
    expires_at: Instant,
}

pub struct JudilibreClient {
    http: reqwest::Client,
    base_url: String,
    auth: PisteAuth,
    token: Mutex<Option<Token>>,
    delay: Duration,
}

#[derive(Debug, Clone)]
pub struct ExportQuery {
    pub jurisdictions: Vec<String>,
    /// Publication levels to keep (empty = all).
    pub publication: Vec<String>,
    pub batch_size: u32,
    /// `creation` (decision date) or `update` (last modification date)
    pub date_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportBatch {
    pub total: u64,
    pub next_batch: Option<String>,
    #[serde(default)]
    pub results: Vec<Value>,
}

#[derive(Debug, Default)]
pub struct ExportStats {
    pub indexed: usize,
    pub chunks: usize,
    pub deleted: usize,
    pub requests: usize,
    pub attachments: AttachmentStats,
}

/// Where documents and passages are written.
#[derive(Debug, Clone)]
pub struct Targets {
    pub index: String,
    /// `None` disables passage indexing.
    pub chunk_index: Option<String>,
}

impl JudilibreClient {
    pub fn new(base_url: &str, auth: PisteAuth, delay_ms: u64) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .gzip(true)
            .user_agent("judilibre-meilisearch-demo/0.1")
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            token: Mutex::new(None),
            delay: Duration::from_millis(delay_ms),
        })
    }

    /// Obtain (or reuse) an OAuth access token from PISTE.
    async fn oauth_token(&self, client_id: &str, client_secret: &str, token_url: &str) -> Result<String> {
        if let Some(t) = self.token.lock().await.as_ref()
            && t.expires_at > Instant::now() + Duration::from_secs(30)
        {
            return Ok(t.value.clone());
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            #[serde(default = "default_expiry")]
            expires_in: u64,
        }
        fn default_expiry() -> u64 {
            3600
        }
        let resp = self
            .http
            .post(token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("scope", "openid"),
            ])
            .send()
            .await
            .context("requesting PISTE OAuth token")?;
        if !resp.status().is_success() {
            bail!(
                "PISTE OAuth token request failed ({}): {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }
        let body: TokenResponse = resp.json().await.context("decoding PISTE OAuth token")?;
        let token = Token {
            value: body.access_token,
            expires_at: Instant::now() + Duration::from_secs(body.expires_in),
        };
        *self.token.lock().await = Some(token.clone());
        info!("obtained PISTE OAuth token");
        Ok(token.value)
    }

    async fn authorize(&self, req: reqwest::RequestBuilder) -> Result<reqwest::RequestBuilder> {
        Ok(match &self.auth {
            PisteAuth::KeyId(key) => req.header("KeyId", key),
            PisteAuth::OAuth { client_id, client_secret, token_url } => {
                let token = self.oauth_token(client_id, client_secret, token_url).await?;
                req.bearer_auth(token)
            }
        })
    }

    /// Fetch one export batch, retrying on throttling and transient errors.
    pub async fn export(
        &self,
        query: &ExportQuery,
        start: NaiveDate,
        end: NaiveDate,
        batch: u32,
    ) -> Result<ExportBatch> {
        let mut params: Vec<(&str, String)> = vec![
            ("date_start", start.to_string()),
            ("date_end", end.to_string()),
            ("date_type", query.date_type.clone()),
            ("batch_size", query.batch_size.to_string()),
            ("batch", batch.to_string()),
            ("order", "desc".to_string()),
            ("resolve_references", "true".to_string()),
        ];
        for j in &query.jurisdictions {
            params.push(("jurisdiction", j.clone()));
        }
        for p in &query.publication {
            params.push(("publication", p.clone()));
        }

        let url = format!("{}/export", self.base_url);
        let mut attempt = 0u32;
        let mut refreshed_token = false;
        loop {
            attempt += 1;
            tokio::time::sleep(self.delay).await;
            let req = self
                .http
                .get(&url)
                .query(&params)
                .header("Accept", "application/json");
            let resp = self.authorize(req).await?.send().await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    return r.json::<ExportBatch>().await.context("decoding export batch");
                }
                Ok(r) if r.status() == StatusCode::UNAUTHORIZED
                    && matches!(self.auth, PisteAuth::OAuth { .. })
                    && !refreshed_token =>
                {
                    warn!("PISTE token rejected, refreshing");
                    *self.token.lock().await = None;
                    refreshed_token = true;
                }
                Ok(r) if r.status() == StatusCode::UNAUTHORIZED || r.status() == StatusCode::FORBIDDEN => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    bail!(
                        "Judilibre rejected the PISTE credentials ({status}). Check that the PISTE application is \
                         subscribed to the Judilibre API for this environment ({}) and that PISTE_KEY_ID (API-key app) \
                         or PISTE_CLIENT_ID/PISTE_CLIENT_SECRET (OAuth app) match it. {body}",
                        self.base_url
                    );
                }
                Ok(r) if r.status() == StatusCode::TOO_MANY_REQUESTS || r.status().is_server_error() => {
                    if attempt > 6 {
                        bail!("Judilibre keeps failing ({}) after {attempt} attempts", r.status());
                    }
                    let wait = Duration::from_secs(2u64.pow(attempt.min(5)));
                    warn!(status = %r.status(), ?wait, "Judilibre throttled, backing off");
                    tokio::time::sleep(wait).await;
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    bail!("Judilibre export failed with {status}: {body}");
                }
                Err(e) if attempt <= 6 => {
                    warn!(error = %e, "Judilibre request error, retrying");
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt.min(5)))).await;
                }
                Err(e) => return Err(e).context("Judilibre request failed"),
            }
        }
    }
}

/// Split `[start, end]` into month-aligned windows, most recent first.
pub fn month_windows(start: NaiveDate, end: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut windows = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        let first_of_next = NaiveDate::from_ymd_opt(cursor.year(), cursor.month(), 1)
            .and_then(|d| d.checked_add_months(Months::new(1)))
            .expect("valid month arithmetic");
        let window_end = std::cmp::min(first_of_next.pred_opt().expect("valid date"), end);
        windows.push((cursor, window_end));
        cursor = first_of_next;
    }
    windows.reverse();
    windows
}

/// Split a window in two halves. Returns `None` for a single day (cannot split further).
pub fn split_window(start: NaiveDate, end: NaiveDate) -> Option<((NaiveDate, NaiveDate), (NaiveDate, NaiveDate))> {
    if start >= end {
        return None;
    }
    let days = (end - start).num_days();
    let mid = start + Days::new((days / 2) as u64);
    Some(((start, mid), (mid.succ_opt()?, end)))
}

/// Everything `run_export` needs besides the clients.
pub struct ExportPlan<'a> {
    pub targets: &'a Targets,
    pub query: &'a ExportQuery,
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// Stop after this many decisions.
    pub limit: Option<usize>,
    /// When set, attached PDFs are downloaded and their text extracted.
    pub attachments: Option<&'a AttachmentFetcher>,
}

pub async fn run_export(
    judilibre: &JudilibreClient,
    meili: &MeiliClient,
    plan: &ExportPlan<'_>,
) -> Result<ExportStats> {
    let ExportPlan { targets, query, start, end, limit, attachments } = *plan;
    let mut stats = ExportStats::default();
    let mut buffer = Buffer::default();
    let mut last_task: Option<u64> = None;
    let mut seen: HashSet<String> = HashSet::new();

    // Depth-first stack of windows to process, most recent first.
    let mut stack: Vec<(NaiveDate, NaiveDate)> = month_windows(start, end);
    stack.reverse(); // pop() yields most recent first

    'windows: while let Some((w_start, w_end)) = stack.pop() {
        let mut batch_no = 0u32;
        loop {
            let page = judilibre.export(query, w_start, w_end, batch_no).await?;
            stats.requests += 1;

            if batch_no == 0 {
                if page.total > MAX_WINDOW_TOTAL {
                    match split_window(w_start, w_end) {
                        Some((a, b)) => {
                            debug!(%w_start, %w_end, total = page.total, "window too large, splitting");
                            stack.push(a);
                            stack.push(b);
                            continue 'windows;
                        }
                        None => warn!(
                            %w_start,
                            total = page.total,
                            "single day exceeds the export cap; some decisions may be skipped"
                        ),
                    }
                }
                info!(%w_start, %w_end, total = page.total, "exporting window");
            }

            let count = page.results.len();
            for raw in page.results {
                if raw.get("to_be_deleted").and_then(Value::as_bool) == Some(true) {
                    if let Some(id) = raw.get("id").and_then(Value::as_str) {
                        meili.delete_document(&targets.index, id).await?;
                        if let Some(chunk_index) = &targets.chunk_index {
                            meili.delete_documents_by_filter(chunk_index, &format!("decision_id = '{id}'")).await?;
                        }
                        stats.deleted += 1;
                    }
                    continue;
                }
                let Some(mut doc) = to_document(&raw) else {
                    warn!("skipping decision without id");
                    continue;
                };
                // Overlapping windows can return the same decision twice.
                if !seen.insert(doc.id.clone()) {
                    continue;
                }
                if let Some(fetcher) = attachments {
                    fetcher.enrich(&mut doc, &mut stats.attachments).await;
                    let chars = extracted_chars(&doc.files);
                    if chars > 0 {
                        debug!(id = %doc.id, files = doc.files.len(), chars, "attachment text added");
                    }
                }
                if targets.chunk_index.is_some() {
                    let chunks = chunk_document(&doc);
                    stats.chunks += chunks.len();
                    for c in chunks {
                        buffer.chunk_bytes += c.approx_size();
                        buffer.chunks.push(c);
                    }
                }
                buffer.doc_bytes += doc.approx_size();
                buffer.docs.push(doc);
                stats.indexed += 1;

                if let Some(task) = buffer.flush_if_full(meili, targets).await? {
                    last_task = Some(task);
                    info!(sent = stats.indexed, chunks = stats.chunks, "batch queued in Meilisearch");
                }
                if limit.is_some_and(|l| stats.indexed >= l) {
                    info!(limit = stats.indexed, "reached --limit");
                    break 'windows;
                }
            }

            if page.next_batch.is_none() || count == 0 {
                break;
            }
            batch_no += 1;
        }
    }

    if let Some(task) = buffer.flush(meili, targets).await? {
        last_task = Some(task);
    }
    if let Some(task) = last_task {
        info!(task, "waiting for Meilisearch to finish indexing");
        meili.wait_for_task(task).await?;
    }
    Ok(stats)
}

/// Pending documents and passages, flushed to Meilisearch in bounded payloads.
#[derive(Default)]
struct Buffer {
    docs: Vec<Document>,
    doc_bytes: usize,
    chunks: Vec<Chunk>,
    chunk_bytes: usize,
}

impl Buffer {
    async fn flush_if_full(&mut self, meili: &MeiliClient, targets: &Targets) -> Result<Option<u64>> {
        if self.doc_bytes < MEILI_PAYLOAD_BUDGET && self.chunk_bytes < MEILI_PAYLOAD_BUDGET {
            return Ok(None);
        }
        self.flush(meili, targets).await
    }

    async fn flush(&mut self, meili: &MeiliClient, targets: &Targets) -> Result<Option<u64>> {
        let mut task = None;
        if !self.docs.is_empty() {
            task = Some(meili.add_documents(&targets.index, &self.docs).await?);
            self.docs.clear();
            self.doc_bytes = 0;
        }
        if let Some(chunk_index) = &targets.chunk_index
            && !self.chunks.is_empty()
        {
            task = Some(meili.add_documents(chunk_index, &self.chunks).await?);
            self.chunks.clear();
            self.chunk_bytes = 0;
        }
        Ok(task)
    }
}

// Bring `Datelike` into scope for `.year()` / `.month()`.
use chrono::Datelike;

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn month_windows_are_aligned_and_most_recent_first() {
        let w = month_windows(d("2024-01-15"), d("2024-03-10"));
        assert_eq!(
            w,
            vec![
                (d("2024-03-01"), d("2024-03-10")),
                (d("2024-02-01"), d("2024-02-29")),
                (d("2024-01-15"), d("2024-01-31")),
            ]
        );
    }

    #[test]
    fn month_windows_single_day() {
        assert_eq!(month_windows(d("2024-05-05"), d("2024-05-05")), vec![(d("2024-05-05"), d("2024-05-05"))]);
    }

    #[test]
    fn split_window_halves_and_stops_at_one_day() {
        let (a, b) = split_window(d("2024-01-01"), d("2024-01-31")).unwrap();
        assert_eq!(a, (d("2024-01-01"), d("2024-01-16")));
        assert_eq!(b, (d("2024-01-17"), d("2024-01-31")));
        assert!(split_window(d("2024-01-01"), d("2024-01-01")).is_none());
        let (a, b) = split_window(d("2024-01-01"), d("2024-01-02")).unwrap();
        assert_eq!(a, (d("2024-01-01"), d("2024-01-01")));
        assert_eq!(b, (d("2024-01-02"), d("2024-01-02")));
    }
}
