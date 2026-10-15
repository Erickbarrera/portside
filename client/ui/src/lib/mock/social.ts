// Browser-only stand-in for the Bluesky commands: a synthetic account, friends,
// conversations and board threads ("Example …" people, never real accounts).
import { emit } from "@tauri-apps/api/event";
import type {
  Account,
  BoardPage,
  BoardPost,
  BoardQuery,
  ChatChanged,
  ChatMessage,
  CommandError,
  Conversation,
  Person,
  SiteBoard,
  StrongRef,
  SubForum,
  ThreadSummary,
  ThreadView,
} from "../ipc";
import { ports } from "./fixtures";

const params = new URLSearchParams(location.search);
const site = "https://example.github.io/portside-catalog/catalog/";

const me: Person = {
  did: "did:plc:mock-me",
  handle: "you.example.test",
  display_name: "Example You",
  avatar: null,
  following: false,
  follows_you: false,
};

function person(n: string, following: boolean, followsYou: boolean): Person {
  return {
    did: `did:plc:mock-${n}`,
    handle: `${n}.example.test`,
    display_name: `Example ${n[0].toUpperCase()}${n.slice(1)}`,
    avatar: null,
    following,
    follows_you: followsYou,
  };
}

const people = [
  person("ada", true, true),
  person("bo", true, true),
  person("cy", true, false),
  person("dee", false, true),
  person("eli", false, false),
];

let account: Account | null = params.has("mock-signed-out")
  ? null
  : {
      did: me.did,
      handle: me.handle,
      display_name: me.display_name,
      avatar: null,
      can_chat: true,
    };

const now = () => new Date().toISOString();
const ago = (mins: number) => new Date(Date.now() - mins * 60_000).toISOString();
let seq = 100;

function message(sender: string, text: string, sentAt: string, portId?: string): ChatMessage {
  seq += 1;
  return {
    id: `m${seq}`,
    rev: String(seq),
    sender,
    text,
    sent_at: sentAt,
    deleted: false,
    invite: portId ? { port_id: portId } : null,
  };
}

const firstPort = ports[0]?.id ?? "example-port";
const convos = new Map<string, { convo: Conversation; messages: ChatMessage[] }>();
function addConvo(friend: Person, messages: ChatMessage[], unread: number) {
  const id = `convo-${friend.handle.split(".")[0]}`;
  convos.set(id, {
    convo: {
      id,
      rev: String(seq),
      members: [friend],
      last_message: messages.at(-1) ?? null,
      unread,
      muted: false,
      accepted: true,
    },
    messages,
  });
  return id;
}
addConvo(
  people[0],
  [
    message(people[0].did, "Did you get the new build working?", ago(60 * 26)),
    message(me.did, "Yes! Runs great on the Deck.", ago(60 * 25)),
    message(
      people[0].did,
      `Want to play? Get it on Portside: ${site}?q=Example&port=${firstPort}`,
      ago(5),
      firstPort,
    ),
  ],
  1,
);
addConvo(people[1], [message(people[1].did, "gg", ago(60 * 24 * 3))], 0);

function emitChat(payload: ChatChanged) {
  void emit("chat-changed", payload);
}

function signedIn(): Account {
  if (!account) {
    throw {
      code: "E_SIGNED_OUT",
      message: "Sign in with Bluesky to use this.",
    } satisfies CommandError;
  }
  return account;
}

function convoFor(did: string): Conversation {
  for (const c of convos.values()) {
    if (c.convo.members[0]?.did === did) return c.convo;
  }
  const friend = people.find((p) => p.did === did);
  if (!friend?.follows_you) {
    throw {
      code: "E_CHAT_NOT_ALLOWED",
      message: "They only accept messages from people they follow.",
    } satisfies CommandError;
  }
  return convos.get(addConvo(friend, [], 0))?.convo as Conversation;
}

function send(convoId: string, text: string, portId?: string): ChatMessage {
  const c = convos.get(convoId);
  if (!c) throw { code: "E_NOT_FOUND", message: "Not found." } satisfies CommandError;
  const m = message(me.did, text, now(), portId);
  c.messages.push(m);
  c.convo.last_message = m;
  c.convo.accepted = true;
  // A friendly reply a moment later, as the poller would deliver it.
  const other = c.convo.members[0];
  if (other) {
    setTimeout(() => {
      const reply = message(other.did, portId ? "On my way!" : "👍", now());
      c.messages.push(reply);
      c.convo.last_message = reply;
      emitChat({ events: [{ kind: "message", convo_id: convoId, message: reply }] });
    }, 1500);
  }
  return m;
}

