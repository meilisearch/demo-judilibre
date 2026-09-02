"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import ReactMarkdown from "react-markdown";
import { AlertCircle, ArrowUp, ChevronDown, ExternalLink, FileText, Scale, Search, Square } from "lucide-react";
import { buttonVariants } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { toast } from "sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupTextarea } from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/spinner";
import { consumeChatStream } from "@/lib/chat-stream";
import type { AssistantTurn, ChatMessage, SearchStep, SourceDoc, Turn } from "@/lib/chat-types";
import { citation } from "@/lib/format";
import { cn } from "@/lib/utils";

const SUGGESTIONS = [
  "Quelles sont les conditions de la nullité du licenciement en cas de harcèlement moral ?",
  "Comment la Cour de cassation apprécie-t-elle le préjudice d'anxiété lié à l'amiante ?",
  "Un bailleur peut-il refuser le renouvellement d'un bail commercial sans indemnité d'éviction ?",
  "Quelle est la portée de l'obligation de sécurité de l'employeur en 2024 ?",
];

let turnSeq = 0;
const nextId = () => `t${++turnSeq}`;

export function ChatPanel() {
  const [turns, setTurns] = useState<Turn[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [setupError, setSetupError] = useState<string | null>(null);
  const historyRef = useRef<ChatMessage[]>([]);
  const abortRef = useRef<AbortController | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [turns]);

  const updateAssistant = useCallback((id: string, patch: (t: AssistantTurn) => AssistantTurn) => {
    setTurns((prev) => prev.map((t) => (t.id === id && t.role === "assistant" ? patch(t) : t)));
  }, []);

  const send = useCallback(
    async (text: string) => {
      const content = text.trim();
      if (!content || busy) return;
      setInput("");
      setSetupError(null);
      setBusy(true);

      const assistantId = nextId();
      setTurns((prev) => [
        ...prev,
        { id: nextId(), role: "user", content },
        { id: assistantId, role: "assistant", content: "", steps: [], pending: true },
      ]);
      historyRef.current.push({ role: "user", content });

      const controller = new AbortController();
      abortRef.current = controller;
      let answer = "";
      let sawAssistantMemory = false;

      try {
        const res = await fetch("/api/chat", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ messages: historyRef.current }),
          signal: controller.signal,
        });
        if (!res.ok) {
          const body = (await res.json().catch(() => ({}))) as { error?: string; detail?: string };
          const message = body.error ?? `HTTP ${res.status}`;
          if (res.status === 503) setSetupError(`${message}${body.detail ? ` — ${body.detail}` : ""}`);
          throw new Error(message);
        }
        await consumeChatStream(res, {
          onContent: (delta) => {
            answer += delta;
            updateAssistant(assistantId, (t) => ({ ...t, content: t.content + delta }));
          },
          onStep: (step) =>
            updateAssistant(assistantId, (t) => {
              const idx = t.steps.findIndex((s) => s.callId === step.callId);
              const steps = idx === -1 ? [...t.steps, step] : t.steps.map((s, i) => (i === idx ? step : s));
              return { ...t, steps };
            }),
          onMemory: (message) => {
            if (message.role === "assistant" && message.content && !message.tool_calls?.length) sawAssistantMemory = true;
            historyRef.current.push(message);
          },
        });
        if (!sawAssistantMemory && answer) historyRef.current.push({ role: "assistant", content: answer });
        updateAssistant(assistantId, (t) => ({ ...t, pending: false }));
      } catch (error) {
        const aborted = error instanceof DOMException && error.name === "AbortError";
        const message = aborted ? "Réponse interrompue." : error instanceof Error ? error.message : "Erreur inconnue";
        if (!aborted) toast.error("L'assistant n'a pas pu répondre", { description: message });
        if (answer) historyRef.current.push({ role: "assistant", content: answer });
        updateAssistant(assistantId, (t) => ({ ...t, pending: false, error: message }));
      } finally {
        abortRef.current = null;
        setBusy(false);
      }
    },
    [busy, updateAssistant],
  );

  const stop = () => abortRef.current?.abort();

  const lastAssistant = [...turns].reverse().find((t): t is AssistantTurn => t.role === "assistant");
  const sources = dedupeSources(lastAssistant?.steps ?? []);

  return (
    <div className="mx-auto grid h-[calc(100svh-var(--header-height))] w-full max-w-7xl grid-rows-[minmax(0,1fr)] gap-x-8 px-4 sm:px-6 lg:grid-cols-[minmax(0,1fr)_20rem]">
      <section className="flex min-h-0 min-w-0 flex-col" aria-label="Conversation">
        {/* The conversation scrolls; the composer below stays put. */}
        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto py-6">
          {turns.length === 0 ? (
            <div className="flex flex-1 flex-col justify-center gap-6 py-10">
              <div className="flex flex-col gap-2">
                <p className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">Assistant jurisprudence</p>
                <h1 className="font-heading text-3xl leading-tight font-medium tracking-tight sm:text-4xl">
                  Posez votre question, <span className="italic">la Cour répond par ses arrêts</span>.
                </h1>
                <p className="text-muted-foreground max-w-2xl text-sm">
                  L&apos;assistant interroge Meilisearch en direct, puis rédige une réponse appuyée sur les décisions
                  trouvées. Chaque source est consultable. Réponse informative, pas un avis juridique.
                </p>
              </div>
              <ul className="grid gap-2 sm:grid-cols-2">
                {SUGGESTIONS.map((s) => (
                  <li key={s}>
                    <button
                      type="button"
                      onClick={() => send(s)}
                      className="bg-card hover:ring-foreground/25 focus-visible:ring-ring/50 w-full rounded-xl p-4 text-left text-sm leading-relaxed ring-1 ring-foreground/10 transition-shadow outline-none hover:shadow-sm focus-visible:ring-3"
                    >
                      {s}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ) : (
            <ol className="flex flex-col gap-6">
              {turns.map((turn) =>
                turn.role === "user" ? (
                  <li key={turn.id} className="flex justify-end">
                    <p className="bg-primary text-primary-foreground max-w-[85%] rounded-2xl rounded-br-md px-4 py-2.5 text-sm leading-relaxed">
                      {turn.content}
                    </p>
                  </li>
                ) : (
                  <li key={turn.id}>
                    <AssistantMessage turn={turn} />
                  </li>
                ),
              )}
            </ol>
          )}

          {setupError ? (
            <Alert variant="destructive">
              <AlertCircle />
              <AlertTitle>Le chat n&apos;est pas configuré</AlertTitle>
              <AlertDescription>
                {setupError}. Renseignez <code>CHAT_API_KEY</code> dans <code>.env</code> puis relancez{" "}
                <code>cargo run -- setup</code> dans <code>indexer/</code>.
              </AlertDescription>
            </Alert>
          ) : null}

          <div ref={bottomRef} />
        </div>

        <form
          className="shrink-0 pb-6"
          onSubmit={(e) => {
            e.preventDefault();
            void send(input);
          }}
        >
          <InputGroup className="bg-background rounded-2xl shadow-md">
            <InputGroupTextarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void send(input);
                }
              }}
              rows={1}
              placeholder="Votre question de droit…"
              aria-label="Votre question"
              className="min-h-11 max-h-40 py-3 text-base"
            />
            <InputGroupAddon align="inline-end">
              {busy ? (
                <InputGroupButton size="icon-sm" variant="outline" aria-label="Interrompre" onClick={stop}>
                  <Square />
                </InputGroupButton>
              ) : (
                <InputGroupButton size="icon-sm" variant="default" type="submit" aria-label="Envoyer" disabled={!input.trim()}>
                  <ArrowUp />
                </InputGroupButton>
              )}
            </InputGroupAddon>
          </InputGroup>
        </form>
      </section>

      <aside aria-label="Sources" className="hidden min-h-0 border-l pl-8 lg:flex lg:flex-col">
        <h2 className="shrink-0 py-6 text-xs font-semibold tracking-wider uppercase">Sources citées</h2>
        {/* Scrolls independently of the conversation: a long answer never buries its sources. */}
        <div className="min-h-0 flex-1 overflow-y-auto pb-6">
          {sources.length === 0 ? (
            <p className="text-muted-foreground text-sm">Les décisions retrouvées par Meilisearch pour la dernière réponse apparaîtront ici.</p>
          ) : (
            <ol className="flex flex-col gap-2">
              {sources.map((doc, i) => (
                <li key={doc.id}>
                  <SourceCard doc={doc} index={i + 1} />
                </li>
              ))}
            </ol>
          )}
        </div>
      </aside>
    </div>
  );
}

