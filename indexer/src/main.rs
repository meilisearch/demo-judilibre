mod attachments;
mod chunk;
mod judilibre;
mod meili;
mod settings;
mod transform;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use tracing::{info, warn};

use crate::judilibre::{ExportPlan, ExportQuery, JudilibreClient, PisteAuth};
use crate::meili::MeiliClient;

/// Index French court decisions (Judilibre, Cour de cassation) into Meilisearch.
#[derive(Parser, Debug)]
#[command(name = "judilibre-indexer", version, about)]
struct Cli {
    /// Meilisearch URL
    #[arg(long, env = "MEILI_URL", default_value = "http://localhost:7700")]
    meili_url: String,

    /// Meilisearch master (or admin) key
    #[arg(long, env = "MEILI_MASTER_KEY")]
    meili_key: Option<String>,

    /// Meilisearch index uid
    #[arg(long, env = "MEILI_INDEX", default_value = "decisions")]
    index: String,

    /// Index holding the passages (chunks). Defaults to `<index>_chunk`.
    #[arg(long, env = "MEILI_CHUNK_INDEX")]
    chunk_index: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create the index, apply search settings, and configure the chat workspace.
    Setup {
        /// Chat workspace name
        #[arg(long, env = "CHAT_WORKSPACE", default_value = "judilibre")]
        workspace: String,
        /// LLM provider used by Meilisearch chat: openAi | mistral | azureOpenAi | vLlm | gemini
        #[arg(long, env = "CHAT_SOURCE", default_value = "openAi")]
        chat_source: String,
        /// API key for the LLM provider
        #[arg(long, env = "CHAT_API_KEY")]
        chat_api_key: Option<String>,
        /// Base URL for the LLM provider (required for mistral / vLlm / azureOpenAi)
        #[arg(long, env = "CHAT_BASE_URL")]
        chat_base_url: Option<String>,
        /// Voyage AI API key for the optional hybrid-search embedder (enables semantic search)
        #[arg(long, env = "VOYAGE_API_KEY")]
        voyage_api_key: Option<String>,
        /// Voyage AI embedding model (voyage-law-2 is tuned for legal text; voyage-4 is the general model)
        #[arg(long, env = "VOYAGE_MODEL", default_value = "voyage-law-2")]
        voyage_model: String,
        /// Skip chat configuration (search only)
        #[arg(long)]
        no_chat: bool,
    },

    /// Export decisions from the Judilibre API and push them into Meilisearch.
    Index {
        /// Judilibre API base URL (PISTE production or sandbox)
        #[arg(
            long,
            env = "JUDILIBRE_API_URL",
            default_value = "https://api.piste.gouv.fr/cassation/judilibre/v1.0"
        )]
        api_url: String,
        /// PISTE application KeyId (API-key application)
        #[arg(long, env = "PISTE_KEY_ID")]
        key_id: Option<String>,
        /// PISTE OAuth client id (OAuth application, alternative to --key-id)
        #[arg(long, env = "PISTE_CLIENT_ID")]
        client_id: Option<String>,
        /// PISTE OAuth client secret
        #[arg(long, env = "PISTE_CLIENT_SECRET")]
        client_secret: Option<String>,
        /// PISTE OAuth token endpoint (sandbox: https://sandbox-oauth.piste.gouv.fr/api/oauth/token)
        #[arg(long, env = "PISTE_OAUTH_URL", default_value = "https://oauth.piste.gouv.fr/api/oauth/token")]
        oauth_url: String,
        /// First decision date to export (inclusive), YYYY-MM-DD
        #[arg(long, env = "DATE_START", default_value = "2023-01-01")]
        date_start: NaiveDate,
        /// Last decision date to export (inclusive), YYYY-MM-DD. Defaults to today.
        #[arg(long, env = "DATE_END")]
        date_end: Option<NaiveDate>,
        /// Jurisdictions to export (repeatable): cc (Cour de cassation), ca (cours d'appel), tj, tcom
        #[arg(long, env = "JURISDICTIONS", value_delimiter = ',', default_value = "cc")]
        jurisdiction: Vec<String>,
        /// Decisions per Judilibre batch (max 1000)
        #[arg(long, default_value_t = 500)]
        batch_size: u32,
        /// Stop after this many decisions (useful for quick demos)
        #[arg(long)]
        limit: Option<usize>,
        /// Milliseconds to sleep between Judilibre requests (rate limiting)
        #[arg(long, default_value_t = 250)]
        delay_ms: u64,
        /// Export decisions updated since date_start instead of decided since (incremental sync)
        #[arg(long)]
        updated: bool,
        /// Only export decisions with these publication levels (e.g. b for "Publié au Bulletin")
        #[arg(long, env = "PUBLICATION", value_delimiter = ',')]
        publication: Vec<String>,
        /// Download the PDFs attached to each decision and index their extracted text
        #[arg(long, env = "WITH_FILES")]
        with_files: bool,
        /// Skip writing passages to the chunk index
        #[arg(long)]
        no_chunks: bool,
    },
}