// Boards
type MockThread = { view: ThreadView; summary: ThreadSummary };
const threads: MockThread[] = [];

function post(
  uri: string,
  author: Person,
  text: string,
  minsAgo: number,
  replyTo: Person | null,
): BoardPost {
  return {
    post: { uri, cid: `cid-${uri}` },
    author,
    text,
    created_at: ago(minsAgo),
    like_count: Math.floor(minsAgo) % 7,
    reply_count: 0,
    reply_to: replyTo,
    images: [],
  };
}

function addThread(
  portId: string | null,
  sub: SubForum | null,
  author: Person,
  title: string,
  body: string,
  minsAgo: number,
  siteBoard: SiteBoard | null = null,
) {
  const uri = `at://${author.did}/app.bsky.feed.post/t${threads.length + 1}`;
  const replies =
    minsAgo > 30
      ? [
          post(
            `${uri}-r1`,
            people[1],
            "Same here. The latest build fixed it for me.",
            minsAgo / 2,
            null,
          ),
          post(`${uri}-r2`, author, "Thanks, that worked!", minsAgo / 3, people[1]),
        ]
      : [];
  const view: ThreadView = {
    title,
    first: post(uri, author, body, minsAgo, null),
    replies,
    tags: siteBoard
      ? ["portsideapp", `portsideapp-site-${siteBoard}`]
      : ["portsideapp", `portsideapp-${portId}`, `psapp-${sub === "multiplayer" ? "mp" : sub}`],
    port_id: portId,
    sub_forum: sub,
    site_board: siteBoard,
    web_url: `https://bsky.app/profile/${author.handle}/post/t${threads.length + 1}`,
  };
  threads.push({
    view,
    summary: {
      root: view.first.post,
      title,
      author,
      created_at: view.first.created_at,
      last_activity: replies.at(-1)?.created_at ?? view.first.created_at,
      reply_count: replies.length,
      like_count: 3,
      port_id: portId,
      sub_forum: sub,
      site_board: siteBoard,
    },
  });
  return view.first.post;
}

ports.slice(0, 3).forEach((p, i) => {
  addThread(
    p.id,
    "help",
    people[i % 4],
    "Controller not detected on first launch",
    "It works after a restart. Anyone else?",
    90 + i * 300,
  );
  addThread(
    p.id,
    "general",
    people[(i + 1) % 4],
    "Best settings for a handheld",
    "Sharing what worked for me: 60 fps cap, scaling set to integer.",
    400 + i * 200,
  );
  addThread(
    p.id,
    "mods",
    people[(i + 2) % 4],
    "Texture packs that work with the latest build",
    "Collecting a list here.",
    2000 + i * 50,
  );
});

addThread(
  null,
  null,
  people[2],
  "Sign-in opens the browser but nothing happens",
  "I approve on Bluesky and the app keeps waiting.",
  700,
  "help",
);
addThread(
  null,
  null,
  people[0],
  "Show friends' play time on their profile",
  "Would love to see what my friends are playing.",
  1500,
  "features",
);

function board(q: BoardQuery): BoardPage {
  const words = q.q?.toLowerCase().trim();
  const list = threads
    .filter(
      (t) =>
        (!q.port_id || t.summary.port_id === q.port_id) &&
        (!q.sub_forum || t.summary.sub_forum === q.sub_forum) &&
        (q.port_id || !q.site_board || t.summary.site_board === q.site_board) &&
        (!words || `${t.view.title} ${t.view.first.text}`.toLowerCase().includes(words)),
    )
    .map((t) => t.summary)
    .sort((a, b) =>
      q.sort === "top"
        ? b.reply_count - a.reply_count
        : b.last_activity.localeCompare(a.last_activity),
    );
  return { threads: list, cursor: null };
}

function reply(root: StrongRef, parent: StrongRef, text: string): StrongRef {
  const t = threads.find((x) => x.view.first.post.uri === root.uri);
  if (!t) throw { code: "E_NOT_FOUND", message: "Not found." } satisfies CommandError;
  const replyTo =
    parent.uri === root.uri
      ? null
      : (t.view.replies.find((r) => r.post.uri === parent.uri)?.author ?? null);
  const p = post(`${root.uri}-r${t.view.replies.length + 1}`, me, text.trim(), 0, replyTo);
  t.view.replies.push(p);
  t.summary.reply_count += 1;
  t.summary.last_activity = p.created_at;
  return p.post;
}

