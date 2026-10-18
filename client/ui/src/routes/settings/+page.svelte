<script lang="ts">
  import { goto } from "$app/navigation";
  import { auth } from "$lib/account.svelte";
  import { appUpdate } from "$lib/app-update.svelte";
  import { catalog } from "$lib/catalog.svelte";
  import AppUpdate from "$lib/components/AppUpdate.svelte";
  import SavesSetup from "$lib/components/SavesSetup.svelte";
  import { type CouchSetting, couch } from "$lib/couch.svelte";
  import { archLabel, formatAgo, formatNumber, osLabel } from "$lib/labels";
  import { onMount } from "svelte";

  let { data } = $props();

  onMount(() => {
    void catalog.start();
    void appUpdate.start();
  });
  const update = $derived(appUpdate.status);
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
      {#if data.app.os === "android"}
        Portside doesn't keep your sign-in on Android yet, so you sign in again after it restarts.
      {:else}
        Your sign-in is kept in this computer's keychain. Signing out removes it from this device.
      {/if}
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

{#if data.app.os === "android"}
  <section aria-labelledby="saves-h">
    <h2 id="saves-h">Cloud saves</h2>
    <p class="muted">Cloud saves don't work on Android yet.</p>
  </section>
{:else}
  <SavesSetup />
{/if}

<section aria-labelledby="couch-h">
  <h2 id="couch-h">Couch mode</h2>
  <p class="muted">
    Bigger, full screen, with an on-screen keyboard for text fields: for a Steam Deck or a TV and a
    controller. Auto turns it on in Steam's Game Mode and Big Picture.
  </p>
  <div class="choices" role="radiogroup" aria-labelledby="couch-h">
    {#each [["auto", "Auto"], ["on", "On"], ["off", "Off"]] as [value, text] (value)}
      <button
        class="btn btn-secondary"
        role="radio"
        aria-checked={couch.setting === value}
        onclick={() => couch.set(value as CouchSetting)}>{text}</button
      >
    {/each}
  </div>
</section>

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
  {#if data.app.os !== "android"}
    <p class="muted">
      To use a different copy of the catalog, start Portside with <code>PORTSIDE_CATALOG_URL</code> set
      to its address.
    </p>
  {/if}
</section>

<section aria-labelledby="updates-h">
  <h2 id="updates-h">Updates</h2>
  {#if update?.off_reason}
    <p class="muted">{update.off_reason}</p>
  {:else if update}
    <p class="muted">
      Portside checks for a new version when it starts and every few hours, and asks before
      installing it.
    </p>
    <AppUpdate />
    {#if !update.available}
      <dl>
        <dt>Last check</dt>
        <dd>
          {#if update.last_error}
            {update.last_error.message}
          {:else if update.last_checked_at}
            Up to date · {formatAgo(update.last_checked_at)}
          {:else}
            Not yet
          {/if}
        </dd>
      </dl>
      <button
        class="btn btn-secondary"
        disabled={update.checking}
        onclick={() => appUpdate.check()}
      >
        {update.checking ? "Checking…" : "Check for updates"}
      </button>
    {/if}
  {/if}
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
  .choices {
    display: flex;
    gap: 8px;
  }
  .choices [aria-checked="true"] {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent);
  }
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
