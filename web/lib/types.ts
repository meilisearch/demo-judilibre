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
  /** Normalised keys of the code articles cited by the visa. */
  visa_refs: string[];
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

/** An in-force article of a French code (see indexer/src/legi.rs). */
export interface Article {
  id: string;
  code: string;
  code_id: string;
  number: string;
  reference: string;
  reference_key: string;
  hierarchy: string[];
  section: string;
  text: string;
  text_length: number;
  date_debut: string;
  date_debut_timestamp: number;
  year: number;
  url: string;
}

export interface ArticleFormatted {
  reference?: string;
  number?: string;
  code?: string;
  section?: string;
  text?: string;
}

export type ArticleHit = Article & { _formatted?: ArticleFormatted };

export const FACET_ATTRIBUTES = [
  "chamber",
  "solution",
  "publication",
  "type",
  "year",
  "formation",
  "themes",
] as const;

/** Facets of the code-article index: a code and its sections, not a chamber. */
export const LEGI_FACET_ATTRIBUTES = ["code", "section", "year"] as const;

export type FacetAttribute = (typeof FACET_ATTRIBUTES)[number] | (typeof LEGI_FACET_ATTRIBUTES)[number];

export type FacetDistribution = Partial<Record<FacetAttribute, Record<string, number>>>;

export interface SearchConfig {
  host: string;
  apiKey: string;
  index: string;
  /** Code-article index, or null when no articles are indexed. */
  legiIndex: string | null;
  legiEmbedder: string | null;
  /** Embedder name when hybrid (semantic) search is configured on the index. */
  embedder: string | null;
}

/** Highlight marker tags used with Meilisearch so we never inject HTML. */
export const HL_PRE = "__hl__";
export const HL_POST = "__/hl__";
