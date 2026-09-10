import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArrowLeft, ChevronRight, ExternalLink, Info, Landmark, Scale } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { citation, formatDate } from "@/lib/format";
import { meiliFetch, meiliSearch, serverEnv } from "@/lib/server-config";
import type { Article, DecisionHit } from "@/lib/types";

/**
 * Fetch one article. Prefers a filtered search so a search-only key suffices;
 * falls back to reading the document directly, which covers an index whose
 * `id` is not filterable yet.
 */
async function getArticle(id: string): Promise<Article | null> {
  if (!/^[A-Za-z0-9_-]+$/.test(id)) return null;
  const result = await meiliSearch<Article>(serverEnv.legiIndex, { q: "", filter: `id = '${id}'`, limit: 1 });
  if (result?.hits[0]) return result.hits[0];

  if (!serverEnv.masterKey && !serverEnv.documentsKey) return null;
  const direct = await meiliFetch(`/indexes/${serverEnv.legiIndex}/documents/${encodeURIComponent(id)}`);
  if (!direct.ok) return null;
  return (await direct.json()) as Article;
}

/**
 * Decisions that apply this article. Judilibre states applied texts as prose, so
 * the indexer reduces both sides to the same key (see indexer/src/refs.rs) —
 * that key is what makes this lookup possible.
 */
async function getApplyingDecisions(referenceKey: string): Promise<{ hits: DecisionHit[]; total: number }> {
  const escaped = referenceKey.replace(/'/g, "\\'");
  const result = await meiliSearch<DecisionHit>(serverEnv.index, {
    q: "",
    filter: `visa_refs = '${escaped}'`,
    limit: 20,
    sort: ["decision_timestamp:desc"],
      attributesToRetrieve: [
      "id",
      "jurisdiction",
      "chamber",
      "number",
      "decision_date",
      "solution",
      "titles",
      "summary",
    ],
  });
  return { hits: result?.hits ?? [], total: result?.estimatedTotalHits ?? 0 };
}

export async function generateMetadata({ params }: { params: Promise<{ id: string }> }): Promise<Metadata> {
  const { id } = await params;
  const a = await getArticle(id);
  return { title: a ? a.reference : "Article introuvable" };
}

export default async function ArticlePage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const article = await getArticle(id);
  if (!article) notFound();

  const { hits: decisions, total } = await getApplyingDecisions(article.reference_key);



  return (
    <div className="mx-auto w-full max-w-7xl px-4 py-6 sm:px-6">
      <Button variant="ghost" size="sm" className="-ml-2 mb-4" nativeButton={false} render={<Link href="/" />}>
        <ArrowLeft data-icon="inline-start" />
        Retour à la recherche
      </Button>

      <div className="grid gap-10 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <article className="flex min-w-0 flex-col gap-6">
          <header className="flex flex-col gap-3">
            <p className="text-muted-foreground flex items-center gap-1.5 font-mono text-xs">
              <Landmark className="size-3.5" aria-hidden />
              {article.code}
            </p>
            <h1 className="font-heading text-3xl leading-tight font-medium tracking-tight sm:text-4xl">
              Article {article.number}
            </h1>
            {article.hierarchy.length > 0 ? (
              <nav aria-label="Emplacement dans le code" className="flex flex-wrap items-center gap-1 text-sm">
                {article.hierarchy.map((level, i) => (
                  <span key={level} className="text-muted-foreground flex items-center gap-1">
                    {i > 0 ? <ChevronRight className="size-3.5 shrink-0" aria-hidden /> : null}
                    {level}
                  </span>
                ))}
              </nav>
            ) : null}
            <div className="flex flex-wrap items-center gap-1.5">
              <Badge>Article en vigueur</Badge>
              {article.date_debut ? <Badge variant="outline">depuis le {formatDate(article.date_debut)}</Badge> : null}
            </div>
          </header>

          <section aria-labelledby="texte" className="flex flex-col gap-3">
            <h2 id="texte" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
              Texte de l&apos;article
            </h2>
            <div className="decision-paper decision-text">{article.text}</div>
          </section>

          <Separator />

          <section aria-labelledby="jurisprudence" className="flex flex-col gap-3">
            <h2 id="jurisprudence" className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
              Jurisprudence appliquant cet article
            </h2>
            {decisions.length === 0 ? (
              <p className="text-muted-foreground text-sm">
                Aucune décision indexée ne cite cet article dans ses textes appliqués.
              </p>
            ) : (
              <>
                <p className="text-muted-foreground text-sm">
                  {total > decisions.length
                    ? `${total.toLocaleString("fr-FR")} décisions citent cet article ; les ${decisions.length} plus récentes :`
                    : `${total} décision${total > 1 ? "s" : ""} cite${total > 1 ? "nt" : ""} cet article :`}
                </p>
                <Alert>
                  <Info />
                  <AlertDescription>
                    Le rapprochement se fait par numéro d&apos;article, sans tenir compte de la version appliquée. Une
                    décision peut donc viser ce numéro « dans sa rédaction antérieure », c&apos;est-à-dire un texte
                    différent de celui affiché ci-dessus — la numérotation du code civil a notamment été modifiée par
                    l&apos;ordonnance du 10 février 2016. Vérifiez la rédaction visée dans la décision.
                  </AlertDescription>
                </Alert>
                <ol className="flex flex-col gap-2">
                  {decisions.map((d) => (
                    <li key={d.id}>
                      <Link
                        href={`/decision/${d.id}`}
                        className="bg-card hover:ring-foreground/20 focus-visible:ring-ring/50 flex flex-col gap-1 rounded-lg p-3 ring-1 ring-foreground/10 transition-shadow outline-none hover:shadow-sm focus-visible:ring-3"
                      >
                        <span className="text-muted-foreground flex items-center gap-2 font-mono text-xs tabular-nums">
                          <Scale className="size-3" aria-hidden />
                          {citation(d)}
                          {d.solution ? <span className="text-foreground">· {d.solution}</span> : null}
                        </span>
                        {d.titles?.length ? (
                          <span className="font-heading line-clamp-1 text-sm">{d.titles[0]}</span>
                        ) : null}
                        {d.summary ? (
                          <span className="text-muted-foreground line-clamp-2 text-xs leading-relaxed">{d.summary}</span>
                        ) : null}
                      </Link>
                    </li>
                  ))}
                </ol>
              </>
            )}
          </section>
        </article>

        <aside className="flex flex-col gap-6 lg:sticky lg:top-20 lg:self-start">
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
            {(
              [
                ["Code", article.code],
                ["Article", article.number],
                ["Subdivision", article.section],
                ["En vigueur", article.date_debut ? formatDate(article.date_debut) : ""],
                ["Identifiant", article.id],
              ] as Array<[string, string]>
            )
              .filter(([, value]) => Boolean(value))
              .map(([label, value]) => (
                <div key={label} className="contents">
                  <dt className="text-muted-foreground">{label}</dt>
                  <dd className="min-w-0 break-words font-medium">{value}</dd>
                </div>
              ))}
          </dl>

          <Button
            variant="outline"
            size="sm"
            nativeButton={false}
            render={<a href={article.url} target="_blank" rel="noreferrer" />}
          >
            <ExternalLink data-icon="inline-start" />
            Voir sur Légifrance
          </Button>
        </aside>
      </div>
    </div>
  );
}