const invalid = (message: string): CommandError => ({ code: "E_INVALID", message });

/** Answers a social command, or returns `undefined` when `cmd` isn't one. */
export function socialCommand(cmd: string, a: Record<string, unknown>): unknown {
  switch (cmd) {
    case "auth_status":
      return account;
    case "auth_sign_in":
      return new Promise((resolve) =>
        setTimeout(() => {
          account = {
            did: me.did,
            handle: String(a.account || me.handle),
            display_name: me.display_name,
            avatar: null,
            can_chat: true,
          };
          void emit("auth-changed", { account });
          resolve(account);
        }, 1200),
      );
    case "auth_cancel":
      return null;
    case "auth_sign_out":
      account = null;
      void emit("auth-changed", { account: null });
      return null;
    case "chat_watch":
      return null;
  }
  if (
    !cmd.startsWith("friend") &&
    !cmd.startsWith("people") &&
    !cmd.startsWith("chat") &&
    !cmd.startsWith("board") &&
    cmd !== "game_invite"
  ) {
    return undefined;
  }
  signedIn();
  switch (cmd) {
    case "friends_list":
      return people.filter((p) => p.following && p.follows_you);
    case "friends_follows_you":
      return people.filter((p) => !p.following && p.follows_you);
    case "people_search": {
      const q = String(a.q).toLowerCase();
      return people.filter((p) => `${p.display_name} ${p.handle}`.toLowerCase().includes(q));
    }
    case "friend_add":
    case "friend_remove": {
      const p = people.find((x) => x.did === a.did);
      if (!p) throw { code: "E_NOT_FOUND", message: "Not found." } satisfies CommandError;
      p.following = cmd === "friend_add";
      return { ...p };
    }
    case "chat_conversations":
      return [...convos.values()].map((c) => c.convo);
    case "chat_open":
      return convoFor(String(a.did));
    case "chat_get": {
      const c = convos.get(String(a.convoId));
      if (!c) throw { code: "E_NOT_FOUND", message: "Not found." } satisfies CommandError;
      return c.convo;
    }
    case "chat_messages":
      return { messages: [...(convos.get(String(a.convoId))?.messages ?? [])], cursor: null };
    case "chat_send": {
      const text = String(a.text).trim();
      if (!text) throw invalid("The message is empty.");
      return send(String(a.convoId), text);
    }
    case "chat_mark_read": {
      const c = convos.get(String(a.convoId));
      if (c) c.convo.unread = 0;
      return null;
    }
    case "chat_accept":
      return null;
    case "game_invite": {
      const convo = convoFor(String(a.did));
      const port = ports.find((p) => p.id === a.portId);
      if (!port)
        throw {
          code: "E_NOT_FOUND",
          message: "That game isn't in the catalog.",
        } satisfies CommandError;
      send(
        convo.id,
        `🎮 Want to play ${port.title}? Get it on Portside: ${site}?q=${encodeURIComponent(port.title)}&port=${port.id}`,
        port.id,
      );
      return convo;
    }
    case "board_threads":
      return board(a.query as BoardQuery);
    case "board_thread": {
      const t = threads.find((x) => x.view.first.post.uri === a.uri);
      if (!t) throw invalid("This thread was deleted or isn't visible to you.");
      return structuredClone(t.view);
    }
    case "board_post_site": {
      const title = String(a.title).trim();
      if (!title) throw invalid("Give the thread a title of up to 120 characters on one line.");
      return addThread(null, null, me, title, String(a.body).trim(), 0, a.board as SiteBoard);
    }
    case "board_post": {
      const title = String(a.title).trim();
      if (!title) throw invalid("Give the thread a title of up to 120 characters on one line.");
      return addThread(
        String(a.portId),
        a.subForum as SubForum,
        me,
        title,
        String(a.body).trim(),
        0,
      );
    }
    case "board_reply": {
      const text = String(a.text).trim();
      if (!text) throw invalid("The reply is empty.");
      return reply(a.root as StrongRef, a.parent as StrongRef, text);
    }
  }
  return undefined;
}
