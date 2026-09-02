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
            "files.type"
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
pub async fn apply_voyage_embedder(
    meili: &MeiliClient,
    index: &str,
    api_key: &str,
    model: &str,
    chunks: bool,
) -> Result<()> {
    info!(index, model, chunks, "configuring Voyage AI embedder for hybrid search");
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
            "documentTemplate": if chunks { CHUNK_EMBEDDING_TEMPLATE } else { EMBEDDING_TEMPLATE },
            "documentTemplateMaxBytes": if chunks { 8000 } else { 4000 }
        }
    });
    meili.update_embedders(index, &embedders).await
}

/// Chat provider and target indexes.
pub struct ChatConfig<'a> {
    pub index: &'a str,
    pub chunk_index: &'a str,
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
    let ChatConfig { index, chunk_index, workspace, source, api_key, base_url, embedder } = *config;
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
            "searchDescription": "Recherche dans la base Judilibre des décisions de justice françaises (Cour de cassation, cours d'appel). Utilise-la pour toute question de droit ou de jurisprudence.",
            "searchQParam": "Mots-clés juridiques en français à rechercher (notions, articles de code, numéro de pourvoi, ECLI). Reste concis : 2 à 8 mots.",
            "searchFilterParam": "Filtre Meilisearch optionnel. Attributs : jurisdiction, chamber, formation, publication, type, solution, themes, year (entier), decision_timestamp (unix). Exemple : chamber = 'Chambre sociale' AND year >= 2022",
            "searchIndexUidParam": format!(
                "Index à interroger : '{chunk_index}' pour retrouver les passages précis d'une décision (recommandé), \
                 '{index}' pour raisonner sur des décisions entières."
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
    let index_chat = json!({
        "description": "Décisions de justice françaises publiées sur Judilibre (Cour de cassation et cours d'appel) : arrêts, avis, QPC, avec titrage, sommaire, textes appliqués et texte intégral pseudonymisé. Une décision par document.",
        "documentTemplate": DOCUMENT_TEMPLATE,
        "documentTemplateMaxBytes": 7000,
        "searchParameters": search_parameters.clone()
    });
    info!(index, "configuring index chat settings");
    meili.update_index_chat_settings(index, &index_chat).await?;

    let chunk_chat = json!({
        "description": "Passages (extraits) des décisions Judilibre et de leurs documents associés (communiqués, rapports, avis). À privilégier pour citer un motif précis : chaque document est un extrait d'une décision, avec sa référence complète.",
        "documentTemplate": CHUNK_DOCUMENT_TEMPLATE,
        "documentTemplateMaxBytes": 4000,
        "searchParameters": search_parameters
    });
    info!(chunk_index, "configuring chunk index chat settings");
    meili.update_index_chat_settings(chunk_index, &chunk_chat).await?;

    match meili.find_key_with_action("chatCompletions").await? {
        Some(key) => info!(
            "chat API key (set MEILI_CHAT_KEY in .env for the web app): {}",
            key
        ),
        None => info!("no chat API key found yet; Meilisearch creates one shortly after enabling the feature"),
    }
    Ok(())
}

const SYSTEM_PROMPT: &str = "Tu es un assistant juridique spécialisé dans la jurisprudence française, \
adossé à la base Judilibre de la Cour de cassation. Réponds en français, de façon précise et structurée. \
Appuie chaque affirmation sur les décisions retournées par la recherche : cite la juridiction, la chambre, \
la date et le numéro de pourvoi (par exemple « Cass. soc., 12 janvier 2024, n° 22-10.123 »). \
Si les décisions trouvées ne permettent pas de répondre, dis-le clairement plutôt que d'inventer. \
Ne donne pas de conseil juridique personnalisé : rappelle que la réponse est informative. \
Lorsque la question porte sur une période ou une chambre précise, utilise le paramètre de filtre. \
Privilégie l'index des passages pour retrouver et citer un motif précis, et l'index des décisions \
pour une vue d'ensemble ; les passages issus des documents associés (communiqués, rapports, avis) \
doivent être présentés comme tels et non comme le texte de l'arrêt. \
Sois économe en recherches : trois à cinq requêtes bien choisies suffisent presque toujours. \
Ne relance pas une recherche pour reformuler la même idée, et réponds dès que les décisions \
trouvées permettent de le faire.";

/// Text embedded per decision: citation line, titrage, sommaire and the start of the
/// motivations. voyage-law-2 handles 16K tokens, so 4 000 bytes is comfortable.
const EMBEDDING_TEMPLATE: &str = "{{doc.jurisdiction}}, {{doc.chamber}}, {{doc.decision_date}}, {{doc.solution}}. \
{{doc.titles}}. {{doc.summary}} {{doc.excerpt}}";

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
