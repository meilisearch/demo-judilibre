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
            "searchDescription": "Recherche dans le droit français : d'abord les articles des codes en vigueur (index 'legi'), puis la jurisprudence de la Cour de cassation. Pour toute question de droit, commence par chercher l'article applicable, puis les décisions qui l'interprètent.",
            "searchQParam": "Mots-clés juridiques en français à rechercher (notions, articles de code, numéro de pourvoi, ECLI). Reste concis : 2 à 8 mots.",
            "searchFilterParam": "Filtre Meilisearch optionnel. Attributs : jurisdiction, chamber, formation, publication, type, solution, themes, year (entier), decision_timestamp (unix). Exemple : chamber = 'Chambre sociale' AND year >= 2022",
            "searchIndexUidParam": format!(
                "Index à interroger, dans cet ordre : d'abord '{legi_index}' (articles des codes en vigueur) pour \
                 établir la règle applicable, puis '{index}' (décisions de la Cour de cassation) pour la \
                 jurisprudence qui l'interprète."
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
        "description": "Décisions de la Cour de cassation (Judilibre) : arrêts, avis et QPC, avec titrage, sommaire, textes appliqués et motivations. À interroger APRÈS l'index des codes, pour la jurisprudence qui interprète l'article applicable.",
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
        "description": "Articles en vigueur des codes français (Légifrance/LEGI) : le texte de la règle elle-même, avec son code et sa place dans celui-ci. COMMENCER PAR CET INDEX pour toute question de droit : il donne la règle applicable, que la jurisprudence viendra ensuite préciser. À interroger pour citer un article ou vérifier sa rédaction actuelle.",
        "documentTemplate": ARTICLE_DOCUMENT_TEMPLATE,
        "documentTemplateMaxBytes": 4000,
        "searchParameters": { "limit": 6 }
    });
    info!(legi_index, "configuring LEGI index chat settings");
    meili.update_index_chat_settings(legi_index, &legi_chat).await?;

    match meili.find_key_with_action("chatCompletions").await? {
        Some(key) => info!(
            "chat API key (set MEILI_CHAT_KEY in .env for the web app): {}",
            key
        ),
        None => info!("no chat API key found yet; Meilisearch creates one shortly after enabling the feature"),
    }
    Ok(())
}

const SYSTEM_PROMPT: &str = "Tu es un assistant juridique adossé à deux sources : les articles des codes \
français en vigueur (Légifrance) et la jurisprudence de la Cour de cassation (Judilibre). \
MÉTHODE, à suivre dans cet ordre : commence toujours par chercher dans l'index des codes l'article qui \
fonde la réponse et cite-le ; cherche ensuite la jurisprudence qui l'interprète. N'interroge pas la \
jurisprudence en premier. \
Réponds en français, de façon précise et structurée. \
Appuie chaque affirmation sur les décisions retournées par la recherche : cite la juridiction, la chambre, \
la date et le numéro de pourvoi (par exemple « Cass. soc., 12 janvier 2024, n° 22-10.123 »). \
Si les décisions trouvées ne permettent pas de répondre, dis-le clairement plutôt que d'inventer. \
Ne donne pas de conseil juridique personnalisé : rappelle que la réponse est informative. \
Lorsque la question porte sur une période ou une chambre précise, utilise le paramètre de filtre. \
Procède dans cet ordre : d'abord le texte applicable dans les codes, ensuite la jurisprudence qui \
l'interprète. Commence donc par chercher les articles pertinents, cite-les, puis appuie-toi sur les \
décisions pour montrer comment ils sont appliqués. \
Ne confonds pas les deux sources : le texte d'un article vient de l'index des codes, la solution \
retenue vient des décisions. Les passages issus des documents associés (communiqués, rapports, \
avis) doivent être présentés comme tels et non comme le texte de l'arrêt. \
Un article peut avoir été renuméroté ou réécrit : si une décision vise un article « dans sa \
rédaction antérieure », signale que le texte en vigueur peut différer de celui qu'elle applique. \
Sois économe en recherches : trois à cinq requêtes bien choisies suffisent presque toujours. \
Ne relance pas une recherche pour reformuler la même idée, et réponds dès que les décisions \
trouvées permettent de le faire.";

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
