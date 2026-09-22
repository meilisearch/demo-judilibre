import { create } from "zustand";
import type { FacetAttribute } from "@/lib/types";

export type SortOption = "relevance" | "date_desc" | "date_asc";
export type SearchMode = "keyword" | "hybrid";
/**
 * Which corpus is being searched: the in-force articles of the French codes, or
 * the decisions. The passage index still exists — it is what a passage search
 * would use — but it is neither browsed nor offered to the assistant.
 */
export type SearchScope = "articles" | "decisions";

/**
 * How Meilisearch narrows a query whose every word cannot be matched: `frequency`
 * drops the most common word first, `last` drops the last one, `all` drops nothing
 * and returns only documents holding every word.
 */
export type MatchingStrategy = "frequency" | "last" | "all";

/** What the settings panel exposes, and nothing else — the gear badge diffs this shape. */
export interface SearchSettings {
  sort: SortOption;
  /** Weight of the semantic side in hybrid search (0 = keyword only, 1 = vectors only). */
  semanticRatio: number;
  matchingStrategy: MatchingStrategy;
  /**
   * Minimum ranking score a hit must reach to be returned. 0 excludes nothing,
   * which is what "no threshold" means to Meilisearch — so 0 *is* the off state.
   */
  rankingScoreThreshold: number;
}

/** Defaults of the settings panel; also what "reset" restores. */
export const DEFAULT_SETTINGS: SearchSettings = {
  sort: "relevance",
  semanticRatio: 0.5,
  matchingStrategy: "frequency",
  rankingScoreThreshold: 0,
};

export type Filters = Partial<Record<FacetAttribute, string[]>>;

interface SearchState extends SearchSettings {
  query: string;
  filters: Filters;
  /** Hybrid by default; falls back to keywords where the index has no embedder. */
  mode: SearchMode;
  scope: SearchScope;
  page: number;
  setQuery: (q: string) => void;
  setMode: (m: SearchMode) => void;
  setScope: (s: SearchScope) => void;
  toggleFilter: (attr: FacetAttribute, value: string) => void;
  clearFilters: () => void;
  setSort: (s: SortOption) => void;
  setSemanticRatio: (r: number) => void;
  setMatchingStrategy: (m: MatchingStrategy) => void;
  setRankingScoreThreshold: (t: number) => void;
  resetSettings: () => void;
  setPage: (p: number) => void;
}

export const useSearchStore = create<SearchState>((set) => ({
  query: "",
  filters: {},
  ...DEFAULT_SETTINGS,
  mode: "hybrid",
  // Codes is the first tab: the law before its application.
  scope: "articles",
  page: 1,
  setQuery: (query) => set({ query, page: 1 }),
  setMode: (mode) => set({ mode, page: 1 }),
  setScope: (scope) => set({ scope, page: 1 }),
  toggleFilter: (attr, value) =>
    set((state) => {
      const current = state.filters[attr] ?? [];
      const next = current.includes(value) ? current.filter((v) => v !== value) : [...current, value];
      const filters = { ...state.filters, [attr]: next };
      if (next.length === 0) delete filters[attr];
      return { filters, page: 1 };
    }),
  clearFilters: () => set({ filters: {}, page: 1 }),
  setSort: (sort) => set({ sort, page: 1 }),
  setSemanticRatio: (semanticRatio) => set({ semanticRatio, page: 1 }),
  setMatchingStrategy: (matchingStrategy) => set({ matchingStrategy, page: 1 }),
  setRankingScoreThreshold: (rankingScoreThreshold) => set({ rankingScoreThreshold, page: 1 }),
  resetSettings: () => set({ ...DEFAULT_SETTINGS, page: 1 }),
  setPage: (page) => set({ page }),
}));

function escapeFilterValue(v: string): string {
  return `'${v.replaceAll("\\", "\\\\").replaceAll("'", "\\'")}'`;
}

/** Build a Meilisearch filter expression: OR inside a facet, AND across facets. */
export function buildFilter(filters: Filters): string[] {
  return Object.entries(filters)
    .filter(([, values]) => values && values.length > 0)
    .map(([attr, values]) =>
      (values as string[])
        .map((v) => (attr === "year" ? `${attr} = ${Number(v)}` : `${attr} = ${escapeFilterValue(v)}`))
        .join(" OR "),
    );
}

export function sortParam(sort: SortOption): string[] | undefined {
  if (sort === "date_desc") return ["decision_timestamp:desc"];
  if (sort === "date_asc") return ["decision_timestamp:asc"];
  return undefined;
}

/** How many settings the user moved away from their default — the count on the gear. */
export function countChangedSettings(settings: SearchSettings): number {
  return (Object.keys(DEFAULT_SETTINGS) as (keyof SearchSettings)[]).filter((k) => settings[k] !== DEFAULT_SETTINGS[k])
    .length;
}