function AssistantMessage({ turn }: { turn: AssistantTurn }) {
  const sources = dedupeSources(turn.steps);
  const current = turn.steps.at(-1);
  return (
    <div className="flex gap-3">
      <div className="bg-secondary text-secondary-foreground mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-full">
        <Scale className="size-4" aria-hidden />
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        {turn.pending ? (
          // One live line, rewritten as each search runs, instead of a growing list.
          <p className="text-muted-foreground flex min-w-0 items-center gap-2 text-sm" aria-live="polite">
            <Spinner />
            {current?.query ? (
              <>
                <span className="shrink-0">Lecture des décisions</span>
                <span className="min-w-0 truncate italic">« {current.query} »</span>
              </>
            ) : (
              <span>Recherche dans Judilibre…</span>
            )}
          </p>
        ) : turn.steps.length > 0 ? (
          <SearchTrace steps={turn.steps} sourceCount={sources.length} />
        ) : null}

        {turn.content ? (
          <div className="md text-[15px]">
            <ReactMarkdown>{turn.content}</ReactMarkdown>
          </div>
        ) : null}

        {turn.error ? <p className="text-destructive text-sm">{turn.error}</p> : null}

        {sources.length > 0 && !turn.pending ? (
          <ol className="flex flex-wrap gap-1.5 lg:hidden" aria-label="Sources">
            {sources.map((doc, i) => (
              <li key={doc.id}>
                <Link href={`/decision/${doc.decisionId}`} className="hover:underline">
                  <Badge variant="secondary" className="font-mono">
                    [{i + 1}] {citation(doc)}
                  </Badge>
                </Link>
              </li>
            ))}
          </ol>
        ) : null}
      </div>
    </div>
  );
}

