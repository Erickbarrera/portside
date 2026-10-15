<script lang="ts">
  import { page } from "$app/state";
  import { auth } from "$lib/account.svelte";
  import { siteBoardLabel, subForumLabel } from "$lib/boards";
  import { personName, shortTime } from "$lib/chat.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import SignInNotice from "$lib/components/SignInNotice.svelte";
  import { openExternal } from "$lib/external";
  import { type BoardPost, type CommandError, type ThreadView, commands } from "$lib/ipc";

  const uri = $derived(page.params.uri ?? "");

  let thread = $state<ThreadView | null>(null);
  let error = $state<CommandError | null>(null);
  let text = $state("");
  let replyTo = $state<BoardPost | null>(null);
  let sending = $state(false);
  let sendError = $state<CommandError | null>(null);

  async function load() {
    error = null;
    const r = await commands.boardThread(uri);
    if (r.status === "ok") thread = r.data;
    else error = r.error;
  }

  $effect(() => {
    if (auth.account) void load();
  });

  async function reply() {
    if (!thread || !text.trim()) return;
    sending = true;
    sendError = null;
    const parent = replyTo?.post ?? thread.first.post;
    const r = await commands.boardReply(thread.first.post, parent, thread.tags, text);
    sending = false;
    if (r.status === "ok") {
      text = "";
      replyTo = null;
      await load();
    } else sendError = r.error;
  }
</script>

<svelte:head><title>{thread?.title ?? "Thread"} · Portside</title></svelte:head>

{#if thread?.port_id}
  <a class="back" href={`/port/${encodeURIComponent(thread.port_id)}/board`}>← Back to the board</a>
{:else if thread?.site_board}
  <a class="back" href={`/community?board=${thread.site_board}`}>← {siteBoardLabel[thread.site_board]}</a>
{:else}
  <a class="back" href="/community">← Community</a>
{/if}

{#if !auth.account}
  <SignInNotice what="to read the discussions" />
{:else if error}
  <p class="error" role="alert">{error.message}</p>
{:else if !thread}
  <p class="muted">Loading…</p>
{:else}
  <article>
    <header>
      <h1>{thread.title}</h1>
      <p class="muted">
        {#if thread.sub_forum}{subForumLabel[thread.sub_forum]} ·{/if}
        {#if thread.site_board}{siteBoardLabel[thread.site_board]} ·{/if}
        <a href={thread.web_url} onclick={(e) => openExternal(e, thread?.web_url ?? "")}>Open in Bluesky</a>
      </p>
    </header>
    {#each [thread.first, ...thread.replies] as p, i (p.post.uri)}
      <section class="post" class:first={i === 0}>
        <div class="author">
          <Avatar src={p.author.avatar} name={personName(p.author)} size={40} />
          <div>
            <strong>{personName(p.author)}</strong>
            <span class="muted small">@{p.author.handle} · {shortTime(p.created_at)}</span>
          </div>
        </div>
        {#if p.reply_to}
          <p class="muted small">Replying to {personName(p.reply_to)}</p>
        {/if}
        {#if p.text}<p class="text">{p.text}</p>{/if}
        {#if p.images.length}
          <div class="images">
            {#each p.images as src (src)}<img {src} alt="" loading="lazy" />{/each}
          </div>
        {/if}
        <div class="actions muted small">
          <span>♥ {p.like_count}</span>
          <button class="link" onclick={() => (replyTo = p)}>Reply</button>
        </div>
      </section>
    {/each}

    <form
      class="composer"
      onsubmit={(e) => {
        e.preventDefault();
        void reply();
      }}
    >
      <label for="reply">
        {replyTo && replyTo !== thread.first ? `Reply to ${personName(replyTo.author)}` : "Reply to the thread"}
      </label>
      <textarea id="reply" rows="4" maxlength="2700" bind:value={text}></textarea>
      <p class="muted small">Posted publicly to your Bluesky account as a reply.</p>
      {#if sendError}<p class="error" role="alert">{sendError.message}</p>{/if}
      <div class="row">
        <button class="btn btn-primary" disabled={!text.trim() || sending}>
          {sending ? "Posting…" : "Post reply"}
        </button>
        {#if replyTo}
          <button type="button" class="btn btn-secondary" onclick={() => (replyTo = null)}>Reply to the thread instead</button>
        {/if}
      </div>
    </form>
  </article>
{/if}

<style>
  .back {
    display: inline-block;
    margin-bottom: 12px;
    color: var(--text-2);
    text-decoration: none;
  }
  article {
    max-width: 780px;
  }
  .post {
    padding: 16px 20px;
    margin-bottom: 8px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
  }
  .post.first {
    border-color: var(--border-strong);
  }
  .author {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .author div {
    display: flex;
    flex-direction: column;
  }
  .text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .images {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .images img {
    max-height: 180px;
    border-radius: var(--radius-s);
  }
  .actions {
    display: flex;
    gap: 16px;
    align-items: center;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
  }
  .composer {
    margin-top: 16px;
  }
  .composer label {
    display: block;
    margin-bottom: 6px;
    font-weight: 500;
  }
  textarea {
    width: 100%;
    padding: 9px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    color: var(--text);
    font: inherit;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .small {
    font-size: 0.85rem;
  }
  .error {
    color: var(--status-boots);
  }
</style>
