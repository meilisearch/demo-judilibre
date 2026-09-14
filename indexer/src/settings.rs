//! Meilisearch index settings, optional embedder, and chat configuration.

use anyhow::Result;
use serde_json::json;
use tracing::info;

use crate::meili::MeiliClient;

pub async fn apply_index_settings(meili: &MeiliClient, index: &str) -> Result<()> {
    meili.create_index(index, "id").await?;
    let settings = json!({
        "searchableAttributes": [
            "number", "numbers", "ecli", "titles", "summary", "themes", "visa", "excerpt", "text",
            "files.content"
        ],
        "filterableAttributes": [
            // `id` is filterable so the web app can fetch a single decision with a
            // search-only key, instead of needing an admin key for GET /documents/:id.
            "id",
            "jurisdiction", "chamber", "formation", "publication", "type", "solution",
            "themes", "year", "decision_timestamp", "particular_interest", "location",
            "files.type",
            // Joins a decision to the LEGI code articles it applies.
            "visa_refs"
        ],
        "sortableAttributes": ["decision_timestamp"],
        "rankingRules": ["words", "typo", "proximity", "attribute", "sort", "exactness"],
        "typoTolerance": {
            "disableOnAttributes": ["number", "numbers", "ecli"]
        },
        "faceting": { "maxValuesPerFacet": 200 },
        "pagination": { "maxTotalHits": 10000 },
        "stopWords": [
            "le", "la", "les", "de", "des", "du", "un", "une", "et", "en", "au", "aux",
            "que", "qui", "dans", "par", "pour", "sur", "ce", "cette", "ces", "est", "a"
        ]
    });
    info!(index, "applying index settings");
    meili.update_settings(index, &settings).await
}

/// Name of the embedder configured on the index (referenced by hybrid search).
pub const EMBEDDER_NAME: &str = "voyage";

/// Settings for the passage index: one document per chunk of a decision.
pub async fn apply_chunk_index_settings(meili: &MeiliClient, index: &str) -> Result<()> {
    meili.create_index(index, "id").await?;
    let settings = json!({
        "searchableAttributes": ["titles", "summary", "themes", "number", "ecli", "content"],
        "filterableAttributes": [
            "decision_id", "source", "attachment_type", "jurisdiction", "chamber", "formation",
            "publication", "type", "solution", "themes", "year", "decision_timestamp", "chunk_index"
        ],
        "sortableAttributes": ["decision_timestamp", "chunk_index"],
        // One passage per decision by default: results read like a list of decisions.
        "distinctAttribute": "decision_id",
        "rankingRules": ["words", "typo", "proximity", "attribute", "sort", "exactness"],
        "typoTolerance": { "disableOnAttributes": ["number", "ecli"] },
        "faceting": { "maxValuesPerFacet": 200 },
        "pagination": { "maxTotalHits": 10000 },
        "stopWords": [
            "le", "la", "les", "de", "des", "du", "un", "une", "et", "en", "au", "aux",
            "que", "qui", "dans", "par", "pour", "sur", "ce", "cette", "ces", "est", "a"
        ]
    });
    info!(index, "applying chunk index settings");
    meili.update_settings(index, &settings).await
}

/// Configure a Voyage AI embedder through Meilisearch's generic REST source.
/// Voyage accepts batched inputs (`input: [...]`) and returns `data[].embedding`.
/// Which document shape an embedder serves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbedderKind {
    /// One whole decision.
    Decision,
    /// One passage of a decision or of an attached PDF.
    Chunk,
    /// One in-force code article.
    Article,
}

pub async fn apply_voyage_embedder(
    meili: &MeiliClient,
    index: &str,
    api_key: &str,
    model: &str,
    kind: EmbedderKind,
) -> Result<()> {
    info!(index, model, ?kind, "configuring Voyage AI embedder for hybrid search");
    let embedders = json!({
        EMBEDDER_NAME: {
            "source": "rest",
            "url": "https://api.voyageai.com/v1/embeddings",
            "apiKey": api_key,
            "dimensions": 1024,
            "request": {
                "model": model,
                "input": ["{{text}}", "{{..}}"],
                "truncation": true
            },
            "response": {
                "data": [{ "embedding": "{{embedding}}" }, "{{..}}"]
            },
            "documentTemplate": match kind {
                EmbedderKind::Decision => EMBEDDING_TEMPLATE,
                EmbedderKind::Chunk => CHUNK_EMBEDDING_TEMPLATE,
                EmbedderKind::Article => ARTICLE_EMBEDDING_TEMPLATE,
            },
            "documentTemplateMaxBytes": match kind {
                EmbedderKind::Decision => 4000,
                EmbedderKind::Chunk => 8000,
                EmbedderKind::Article => 6000,
            }
        }
    });
    meili.update_embedders(index, &embedders).await
}

