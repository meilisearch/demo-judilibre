export interface ToolCall {
  id: string;
  type: "function";
  function: { name: string; arguments: string };
}

/** OpenAI-compatible message, including the internal ones Meilisearch asks us to keep. */
export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: ToolCall[] | null;
  tool_call_id?: string | null;
}

/** A retrieved document, trimmed to what the sources panel shows. */
export interface SourceDoc {
  id: string;
  /** Decision this document belongs to: itself, or the parent of a passage. */
  decisionId: string;
  jurisdiction: string;
  chamber: string;
  decision_date: string;
  number: string;
  solution: string;
  summary: string;
  titles: string[];
  publication: string[];
  /** `decision` or `attachment` for passages from the chunk index. */
  source: string;
  attachmentName: string;
  attachmentType: string;
  /** Public URL of the PDF this passage was extracted from. */
  attachmentUrl: string;
  /** Documents attached to the decision (decision-index hits). */
  files: Array<{ name: string; type: string; url: string }>;
  /** Passage text when the hit comes from the chunk index. */
  content: string;
  /** Code the article belongs to. Set only on `legi` hits: it tells the two shapes apart. */
  code: string;
  /** Ready-made label, e.g. "Article L110-1 du Code de commerce" (`legi` hits). */
  reference: string;
  /** Innermost section of the code (`legi` hits). */
  section: string;
  /** The article's own text (`legi` hits). */
  text: string;
}

export interface SearchStep {
  callId: string;
  query: string;
  filter?: string;
  indexUid: string;
  documents: SourceDoc[];
}

export interface AssistantTurn {
  id: string;
  role: "assistant";
  content: string;
  steps: SearchStep[];
  pending: boolean;
  error?: string;
}

export interface UserTurn {
  id: string;
  role: "user";
  content: string;
}

export type Turn = UserTurn | AssistantTurn;
