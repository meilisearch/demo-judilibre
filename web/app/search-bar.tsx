"use client";

import { Search, Sparkles, X } from "lucide-react";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
  InputGroupText,
} from "@/components/ui/input-group";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { formatCount } from "@/lib/format";
import { useSearchStore } from "@/lib/search-store";
import { cn } from "@/lib/utils";

interface Props {
  totalHits?: number;
  processingTimeMs?: number;
  isFetching: boolean;
  /** Semantic search is available on the index being queried. */
  aiAvailable: boolean;
}

export function SearchBar({ totalHits, processingTimeMs, isFetching, aiAvailable }: Props) {
  const { query, setQuery, mode, setMode, modePinned, scope } = useSearchStore();
  const aiOn = mode === "hybrid";

  return (
    // `contents` below `lg` so the sticky input below is positioned against the page
    // container, not against this section — which ends just after the input.
    <section className="max-lg:contents lg:flex lg:flex-col lg:gap-3">
      <div className="flex flex-col gap-1">
        <h1 className="font-heading text-2xl leading-tight font-medium tracking-tight sm:text-4xl">
          Le droit français, <span className="italic">au mot près</span>.
        </h1>
        <p className="text-muted-foreground max-w-2xl text-sm">
          Les articles des codes en vigueur (Légifrance) et la jurisprudence de la Cour de cassation (Judilibre).
          Tapez une notion, un article de code ou un numéro de pourvoi : les résultats s&apos;affichent à chaque frappe.
        </p>
      </div>

      {/* Below `lg` the input sticks under the header: refining a query on a phone
          should not mean scrolling back to the top of the results. */}
      <div className="max-lg:bg-background/95 max-lg:sticky max-lg:top-(--header-height) max-lg:z-10 max-lg:-mx-4 max-lg:px-4 max-lg:py-2 max-lg:backdrop-blur sm:max-lg:-mx-6 sm:max-lg:px-6">
      <InputGroup className="h-12 rounded-xl text-base shadow-sm">
        <InputGroupAddon align="inline-start">
          <Search />
        </InputGroupAddon>
        <InputGroupInput
          type="search"
          enterKeyHint="search"
          aria-label={scope === "articles" ? "Rechercher un article de code" : "Rechercher une décision"}
          placeholder={
            scope === "articles"
              ? "Rechercher un article, une notion, un numéro d'article…"
              : "Rechercher une décision, une notion, un numéro de pourvoi…"
          }
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="text-base"
        />
        <InputGroupAddon align="inline-end">
          {isFetching ? (
            <Spinner />
          ) : totalHits !== undefined ? (
            <InputGroupText className="hidden font-mono text-xs tabular-nums sm:flex">
              {formatCount(totalHits)} {scope === "articles" ? "article" : "décision"}
              {totalHits > 1 ? "s" : ""}
              {processingTimeMs !== undefined ? ` · ${processingTimeMs} ms` : ""}
            </InputGroupText>
          ) : null}
          {query ? (
            <InputGroupButton aria-label="Effacer la recherche" size="icon-xs" onClick={() => setQuery("")}>
              <X />
            </InputGroupButton>
          ) : null}
          {aiAvailable ? (
            <>
              <Separator orientation="vertical" className="mx-0.5 !h-5" />
              <Tooltip>
                <TooltipTrigger
                  render={
                    <InputGroupButton
                      size="sm"
                      variant={aiOn ? "default" : "ghost"}
                      aria-pressed={aiOn}
                      onClick={() => setMode(aiOn ? "keyword" : "hybrid")}
                      className={cn("gap-1.5", aiOn && "shadow-sm")}
                    >
                      <Sparkles />
                      IA
                    </InputGroupButton>
                  }
                />
                <TooltipContent>
                  {aiOn
                    ? modePinned
                      ? "Recherche hybride active : mots-clés + sens (embeddings Voyage AI). Cliquez pour revenir aux mots-clés."
                      : "Recherche hybride activée automatiquement : votre requête ressemble à une question. Cliquez pour revenir aux mots-clés."
                    : "Activer la recherche hybride : combine les mots-clés et le sens de la question."}
                </TooltipContent>
              </Tooltip>
            </>
          ) : null}
        </InputGroupAddon>
      </InputGroup>
      {/* The addon that carries this is hidden under `sm`; the number still matters there. */}
      {totalHits !== undefined ? (
        <p className="text-muted-foreground mt-1.5 font-mono text-xs tabular-nums sm:hidden">
          {formatCount(totalHits)} {scope === "articles" ? "article" : "décision"}
          {totalHits > 1 ? "s" : ""}
          {processingTimeMs !== undefined ? ` · ${processingTimeMs} ms` : ""}
        </p>
      ) : null}
      </div>
    </section>
  );
}
