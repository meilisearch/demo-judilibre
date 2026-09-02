import Link from "next/link";
import { Paperclip, Star } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { citation, formatDate, shortChamber, shortJurisdiction } from "@/lib/format";
import { Highlight } from "@/lib/highlight";
import { HL_PRE, type SearchHit } from "@/lib/types";

export function HitCard({ hit, position }: { hit: SearchHit; position: number }) {
  const f = hit._formatted;
  const titles = (f?.titles?.length ? f.titles : hit.titles).slice(0, 3);
  const publication = hit.publication.filter((p) => p && !/non publi/i.test(p));

  // Prefer a snippet that actually contains the match; the motivations excerpt reads
  // better than the raw text, whose opening is administrative boilerplate.
  const matched = [f?.excerpt, f?.text].find((t) => t?.includes(HL_PRE));
  // Without a sommaire, still give the card some substance.
  const fallback = hit.summary ? undefined : (f?.excerpt ?? hit.excerpt);
  const snippet = matched ?? fallback;

  return (
    <article className="group bg-card hover:ring-foreground/20 relative flex flex-col gap-2.5 rounded-xl p-5 ring-1 ring-foreground/10 transition-shadow hover:shadow-sm">
      <div className="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-xs tabular-nums">
        <span className="text-foreground font-medium">
          {shortJurisdiction(hit.jurisdiction)} {shortChamber(hit.chamber)}
        </span>
        <span aria-hidden>·</span>
        <time dateTime={hit.decision_date}>{formatDate(hit.decision_date, "d MMM yyyy")}</time>
        {hit.number ? (
          <>
            <span aria-hidden>·</span>
            <span>
              n° <Highlight text={f?.number ?? hit.number} />
            </span>
          </>
        ) : null}
        {hit.particular_interest ? (
          <span className="text-seal ml-auto inline-flex items-center gap-1" title="Décision d'intérêt particulier">
            <Star className="size-3.5 fill-current" aria-hidden />
          </span>
        ) : null}
      </div>

      <h3 className="font-heading text-lg leading-snug font-medium">
        <Link href={`/decision/${hit.id}`} className="after:absolute after:inset-0 focus-visible:outline-none">
          <span className="sr-only">Résultat {position} : </span>
          {titles.length > 0 ? (
            titles.map((t, i) => (
              <span key={i}>
                {i > 0 ? <span className="text-muted-foreground"> — </span> : null}
                <Highlight text={t} />
              </span>
            ))
          ) : (
            <span>{citation(hit)}</span>
          )}
        </Link>
      </h3>

      {hit.summary ? (
        <p className="text-foreground/85 line-clamp-3 text-sm leading-relaxed">
          <Highlight text={f?.summary ?? hit.summary} />
        </p>
      ) : null}

      {snippet ? (
        <p className="text-muted-foreground border-l-2 pl-3 text-sm leading-relaxed">
          <Highlight text={snippet} />
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-1.5 pt-0.5">
        {hit.solution ? <Badge variant="secondary">{hit.solution}</Badge> : null}
        {publication.slice(0, 2).map((p) => (
          <Badge key={p} variant="outline">
            {p}
          </Badge>
        ))}
        {hit.type && !/arr[êe]t/i.test(hit.type) ? <Badge variant="outline">{hit.type}</Badge> : null}
        {hit.files?.length ? (
          <span className="text-muted-foreground ml-auto inline-flex items-center gap-1 text-xs">
            <Paperclip className="size-3.5" aria-hidden />
            {hit.files.length}
          </span>
        ) : null}
      </div>
    </article>
  );
}
