<script lang="ts">
  // A discussion board: all of Portside, one of Portside's own boards (help, feature
  // requests), or one game's board with its sub-forums.
  import { goto } from "$app/navigation";
  import { auth } from "$lib/account.svelte";
  import { siteBoardLabel, subForumLabel, subForums, threadHref } from "$lib/boards";
  import { personName, shortTime } from "$lib/chat.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import SignInNotice from "$lib/components/SignInNotice.svelte";
  import {
    type BoardSort,
    type CommandError,
    type SiteBoard,
    type SubForum,
    type ThreadSummary,
    commands,
  } from "$lib/ipc";

  let {
    portId = null,
    siteBoard = null,
  }: { portId?: string | null; siteBoard?: SiteBoard | null } = $props();

  const canPost = $derived(!!portId || !!siteBoard);

  let sub = $state<SubForum | null>(null);
  let sort = $state<BoardSort>("latest");
  let q = $state("");
  let threads = $state<ThreadSummary[] | null>(null);
  let cursor = $state<string | null>(null);
  let error = $state<CommandError | null>(null);
  let composing = $state(false);
  let title = $state("");
  let body = $state("");
  let newSub = $state<SubForum>("general");
  let posting = $state(false);
  let postError = $state<CommandError | null>(null);

  async function load(more = false) {
    error = null;
    if (!more) threads = null;
    const r = await commands.boardThreads({
      port_id: portId,
      sub_forum: portId ? sub : null,
      site_board: portId ? null : siteBoard,
      q: q.trim() || null,
      sort,
      cursor: more ? cursor : null,
    });
    if (r.status === "ok") {
      threads = more ? [...(threads ?? []), ...r.data.threads] : r.data.threads;
      cursor = r.data.cursor;
    } else {
      error = r.error;
      threads ??= [];
    }
  }

  $effect(() => {
    // Reload when the board, sub-forum or sort changes.
    void [portId, siteBoard, sub, sort];
    if (auth.account) void load();
  });

  async function post() {
    if (!canPost) return;
    posting = true;
    postError = null;
    const r = portId
      ? await commands.boardPost(portId, newSub, title, body)
      : await commands.boardPostSite(siteBoard ?? "help", title, body);
    posting = false;
    if (r.status === "ok") {
      title = "";
      body = "";
      composing = false;
      await goto(threadHref(r.data.uri));
    } else postError = r.error;
  }
</script>

{#if !auth.loaded}
  <p class="muted">Loading…</p>
{:else if !auth.account}
  <SignInNotice what="to read and join the discussions" />
{:else}
  <div class="toolbar">
    {#if portId}
      <div class="tabs" role="tablist" aria-label="Sub-forums">
        <button role="tab" aria-selected={sub === null} onclick={() => (sub = null)}>All</button>
        {#each subForums as s (s)}
          <button role="tab" aria-selected={sub === s} onclick={() => (sub = s)}>{subForumLabel[s]}</button>
        {/each}
      </div>
    {/if}
    <form
      class="search"
      onsubmit={(e) => {
        e.preventDefault();
        void load();
      }}
    >
      <label class="visually-hidden" for="board-q">Search this board</label>
      <input id="board-q" type="search" placeholder="Search this board" bind:value={q} />
      <label class="visually-hidden" for="board-sort">Sort</label>
      <select id="board-sort" bind:value={sort}>
        <option value="latest">Latest activity</option>
        <option value="top">Most popular</option>
      </select>
    </form>
    {#if canPost}
      <button class="btn btn-primary" onclick={() => (composing = !composing)}>
        {composing ? "Cancel" : "New thread"}
      </button>
    {/if}
  </div>

  {#if composing && canPost}
    <form
      class="composer"
      onsubmit={(e) => {
        e.preventDefault();
        void post();
      }}
    >
      <div class="row">
        <label for="t-title">Title</label>
        <input id="t-title" maxlength="120" bind:value={title} required />
      </div>
      {#if portId}
        <div class="row">
          <label for="t-sub">Sub-forum</label>
          <select id="t-sub" bind:value={newSub}>
            {#each subForums as s (s)}<option value={s}>{subForumLabel[s]}</option>{/each}
          </select>
        </div>
      {/if}
      <div class="row">
        <label for="t-body">Post</label>
        <textarea id="t-body" rows="6" maxlength="2700" bind:value={body}></textarea>
      </div>
      <p class="muted small">
        Posted publicly to your Bluesky account. Long posts become a numbered chain of posts.
      </p>
      {#if postError}<p class="error" role="alert">{postError.message}</p>{/if}
      <button class="btn btn-primary" disabled={!title.trim() || posting}>
        {posting ? "Posting…" : "Post thread"}
      </button>
    </form>
  {/if}

  {#if error}
    <p class="error" role="alert">{error.message}</p>
  {/if}
  {#if threads === null}
    <p class="muted">Loading threads…</p>
  {:else if threads.length === 0 && !error}
    <p class="muted">No threads yet.{canPost ? " Start the first one." : ""}</p>
  {:else}
    <ul class="threads">
      {#each threads as t (t.root.uri)}
        <li>
          <a href={threadHref(t.root.uri)}>
            <Avatar src={t.author.avatar} name={personName(t.author)} size={32} />
            <span class="main">
              <span class="title">{t.title}</span>
              <span class="muted small">
                {personName(t.author)}
                {#if t.sub_forum}· {subForumLabel[t.sub_forum]}{/if}
                {#if !siteBoard && t.site_board}· {siteBoardLabel[t.site_board]}{/if}
                {#if !portId && t.port_id}· {t.port_id}{/if}
              </span>
            </span>
            <span class="stats muted small">
              <span>{t.reply_count} {t.reply_count === 1 ? "reply" : "replies"}</span>
              <span>{shortTime(t.last_activity || t.created_at)}</span>
            </span>
          </a>
        </li>
      {/each}
    </ul>
    {#if cursor}
      <button class="btn btn-secondary" onclick={() => load(true)}>Load more</button>
    {/if}
  {/if}
{/if}

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }
  .tabs {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .tabs button {
    padding: 6px 12px;
    border-radius: var(--radius-pill);
    border: 1px solid var(--border-strong);
    background: none;
    color: var(--text-2);
    cursor: pointer;
  }
  .tabs button[aria-selected="true"] {
    background: var(--surface-3);
    color: var(--text);
    border-color: var(--accent);
  }
  .search {
    display: flex;
    gap: 8px;
    flex: 1;
    min-width: 260px;
  }
  input,
  select,
  textarea {
    padding: 8px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    color: var(--text);
    font: inherit;
  }
  .search input {
    flex: 1;
  }
  .composer {
    max-width: 720px;
    padding: 20px;
    margin-bottom: 16px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
  }
  .row {
    display: grid;
    grid-template-columns: 100px 1fr;
    gap: 12px;
    margin-bottom: 12px;
    align-items: start;
  }
  .row label {
    padding-top: 8px;
    color: var(--text-2);
  }
  .threads {
    list-style: none;
    margin: 0 0 16px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    overflow: hidden;
  }
  .threads li + li {
    border-top: 1px solid var(--border);
  }
  .threads a {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--surface-1);
    color: var(--text);
    text-decoration: none;
  }
  .threads a:hover {
    background: var(--surface-2);
  }
  .main {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stats {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .small {
    font-size: 0.85rem;
  }
  .error {
    color: var(--status-boots);
  }
</style>
