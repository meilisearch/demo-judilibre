import { toast } from "sonner";
import { create } from "zustand";
import { createJSONStorage, persist, type StateStorage } from "zustand/middleware";
import { consumeChatStream } from "@/lib/chat-stream";
import type { AssistantTurn, ChatMessage, Turn } from "@/lib/chat-types";

const INTERRUPTED = "Réponse interrompue.";

interface ChatState {
  /** What the conversation shows. */
  turns: Turn[];
  /** What `/api/chat` is sent: the visible turns plus the tool messages Meilisearch asks us to keep. */
  history: ChatMessage[];
  busy: boolean;
  setupError: string | null;
  send: (text: string) => Promise<void>;
  stop: () => void;
  reset: () => void;
}

// The in-flight request lives beside the store, not in a component, so an answer
// keeps streaming while the user reads one of its sources.
let controller: AbortController | null = null;

// Leaving the page kills the request anyway; aborting it first makes the saved
// answer read "interrupted" rather than "network error". Not `pagehide`: by then
// the browser has already cancelled the request.
if (typeof window !== "undefined") window.addEventListener("beforeunload", () => controller?.abort());

// Ids must not collide with those of turns restored after a reload, so a module
// counter alone would not do. Not `crypto.randomUUID`: it needs a secure context,
// which a plain-http `*.orb.local` URL is not.
let turnSeq = 0;
const nextId = () => `t${Date.now().toString(36)}-${++turnSeq}`;

/** sessionStorage that never throws: a full quota or a blocked origin only costs persistence. */
const safeSessionStorage: StateStorage = {
  getItem: (name) => {
    try {
      return sessionStorage.getItem(name);
    } catch {
      return null;
    }
  },
  setItem: (name, value) => {
    try {
      sessionStorage.setItem(name, value);
    } catch {
      // Keep the conversation in memory; it just won't survive a reload.
    }
  },
  removeItem: (name) => {
    try {
      sessionStorage.removeItem(name);
    } catch {
      // Nothing to clean up.
    }
  },
};

export const useChatStore = create<ChatState>()(
  persist(
    (set, get) => {
      const updateAssistant = (id: string, patch: (t: AssistantTurn) => AssistantTurn) =>
        set((state) => ({
          turns: state.turns.map((t) => (t.id === id && t.role === "assistant" ? patch(t) : t)),
        }));
      const remember = (message: ChatMessage) => set((state) => ({ history: [...state.history, message] }));

      return {
        turns: [],
        history: [],
        busy: false,
        setupError: null,

        send: async (text) => {
          const content = text.trim();
          if (!content || get().busy) return;

          const assistantId = nextId();
          set((state) => ({
            setupError: null,
            busy: true,
            turns: [
              ...state.turns,
              { id: nextId(), role: "user", content },
              { id: assistantId, role: "assistant", content: "", steps: [], pending: true },
            ],
            history: [...state.history, { role: "user", content }],
          }));

          const own = new AbortController();
          controller = own;
          let answer = "";
          let sawAssistantMemory = false;

          try {
            const res = await fetch("/api/chat", {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ messages: get().history }),
              signal: own.signal,
            });
            if (!res.ok) {
              const body = (await res.json().catch(() => ({}))) as { error?: string; detail?: string };
              const message = body.error ?? `HTTP ${res.status}`;
              if (res.status === 503) set({ setupError: `${message}${body.detail ? ` — ${body.detail}` : ""}` });
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
                if (message.role === "assistant" && message.content && !message.tool_calls?.length) {
                  sawAssistantMemory = true;
                }
                remember(message);
              },
            });
            if (!sawAssistantMemory && answer) remember({ role: "assistant", content: answer });
            updateAssistant(assistantId, (t) => ({ ...t, pending: false }));
          } catch (error) {
            // A reset aborts the request and has already cleared the conversation.
            if (controller !== own) return;
            const aborted = error instanceof DOMException && error.name === "AbortError";
            const message = aborted ? INTERRUPTED : error instanceof Error ? error.message : "Erreur inconnue";
            if (!aborted) toast.error("L'assistant n'a pas pu répondre", { description: message });
            if (answer) remember({ role: "assistant", content: answer });
            updateAssistant(assistantId, (t) => ({ ...t, pending: false, error: message }));
          } finally {
            if (controller === own) {
              controller = null;
              set({ busy: false });
            }
          }
        },

        stop: () => controller?.abort(),

        reset: () => {
          const running = controller;
          controller = null;
          running?.abort();
          set({ turns: [], history: [], busy: false, setupError: null });
        },
      };
    },
    {
      name: "judilibre-chat",
      version: 1,
      storage: createJSONStorage(() => safeSessionStorage),
      // Read after mount (see `ChatPanel`), so the server render and the first
      // client render agree on an empty conversation.
      skipHydration: true,
      // A reload kills the stream, so an answer still pending is saved as interrupted.
      partialize: (state) => ({
        turns: state.turns.map((t) =>
          t.role === "assistant" && t.pending ? { ...t, pending: false, error: t.error ?? INTERRUPTED } : t,
        ),
        history: state.history,
      }),
    },
  ),
);