/// `.env` files often contain `KEY=` placeholders; treat those as unset.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_filename("../.env");
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                // lopdf logs a warning per unusual font encoding; decisions' PDFs trip it often.
                .unwrap_or_else(|_| "info,reqwest=warn,lopdf=error".into()),
        )
        .init();

    let cli = Cli::parse();
    let meili = MeiliClient::new(&cli.meili_url, non_empty(cli.meili_key.as_deref()))?;
    let chunk_index = cli
        .chunk_index
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{}_chunk", cli.index));
    meili.health().await.context("Meilisearch is not reachable")?;

    match cli.command {
        Command::Setup {
            workspace,
            chat_source,
            chat_api_key,
            chat_base_url,
            voyage_api_key,
            voyage_model,
            no_chat,
        } => {
            settings::apply_index_settings(&meili, &cli.index).await?;
            settings::apply_chunk_index_settings(&meili, &chunk_index).await?;
            let embedder = match non_empty(voyage_api_key.as_deref()) {
                Some(key) => {
                    settings::apply_voyage_embedder(&meili, &cli.index, key, &voyage_model, false).await?;
                    settings::apply_voyage_embedder(&meili, &chunk_index, key, &voyage_model, true).await?;
                    Some(settings::EMBEDDER_NAME.to_string())
                }
                None => {
                    info!("no VOYAGE_API_KEY set: hybrid/semantic search disabled");
                    None
                }
            };
            if no_chat {
                info!("--no-chat: skipping chat workspace configuration");
            } else {
                let Some(api_key) = non_empty(chat_api_key.as_deref()) else {
                    warn!("CHAT_API_KEY not set: skipping chat workspace configuration");
                    return Ok(());
                };
                settings::apply_chat(
                    &meili,
                    &settings::ChatConfig {
                        index: &cli.index,
                        chunk_index: &chunk_index,
                        workspace: &workspace,
                        source: &chat_source,
                        api_key,
                        base_url: non_empty(chat_base_url.as_deref()),
                        embedder: embedder.as_deref(),
                    },
                )
                .await?;
            }
            info!("setup complete");
        }

        Command::Index {
            api_url,
            key_id,
            client_id,
            client_secret,
            oauth_url,
            date_start,
            date_end,
            jurisdiction,
            batch_size,
            limit,
            delay_ms,
            updated,
            publication,
            with_files,
            no_chunks,
        } => {
            let date_end = date_end.unwrap_or_else(|| chrono::Utc::now().date_naive());
            let auth = match (
                non_empty(key_id.as_deref()),
                non_empty(client_id.as_deref()),
                non_empty(client_secret.as_deref()),
            ) {
                (_, Some(id), Some(secret)) => PisteAuth::OAuth {
                    client_id: id.to_string(),
                    client_secret: secret.to_string(),
                    token_url: oauth_url,
                },
                (Some(key), _, _) => PisteAuth::KeyId(key.to_string()),
                _ => anyhow::bail!(
                    "no PISTE credentials: set PISTE_KEY_ID (API-key application) or PISTE_CLIENT_ID + \
                     PISTE_CLIENT_SECRET (OAuth application). Create the application on https://piste.gouv.fr \
                     and subscribe it to Judilibre."
                ),
            };
            anyhow::ensure!(date_start <= date_end, "date_start must be <= date_end");
            anyhow::ensure!((1..=1000).contains(&batch_size), "batch_size must be 1..=1000");

            let judilibre = JudilibreClient::new(&api_url, auth, delay_ms)?;
            let query = ExportQuery {
                jurisdictions: jurisdiction,
                publication,
                batch_size,
                date_type: if updated { "update" } else { "creation" }.to_string(),
            };
            let fetcher = if with_files {
                Some(attachments::AttachmentFetcher::new(None)?)
            } else {
                None
            };
            let targets = judilibre::Targets {
                index: cli.index.clone(),
                chunk_index: (!no_chunks).then_some(chunk_index.clone()),
            };
            let stats = judilibre::run_export(
                &judilibre,
                &meili,
                &ExportPlan {
                    targets: &targets,
                    query: &query,
                    start: date_start,
                    end: date_end,
                    limit,
                    attachments: fetcher.as_ref(),
                },
            )
            .await?;
            info!(
                indexed = stats.indexed,
                chunks = stats.chunks,
                deleted = stats.deleted,
                requests = stats.requests,
                attachments_extracted = stats.attachments.extracted,
                attachments_skipped = stats.attachments.skipped,
                attachments_failed = stats.attachments.failed,
                "export finished"
            );
        }
    }
    Ok(())
}
