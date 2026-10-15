<script lang="ts">
  import { goto } from "$app/navigation";
  import { auth } from "$lib/account.svelte";
  import { catalog } from "$lib/catalog.svelte";
  import SavesSetup from "$lib/components/SavesSetup.svelte";
  import { archLabel, formatAgo, formatNumber, osLabel } from "$lib/labels";
  import { onMount } from "svelte";

  let { data } = $props();

  onMount(() => {
    void catalog.start();
  });
</script>

<svelte:head><title>Settings · Portside</title></svelte:head>

<h1>Settings</h1>

<section id="account" aria-labelledby="account-h">
  <h2 id="account-h">Bluesky account</h2>
  {#if auth.account}
    <dl>
      <dt>Signed in as</dt>
      <dd>@{auth.account.handle}</dd>
      <dt>Messages</dt>
      <dd>
        {auth.account.can_chat
          ? "Allowed"
          : "Not allowed by this sign-in. Sign out and in again to use messages."}
      </dd>
    </dl>
    <p class="muted">
      Your sign-in is kept in this computer's keychain. Signing out removes it from this device.
    </p>
    <button
      class="btn btn-secondary"
      onclick={async () => {
        await auth.signOut();
        await goto("/settings");
      }}>Sign out</button
    >
  {:else}
    <p class="muted">Not signed in. Friends, messages and discussions need a Bluesky account.</p>
    <a class="btn btn-primary" href="/signin">Sign in</a>
  {/if}
</section>

<SavesSetup />

<section aria-labelledby="about-h">
  <h2 id="about-h">About this app</h2>
  <dl>
    <dt>Version</dt>
    <dd>{data.app.version}</dd>
    <dt>System</dt>
    <dd>{osLabel[data.app.os]} · {archLabel[data.app.arch]}</dd>
    <dt>Catalog</dt>
    <dd class="mono">{data.app.catalog_url}</dd>
  </dl>
  <p class="muted">
    To use a different copy of the catalog, start Portside with <code>PORTSIDE_CATALOG_URL</code> set
    to its address.
  </p>
</section>

<section aria-labelledby="catalog-h">
  <h2 id="catalog-h">Saved catalog</h2>
  <p class="muted">
    Portside keeps a copy of the catalog on this device so you can browse offline. It checks for
    changes every 15 minutes while the app is open.
  </p>
  {#if catalog.status}
    <dl>
      <dt>Ports saved</dt>
      <dd>{formatNumber(catalog.status.port_count)}</dd>
      <dt>Last updated</dt>
      <dd>
        {catalog.status.last_synced_at ? formatAgo(catalog.status.last_synced_at) : "Never"}
      </dd>
      {#if catalog.status.last_error}
        <dt>Last check</dt>
        <dd>{catalog.status.last_error.message}</dd>
      {/if}
    </dl>
  {/if}
  <button class="btn btn-secondary" disabled={catalog.refreshing} onclick={() => catalog.refresh()}>
    {catalog.refreshing ? "Checking…" : "Check for changes"}
  </button>
</section>

<style>
  section {
    max-width: 640px;
    margin-top: 16px;
    padding: 24px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
  }
  h2 {
    font-size: 1.05rem;
  }
  dl {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 8px 16px;
    margin: 0 0 16px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
  }
  .mono,
  code {
    font-family: var(--font-mono);
  }
  code {
    background: var(--surface-3);
    padding: 1px 6px;
    border-radius: 4px;
  }
</style>
