<script lang="ts">
  import { page } from "$app/state";
  import { auth } from "$lib/account.svelte";
  import { chat, personName, shortTime } from "$lib/chat.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import InviteCard from "$lib/components/InviteCard.svelte";
  import SignInNotice from "$lib/components/SignInNotice.svelte";
  import { installs } from "$lib/installs.svelte";
  import { type ChatMessage, type CommandError, type Conversation, commands } from "$lib/ipc";
  import { onDestroy, tick } from "svelte";

  const convoId = $derived(page.params.id ?? "");

  let convo = $state<Conversation | null>(null);
  let messages = $state<ChatMessage[]>([]);
  let cursor = $state<string | null>(null);
  let error = $state<CommandError | null>(null);
  let text = $state("");
  let sending = $state(false);
  let inviting = $state(false);
  let invitePort = $state("");
  let list: HTMLElement | undefined = $state();

  const other = $derived(convo ? chat.other(convo) : null);
  const names = $derived(
    new Map((convo?.members ?? []).map((m) => [m.did, personName(m)] as const)),
  );
  const installedGames = $derived(
    Object.values(installs.installed).sort((a, b) => a.title.localeCompare(b.title)),
  );

  async function scrollDown() {
    await tick();
    list?.scrollTo({ top: list.scrollHeight });
  }

  async function load(id: string) {
    error = null;
    convo = null;
    messages = [];
    const [c, m] = await Promise.all([commands.chatGet(id), commands.chatMessages(id, null)]);
    if (c.status === "ok") convo = c.data;
    else error = c.error;
    if (m.status === "ok") {
      messages = m.data.messages;
      cursor = m.data.cursor;
      chat.markRead(id);
      await scrollDown();
    } else error = m.error;
  }

  async function older() {
    if (!cursor) return;
    const m = await commands.chatMessages(convoId, cursor);
    if (m.status === "ok") {
      const known = new Set(messages.map((x) => x.id));
      messages = [...m.data.messages.filter((x) => !known.has(x.id)), ...messages];
      cursor = m.data.cursor;
    }
  }

  $effect(() => {
    if (!auth.account) return;
    void installs.start();
    void chat.start(auth.account.did);
    void load(convoId);
  });

  $effect(() => {
    void commands.chatWatch(true);
    const stop = chat.subscribe((event) => {
      if (event.convo_id !== convoId) return;
      if (event.kind === "message" && !messages.some((m) => m.id === event.message.id)) {
        messages = [...messages, event.message];
        chat.markRead(convoId);
        void scrollDown();
      } else if (event.kind === "deleted") {
        messages = messages.map((m) =>
          m.id === event.message_id ? { ...m, deleted: true, text: "", invite: null } : m,
        );
      }
    });
    return stop;
  });
  onDestroy(() => void commands.chatWatch(false));

  async function send() {
    const body = text.trim();
    if (!body || sending) return;
    sending = true;
    const r = await commands.chatSend(convoId, body);
    sending = false;
    if (r.status === "ok") {
      text = "";
      if (!messages.some((m) => m.id === r.data.id)) messages = [...messages, r.data];
      if (convo) convo.accepted = true;
      await scrollDown();
    } else error = r.error;
  }

  async function sendInvite() {
    if (!other || !invitePort) return;
    inviting = true;
    const r = await commands.gameInvite(other.did, invitePort);
    inviting = false;
    if (r.status === "error") error = r.error;
    else {
      invitePort = "";
      await load(convoId);
    }
  }

  async function accept() {
    const r = await commands.chatAccept(convoId);
    if (r.status === "ok" && convo) convo.accepted = true;
  }
</script>

<svelte:head><title>{other ? personName(other) : "Chat"} · Portside</title></svelte:head>

<a class="back" href="/friends">← Friends</a>

