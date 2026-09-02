import { serverEnv } from "@/lib/server-config";
import type { ChatMessage } from "@/lib/chat-types";

/**
 * Tools Meilisearch understands. Declaring them makes Meilisearch stream
 * search progress, retrieved sources and conversation-memory messages back.
 */
const MEILI_TOOLS = [
  {
    type: "function",
    function: {
      name: "_meiliSearchProgress",
      description: "Reports real-time search progress to the user",
      parameters: {
        type: "object",
        properties: {
          call_id: { type: "string" },
          function_name: { type: "string" },
          function_parameters: { type: "string" },
        },
        required: ["call_id", "function_name", "function_parameters"],
        additionalProperties: false,
      },
      strict: true,
    },
  },
  {
    type: "function",
    function: {
      name: "_meiliSearchSources",
      description: "Provides sources and references for search results",
      parameters: {
        type: "object",
        properties: {
          call_id: { type: "string" },
          documents: { type: "object" },
        },
        required: ["call_id", "documents"],
        additionalProperties: false,
      },
      strict: true,
    },
  },
  {
    type: "function",
    function: {
      name: "_meiliAppendConversationMessage",
      description: "Append a new message to the conversation based on what happened internally",
      parameters: {
        type: "object",
        properties: {
          role: { type: "string" },
          content: { type: "string" },
          tool_calls: {
            type: ["array", "null"],
            items: {
              type: "object",
              properties: {
                function: {
                  type: "object",
                  properties: { name: { type: "string" }, arguments: { type: "string" } },
                  required: ["name", "arguments"],
                  additionalProperties: false,
                },
                id: { type: "string" },
                type: { type: "string" },
              },
              required: ["function", "id", "type"],
              additionalProperties: false,
            },
          },
          tool_call_id: { type: ["string", "null"] },
        },
        required: ["role", "content", "tool_calls", "tool_call_id"],
        additionalProperties: false,
      },
      strict: true,
    },
  },
];

export async function POST(request: Request) {
  let messages: ChatMessage[];
  try {
    const body = (await request.json()) as { messages?: ChatMessage[] };
    messages = body.messages ?? [];
  } catch {
    return Response.json({ error: "Invalid JSON body" }, { status: 400 });
  }
  if (messages.length === 0) {
    return Response.json({ error: "messages is required" }, { status: 400 });
  }
  if (!serverEnv.chatKey) {
    return Response.json({ error: "MEILI_CHAT_KEY (or MEILI_MASTER_KEY) is not configured" }, { status: 503 });
  }

  const upstream = await fetch(`${serverEnv.meiliUrl}/chats/${serverEnv.chatWorkspace}/chat/completions`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${serverEnv.chatKey}`,
      "Content-Type": "application/json",
      Accept: "text/event-stream",
    },
    body: JSON.stringify({ model: serverEnv.chatModel, stream: true, messages, tools: MEILI_TOOLS }),
    signal: request.signal,
  });

  if (!upstream.ok || !upstream.body) {
    const detail = await upstream.text().catch(() => "");
    // Any upstream failure (feature disabled, workspace missing, bad provider key…) is a
    // configuration problem from the user's point of view: surface it as 503 with details.
    return Response.json(
      { error: `Meilisearch chat returned ${upstream.status}`, detail: detail.slice(0, 2000) },
      { status: 503 },
    );
  }

  return new Response(upstream.body, {
    headers: {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
    },
  });
}
