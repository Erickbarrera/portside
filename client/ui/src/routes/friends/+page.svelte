<script lang="ts">
  import { goto } from "$app/navigation";
  import { auth } from "$lib/account.svelte";
  import { chat, personName, preview, shortTime } from "$lib/chat.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import SignInNotice from "$lib/components/SignInNotice.svelte";
  import { type CommandError, type Person, commands } from "$lib/ipc";

  let friends = $state<Person[] | null>(null);
  let followers = $state<Person[]>([]);
  let error = $state<CommandError | null>(null);
  let q = $state("");
  let results = $state<Person[]>([]);
  let busy = $state<string | null>(null);
  let notice = $state<string | null>(null);

  async function load() {
    error = null;
    const [f, fy] = await Promise.all([commands.friendsList(), commands.friendsFollowsYou()]);
    if (f.status === "ok") friends = f.data;
    else error = f.error;
    if (fy.status === "ok") followers = fy.data;
  }

  $effect(() => {
    if (auth.account) {
      void load();
      if (auth.account.can_chat) void chat.start(auth.account.did);
    }
  });

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function search() {
    clearTimeout(searchTimer);
    const term = q.trim();
    if (!term) {
      results = [];
      return;
    }
    searchTimer = setTimeout(async () => {
      const r = await commands.peopleSearch(term);
      if (r.status === "ok" && q.trim() === term) results = r.data;
    }, 250);
  }

  async function add(p: Person) {
    busy = p.did;
    const r = await commands.friendAdd(p.did);
    busy = null;
    if (r.status === "error") {
      error = r.error;
      return;
    }
    notice = r.data.follows_you
      ? `${personName(p)} is now your friend.`
      : `You follow ${personName(p)}. They'll show up as a friend once they follow you back.`;
    results = results.map((x) => (x.did === p.did ? r.data : x));
    await load();
  }

  async function remove(p: Person) {
    busy = p.did;
    const r = await commands.friendRemove(p.did);
    busy = null;
    if (r.status === "error") error = r.error;
    else {
      notice = `You no longer follow ${personName(p)} on Bluesky.`;
      await load();
    }
  }

  async function message(p: Person) {
    busy = p.did;
    const r = await commands.chatOpen(p.did);
    busy = null;
    if (r.status === "ok") await goto(`/chat/${encodeURIComponent(r.data.id)}`);
    else error = r.error;
  }
</script>

<svelte:head><title>Friends · Portside</title></svelte:head>

<h1>Friends</h1>