/// Chat provider and target indexes.
pub struct ChatConfig<'a> {
    pub index: &'a str,
    pub chunk_index: &'a str,
    pub legi_index: &'a str,
    pub workspace: &'a str,
    /// `openAi`, `mistral`, `azureOpenAi`, `vLlm` or `gemini`.
    pub source: &'a str,
    pub api_key: &'a str,
    /// Required by mistral, vLlm and azureOpenAi.
    pub base_url: Option<&'a str>,
    /// Embedder name enabling hybrid retrieval, when configured.
    pub embedder: Option<&'a str>,
}

pub async fn apply_chat(meili: &MeiliClient, config: &ChatConfig<'_>) -> Result<()> {
    let ChatConfig { index, chunk_index, legi_index, workspace, source, api_key, base_url, embedder } = *config;
    info!("enabling chatCompletions experimental feature");
    if let Err(e) = meili.enable_experimental(&json!({ "chatCompletions": true })).await {
        tracing::warn!(
            error = %e,
            "could not toggle experimental features (on Meilisearch Cloud, enable \"Chat completions\" from the project settings); continuing"
        );
    }

    let mut workspace_settings = json!({
        "source": source,
        "apiKey": api_key,
        "prompts": {
            "system": SYSTEM_PROMPT,
            "searchDescription": "Recherche dans les articles en vigueur des codes français (Légifrance). Pour toute question de droit, cherche l'article qui fonde la réponse et cite-le.",
            "searchQParam": "Mots-clés juridiques en français à rechercher (notions, numéro d'article, intitulé de code). Reste concis : 2 à 8 mots.",
            "searchFilterParam": "Filtre Meilisearch optionnel. Attributs : code, code_id, number, reference_key, section, year (entier), date_debut_timestamp (unix). Exemple : code = 'Code du travail'",
            "searchIndexUidParam": format!(
                "Index à interroger : '{legi_index}', les articles en vigueur des codes français. \
                 C'est le seul index disponible."
            )
        }
    });
    if let Some(url) = base_url {
        workspace_settings["baseUrl"] = json!(url);
    }
    info!(workspace, source, "configuring chat workspace");
    meili.update_chat_workspace(workspace, &workspace_settings).await?;

    let mut search_parameters = json!({ "limit": 6 });
    if let Some(embedder) = embedder {
        search_parameters["hybrid"] = json!({ "embedder": embedder, "semanticRatio": 0.5 });
    }
    // Withdrawn from the chat: no wording of the prompts kept the model from reaching
    // for the decisions first, and an empty `description` is the one control that
    // reliably removes an index from the chat. The index itself is untouched — the
    // search UI still browses it, and restoring a description brings it back.
    let index_chat = json!({
        "description": "",
        "documentTemplate": DOCUMENT_TEMPLATE,
        "documentTemplateMaxBytes": 7000,
        "searchParameters": search_parameters.clone()
    });
    info!(index, "configuring index chat settings");
    meili.update_index_chat_settings(index, &index_chat).await?;

    // Withdrawn from the chat: with three indexes offered, the model reached for
    // the passages first whatever the prompts said. The assistant now works from
    // the codes and then whole decisions, which is the order we want. The index
    // itself stays — it is what the passage search would use if reinstated.
    let chunk_chat = json!({
        "description": "",
        "documentTemplate": CHUNK_DOCUMENT_TEMPLATE,
        "documentTemplateMaxBytes": 4000,
        "searchParameters": search_parameters
    });
    info!(chunk_index, "configuring chunk index chat settings");
    meili.update_index_chat_settings(chunk_index, &chunk_chat).await?;

    let legi_chat = json!({
        "description": "Articles en vigueur des codes français (Légifrance/LEGI) : le texte de la règle elle-même, avec son code et sa place dans celui-ci. Seule source de l'assistant : toute réponse doit s'appuyer sur les articles qu'il renvoie.",
        "documentTemplate": ARTICLE_DOCUMENT_TEMPLATE,
        "documentTemplateMaxBytes": 4000,
        "searchParameters": { "limit": 6 }
    });
    info!(legi_index, "configuring LEGI index chat settings");
    meili.update_index_chat_settings(legi_index, &legi_chat).await?;

    // Scoped to exactly the indexes the assistant is offered. This, not the per-index
    // `description`, is what actually confines the assistant to one corpus: an empty
    // description does not stop the model passing an `index_uid` of its own, and
    // Meilisearch will run that search if the key allows it.
    let chat_indexes = [legi_index];
    match meili.find_chat_key(&chat_indexes).await? {
        Some(key) => info!(
            "chat API key (set MEILI_CHAT_KEY in .env for the web app): {}",
            key
        ),
        None => {
            info!(indexes = ?chat_indexes, "no chat key covers the assistant's indexes; creating one");
            let key = meili
                .create_key("judilibre-chat", &["search", "chatCompletions"], &chat_indexes)
                .await?;
            info!(
                "chat API key created — set MEILI_CHAT_KEY in .env for the web app: {}",
                key
            );
        }
    }
    Ok(())
}

