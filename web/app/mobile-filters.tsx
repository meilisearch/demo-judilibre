"use client";

import { SlidersHorizontal } from "lucide-react";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { formatCount } from "@/lib/format";
import { useSearchStore } from "@/lib/search-store";
import { type FacetAttribute, type FacetDistribution } from "@/lib/types";
import { cn } from "@/lib/utils";
import { FacetList } from "@/app/facets";

interface Props {
  distribution?: FacetDistribution;
  attributes: readonly FacetAttribute[];
  loading: boolean;
  totalHits?: number;
  /** Articles or decisions, for the count on the confirm button. */
  scopeNoun: string;
}

/**
 * Filters on a phone. The sidebar would otherwise stack above the results and
 * push every hit off the first screen, so below `lg` the same facet groups live
 * in a bottom sheet behind a button that carries the active count.
 */
export function MobileFilters({ distribution, attributes, loading, totalHits, scopeNoun }: Props) {
  const { filters, clearFilters } = useSearchStore();
  const activeCount = Object.values(filters).reduce((n, v) => n + (v?.length ?? 0), 0);

  return (
    <Sheet>
      <SheetTrigger
        className={cn(
          buttonVariants({ variant: activeCount > 0 ? "default" : "outline", size: "sm" }),
          "h-9 gap-1.5 lg:hidden",
        )}
        aria-label={activeCount > 0 ? `Filtres, ${activeCount} actifs` : "Filtres"}
      >
        <SlidersHorizontal data-icon="inline-start" />
        Filtres
        {activeCount > 0 ? <span className="font-mono text-xs tabular-nums">({activeCount})</span> : null}
      </SheetTrigger>

      <SheetContent side="bottom" className="max-h-[85svh] rounded-t-2xl lg:hidden">
        <SheetHeader className="flex-row items-center justify-between gap-2 pr-12">
          <SheetTitle>Filtres</SheetTitle>
          <SheetDescription className="sr-only">
            Affinez les résultats par juridiction, chambre, année ou code.
          </SheetDescription>
          {activeCount > 0 ? (
            <Button variant="ghost" size="sm" onClick={clearFilters}>
              Tout effacer ({activeCount})
            </Button>
          ) : null}
        </SheetHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-4">
          <FacetList distribution={distribution} attributes={attributes} loading={loading} />
        </div>

        <SheetFooter className="pb-[max(1rem,env(safe-area-inset-bottom))]">
          <SheetClose className={cn(buttonVariants({ size: "lg" }), "h-11 w-full text-sm")}>
            {totalHits === undefined
              ? "Voir les résultats"
              : `Voir ${formatCount(totalHits)} ${scopeNoun}${totalHits > 1 ? "s" : ""}`}
          </SheetClose>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