{#if !auth.loaded}
  <p class="muted">Loading…</p>
{:else if !auth.account}
  <SignInNotice what="to see your friends and messages" />
{:else}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
  {#if error}<p class="error" role="alert">{error.message}</p>{/if}

  <div class="columns">
    <section aria-labelledby="messages-h">
      <h2 id="messages-h">Messages</h2>
      {#if !auth.account.can_chat}
        <p class="muted">
          Sign out and in again to let Portside use your Bluesky messages.
        </p>
      {:else if !chat.loaded}
        <p class="muted">Loading…</p>
      {:else if chat.conversations.length === 0}
        <p class="muted">No conversations yet. Message a friend to start one.</p>
      {:else}
        <ul class="list">
          {#each chat.conversations as convo (convo.id)}
            {@const other = chat.other(convo)}
            <li>
              <a class="convo" href={`/chat/${encodeURIComponent(convo.id)}`}>
                <Avatar src={other?.avatar} name={other ? personName(other) : "?"} />
                <span class="text">
                  <span class="name">
                    {convo.members.map(personName).join(", ") || "Just you"}
                    {#if !convo.accepted}<span class="tag">Request</span>{/if}
                  </span>
                  <span class="muted line">{preview(convo.last_message)}</span>
                </span>
                <span class="side">
                  {#if convo.last_message}<span class="muted small">{shortTime(convo.last_message.sent_at)}</span>{/if}
                  {#if convo.unread > 0}<span class="badge" aria-label={`${convo.unread} unread`}>{convo.unread}</span>{/if}
                </span>
              </a>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <div>
      <section aria-labelledby="find-h">
        <h2 id="find-h">Add a friend</h2>
        <label class="visually-hidden" for="find">Search Bluesky by name or handle</label>
        <input id="find" type="search" placeholder="Search Bluesky by name or handle" bind:value={q} oninput={search} />
        {#if results.length}
          <ul class="list">
            {#each results as p (p.did)}
              <li class="person">
                <Avatar src={p.avatar} name={personName(p)} />
                <span class="text"><span class="name">{personName(p)}</span><span class="muted line">@{p.handle}</span></span>
                {#if p.following && p.follows_you}
                  <span class="muted small">Friend</span>
                {:else if p.following}
                  <span class="muted small">Waiting for them to follow back</span>
                {:else}
                  <button class="btn btn-secondary" disabled={busy === p.did} onclick={() => add(p)}>
                    {p.follows_you ? "Follow back" : "Add friend"}
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        <p class="muted small">
          Adding a friend follows them on Bluesky. You're friends once you follow each other.
        </p>
      </section>

      <section aria-labelledby="friends-h">
        <h2 id="friends-h">Your friends {#if friends}<span class="muted">({friends.length})</span>{/if}</h2>
        {#if friends === null}
          <p class="muted">Loading…</p>
        {:else if friends.length === 0}
          <p class="muted">No friends yet. Search above, or follow back someone below.</p>
        {:else}
          <ul class="list">
            {#each friends as p (p.did)}
              <li class="person">
                <Avatar src={p.avatar} name={personName(p)} />
                <span class="text"><span class="name">{personName(p)}</span><span class="muted line">@{p.handle}</span></span>
                {#if auth.account.can_chat}
                  <button class="btn btn-secondary" disabled={busy === p.did} onclick={() => message(p)}>Message</button>
                {/if}
                <button class="btn link" disabled={busy === p.did} onclick={() => remove(p)} aria-label={`Unfollow ${personName(p)}`}>Unfollow</button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      {#if followers.length}
        <section aria-labelledby="fy-h">
          <h2 id="fy-h">Follows you</h2>
          <ul class="list">
            {#each followers as p (p.did)}
              <li class="person">
                <Avatar src={p.avatar} name={personName(p)} />
                <span class="text"><span class="name">{personName(p)}</span><span class="muted line">@{p.handle}</span></span>
                <button class="btn btn-secondary" disabled={busy === p.did} onclick={() => add(p)}>Follow back</button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style>
  .columns {
    display: grid;
    grid-template-columns: minmax(280px, 1fr) minmax(320px, 1.2fr);
    gap: 24px;
    align-items: start;
  }
  section {
    padding: 20px;
    margin-bottom: 16px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
  }
  h2 {
    font-size: 1.05rem;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .person,
  .convo {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
  }
  .convo {
    padding: 8px;
    margin: 0 -8px;
    border-radius: var(--radius-s);
    color: var(--text);
    text-decoration: none;
  }
  .convo:hover {
    background: var(--surface-3);
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 500;
  }
  .line {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.9rem;
  }
  .side {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .small {
    font-size: 0.85rem;
  }
  .badge {
    min-width: 20px;
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--on-accent);
    font-size: 0.8rem;
    font-weight: 600;
    text-align: center;
  }
  .tag {
    margin-left: 6px;
    padding: 1px 8px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    color: var(--text-2);
    font-size: 0.75rem;
  }
  input {
    width: 100%;
    padding: 9px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
  }
  .link {
    padding: 8px;
    color: var(--text-3);
  }
  .link:hover {
    color: var(--text);
  }
  .notice {
    padding: 10px 14px;
    border-left: 3px solid var(--accent);
    background: var(--surface-2);
    border-radius: var(--radius-s);
  }
  .error {
    color: var(--status-boots);
  }
</style>
