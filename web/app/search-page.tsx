"use client";

import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { AlertCircle } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { getMeili } from "@/lib/meili-client";
import { HYBRID_SEMANTIC_RATIO, buildFilter, sortParam, useSearchStore } from "@/lib/search-store";
import {
  FACET_ATTRIBUTES,
  HL_POST,
  HL_PRE,
  type ChunkHit,
  type FacetDistribution,
  type SearchHit,
} from "@/lib/types";
import { SearchBar } from "@/app/search-bar";
import { Facets } from "@/app/facets";
import { Results } from "@/app/results";

export const HITS_PER_PAGE = 10;

export interface SearchResult {
  hits: SearchHit[];
  passages: ChunkHit[];
  /** Embedder available on the index being searched. */
  embedder: string | null;
  /** True when a passage index holds documents. */
  hasPassages: boolean;
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

const PASSAGE_FIELDS = [
  "id",
  "decision_id",
  "chunk_index",
  "chunk_count",
  "source",
  "attachment_name",
  "attachment_type",
  "content",
  "content_chars",
  "jurisdiction",
  "chamber",
  "formation",
  "number",
  "ecli",
  "publication",
  "decision_date",
  "year",
  "type",
  "solution",
  "titles",
  "themes",
  "summary",
  "url",
];

export function SearchPage() {
  const { query, filters, sort, mode, scope, page } = useSearchStore();

  const search = useQuery({
    queryKey: ["search", query, filters, sort, mode, scope, page],
    placeholderData: keepPreviousData,
    queryFn: async (): Promise<SearchResult> => {
      const { client, config } = await getMeili();
      const passages = scope === "passages" && Boolean(config.chunkIndex);
      const index = passages ? (config.chunkIndex as string) : config.index;
      const embedder = passages ? config.chunkEmbedder : config.embedder;
      const hybrid =
        mode === "hybrid" && embedder && query.trim() ? { embedder, semanticRatio: HYBRID_SEMANTIC_RATIO } : undefined;

      const res = await client.index(index).search<SearchHit & ChunkHit>(query, {
        hybrid,
        filter: buildFilter(filters),
        facets: [...FACET_ATTRIBUTES],
        attributesToRetrieve: passages ? PASSAGE_FIELDS : DECISION_FIELDS,
        attributesToHighlight: passages
          ? ["content", "titles", "summary", "number"]
          : ["summary", "titles", "themes", "number", "excerpt", "text"],
        highlightPreTag: HL_PRE,
        highlightPostTag: HL_POST,
        // Crop the motivations excerpt as well as the raw text: it carries the reasoning,
        // which is the context a result card needs when there is no sommaire.
        attributesToCrop: passages ? ["content"] : ["excerpt", "text"],
        cropLength: passages ? 60 : 55,
        cropMarker: "…",
        sort: sortParam(sort),
        hitsPerPage: HITS_PER_PAGE,
        page,
      });

      // meilisearch-js types the response as a union; with `page`/`hitsPerPage` set, the
      // exhaustive fields are present.
      const paged = res as typeof res & { totalHits?: number; totalPages?: number };
      return {
        hits: passages ? [] : (res.hits as SearchHit[]),
        passages: passages ? (res.hits as ChunkHit[]) : [],
        embedder,
        hasPassages: Boolean(config.chunkIndex),
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
        hasPassages={Boolean(search.data?.hasPassages)}
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
          <Facets distribution={search.data?.facetDistribution} loading={search.isPending} />
          <Results result={search.data} loading={search.isPending} />
        </div>
      )}
    </div>
  );
}
