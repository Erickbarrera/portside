<script lang="ts">
  import { statusFor } from "$lib/actions";
  import type { Os, PortSummary } from "$lib/ipc";
  import { formatNumber, osLabel, typeLabel } from "$lib/labels";
  import Cover from "./Cover.svelte";
  import StatusBadge from "./StatusBadge.svelte";

  let { port, os }: { port: PortSummary; os: Os } = $props();
  const here = $derived(statusFor(port, os));
</script>

<a class="card" href="/port/{encodeURIComponent(port.id)}">
  <Cover url={port.screenshot_url} gameKey={port.game_key} title={port.title} />
  <div class="body">
    <span class="title">{port.title}</span>
    <span class="meta">
      <span>{typeLabel[port.type] ?? port.type}</span>
      {#if port.stars != null}<span>★ {formatNumber(port.stars)}</span>{/if}
      {#if port.archived}<span class="archived">Archived</span>{/if}
    </span>
    <div class="status">
      <StatusBadge status={port.status} prefix="Overall" />
      {#if here}
        <StatusBadge status={here} prefix={osLabel[os]} />
      {:else if port.play_url}
        <span class="note">Plays in browser</span>
      {:else}
        <span class="note">No {osLabel[os]} build yet</span>
      {/if}
    </div>
  </div>
</a>

<style>
  .card {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-radius: var(--radius-m);
    background: var(--surface-1);
    border: 1px solid var(--border);
    color: inherit;
    text-decoration: none;
    overflow: hidden;
  }
  .card:hover {
    background: var(--surface-2);
    border-color: var(--border-strong);
  }
  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px 14px;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    gap: 14px;
    color: var(--text-3);
    font-size: 0.85rem;
  }
  .archived {
    color: var(--status-boots);
  }
  .status {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .note {
    color: var(--text-4);
    font-size: 0.8rem;
  }
</style>
