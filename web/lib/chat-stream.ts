import type { ChatMessage, SearchStep, SourceDoc, ToolCall } from "@/lib/chat-types";

interface StreamDelta {
  content?: string | null;
  tool_calls?: Array<{ index?: number; id?: string; type?: string; function?: { name?: string; arguments?: string } }>;
}

interface StreamChunk {
  type?: string;
  choices?: Array<{ delta?: StreamDelta; finish_reason?: string | null }>;
  error?: { message?: string };
}

export interface StreamHandlers {
  onContent: (delta: string) => void;
  onStep: (step: SearchStep) => void;
  /** Meilisearch asks the client to keep these in history for the next request. */
  onMemory: (message: ChatMessage) => void;
}

function parseJson<T>(raw: string): T | null {
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

/**
 * Sources arrive as whole index documents, full text included. Keep only what the
 * UI renders so a conversation does not hold megabytes of decision text.
 */
function trimSource(doc: Record<string, unknown>): SourceDoc {
  const str = (k: string) => (typeof doc[k] === "string" ? (doc[k] as string) : "");
  const arr = (k: string) => (Array.isArray(doc[k]) ? (doc[k] as unknown[]).filter((v): v is string => typeof v === "string") : []);
  const id = str("id");
  return {
    id,
    // Passages from the chunk index carry the parent decision id.
    decisionId: str("decision_id") || id,
    jurisdiction: str("jurisdiction"),
    chamber: str("chamber"),
    decision_date: str("decision_date"),
    number: str("number"),
    solution: str("solution"),
    summary: str("summary"),
    titles: arr("titles"),
    publication: arr("publication"),
    source: str("source"),
    attachmentName: str("attachment_name"),
    attachmentType: str("attachment_type"),
    attachmentUrl: str("attachment_url"),
    files: Array.isArray(doc.files)
      ? (doc.files as Array<Record<string, unknown>>)
          .filter((f) => f && typeof f === "object" && typeof f.url === "string" && f.url)
          .map((f) => ({
            name: typeof f.name === "string" ? f.name : "",
            type: typeof f.type === "string" ? f.type : "",
            url: f.url as string,
          }))
      : [],
    content: str("content"),
  };
}

function toSourceList(raw: unknown): SourceDoc[] {
  const list = Array.isArray(raw) ? raw : raw && typeof raw === "object" ? Object.values(raw) : [];
  return list.filter((d): d is Record<string, unknown> => !!d && typeof d === "object").map(trimSource);
}

/**
 * Consume the SSE stream from /api/chat and dispatch text, search steps and
 * conversation-memory messages. Resolves when the stream ends.
 *
 * Field names follow Meilisearch's chat tooling: `_meiliSearchProgress` carries
 * `function_arguments` (the search Meilisearch is about to run) and
 * `_meiliSearchSources` carries `sources` (the retrieved documents). Older and
 * newer spellings are accepted so an upgrade does not blank the sources panel.
 */
export async function consumeChatStream(response: Response, handlers: StreamHandlers): Promise<void> {
  if (!response.body) throw new Error("Empty response body");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const steps = new Map<string, SearchStep>();
  let buffer = "";

  const emit = (step: SearchStep) => {
    steps.set(step.callId, step);
    handlers.onStep(step);
  };

  const handleToolCall = (call: NonNullable<StreamDelta["tool_calls"]>[number]) => {
    const name = call.function?.name;
    const rawArgs = call.function?.arguments;
    if (!name || !rawArgs) return;

    if (name === "_meiliSearchProgress") {
      const args = parseJson<{ call_id: string; function_name?: string; function_arguments?: string; function_parameters?: string }>(rawArgs);
      if (!args) return;
      const inner = args.function_arguments ?? args.function_parameters ?? "";
      const params = parseJson<{ q?: string; filter?: string; index_uid?: string }>(inner) ?? {};
      emit({
        callId: args.call_id,
        query: params.q ?? "",
        filter: params.filter,
        indexUid: params.index_uid ?? "",
        documents: steps.get(args.call_id)?.documents ?? [],
      });
    } else if (name === "_meiliSearchSources") {
      const args = parseJson<{ call_id: string; sources?: unknown; documents?: unknown }>(rawArgs);
      if (!args) return;
      const existing = steps.get(args.call_id);
      emit({
        callId: args.call_id,
        query: existing?.query ?? "",
        filter: existing?.filter,
        indexUid: existing?.indexUid ?? "",
        documents: toSourceList(args.sources ?? args.documents),
      });
    } else if (name === "_meiliAppendConversationMessage") {
      const msg = parseJson<ChatMessage & { tool_calls?: ToolCall[] | null }>(rawArgs);
      if (msg) handlers.onMemory(msg);
    }
  };

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";

    for (const line of lines) {
      if (!line.startsWith("data:")) continue;
      const data = line.slice(5).trim();
      if (data === "[DONE]") return;
      const chunk = parseJson<StreamChunk>(data);
      if (!chunk) continue;
      if (chunk.error?.message) throw new Error(chunk.error.message);
      const delta = chunk.choices?.[0]?.delta;
      if (!delta) continue;
      if (delta.content) handlers.onContent(delta.content);
      for (const call of delta.tool_calls ?? []) handleToolCall(call);
    }
  }
}
