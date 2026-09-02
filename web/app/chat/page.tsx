import type { Metadata } from "next";
import { ChatPanel } from "@/app/chat/chat-panel";

export const metadata: Metadata = { title: "Assistant" };

export default function ChatPage() {
  return <ChatPanel />;
}
