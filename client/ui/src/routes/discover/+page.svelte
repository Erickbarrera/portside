<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ErrorNotice from "$lib/components/ErrorNotice.svelte";
  import PortCard from "$lib/components/PortCard.svelte";
  import {
    type CatalogQuery,
    type CommandError,
    commands,
    type Os,
    type PlayableStatus,
    type PortSummary,
    type PortType,
  } from "$lib/ipc";
  import { catalog } from "$lib/catalog.svelte";
  import { formatAgo, formatNumber, osLabel, statusLabel, statusOrder, typeLabel } from "$lib/labels";
  import { onMount, untrack } from "svelte";

  let { data } = $props();

  const PAGE_SIZE = 50;

  // Filters live in the URL so Back from a game page restores them.
  const params = $derived(page.url.searchParams);
  const filters = $derived<CatalogQuery>({
    q: params.get("q") || null,
    os: (params.get("os") as Os | null) || null,
    status: (params.get("status") as PlayableStatus | null) || null,
    port_type: (params.get("type") as PortType | null) || null,
    limit: PAGE_SIZE,
  });

  let text = $state(untrack(() => page.url.searchParams.get("q") ?? ""));
  let items = $state<PortSummary[]>([]);
  let cursor = $state<string | null>(null);
  let loading = $state(false);
  let error = $state<CommandError | null>(null);
  let requestId = 0;

  async function load(query: CatalogQuery, append: boolean) {
    const id = ++requestId;
    loading = true;
    error = null;
    const result = await commands.catalogSearch(query);
    if (id !== requestId) return; // a newer search started
    loading = false;
    if (result.status === "error") {
      error = result.error;
      return;
    }
    items = append ? [...items, ...result.data.items] : result.data.items;
    cursor = result.data.cursor;
  }

  $effect(() => {
    const query = filters;
    untrack(() => load(query, false));
  });

  function setParam(key: string, value: string | null) {
    const next = new URLSearchParams(page.url.searchParams);
    if (value) next.set(key, value);
    else next.delete(key);
    const qs = next.toString();
    goto(qs ? `?${qs}` : "/discover", { replaceState: true, keepFocus: true, noScroll: true });
  }

  let typing: ReturnType<typeof setTimeout> | undefined;
  function onInput() {
    clearTimeout(typing);
    typing = setTimeout(() => setParam("q", text.trim() || null), 250);
  }

  const os = $derived(data.app.os);

  // Re-run the current search when a background sync changes the saved catalog.
  onMount(() => {
    void catalog.start();
    return catalog.onChange(() => load(filters, false));
  });
</script>

<svelte:head><title>Discover · Portside</title></svelte:head>

<header>
  <h1>Discover</h1>
  <p class="muted">Community ports and how well they play on each system.</p>
  {#if catalog.status && catalog.status.port_count > 0}
    <p class="freshness" role="status">
      {#if catalog.status.last_error?.code === "E_API_UNREACHABLE"}
        <span class="offline">Offline</span> · Showing the {formatNumber(catalog.status.port_count)}
        ports saved on this device{#if catalog.status.last_synced_at}, updated {formatAgo(
            catalog.status.last_synced_at,
          )}{/if}.
        <button class="link" disabled={catalog.refreshing} onclick={() => catalog.refresh()}>
          {catalog.refreshing ? "Checking…" : "Try again"}
        </button>
      {:else if catalog.status.last_synced_at}
        {formatNumber(catalog.status.port_count)} ports · updated {formatAgo(
          catalog.status.last_synced_at,
        )}
      {/if}
    </p>
  {/if}
</header>

<form class="filters" role="search" onsubmit={(e) => e.preventDefault()}>
  <label class="search">
    <span class="visually-hidden">Search</span>
    <input type="search" placeholder="Search ports" bind:value={text} oninput={onInput} />
  </label>
  <label>
    <span class="visually-hidden">System</span>
    <select value={filters.os ?? ""} onchange={(e) => setParam("os", e.currentTarget.value)}>
      <option value="">Any system</option>
      {#each Object.entries(osLabel) as [value, label] (value)}
        <option {value}>Has a {label} build</option>
      {/each}
    </select>
  </label>
  <label>
    <span class="visually-hidden">Status</span>
    <select
      value={filters.status ?? ""}
      onchange={(e) => setParam("status", e.currentTarget.value)}
    >
      <option value="">Any status</option>
      {#each statusOrder as value (value)}
        <option {value}>{statusLabel[value]}</option>
      {/each}
    </select>
  </label>
  <label>
    <span class="visually-hidden">Type</span>
    <select
      value={filters.port_type ?? ""}
      onchange={(e) => setParam("type", e.currentTarget.value)}
    >
      <option value="">Any type</option>
      {#each Object.entries(typeLabel) as [value, label] (value)}
        <option {value}>{label}</option>
      {/each}
    </select>
  </label>
</form>

{#if error}
  <ErrorNotice
    {error}
    title="The catalog isn't available right now"
    onretry={() => load(filters, false)}
  />
{:else if !loading && items.length === 0}
  <p class="muted">No ports match these filters.</p>
{:else}
  <ul class="list" aria-busy={loading}>
    {#each items as port (port.id)}
      <li><PortCard {port} {os} /></li>
    {/each}
  </ul>
  {#if cursor}
    <div class="more">
      <button
        class="btn btn-secondary"
        disabled={loading}
        onclick={() => load({ ...filters, cursor }, true)}
      >
        {loading ? "Loading…" : "Show more"}
      </button>
    </div>
  {/if}
{/if}

<style>
  header {
    margin-bottom: 20px;
  }
  header p {
    margin-bottom: 4px;
  }
  .freshness {
    color: var(--text-4);
    font-size: 0.85rem;
  }
  .offline {
    color: var(--status-in-progress);
    font-weight: 600;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-bottom: 20px;
  }
  .search {
    flex: 1 1 280px;
  }
  input,
  select {
    width: 100%;
    height: 42px;
    padding: 0 12px;
    background-color: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
  }
  select {
    appearance: none;
    padding-right: 36px;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath d='M1 1.5 6 6.5l5-5' fill='none' stroke='%23aeb6c4' stroke-width='1.6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 12px center;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 16px;
  }
  .list[aria-busy="true"] {
    opacity: 0.7;
  }
  .more {
    display: flex;
    justify-content: center;
    padding: 20px 0;
  }
</style>