const SYSTEM_PROMPT: &str = "Tu es un assistant juridique adossé à une seule source : les articles \
des codes français en vigueur (Légifrance). \
Réponds en français, de façon précise et structurée. \
Appuie chaque affirmation sur les articles retournés par la recherche : cite le code et le numéro \
d'article (par exemple « article L. 1152-1 du code du travail »), et reprends la formulation du texte \
plutôt que de la paraphraser librement. \
Si les articles trouvés ne permettent pas de répondre, dis-le clairement plutôt que d'inventer, et \
n'invente jamais un numéro d'article. \
Tu n'as pas accès à la jurisprudence : si la question appelle une interprétation que le texte ne \
tranche pas, signale-le au lieu de citer des décisions que tu n'as pas consultées. \
Ne donne pas de conseil juridique personnalisé : rappelle que la réponse est informative. \
Lorsque la question vise un code précis, utilise le paramètre de filtre. \
Les articles renvoyés sont ceux en vigueur aujourd'hui : si la question porte sur une situation \
passée, signale que la rédaction applicable à l'époque pouvait différer. \
Sois économe en recherches : trois à cinq requêtes bien choisies suffisent presque toujours. \
Ne relance pas une recherche pour reformuler la même idée, et réponds dès que les articles \
trouvés permettent de le faire.";

/// Text embedded per decision: citation line, titrage, sommaire and the start of the
/// motivations. voyage-law-2 handles 16K tokens, so 4 000 bytes is comfortable.
const EMBEDDING_TEMPLATE: &str = "{{doc.jurisdiction}}, {{doc.chamber}}, {{doc.decision_date}}, {{doc.solution}}. \
{{doc.titles}}. {{doc.summary}} {{doc.excerpt}}";

/// What the assistant sees for each retrieved code article.
const ARTICLE_DOCUMENT_TEMPLATE: &str = "{{doc.reference}} ({{doc.section}})\n{{doc.text}}";

/// Text embedded per code article: its reference, where it sits in the code, and its rule.
const ARTICLE_EMBEDDING_TEMPLATE: &str = "{{doc.reference}}. {{doc.hierarchy}}. {{doc.text}}";

/// Text embedded per passage: a short citation header plus the passage itself.
const CHUNK_EMBEDDING_TEMPLATE: &str = "{{doc.jurisdiction}}, {{doc.chamber}}, {{doc.decision_date}}, pourvoi n° {{doc.number}}. \
{{doc.titles}}. {{doc.content}}";

/// What the assistant sees for each retrieved passage.
const CHUNK_DOCUMENT_TEMPLATE: &str = "{{doc.jurisdiction}}, {{doc.chamber}}, {{doc.decision_date}}, pourvoi n° {{doc.number}} \
(solution : {{doc.solution}}){% if doc.source == 'attachment' %} — extrait du document associé « {{doc.attachment_name}} » ({{doc.attachment_type}}){% endif %}.\n\
Titrage : {{doc.titles}}\n\
Extrait : {{doc.content}}";

const DOCUMENT_TEMPLATE: &str = "{{doc.jurisdiction}}, {{doc.chamber}}{% if doc.formation != '' %} ({{doc.formation}}){% endif %}, \
{{doc.decision_date}}, pourvoi n° {{doc.number}}{% if doc.ecli != '' %}, {{doc.ecli}}{% endif %}. \
Solution : {{doc.solution}}. Publication : {{doc.publication}}.\n\
Titrage : {{doc.titles}}\n\
Sommaire : {{doc.summary}}\n\
Textes appliqués : {{doc.visa}}\n\
Motivations et dispositif : {{doc.excerpt}}";
