// Conversations and unread counts, kept current by `chat-changed` events from Rust.
import {
  events,
  type ChatEvent,
  type ChatMessage,
  type Conversation,
  type Person,
  commands,
} from "./ipc";

let conversations = $state<Conversation[]>([]);
let loaded = $state(false);
let listening: Promise<unknown> | null = null;
/** The account the list was loaded for, so a different sign-in reloads it. */
let loadedFor: string | null = null;
const listeners = new Set<(event: ChatEvent) => void>();

async function reload() {
  const result = await commands.chatConversations();
  if (result.status === "ok") {
    conversations = result.data;
    loaded = true;
  }
}

function apply(event: ChatEvent) {
  const convo = conversations.find((c) => c.id === event.convo_id);
  if (event.kind === "message" && convo) {
    convo.last_message = event.message;
    conversations = [convo, ...conversations.filter((c) => c !== convo)];
  } else {
    void reload();
  }
  for (const fn of listeners) fn(event);
}

export const chat = {
  get conversations() {
    return conversations;
  },
  get loaded() {
    return loaded;
  },
  /** Unread messages across conversations you've accepted. */
  get unread() {
    return conversations.reduce((n, c) => n + (c.accepted && !c.muted ? c.unread : 0), 0);
  },
  /** Loads the conversations of `did` (the signed-in account) and follows changes. */
  async start(did: string): Promise<void> {
    listening ??= events.chatChanged.listen(({ payload }) => {
      for (const e of payload.events) apply(e);
    });
    await listening;
    if (loadedFor === did) return;
    loadedFor = did;
    await reload();
  },
  reload,
  /** Forget everything (signed out). */
  clear() {
    conversations = [];
    loaded = false;
    loadedFor = null;
  },
  /** Runs `fn` for every change; returns a function that stops it. */
  subscribe(fn: (event: ChatEvent) => void) {
    listeners.add(fn);
    return () => listeners.delete(fn);
  },
  markRead(convoId: string) {
    const convo = conversations.find((c) => c.id === convoId);
    if (convo) convo.unread = 0;
    void commands.chatMarkRead(convoId);
  },
  /** Someone in a one-to-one conversation, for its title. */
  other(convo: Conversation): Person | null {
    return convo.members[0] ?? null;
  },
};

/** Display name, or the handle when there is none. */
export function personName(p: Pick<Person, "display_name" | "handle">): string {
  return p.display_name?.trim() || p.handle;
}

/** "Sent 12:04", "Yesterday", "Oct 3": short times for messages and posts. */
export function shortTime(iso: string, now = new Date()): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) return d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) return "Yesterday";
  return d.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: d.getFullYear() === now.getFullYear() ? undefined : "numeric",
  });
}

/** A message's text for a list preview. */
export function preview(m: ChatMessage | null): string {
  if (!m) return "No messages yet";
  if (m.deleted) return "Message deleted";
  if (m.invite) return "🎮 Game invite";
  return m.text;
}
