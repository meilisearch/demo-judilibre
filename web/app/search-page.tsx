"use client";

import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { AlertCircle } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { getMeili } from "@/lib/meili-client";
import { buildFilter, sortParam, useSearchStore } from "@/lib/search-store";
import {
  FACET_ATTRIBUTES,
  HL_POST,
  HL_PRE,
  LEGI_FACET_ATTRIBUTES,
  type ArticleHit,
  type FacetAttribute,
  type FacetDistribution,
  type SearchHit,
} from "@/lib/types";
import { SearchBar } from "@/app/search-bar";
import { Facets } from "@/app/facets";
import { Results } from "@/app/results";

export const HITS_PER_PAGE = 10;

export interface SearchResult {
  hits: SearchHit[];
  articles: ArticleHit[];
  /** Embedder available on the index being searched. */
  embedder: string | null;
  /** True when a code-article index holds documents. */
  hasArticles: boolean;
  /** Facets offered for the corpus being searched. */
  facetAttributes: readonly FacetAttribute[];
  totalHits: number;
  totalPages: number;
  processingTimeMs: number;
  facetDistribution: FacetDistribution;
}

const DECISION_FIELDS = [
  "id",
  "jurisdiction",
  "chamber",
  "formation",
  "number",
  "numbers",
  "ecli",
  "publication",
  "decision_date",
  "year",
  "type",
  "solution",
  "summary",
  "titles",
  "themes",
  "files",
  "particular_interest",
  "text_length",
  "url",
  "excerpt",
];

const ARTICLE_FIELDS = [
  "id",
  "code",
  "code_id",
  "number",
  "reference",
  "reference_key",
  "hierarchy",
  "section",
  "text",
  "text_length",
  "date_debut",
  "year",
  "url",
];

export function SearchPage() {
  const { query, filters, sort, mode, scope, page, semanticRatio, matchingStrategy, rankingScoreThreshold } =
    useSearchStore();

  const search = useQuery({
    queryKey: [
      "search",
      query,
      filters,
      sort,
      mode,
      scope,
      page,
      semanticRatio,
      matchingStrategy,
      rankingScoreThreshold,
    ],
    placeholderData: keepPreviousData,
    queryFn: async (): Promise<SearchResult> => {
      const { client, config } = await getMeili();
      const kind = scope === "articles" && config.legiIndex ? "articles" : "decisions";
      const index = kind === "articles" ? (config.legiIndex as string) : config.index;
      const embedder = kind === "articles" ? config.legiEmbedder : config.embedder;
      const facetAttributes = kind === "articles" ? LEGI_FACET_ATTRIBUTES : FACET_ATTRIBUTES;
      const hybrid = mode === "hybrid" && embedder && query.trim() ? { embedder, semanticRatio } : undefined;

      const res = await client.index(index).search<SearchHit & ArticleHit>(query, {
        hybrid,
        filter: buildFilter(filters),
        facets: [...facetAttributes],
        attributesToRetrieve: kind === "articles" ? ARTICLE_FIELDS : DECISION_FIELDS,
        attributesToHighlight:
          kind === "articles"
            ? ["reference", "number", "code", "section", "text"]
            : ["summary", "titles", "themes", "number", "excerpt", "text"],
        highlightPreTag: HL_PRE,
        highlightPostTag: HL_POST,
        // Crop the motivations excerpt as well as the raw text: it carries the reasoning,
        // which is the context a result card needs when there is no sommaire.
        attributesToCrop: kind === "articles" ? ["text"] : ["excerpt", "text"],
        cropLength: kind === "articles" ? 60 : 55,
        cropMarker: "…",
        sort: kind === "articles" ? undefined : sortParam(sort),
        matchingStrategy,
        // 0 is the off state: Meilisearch has no "no threshold" value, and a threshold
        // of 0 would still be sent — omitting it says the same thing more cheaply.
        rankingScoreThreshold: rankingScoreThreshold > 0 ? rankingScoreThreshold : undefined,
        hitsPerPage: HITS_PER_PAGE,
        page,
      });

      // meilisearch-js types the response as a union; with `page`/`hitsPerPage` set, the
      // exhaustive fields are present.
      const paged = res as typeof res & { totalHits?: number; totalPages?: number };
      return {
        hits: kind === "decisions" ? (res.hits as SearchHit[]) : [],
        articles: kind === "articles" ? (res.hits as ArticleHit[]) : [],
        embedder,
        hasArticles: Boolean(config.legiIndex),
        facetAttributes,
        totalHits: paged.totalHits ?? 0,
        totalPages: paged.totalPages ?? 0,
        processingTimeMs: res.processingTimeMs,
        facetDistribution: (res.facetDistribution ?? {}) as FacetDistribution,
      };
    },
  });

  return (
    <div className="mx-auto flex w-full max-w-7xl flex-1 flex-col gap-6 px-4 py-6 sm:px-6">
      <SearchBar
        totalHits={search.data?.totalHits}
        processingTimeMs={search.data?.processingTimeMs}
        isFetching={search.isFetching}
        aiAvailable={Boolean(search.data?.embedder)}
      />

      {search.isError ? (
        <Alert variant="destructive">
          <AlertCircle />
          <AlertTitle>Meilisearch est injoignable</AlertTitle>
          <AlertDescription>
            {search.error instanceof Error ? search.error.message : "Erreur inconnue"}. Vérifiez MEILI_URL et la clé de
            recherche dans <code>.env</code>, puis rechargez la page.
          </AlertDescription>
        </Alert>
      ) : (
        <div className="grid gap-8 lg:grid-cols-[15rem_minmax(0,1fr)]">
          <Facets
            distribution={search.data?.facetDistribution}
            attributes={search.data?.facetAttributes ?? FACET_ATTRIBUTES}
            loading={search.isPending}
          />
          <Results result={search.data} loading={search.isPending} />
        </div>
      )}
    </div>
  );
}
