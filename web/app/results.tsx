"use client";

import { ArrowDownWideNarrow, ChevronLeft, ChevronRight, FileSearch, Landmark, ScrollText } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { type SearchScope, type SortOption, useSearchStore } from "@/lib/search-store";
import { FACET_ATTRIBUTES } from "@/lib/types";
import { MobileFilters } from "@/app/mobile-filters";
import { ArticleCard } from "@/app/article-card";
import { HitCard } from "@/app/hit-card";
import { HITS_PER_PAGE, type SearchResult } from "@/app/search-page";

interface Props {
  result?: SearchResult;
  loading: boolean;
}

const SORT_LABELS: Record<SortOption, string> = {
  relevance: "Pertinence",
  date_desc: "Plus récentes",
  date_asc: "Plus anciennes",
};

export function Results({ result, loading }: Props) {
  const { sort, setSort, scope, setScope, page, setPage, query, filters } = useSearchStore();
  const hasCriteria = query.trim().length > 0 || Object.keys(filters).length > 0;

  return (
    <section aria-label="Résultats" className="flex min-w-0 flex-col gap-4">
      {/* Mobile stacks the corpus switch over a row of controls; from `sm` it is one line. */}
      <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between sm:gap-x-3">
        {result?.hasArticles ? (
          <ToggleGroup
            value={[scope]}
            onValueChange={(v: string[]) => {
              const next = v.at(-1) as SearchScope | undefined;
              if (next) setScope(next);
            }}
            variant="outline"
            size="sm"
            aria-label="Corpus interrogé"
            className="max-sm:w-full"
          >
            <ToggleGroupItem value="articles" className="max-sm:h-9 max-sm:flex-1">
              <Landmark data-icon="inline-start" />
              Codes
            </ToggleGroupItem>
            <ToggleGroupItem value="decisions" className="max-sm:h-9 max-sm:flex-1">
              <ScrollText data-icon="inline-start" />
              Décisions
            </ToggleGroupItem>
          </ToggleGroup>
        ) : (
          <span />
        )}
        <div className="flex items-center gap-2 sm:gap-3">
          <MobileFilters
            distribution={result?.facetDistribution}
            attributes={result?.facetAttributes ?? FACET_ATTRIBUTES}
            loading={loading}
            totalHits={result?.totalHits}
            scopeNoun={scope === "articles" ? "article" : "décision"}
          />
          {/* The pager at the foot of the list already says this; on a phone the room is better spent. */}
          <p className="text-muted-foreground hidden text-xs sm:block">
            {result && result.totalPages > 0 ? `Page ${page} sur ${result.totalPages}` : " "}
          </p>
          {scope === "articles" ? null : (
          <Select value={sort} onValueChange={(v: string | null) => {
              if (v) setSort(v as SortOption);
            }}>
            <SelectTrigger size="sm" aria-label="Trier les résultats" className="min-w-[9.5rem] max-sm:h-9 max-sm:min-w-0 max-sm:flex-1">
              <ArrowDownWideNarrow />
              <SelectValue>{SORT_LABELS[sort]}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {(Object.keys(SORT_LABELS) as SortOption[]).map((option) => (
                  <SelectItem key={option} value={option}>
                    {SORT_LABELS[option]}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          )}
        </div>
      </div>

      {loading && !result ? (
        <div className="flex flex-col gap-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="flex flex-col gap-2 rounded-xl border p-5">
              <Skeleton className="h-3 w-56" />
              <Skeleton className="h-5 w-4/5" />
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-2/3" />
            </div>
          ))}
        </div>
      ) : result && result.hits.length === 0 && result.articles.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FileSearch />
            </EmptyMedia>
            <EmptyTitle>
              {hasCriteria
                ? scope === "articles"
                  ? "Aucun article ne correspond"
                  : "Aucune décision ne correspond"
                : "L'index est vide"}
            </EmptyTitle>
            <EmptyDescription>
              {hasCriteria
                ? "Essayez d'autres termes, ou retirez un filtre."
                : "Lancez l'indexeur pour importer des décisions depuis l'API Judilibre : cargo run -- index --limit 2000"}
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ol className="flex flex-col gap-3">
          {result?.hits.map((hit, i) => (
            <li key={hit.id}>
              <HitCard hit={hit} position={(page - 1) * HITS_PER_PAGE + i + 1} />
            </li>
          ))}
          {result?.articles.map((hit, i) => (
            <li key={hit.id}>
              <ArticleCard hit={hit} position={(page - 1) * HITS_PER_PAGE + i + 1} />
            </li>
          ))}
        </ol>
      )}

      {result && result.totalPages > 1 ? (
        <nav aria-label="Pagination" className="flex items-center justify-center gap-2 pt-2">
          <Button variant="outline" size="sm" className="max-sm:h-10 max-sm:px-4" disabled={page <= 1} onClick={() => setPage(page - 1)}>
            <ChevronLeft data-icon="inline-start" />
            Précédent
          </Button>
          <span className="text-muted-foreground px-2 font-mono text-xs tabular-nums">
            {page} / {result.totalPages}
          </span>
          <Button variant="outline" size="sm" className="max-sm:h-10 max-sm:px-4" disabled={page >= result.totalPages} onClick={() => setPage(page + 1)}>
            Suivant
            <ChevronRight data-icon="inline-end" />
          </Button>
        </nav>
      ) : null}
    </section>
  );
}
