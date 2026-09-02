import { create } from "zustand";
import type { FacetAttribute } from "@/lib/types";

export type SortOption = "relevance" | "date_desc" | "date_asc";
export type SearchMode = "keyword" | "hybrid";
/** Search whole decisions, or the passages extracted from them and their PDFs. */
export type SearchScope = "decisions" | "passages";

/** Weight of the semantic side in hybrid search (0 = keyword only, 1 = vectors only). */
export const HYBRID_SEMANTIC_RATIO = 0.6;

export type Filters = Partial<Record<FacetAttribute, string[]>>;

interface SearchState {
  query: string;
  filters: Filters;
  sort: SortOption;
  mode: SearchMode;
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
  scope: "decisions",
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
