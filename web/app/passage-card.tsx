import Link from "next/link";
import { FileText, Quote } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { citation } from "@/lib/format";
import { Highlight } from "@/lib/highlight";
import type { ChunkHit } from "@/lib/types";

/** One passage of a decision (or of a document attached to it). */
export function PassageCard({ hit, position }: { hit: ChunkHit; position: number }) {
  const f = hit._formatted;
  const isAttachment = hit.source === "attachment";
  const title = (f?.titles?.length ? f.titles : hit.titles)[0];

  return (
    <article className="group bg-card hover:ring-foreground/20 relative flex flex-col gap-2.5 rounded-xl p-5 ring-1 ring-foreground/10 transition-shadow hover:shadow-sm">
      <div className="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-xs tabular-nums">
        <span className="text-foreground font-medium">{citation(hit)}</span>
        <span aria-hidden>·</span>
        <span>
          passage {hit.chunk_index + 1} / {hit.chunk_count}
        </span>
      </div>

      <h3 className="font-heading text-base leading-snug font-medium">
        <Link href={`/decision/${hit.decision_id}`} className="after:absolute after:inset-0 focus-visible:outline-none">
          <span className="sr-only">Résultat {position} : </span>
          {title ? <Highlight text={title} /> : citation(hit)}
        </Link>
      </h3>

      <blockquote className="text-foreground/85 border-l-2 pl-3 text-sm leading-relaxed">
        <Quote className="text-muted-foreground/50 mb-1 size-3.5" aria-hidden />
        <Highlight text={f?.content ?? hit.content} />
      </blockquote>

      <div className="flex flex-wrap items-center gap-1.5 pt-0.5">
        {isAttachment ? (
          <Badge variant="outline" className="gap-1">
            <FileText aria-hidden />
            {hit.attachment_type || "Document associé"}
          </Badge>
        ) : (
          <Badge variant="secondary">Texte de la décision</Badge>
        )}
        {hit.solution ? <Badge variant="secondary">{hit.solution}</Badge> : null}
        {isAttachment && hit.attachment_name ? (
          <span className="text-muted-foreground truncate font-mono text-xs">{hit.attachment_name}</span>
        ) : null}
      </div>
    </article>
  );
}