/** Collapsed record of the searches Meilisearch ran for one answer. */
function SearchTrace({ steps, sourceCount }: { steps: SearchStep[]; sourceCount: number }) {
  return (
    <Collapsible>
      <CollapsibleTrigger className={cn(buttonVariants({ variant: "ghost", size: "xs" }), "text-muted-foreground -ml-2")}>
        <Search data-icon="inline-start" />
        {steps.length} recherche{steps.length > 1 ? "s" : ""} · {sourceCount} source
        {sourceCount > 1 ? "s" : ""}
        <ChevronDown data-icon="inline-end" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <ul className="mt-2 flex flex-col gap-1" aria-label="Recherches effectuées">
          {steps.map((s) => (
            <li key={s.callId} className="text-muted-foreground flex min-w-0 items-center gap-2 text-xs">
              <span className="min-w-0 truncate italic">« {s.query || "…"} »</span>
              <span className="shrink-0 font-mono text-[11px]">
                {s.indexUid.endsWith("_chunk") ? "passages" : "décisions"}
              </span>
              <span className="shrink-0 font-mono text-[11px] tabular-nums">{s.documents.length}</span>
            </li>
          ))}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  );
}

function SourceCard({ doc, index }: { doc: SourceDoc; index: number }) {
  const title = doc.titles?.[0] ?? "";
  const isAttachment = doc.source === "attachment";
  const excerpt = doc.content || doc.summary;
  // A passage links its own PDF; a whole decision links every attached document.
  const files = isAttachment && doc.attachmentUrl
    ? [{ name: doc.attachmentName, type: doc.attachmentType, url: doc.attachmentUrl }]
    : doc.files;

  return (
    <div className="bg-card flex flex-col gap-1 rounded-lg p-3 ring-1 ring-foreground/10 transition-shadow hover:shadow-sm">
      <Link
        href={`/decision/${doc.decisionId}`}
        className="focus-visible:ring-ring/50 flex flex-col gap-1 rounded outline-none focus-visible:ring-3"
      >
        <span className="text-muted-foreground flex items-center gap-2 font-mono text-[11px] tabular-nums">
          <span className="text-seal font-medium">[{index}]</span>
          {citation(doc)}
        </span>
        {title ? <span className="font-heading line-clamp-2 text-sm leading-snug">{title}</span> : null}
        {excerpt ? <span className="text-muted-foreground line-clamp-3 text-xs leading-relaxed">{excerpt}</span> : null}
      </Link>

      {files.length > 0 ? (
        <ul className="mt-0.5 flex flex-col gap-0.5">
          {files.map((f) => (
            <li key={f.url}>
              <a
                href={f.url}
                target="_blank"
                rel="noreferrer"
                className="text-muted-foreground hover:text-foreground inline-flex max-w-full items-center gap-1 text-[11px] hover:underline"
              >
                <FileText className="size-3 shrink-0" aria-hidden />
                <span className="truncate">{f.type || f.name || "Document associé"}</span>
                <ExternalLink className="size-2.5 shrink-0" aria-hidden />
              </a>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function dedupeSources(steps: SearchStep[]): SourceDoc[] {
  const seen = new Set<string>();
  const out: SourceDoc[] = [];
  for (const step of steps) {
    for (const doc of step.documents) {
      if (doc?.id && !seen.has(doc.id)) {
        seen.add(doc.id);
        out.push(doc);
      }
    }
  }
  return out;
}
