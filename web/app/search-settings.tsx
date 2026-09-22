"use client";

import { RotateCcw, Settings } from "lucide-react";
import { Button, buttonVariants } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
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
import { Slider } from "@/components/ui/slider";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { countChangedSettings, type MatchingStrategy, type SortOption, useSearchStore } from "@/lib/search-store";
import { cn } from "@/lib/utils";

const SORT_LABELS: Record<SortOption, string> = {
  relevance: "Pertinence",
  date_desc: "Plus récentes",
  date_asc: "Plus anciennes",
};

const STRATEGY_LABELS: Record<MatchingStrategy, string> = {
  frequency: "Fréquence",
  last: "Derniers mots",
  all: "Tous les mots",
};

const STRATEGY_HINTS: Record<MatchingStrategy, string> = {
  frequency: "Quand la requête ne peut pas être satisfaite entière, le mot le plus courant est abandonné en premier.",
  last: "Les mots sont abandonnés en partant de la fin de la requête.",
  all: "Aucun mot n'est abandonné : seuls les documents contenant toute la requête sortent.",
};

/**
 * Each slider here carries one thumb. Its value is still passed as a one-element
 * array: the shadcn wrapper counts the thumbs from the array it is given, and a
 * bare number makes it fall back to `[min, max]` — two thumbs.
 */
function single(value: number | readonly number[]): number {
  return Array.isArray(value) ? value[0] : (value as number);
}

/** Two decimals, French separator — the sliders step by 0,05. */
function decimal(n: number): string {
  return n.toFixed(2).replace(".", ",");
}

function SettingRow({
  label,
  value,
  hint,
  children,
}: {
  label: string;
  value?: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-2">
        <Label className="text-sm font-medium">{label}</Label>
        {value ? <span className="text-muted-foreground font-mono text-xs tabular-nums">{value}</span> : null}
      </div>
      {children}
      <p className="text-muted-foreground text-xs leading-snug">{hint}</p>
    </div>
  );
}

/**
 * The knobs Meilisearch exposes on a search request, behind a gear next to the
 * input. They are advanced — the defaults are what a visitor gets — so they live
 * in a sheet rather than in the toolbar, where only the corpus switch and the
 * filters earn their room.
 */
export function SearchSettings({ aiAvailable }: { aiAvailable: boolean }) {
  const {
    sort,
    setSort,
    semanticRatio,
    setSemanticRatio,
    matchingStrategy,
    setMatchingStrategy,
    rankingScoreThreshold,
    setRankingScoreThreshold,
    resetSettings,
    mode,
    scope,
  } = useSearchStore();

  const changed = countChangedSettings({
    sort,
    semanticRatio,
    matchingStrategy,
    rankingScoreThreshold,
  });
  // The decisions index is the only one sorted by date; `legi` has no such attribute.
  const sortable = scope === "decisions";

  return (
    <Sheet>
      <SheetTrigger
        render={
          <Button
            size="icon"
            variant={changed > 0 ? "default" : "outline"}
            title="Paramètres de recherche"
            aria-label={changed > 0 ? `Paramètres de recherche, ${changed} modifiés` : "Paramètres de recherche"}
            className="size-12 shrink-0 rounded-xl shadow-sm [&_svg:not([class*='size-'])]:size-5"
          >
            <Settings />
          </Button>
        }
      />

      <SheetContent side="right" className="data-[side=right]:w-full sm:max-w-sm">
        <SheetHeader className="flex-row items-center justify-between gap-2 pr-12">
          <div className="flex flex-col gap-1">
            <SheetTitle>Paramètres</SheetTitle>
            <SheetDescription>Comment Meilisearch classe et filtre les résultats.</SheetDescription>
          </div>
        </SheetHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-1">
          {sortable ? (
            <SettingRow
              label="Tri"
              hint="Par défaut, aucun tri : les décisions sortent dans l'ordre de pertinence calculé par Meilisearch."
            >
              <Select
                value={sort}
                onValueChange={(v: string | null) => {
                  if (v) setSort(v as SortOption);
                }}
              >
                <SelectTrigger aria-label="Trier les résultats" className="w-full">
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
            </SettingRow>
          ) : null}

          <SettingRow
            label="Équilibre hybride"
            value={decimal(semanticRatio)}
            hint={
              aiAvailable
                ? mode === "hybrid"
                  ? "Part du sens (embeddings Voyage AI) face aux mots-clés : 0 ne garde que les mots-clés, 1 que le sens."
                  : "Part du sens face aux mots-clés. N'agit que lorsque la recherche IA est active."
                : "Aucun embedder n'est configuré sur cet index : la recherche hybride est indisponible."
            }
          >
            <Slider
              value={[semanticRatio]}
              onValueChange={(v) => setSemanticRatio(single(v))}
              min={0}
              max={1}
              step={0.05}
              disabled={!aiAvailable}
              aria-label="Équilibre entre mots-clés et sens"
            />
            <div className="text-muted-foreground flex justify-between text-[0.6875rem]">
              <span>Mots-clés</span>
              <span>Sens</span>
            </div>
          </SettingRow>

          <SettingRow label="Correspondance" hint={STRATEGY_HINTS[matchingStrategy]}>
            <ToggleGroup
              value={[matchingStrategy]}
              onValueChange={(v: string[]) => {
                const next = v.at(-1) as MatchingStrategy | undefined;
                if (next) setMatchingStrategy(next);
              }}
              variant="outline"
              size="sm"
              aria-label="Stratégie de correspondance"
              className="w-full"
            >
              {(Object.keys(STRATEGY_LABELS) as MatchingStrategy[]).map((option) => (
                <ToggleGroupItem key={option} value={option} className="h-9 flex-1">
                  {STRATEGY_LABELS[option]}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </SettingRow>

          <SettingRow
            label="Seuil de score"
            value={rankingScoreThreshold === 0 ? "Aucun" : decimal(rankingScoreThreshold)}
            hint="Écarte les résultats dont le score de pertinence reste sous le seuil. À 0, rien n'est écarté."
          >
            <Slider
              value={[rankingScoreThreshold]}
              onValueChange={(v) => setRankingScoreThreshold(single(v))}
              min={0}
              max={1}
              step={0.05}
              aria-label="Seuil de score de pertinence"
            />
          </SettingRow>
        </div>

        <SheetFooter className="flex-row gap-2 pb-[max(1rem,env(safe-area-inset-bottom))]">
          <Button
            variant="outline"
            size="lg"
            className="h-11 flex-1 text-sm"
            disabled={changed === 0}
            onClick={resetSettings}
          >
            <RotateCcw data-icon="inline-start" />
            Réinitialiser
          </Button>
          <SheetClose className={cn(buttonVariants({ size: "lg" }), "h-11 flex-1 text-sm")}>
            Voir les résultats
          </SheetClose>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
