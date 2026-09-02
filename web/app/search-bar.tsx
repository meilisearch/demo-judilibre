"use client";

import { Quote, ScrollText, Search, Sparkles, X } from "lucide-react";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
  InputGroupText,
} from "@/components/ui/input-group";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { formatCount } from "@/lib/format";
import { type SearchScope, useSearchStore } from "@/lib/search-store";
import { cn } from "@/lib/utils";

interface Props {
  totalHits?: number;
  processingTimeMs?: number;
  isFetching: boolean;
  /** Semantic search is available on the index being queried. */
  aiAvailable: boolean;
  /** Passages are indexed, so the scope switch is meaningful. */
  hasPassages: boolean;
}

export function SearchBar({ totalHits, processingTimeMs, isFetching, aiAvailable, hasPassages }: Props) {
  const { query, setQuery, mode, setMode, scope, setScope } = useSearchStore();
  const aiOn = mode === "hybrid";

  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <h1 className="font-heading text-3xl leading-tight font-medium tracking-tight sm:text-4xl">
          La jurisprudence, <span className="italic">au mot près</span>.
        </h1>
        <p className="text-muted-foreground max-w-2xl text-sm">
          Décisions de la Cour de cassation publiées sur Judilibre. Tapez une notion, un article de code ou un numéro de
          pourvoi : les résultats s&apos;affichent à chaque frappe.
        </p>
      </div>

      <InputGroup className="h-12 rounded-xl text-base shadow-sm">
        <InputGroupAddon align="inline-start">
          <Search />
        </InputGroupAddon>
        <InputGroupInput
          type="search"
          autoFocus
          enterKeyHint="search"
          aria-label="Rechercher une décision"
          placeholder="Rechercher une décision, une notion, un numéro de pourvoi…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="text-base"
        />
        <InputGroupAddon align="inline-end">
          {isFetching ? (
            <Spinner />
          ) : totalHits !== undefined ? (
            <InputGroupText className="hidden font-mono text-xs tabular-nums sm:flex">
              {formatCount(totalHits)} {scope === "passages" ? "passage" : "décision"}
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
                    ? "Recherche hybride active : mots-clés + sens (embeddings Voyage AI). Cliquez pour revenir aux mots-clés."
                    : "Activer la recherche hybride : combine les mots-clés et le sens de la question."}
                </TooltipContent>
              </Tooltip>
            </>
          ) : null}
        </InputGroupAddon>
      </InputGroup>

      {hasPassages ? (
        <ToggleGroup
          value={[scope]}
          onValueChange={(v: string[]) => {
            const next = v.at(-1) as SearchScope | undefined;
            if (next) setScope(next);
          }}
          variant="outline"
          size="sm"
          aria-label="Portée de la recherche"
          className="self-start"
        >
          <ToggleGroupItem value="decisions">
            <ScrollText data-icon="inline-start" />
            Décisions
          </ToggleGroupItem>
          <ToggleGroupItem value="passages">
            <Quote data-icon="inline-start" />
            Passages
          </ToggleGroupItem>
        </ToggleGroup>
      ) : null}
    </section>
  );
}
