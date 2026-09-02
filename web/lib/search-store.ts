import { create } from "zustand";
import type { FacetAttribute } from "@/lib/types";

export type SortOption = "relevance" | "date_desc" | "date_asc";
export type SearchMode = "keyword" | "hybrid";
/** Search whole decisions, or the passages extracted from them and their PDFs. */
export type SearchScope = "decisions" | "passages";

/** Above this many words, a query reads as a sentence rather than a set of keywords. */
const NATURAL_LANGUAGE_WORDS = 5;

/**
 * A question, or a query long enough to be a sentence, is better served by hybrid search:
 * the wording rarely matches the decision's own words.
 */
export function looksNaturalLanguage(query: string): boolean {
  const q = query.trim();
  if (!q) return false;
  if (q.includes("?")) return true;
  return q.split(/\s+/).length > NATURAL_LANGUAGE_WORDS;
}

/** Weight of the semantic side in hybrid search (0 = keyword only, 1 = vectors only). */
export const HYBRID_SEMANTIC_RATIO = 0.6;

export type Filters = Partial<Record<FacetAttribute, string[]>>;

interface SearchState {
  query: string;
  filters: Filters;
  sort: SortOption;
  mode: SearchMode;
  /** The user set the mode by hand, so typing no longer changes it. */
  modePinned: boolean;
  scope: SearchScope;
  page: number;
  setQuery: (q: string) => void;
  setMode: (m: SearchMode) => void;
  setScope: (s: SearchScope) => void;
  toggleFilter: (attr: FacetAttribute, value: string) => void;
  clearFilters: () => void;
  setSort: (s: SortOption) => void;
  setPage: (p: number) => void;
}

export const useSearchStore = create<SearchState>((set) => ({
  query: "",
  filters: {},
  sort: "relevance",
  mode: "keyword",
  modePinned: false,
  scope: "decisions",
  page: 1,
  setQuery: (query) =>
    set((state) => ({
      query,
      page: 1,
      mode: state.modePinned ? state.mode : looksNaturalLanguage(query) ? "hybrid" : "keyword",
    })),
  setMode: (mode) => set({ mode, modePinned: true, page: 1 }),
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
