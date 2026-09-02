"use client";

import { ArrowDownWideNarrow, ChevronLeft, ChevronRight, FileSearch } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { type SortOption, useSearchStore } from "@/lib/search-store";
import { HitCard } from "@/app/hit-card";
import { PassageCard } from "@/app/passage-card";
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
  const { sort, setSort, scope, page, setPage, query, filters } = useSearchStore();
  const hasCriteria = query.trim().length > 0 || Object.keys(filters).length > 0;

  return (
    <section aria-label="Résultats" className="flex min-w-0 flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <p className="text-muted-foreground text-xs">
          {result && result.totalPages > 0 ? `Page ${page} sur ${result.totalPages}` : " "}
        </p>
        <Select value={sort} onValueChange={(v: string | null) => {
            if (v) setSort(v as SortOption);
          }}>
          <SelectTrigger size="sm" aria-label="Trier les résultats" className="min-w-[9.5rem]">
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
      ) : result && result.hits.length === 0 && result.passages.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FileSearch />
            </EmptyMedia>
            <EmptyTitle>
              {hasCriteria
                ? scope === "passages"
                  ? "Aucun passage ne correspond"
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
          {result?.passages.map((hit, i) => (
            <li key={hit.id}>
              <PassageCard hit={hit} position={(page - 1) * HITS_PER_PAGE + i + 1} />
            </li>
          ))}
        </ol>
      )}

      {result && result.totalPages > 1 ? (
        <nav aria-label="Pagination" className="flex items-center justify-center gap-2 pt-2">
          <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => setPage(page - 1)}>
            <ChevronLeft data-icon="inline-start" />
            Précédent
          </Button>
          <span className="text-muted-foreground px-2 font-mono text-xs tabular-nums">
            {page} / {result.totalPages}
          </span>
          <Button variant="outline" size="sm" disabled={page >= result.totalPages} onClick={() => setPage(page + 1)}>
            Suivant
            <ChevronRight data-icon="inline-end" />
          </Button>
        </nav>
      ) : null}
    </section>
  );
}
