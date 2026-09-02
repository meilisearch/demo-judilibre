//! Minimal Meilisearch HTTP client (documents, settings, tasks, chat).

use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, warn};

pub struct MeiliClient {
    http: reqwest::Client,
    base_url: String,
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskRef {
    #[serde(rename = "taskUid")]
    pub task_uid: u64,
}

#[derive(Debug, Deserialize)]
struct Task {
    status: String,
    error: Option<Value>,
}

impl MeiliClient {
    pub fn new(base_url: &str, key: Option<&str>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            key: key.map(str::to_string),
        })
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self.http.request(method, format!("{}{}", self.base_url, path));
        if let Some(k) = &self.key {
            req = req.bearer_auth(k);
        }
        req
    }

    async fn send_json<T: for<'de> Deserialize<'de>>(&self, req: reqwest::RequestBuilder) -> Result<T> {
        let resp = req.send().await.context("Meilisearch request failed")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("Meilisearch returned {status}: {body}");
        }
        serde_json::from_str(&body).with_context(|| format!("decoding Meilisearch response: {body}"))
    }

    pub async fn health(&self) -> Result<()> {
        let _: Value = self.send_json(self.request(Method::GET, "/health")).await?;
        Ok(())
    }

    /// Wait for a task to succeed. Fails if the task fails.
    pub async fn wait_for_task(&self, uid: u64) -> Result<()> {
        loop {
            let task: Task = self.send_json(self.request(Method::GET, &format!("/tasks/{uid}"))).await?;
            match task.status.as_str() {
                "succeeded" => return Ok(()),
                "failed" | "canceled" => bail!("Meilisearch task {uid} {}: {:?}", task.status, task.error),
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    }

    /// Create the index if needed. Meilisearch reports an existing index as a failed
    /// task (`index_already_exists`), which we treat as success.
    pub async fn create_index(&self, uid: &str, primary_key: &str) -> Result<()> {
        let resp = self
            .request(Method::POST, "/indexes")
            .json(&json!({ "uid": uid, "primaryKey": primary_key }))
            .send()
            .await?;
        if !resp.status().is_success() {
            bail!("creating index failed with {}: {}", resp.status(), resp.text().await.unwrap_or_default());
        }
        let task: TaskRef = resp.json().await?;
        match self.wait_for_task(task.task_uid).await {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("index_already_exists") => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn update_settings(&self, uid: &str, settings: &Value) -> Result<()> {
        let task: TaskRef = self
            .send_json(self.request(Method::PATCH, &format!("/indexes/{uid}/settings")).json(settings))
            .await?;
        self.wait_for_task(task.task_uid).await
    }

    pub async fn update_embedders(&self, uid: &str, embedders: &Value) -> Result<()> {
        let task: TaskRef = self
            .send_json(self.request(Method::PATCH, &format!("/indexes/{uid}/settings/embedders")).json(embedders))
            .await?;
        self.wait_for_task(task.task_uid).await
    }

    pub async fn add_documents<T: Serialize>(&self, uid: &str, docs: &[T]) -> Result<u64> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let resp = self
                .request(Method::POST, &format!("/indexes/{uid}/documents"))
                .json(docs)
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let task: TaskRef = r.json().await?;
                    debug!(task = task.task_uid, count = docs.len(), "documents enqueued");
                    return Ok(task.task_uid);
                }
                Ok(r) if attempt <= 5 && (r.status() == StatusCode::TOO_MANY_REQUESTS || r.status().is_server_error()) => {
                    warn!(status = %r.status(), "Meilisearch busy, retrying");
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
                Ok(r) => bail!("adding documents failed with {}: {}", r.status(), r.text().await.unwrap_or_default()),
                Err(e) if attempt <= 5 => {
                    warn!(error = %e, "Meilisearch request error, retrying");
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub async fn delete_document(&self, uid: &str, id: &str) -> Result<u64> {
        let task: TaskRef = self
            .send_json(self.request(Method::DELETE, &format!("/indexes/{uid}/documents/{id}")))
            .await?;
        Ok(task.task_uid)
    }

    /// Delete every document matching a Meilisearch filter expression.
    pub async fn delete_documents_by_filter(&self, uid: &str, filter: &str) -> Result<u64> {
        let task: TaskRef = self
            .send_json(
                self.request(Method::POST, &format!("/indexes/{uid}/documents/delete"))
                    .json(&json!({ "filter": filter })),
            )
            .await?;
        Ok(task.task_uid)
    }

    pub async fn enable_experimental(&self, features: &Value) -> Result<()> {
        let _: Value = self
            .send_json(self.request(Method::PATCH, "/experimental-features").json(features))
            .await?;
        Ok(())
    }

    pub async fn update_chat_workspace(&self, workspace: &str, settings: &Value) -> Result<()> {
        let _: Value = self
            .send_json(self.request(Method::PATCH, &format!("/chats/{workspace}/settings")).json(settings))
            .await?;
        Ok(())
    }

    pub async fn update_index_chat_settings(&self, uid: &str, settings: &Value) -> Result<()> {
        let task: TaskRef = self
            .send_json(self.request(Method::PATCH, &format!("/indexes/{uid}/settings/chat")).json(settings))
            .await?;
        self.wait_for_task(task.task_uid).await
    }

    /// Return the value of the first API key that has the given action (e.g. `chatCompletions`).
    pub async fn find_key_with_action(&self, action: &str) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Key {
            key: String,
            actions: Vec<String>,
        }
        #[derive(Deserialize)]
        struct Keys {
            results: Vec<Key>,
        }
        let keys: Keys = self.send_json(self.request(Method::GET, "/keys?limit=100")).await?;
        Ok(keys
            .results
            .into_iter()
            .find(|k| k.actions.iter().any(|a| a == action))
            .map(|k| k.key))
    }
}
