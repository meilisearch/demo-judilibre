import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArrowLeft, ChevronDown, ExternalLink, FileText, Landmark, Link2, Star } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { citation, formatDate } from "@/lib/format";
import { meiliFetch, meiliSearch, serverEnv } from "@/lib/server-config";
import type { Article, Decision } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * Fetch one decision. Uses a filtered search rather than `GET /documents/:id`,
 * so the page works with a search-only key and the deployment needs no admin key.
 */
async function getDecision(id: string): Promise<Decision | null> {
  if (!/^[A-Za-z0-9_-]+$/.test(id)) return null;
  const result = await meiliSearch<Decision>(serverEnv.index, {
    q: "",
    filter: `id = '${id}'`,
    limit: 1,
  });
  if (result?.hits[0]) return result.hits[0];

  // `id` may not be filterable yet on an index created by an older run. A key with
  // `documents.get` (or an admin key) can still read the document directly.
  if (!serverEnv.masterKey && !serverEnv.documentsKey) return null;
  const direct = await meiliFetch(`/indexes/${serverEnv.index}/documents/${encodeURIComponent(id)}`);
  if (!direct.ok) return null;
  return (await direct.json()) as Decision;
}

/**
 * Resolve the decision's visa to indexed code articles. The visa is prose, so
 * the join goes through the normalised keys the indexer stores in `visa_refs`.
 */
async function getCitedArticles(keys: string[]): Promise<Article[]> {
  if (keys.length === 0) return [];
  const filter = keys.map((k) => `reference_key = '${k.replace(/'/g, "\\'")}'`).join(" OR ");
  const result = await meiliSearch<Article>(serverEnv.legiIndex, {
    q: "",
    filter,
    limit: 50,
    attributesToRetrieve: ["id", "code", "number", "reference", "reference_key", "section", "text"],
  });
  return result?.hits ?? [];
}

export async function generateMetadata({ params }: { params: Promise<{ id: string }> }): Promise<Metadata> {
  const { id } = await params;
  const d = await getDecision(id);
  return { title: d ? citation(d) : "Décision introuvable" };
}

