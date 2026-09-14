import Link from "next/link";
import { ChevronRight, Landmark } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Highlight } from "@/lib/highlight";
import type { ArticleHit } from "@/lib/types";

/** One in-force article of a French code. */
export function ArticleCard({ hit, position }: { hit: ArticleHit; position: number }) {
  const f = hit._formatted;
  // The outermost level is usually "Partie législative"; the inner levels locate the rule.
  const path = hit.hierarchy.slice(-2);

  return (
    <article className="group bg-card hover:ring-foreground/20 relative flex flex-col gap-2.5 rounded-xl p-4 ring-1 sm:p-5 ring-foreground/10 transition-shadow hover:shadow-sm">
      <div className="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-xs tabular-nums">
        <span className="text-foreground flex items-center gap-1.5 font-medium">
          <Landmark className="size-3.5" aria-hidden />
          {f?.code ? <Highlight text={f.code} /> : hit.code}
        </span>
        {hit.date_debut ? (
          <>
            <span aria-hidden>·</span>
            <span>en vigueur depuis {hit.date_debut}</span>
          </>
        ) : null}
      </div>

      <h3 className="font-heading text-base leading-snug font-medium">
        <Link href={`/article/${hit.id}`} className="after:absolute after:inset-0 focus-visible:outline-none">
          <span className="sr-only">Résultat {position} : </span>
          Article {f?.number ? <Highlight text={f.number} /> : hit.number}
        </Link>
      </h3>

      {path.length > 0 ? (
        <p className="text-muted-foreground flex flex-wrap items-center gap-1 text-xs">
          {path.map((level, i) => (
            <span key={level} className="flex items-center gap-1">
              {i > 0 ? <ChevronRight className="size-3 shrink-0" aria-hidden /> : null}
              <span className="line-clamp-1">{level}</span>
            </span>
          ))}
        </p>
      ) : null}

      <p className="text-foreground/85 line-clamp-4 text-sm leading-relaxed">
        <Highlight text={f?.text ?? hit.text} />
      </p>

      <div className="flex flex-wrap items-center gap-1.5 pt-0.5">
        <Badge variant="secondary">Article en vigueur</Badge>
        <span className="text-muted-foreground font-mono text-xs tabular-nums">
          {hit.text_length.toLocaleString("fr-FR")} caractères
        </span>
      </div>
    </article>
  );
}
