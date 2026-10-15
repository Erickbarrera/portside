<script lang="ts">
  import "@fontsource/ibm-plex-sans/400.css";
  import "@fontsource/ibm-plex-sans/500.css";
  import "@fontsource/ibm-plex-sans/600.css";
  import "@fontsource/ibm-plex-mono/400.css";
  import "../app.css";
  import { page } from "$app/state";
  import { auth } from "$lib/account.svelte";
  import { chat, personName } from "$lib/chat.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();

  // Downloads, Cloud saves and Collections join as their milestones land
  // (docs/ui.md, docs/roadmap.md).
  const nav = [
    { href: "/library", label: "Library" },
    { href: "/discover", label: "Discover" },
    { href: "/community", label: "Community" },
    { href: "/friends", label: "Friends" },
  ];

  $effect(() => {
    void auth.start();
  });
  // Unread counts for the sidebar once signed in; forget them on sign-out.
  $effect(() => {
    if (auth.account?.can_chat) void chat.start(auth.account.did);
    else if (auth.loaded && !auth.account) chat.clear();
  });
  const current = (href: string) =>
    page.url.pathname === href || page.url.pathname.startsWith(`${href}/`);
</script>

<a class="skip" href="#content">Skip to content</a>
<div class="shell">
  <nav class="sidebar" aria-label="Main">
    <a class="brand" href="/library">
      <img src="/favicon.svg" alt="" width="32" height="32" />
      <span>Portside</span>
    </a>
    <ul>
      {#each nav as item (item.href)}
        <li>
          <a href={item.href} aria-current={current(item.href) ? "page" : undefined}
            >{item.label}
            {#if item.href === "/friends" && chat.unread > 0}
              <span class="badge" aria-label={`${chat.unread} unread messages`}>{chat.unread}</span>
            {/if}</a
          >
        </li>
      {/each}
    </ul>
    <div class="bottom">
      {#if auth.account}
        <a class="me" href="/settings#account" title={`Signed in as @${auth.account.handle}`}>
          <Avatar src={auth.account.avatar} name={personName(auth.account)} size={28} />
          <span>{personName(auth.account)}</span>
        </a>
      {:else if auth.loaded}
        <a href="/signin" aria-current={current("/signin") ? "page" : undefined}>Sign in</a>
      {/if}
      <a href="/settings" aria-current={current("/settings") ? "page" : undefined}>Settings</a>
    </div>
  </nav>
  <main id="content">
    {@render children()}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100vh;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 12px;
    background: var(--surface-1);
    border-right: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 8px;
    color: var(--text);
    font-weight: 600;
    font-size: 1.1rem;
    text-decoration: none;
  }
  .brand img {
    border-radius: 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sidebar a:not(.brand) {
    display: block;
    padding: 8px 12px;
    border-radius: var(--radius-s);
    color: var(--text-2);
    text-decoration: none;
  }
  .sidebar a:not(.brand):hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .sidebar a[aria-current="page"] {
    background: var(--surface-3);
    color: var(--text);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .bottom {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sidebar a.me {
    display: flex;
    align-items: center;
    gap: 10px;
    overflow: hidden;
  }
  .me span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    float: right;
    min-width: 20px;
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--on-accent);
    font-size: 0.8rem;
    font-weight: 600;
    text-align: center;
  }
  main {
    overflow-y: auto;
    padding: 32px 40px;
  }
  .skip {
    position: absolute;
    left: -9999px;
  }
  .skip:focus {
    left: 16px;
    top: 16px;
    z-index: 10;
    background: var(--surface-3);
    padding: 8px 12px;
  }
</style>
