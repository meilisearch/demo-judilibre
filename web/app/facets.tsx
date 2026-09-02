"use client";

import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { Button, buttonVariants } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { facetLabel, formatCount } from "@/lib/format";
import { useSearchStore } from "@/lib/search-store";
import { FACET_ATTRIBUTES, type FacetAttribute, type FacetDistribution } from "@/lib/types";
import { cn } from "@/lib/utils";

const VISIBLE = 6;

interface Props {
  distribution?: FacetDistribution;
  loading: boolean;
}

export function Facets({ distribution, loading }: Props) {
  const { filters, clearFilters } = useSearchStore();
  const activeCount = Object.values(filters).reduce((n, v) => n + (v?.length ?? 0), 0);

  return (
    <aside aria-label="Filtres" className="flex min-w-0 flex-col gap-5 lg:sticky lg:top-20 lg:self-start">
      <div className="flex items-center justify-between">
        <h2 className="text-xs font-semibold tracking-wider uppercase">Filtres</h2>
        {activeCount > 0 ? (
          <Button variant="ghost" size="xs" onClick={clearFilters}>
            Tout effacer ({activeCount})
          </Button>
        ) : null}
      </div>
      {FACET_ATTRIBUTES.map((attr, i) => (
        <div key={attr} className="flex flex-col gap-3">
          {i > 0 ? <Separator /> : null}
          <FacetGroup attr={attr} values={distribution?.[attr]} loading={loading} />
        </div>
      ))}
    </aside>
  );
}

function FacetGroup({ attr, values, loading }: { attr: FacetAttribute; values?: Record<string, number>; loading: boolean }) {
  const { filters, toggleFilter } = useSearchStore();
  const [open, setOpen] = useState(false);
  const selected = filters[attr] ?? [];

  // Selected values always stay visible, even when they fall out of the distribution.
  const entries = Object.entries(values ?? {});
  for (const s of selected) if (!entries.some(([v]) => v === s)) entries.push([s, 0]);
  const sorted =
    attr === "year"
      ? entries.sort(([a], [b]) => Number(b) - Number(a))
      : entries.sort(([, a], [, b]) => b - a);

  if (loading && sorted.length === 0) {
    return (
      <div className="flex flex-col gap-2">
        <Skeleton className="h-3 w-20" />
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-4 w-4/5" />
        <Skeleton className="h-4 w-3/5" />
      </div>
    );
  }
  if (sorted.length === 0) return null;

  const head = sorted.slice(0, VISIBLE);
  const tail = sorted.slice(VISIBLE);

  return (
    <fieldset className="flex min-w-0 flex-col gap-1.5">
      <legend className="text-muted-foreground mb-1.5 text-xs font-medium">{facetLabel(attr)}</legend>
      {head.map(([value, count]) => (
        <FacetOption key={value} attr={attr} value={value} count={count} checked={selected.includes(value)} onToggle={toggleFilter} />
      ))}
      {tail.length > 0 ? (
        <Collapsible open={open} onOpenChange={setOpen}>
          <CollapsibleContent className="flex flex-col gap-1.5">
            {tail.map(([value, count]) => (
              <FacetOption key={value} attr={attr} value={value} count={count} checked={selected.includes(value)} onToggle={toggleFilter} />
            ))}
          </CollapsibleContent>
          <CollapsibleTrigger
            className={cn(buttonVariants({ variant: "ghost", size: "xs" }), "text-muted-foreground mt-1 -ml-2")}
          >
            <ChevronDown data-icon="inline-start" className={cn("transition-transform", open && "rotate-180")} />
            {open ? "Voir moins" : `Voir ${tail.length} de plus`}
          </CollapsibleTrigger>
        </Collapsible>
      ) : null}
    </fieldset>
  );
}

function FacetOption({
  attr,
  value,
  count,
  checked,
  onToggle,
}: {
  attr: FacetAttribute;
  value: string;
  count: number;
  checked: boolean;
  onToggle: (attr: FacetAttribute, value: string) => void;
}) {
  return (
    <label className="hover:text-foreground flex min-w-0 cursor-pointer items-center gap-2 text-sm">
      <Checkbox checked={checked} onCheckedChange={() => onToggle(attr, value)} aria-label={value} />
      <span className="min-w-0 flex-1 truncate" title={value}>
        {value}
      </span>
      <span className="text-muted-foreground shrink-0 font-mono text-xs tabular-nums">{formatCount(count)}</span>
    </label>
  );
}
