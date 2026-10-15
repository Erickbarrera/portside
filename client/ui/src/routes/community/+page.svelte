<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { siteBoardLabel, siteBoards } from "$lib/boards";
  import Board from "$lib/components/Board.svelte";
  import type { SiteBoard } from "$lib/ipc";

  // `?board=help` or `?board=features` opens one of Portside's own boards.
  const board = $derived(
    siteBoards.find((b) => b === page.url.searchParams.get("board")) ?? null,
  ) as SiteBoard | null;

  const tabs: { board: SiteBoard | null; label: string }[] = [
    { board: null, label: "All discussions" },
    ...siteBoards.map((b) => ({ board: b, label: siteBoardLabel[b] })),
  ];
</script>

<svelte:head><title>Community · Portside</title></svelte:head>

<h1>Community</h1>
<div class="tabs" role="tablist" aria-label="Boards">
  {#each tabs as t (t.label)}
    <button
      role="tab"
      aria-selected={board === t.board}
      onclick={() => goto(t.board ? `/community?board=${t.board}` : "/community", { replaceState: true })}
      >{t.label}</button
    >
  {/each}
</div>
<p class="muted">
  {#if board === "help"}
    Questions about Portside itself: installing, signing in, something not working.
  {:else if board === "features"}
    Ideas and requests for Portside. Reply to a request to back it.
  {:else}
    Recent discussions about every game and Portside itself. To talk about a game, open it and go
    to its discussions. Threads are Bluesky posts, so they show in the Bluesky app too.
  {/if}
</p>
<Board siteBoard={board} />

<style>
  .tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    padding: 8px 14px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  .tabs button[aria-selected="true"] {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
</style>