{#if !auth.account}
  <SignInNotice what="to chat" />
{:else}
  <div class="chat">
    <header>
      {#if other}
        <Avatar src={other.avatar} name={personName(other)} size={40} />
        <div>
          <h1>{convo?.members.map(personName).join(", ")}</h1>
          <p class="muted">@{other.handle}</p>
        </div>
      {:else}
        <h1>Chat</h1>
      {/if}
    </header>

    {#if error}<p class="error" role="alert">{error.message}</p>{/if}

    {#if convo && !convo.accepted}
      <p class="request" role="status">
        This is a message request. Reply or accept it to move it into your messages.
        <button class="btn btn-secondary" onclick={accept}>Accept</button>
      </p>
    {/if}

    <div class="messages" bind:this={list} aria-live="polite">
      {#if cursor}
        <button class="btn btn-secondary older" onclick={older}>Show earlier messages</button>
      {/if}
      {#each messages as m (m.id)}
        {@const mine = m.sender === auth.account.did}
        <div class="msg" class:mine>
          {#if m.deleted}
            <p class="bubble deleted">Message deleted</p>
          {:else}
            <!-- An invite shows as its card; the link text is for other apps. -->
            {#if m.text && !m.invite}<p class="bubble">{m.text}</p>{/if}
            {#if m.invite}
              <InviteCard portId={m.invite.port_id} fromMe={mine} senderName={names.get(m.sender) ?? "Someone"} />
            {/if}
          {/if}
          <span class="time">{shortTime(m.sent_at)}</span>
        </div>
      {:else}
        {#if convo}<p class="muted empty">No messages yet. Say hi, or invite them to a game.</p>{/if}
      {/each}
    </div>

    <form
      class="composer"
      onsubmit={(e) => {
        e.preventDefault();
        void send();
      }}
    >
      <label class="visually-hidden" for="msg">Message</label>
      <textarea
        id="msg"
        rows="2"
        maxlength="1000"
        placeholder="Message (Enter to send, Shift+Enter for a new line)"
        bind:value={text}
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            void send();
          }
        }}
      ></textarea>
      <button class="btn btn-primary" disabled={!text.trim() || sending}>
        {sending ? "Sending…" : "Send"}
      </button>
    </form>

    {#if other && installedGames.length}
      <form
        class="invite"
        onsubmit={(e) => {
          e.preventDefault();
          void sendInvite();
        }}
      >
        <label for="invite-game">Invite to play</label>
        <select id="invite-game" bind:value={invitePort}>
          <option value="">Choose one of your games…</option>
          {#each installedGames as g (g.port_id)}
            <option value={g.port_id}>{g.title}</option>
          {/each}
        </select>
        <button class="btn btn-secondary" disabled={!invitePort || inviting}>
          {inviting ? "Sending…" : "Send invite"}
        </button>
      </form>
    {/if}
    <p class="muted small">
      These are Bluesky direct messages. They also show in the Bluesky app.
    </p>
  </div>
{/if}

<style>
  .back {
    display: inline-block;
    margin-bottom: 12px;
    color: var(--text-2);
    text-decoration: none;
  }
  .chat {
    display: flex;
    flex-direction: column;
    max-width: 760px;
    height: calc(100vh - 120px);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }
  header h1 {
    font-size: 1.2rem;
    margin: 0;
  }
  header p {
    margin: 0;
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 16px 4px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .older {
    align-self: center;
  }
  .msg {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    max-width: 75%;
  }
  .msg.mine {
    align-self: flex-end;
    align-items: flex-end;
  }
  .bubble {
    margin: 0;
    padding: 8px 12px;
    border-radius: 14px;
    background: var(--surface-3);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .mine .bubble {
    background: var(--accent);
    color: var(--on-accent);
  }
  .bubble.deleted {
    font-style: italic;
    color: var(--text-3);
    background: var(--surface-2);
  }
  .time {
    font-size: 0.75rem;
    color: var(--text-4);
    margin-top: 2px;
  }
  .empty {
    margin: auto;
  }
  .composer,
  .invite {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    padding-top: 12px;
  }
  .composer {
    border-top: 1px solid var(--border);
  }
  .invite {
    align-items: center;
  }
  .invite label {
    color: var(--text-2);
    white-space: nowrap;
  }
  textarea,
  select {
    flex: 1;
    padding: 9px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    color: var(--text);
    font: inherit;
    resize: none;
  }
  .request {
    margin-top: 12px;
    padding: 10px 14px;
    border-left: 3px solid var(--accent);
    background: var(--surface-2);
    border-radius: var(--radius-s);
  }
  .small {
    font-size: 0.85rem;
    margin: 8px 0 0;
  }
  .error {
    color: var(--status-boots);
  }
</style>
