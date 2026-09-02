export interface FileLink {
  name: string;
  type: string;
  url: string;
  /** Text extracted from the PDF by the indexer (empty when not processed). */
  content: string;
  pages: number;
  /** `text_based`, `scanned`, `image_based`, `mixed`, or empty. */
  pdf_type: string;
}

export interface DecisionLink {
  title: string;
  number: string;
  url: string;
}

/** Document shape produced by the Rust indexer (see indexer/src/transform.rs). */
export interface Decision {
  id: string;
  jurisdiction: string;
  chamber: string;
  formation: string;
  number: string;
  numbers: string[];
  ecli: string;
  publication: string[];
  decision_date: string;
  decision_timestamp: number;
  year: number;
  type: string;
  solution: string;
  summary: string;
  titles: string[];
  themes: string[];
  visa: string[];
  files: FileLink[];
  rapprochements: DecisionLink[];
  particular_interest: boolean;
  location: string;
  bulletin: string;
  excerpt: string;
  text: string;
  text_length: number;
  url: string;
}

/** Fields retrieved for result lists (full text is fetched on the detail page only). */
export type DecisionHit = Omit<Decision, "text" | "visa" | "rapprochements">;

export interface DecisionFormatted {
  summary?: string;
  titles?: string[];
  themes?: string[];
  number?: string;
  text?: string;
  excerpt?: string;
}

export type SearchHit = DecisionHit & { _formatted?: DecisionFormatted };

export const FACET_ATTRIBUTES = [
  "chamber",
  "solution",
  "publication",
  "type",
  "year",
  "formation",
  "themes",
] as const;

export type FacetAttribute = (typeof FACET_ATTRIBUTES)[number];

export type FacetDistribution = Partial<Record<FacetAttribute, Record<string, number>>>;

export interface SearchConfig {
  host: string;
  apiKey: string;
  index: string;
  /** Passage index, or null when no passages are indexed. */
  chunkIndex: string | null;
  /** Embedder name when hybrid (semantic) search is configured on the index. */
  embedder: string | null;
  chunkEmbedder: string | null;
}

/** A passage of a decision, from the chunk index. */
export interface ChunkHit {
  id: string;
  decision_id: string;
  chunk_index: number;
  chunk_count: number;
  /** `decision` or `attachment`. */
  source: string;
  attachment_name: string;
  attachment_type: string;
  attachment_url: string;
  content: string;
  content_chars: number;
  jurisdiction: string;
  chamber: string;
  formation: string;
  number: string;
  ecli: string;
  publication: string[];
  decision_date: string;
  year: number;
  type: string;
  solution: string;
  titles: string[];
  themes: string[];
  summary: string;
  url: string;
  _formatted?: { content?: string; titles?: string[]; summary?: string; number?: string };
}

/** Highlight marker tags used with Meilisearch so we never inject HTML. */
export const HL_PRE = "__hl__";
export const HL_POST = "__/hl__";