export default async function DecisionPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const d = await getDecision(id);
  if (!d) notFound();

  const extracted = (d.files ?? []).filter((f) => f.content?.trim());
  const articles = await getCitedArticles(d.visa_refs ?? []);

  const meta: Array<[string, string]> = [
    ["Juridiction", d.jurisdiction],
    ["Chambre", d.chamber],
    ["Formation", d.formation],
    ["Date", formatDate(d.decision_date)],
    ["Pourvoi", d.numbers.length > 1 ? d.numbers.join(", ") : d.number],
    ["ECLI", d.ecli],
    ["Nature", d.type],
    ["Solution", d.solution],
    ["Publication", d.publication.join(", ")],
    ["Bulletin", d.bulletin],
  ].filter((row): row is [string, string] => Boolean(row[1]));

  return (
    <div className="mx-auto w-full max-w-7xl px-4 py-6 sm:px-6">
      <Button variant="ghost" size="sm" className="-ml-2 mb-4 max-sm:h-10 max-sm:px-3" nativeButton={false} render={<Link href="/" />}>
        <ArrowLeft data-icon="inline-start" />
        Retour à la recherche
      </Button>

      {/* On a phone the aside comes between the title and the text: its dates, applied
          texts and links are what you want first, not after pages of legal prose.
          Explicit grid placement keeps the desktop layout exactly as it was. */}
      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_18rem] lg:gap-x-10 lg:gap-y-6">
        <header className="flex flex-col gap-3 lg:col-start-1 lg:row-start-1">
          <p className="text-muted-foreground font-mono text-xs tabular-nums">{citation(d)}</p>
          <h1 className="font-heading text-2xl leading-tight font-medium tracking-tight sm:text-4xl">
            {d.titles.length > 0 ? d.titles.join(" — ") : `${d.type || "Décision"} ${d.number}`}
          </h1>
          <div className="flex flex-wrap items-center gap-1.5">
            {d.solution ? <Badge>{d.solution}</Badge> : null}
            {d.publication.map((p) => (
              <Badge key={p} variant="outline">
                {p}
              </Badge>
            ))}
            {d.particular_interest ? (
              <Badge variant="outline" className="text-seal border-seal/40">
                <Star className="fill-current" aria-hidden />
                Intérêt particulier
              </Badge>
            ) : null}
          </div>
        </header>

        <aside className="flex flex-col gap-6 lg:col-start-2 lg:row-span-2 lg:row-start-1 lg:sticky lg:top-20 lg:self-start">
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
            {meta.map(([label, value]) => (
              <div key={label} className="contents">
                <dt className="text-muted-foreground">{label}</dt>
                <dd className="min-w-0 break-words font-medium">{value}</dd>
              </div>
            ))}
          </dl>

          <Button variant="outline" size="sm" nativeButton={false} render={<a href={d.url} target="_blank" rel="noreferrer" />}>
            <ExternalLink data-icon="inline-start" />
            Voir sur courdecassation.fr
          </Button>

          {d.visa.length > 0 ? (
            <section aria-labelledby="visa" className="flex flex-col gap-2">
              <h2 id="visa" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
                Textes appliqués
              </h2>
              <ul className="flex flex-col gap-1 text-sm">
                {d.visa.map((v) => (
                  <li key={v}>{v}</li>
                ))}
              </ul>
              {articles.length > 0 ? (
                <ul className="mt-1 flex flex-col gap-1">
                  {articles.map((a) => (
                    <li key={a.id}>
                      <Link
                        href={`/article/${a.id}`}
                        className="text-seal inline-flex max-w-full items-center gap-1 text-xs hover:underline"
                      >
                        <Landmark className="size-3 shrink-0" aria-hidden />
                        <span className="truncate">
                          Article {a.number} · {a.code}
                        </span>
                      </Link>
                    </li>
                  ))}
                </ul>
              ) : null}
            </section>
          ) : null}

          {d.files.length > 0 ? (
            <section aria-labelledby="documents" className="flex flex-col gap-2">
              <h2 id="documents" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
                Documents associés
              </h2>
              <ul className="flex flex-col gap-1.5 text-sm">
                {d.files.map((f) => (
                  <li key={f.url}>
                    <a href={f.url} target="_blank" rel="noreferrer" className="inline-flex items-start gap-1.5 hover:underline">
                      <FileText className="mt-0.5 size-4 shrink-0" aria-hidden />
                      <span>
                        {f.name || f.type}
                        {f.name && f.type ? <span className="text-muted-foreground"> · {f.type}</span> : null}
                        {f.pages > 0 ? <span className="text-muted-foreground font-mono text-xs"> · {f.pages} p.</span> : null}
                      </span>
                    </a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {d.rapprochements.length > 0 ? (
            <section aria-labelledby="rapprochements" className="flex flex-col gap-2">
              <h2 id="rapprochements" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
                Rapprochements de jurisprudence
              </h2>
              <ul className="flex flex-col gap-1.5 text-sm">
                {d.rapprochements.map((r, i) => (
                  <li key={`${r.url}-${i}`}>
                    <a href={r.url} target="_blank" rel="noreferrer" className="inline-flex items-start gap-1.5 hover:underline">
                      <Link2 className="mt-0.5 size-4 shrink-0" aria-hidden />
                      <span>
                        {r.title}
                        {r.number ? <span className="text-muted-foreground font-mono text-xs"> {r.number}</span> : null}
                      </span>
                    </a>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </aside>

        <article className="flex min-w-0 flex-col gap-6 lg:col-start-1 lg:row-start-2">

          {d.summary ? (
            <section aria-labelledby="sommaire" className="border-seal rounded-r-lg border-l-2 bg-card/60 py-3 pl-4">
              <h2 id="sommaire" className="text-muted-foreground mb-1 text-xs font-semibold tracking-wider uppercase">
                Sommaire
              </h2>
              <p className="font-heading text-lg leading-relaxed">{d.summary}</p>
            </section>
          ) : null}

          {d.themes.length > 0 ? (
            <section aria-label="Matières" className="flex flex-wrap gap-1.5">
              {d.themes.map((t) => (
                <Badge key={t} variant="secondary">
                  {t}
                </Badge>
              ))}
            </section>
          ) : null}

          <Separator />

          <section aria-labelledby="texte" className="flex flex-col gap-3">
            <h2 id="texte" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
              Texte intégral
            </h2>
            <div className="decision-paper decision-text">{d.text}</div>
          </section>

          {extracted.length > 0 ? (
            <section aria-labelledby="extraits" className="flex flex-col gap-3">
              <h2 id="extraits" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
                Contenu des documents associés
              </h2>
              <p className="text-muted-foreground text-sm">
                Texte extrait des PDF joints à la décision. Il est indexé avec la décision et interrogeable.
              </p>
              {extracted.map((f) => (
                <Collapsible key={f.url}>
                  <CollapsibleTrigger
                    className={cn(buttonVariants({ variant: "outline", size: "sm" }), "w-full max-w-[46rem] justify-start")}
                  >
                    <FileText data-icon="inline-start" />
                    <span className="truncate">{f.name || f.type}</span>
                    <span className="text-muted-foreground ml-auto font-mono text-xs">
                      {f.pages > 0 ? `${f.pages} p.` : ""}
                    </span>
                    <ChevronDown data-icon="inline-end" />
                  </CollapsibleTrigger>
                  <CollapsibleContent>
                    <div className="decision-paper decision-text mt-2 text-[0.95rem]">{f.content}</div>
                  </CollapsibleContent>
                </Collapsible>
              ))}
            </section>
          ) : null}
        </article>
      </div>
    </div>
  );
}
