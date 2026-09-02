import { Fragment, type ReactNode } from "react";
import { HL_POST, HL_PRE } from "@/lib/types";

/**
 * Turn a Meilisearch `_formatted` string into React nodes, rendering matches as
 * <mark>. We never use innerHTML: the markers are plain-text sentinels.
 */
export function Highlight({ text, className }: { text: string | undefined; className?: string }): ReactNode {
  if (!text) return null;
  const parts = text.split(HL_PRE);
  if (parts.length === 1) return <span className={className}>{text}</span>;
  return (
    <span className={className}>
      {parts.map((part, i) => {
        if (i === 0) return <Fragment key={i}>{part}</Fragment>;
        const [hit, rest] = part.split(HL_POST);
        return (
          <Fragment key={i}>
            <mark>{hit}</mark>
            {rest}
          </Fragment>
        );
      })}
    </span>
  );
}

export function stripHighlight(text: string | undefined): string {
  return (text ?? "").replaceAll(HL_PRE, "").replaceAll(HL_POST, "");
}
