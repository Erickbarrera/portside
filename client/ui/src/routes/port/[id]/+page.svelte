<script lang="ts">
  import { invalidateAll } from "$app/navigation";
  import { primaryAction } from "$lib/actions";
  import CloudSaves from "$lib/components/CloudSaves.svelte";
  import Cover from "$lib/components/Cover.svelte";
  import ErrorNotice from "$lib/components/ErrorNotice.svelte";
  import InstallPanel from "$lib/components/InstallPanel.svelte";
  import InviteFriend from "$lib/components/InviteFriend.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import { openExternal } from "$lib/external";
  import { games } from "$lib/games.svelte";
  import { installs } from "$lib/installs.svelte";
  import { archLabel, formatBytes, formatDate, formatNumber, osLabel, typeLabel } from "$lib/labels";

  let { data } = $props();

  const os = $derived(data.app.os);
  const view = $derived(data.result.status === "ok" ? data.result.data : null);
  const port = $derived(view?.port ?? null);
  const action = $derived(port ? primaryAction(port, os) : null);
  const release = $derived(port?.latest_releases.find((r) => !r.revoked) ?? null);
  // Installed games keep their panel offline too, when the saved summary has no manifest.
  const showInstall = $derived(
    !!port && (action?.kind === "install" || port.id in installs.installed || port.id in installs.active),
  );

  $effect(() => {
    installs.start();
    games.start();
  });
</script>

<svelte:head><title>{port?.title ?? "Port"} · Portside</title></svelte:head>

<a class="back" href="/discover" onclick={(e) => { if (history.length > 1) { e.preventDefault(); history.back(); } }}>← Back</a>

{#if data.result.status === "error"}
  <ErrorNotice
    error={data.result.error}
    title={data.result.error.code === "E_NOT_FOUND" ? "This port isn't in the catalog" : "Couldn't load this port"}
    onretry={data.result.error.code === "E_NOT_FOUND" ? undefined : () => invalidateAll()}
  />
{:else if port && action}
  {#if view?.offline}
    <p class="offline" role="status">
      You're offline. This is the summary saved on this device; details and releases need a
      connection.
      <button class="btn btn-secondary" onclick={() => invalidateAll()}>Try again</button>
    </p>
  {/if}
  <div class="hero">
    <Cover url={port.screenshot_url} gameKey={port.game_key} title={port.title} size="hero" />
  </div>
  <header>
    <div>
      <h1>{port.title}</h1>
      <p class="meta">
        <span>{typeLabel[port.type] ?? port.type}</span>
        {#if port.stars != null}<span>★ {formatNumber(port.stars)}</span>{/if}
        {#if port.license}<span>{port.license}</span>{/if}
        {#if port.archived}<span class="archived">Archived upstream</span>{/if}
      </p>
    </div>
    <div class="cta">
      {#if showInstall}
        <!-- The install panel below has the actions. -->
      {:else if action.kind === "play_in_browser"}
        <a class="btn btn-primary" href={action.url} onclick={(e) => openExternal(e, action.url)}>
          Play in browser
        </a>
      {:else if action.kind === "view_project" && action.url}
        <a class="btn btn-secondary" href={action.url} onclick={(e) => openExternal(e, action.url as string)}>
          View project
        </a>
      {/if}
      <InviteFriend portId={port.id} title={port.title} />
    </div>
  </header>

  {#if showInstall}
    <InstallPanel portId={port.id} {os} />
  {/if}

  {#if port.id in installs.installed}
    <CloudSaves portId={port.id} title={port.title} {os} />
  {/if}

  <section class="status" aria-labelledby="status-h">
    <h2 id="status-h">Status</h2>
    <div class="badges">
      <StatusBadge status={port.status} prefix="Overall" />
      {#each port.os_status as s (s.os)}
        <StatusBadge status={s.status} prefix={osLabel[s.os]} />
      {/each}
    </div>
    {#if port.os_status.length === 0}
      <p class="muted">No builds are listed for any system yet.</p>
    {/if}
    {#if port.status_notes}<p>{port.status_notes}</p>{/if}
  </section>

  {#if port.description}
    <section aria-labelledby="about-h">
      <h2 id="about-h">About</h2>
      <p>{port.description}</p>
      {#if port.note}<p class="muted">{port.note}</p>{/if}
    </section>
  {/if}

  {#if release}
    <section aria-labelledby="release-h">
      <h2 id="release-h">Latest release</h2>
      <p>
        <a href={release.html_url} onclick={(e) => openExternal(e, release.html_url)}
          >{release.name ?? release.tag}</a
        >
        <span class="muted">· {formatDate(release.published_at)}</span>
        {#if release.prerelease}<span class="muted">· Pre-release</span>{/if}
      </p>
      {#if release.assets.length > 0}
        <ul class="assets">
          {#each release.assets as asset (asset.name)}
            <li>
              <span>{osLabel[asset.os]} · {archLabel[asset.arch]}</span>
              <span class="mono">{asset.name}</span>
              <span class="muted">{formatBytes(asset.size)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  <section aria-labelledby="discuss-h">
    <h2 id="discuss-h">Discussions</h2>
    <p class="muted">Ask for help, report bugs and share mods with other players.</p>
    <a class="btn btn-secondary" href={`/port/${encodeURIComponent(port.id)}/board`}>Open discussions</a>
  </section>

  <section aria-labelledby="links-h">
    <h2 id="links-h">Links</h2>
    <ul class="links">
      {#if port.repo}
        <li><a href={port.repo} onclick={(e) => openExternal(e, port.repo!)}>Source code</a></li>
      {/if}
      {#if port.homepage}
        <li><a href={port.homepage} onclick={(e) => openExternal(e, port.homepage!)}>Homepage</a></li>
      {/if}
      {#if port.last_commit}
        <li class="muted">Last commit {formatDate(port.last_commit)}</li>
      {/if}
    </ul>
  </section>
{/if}

<style>
  .hero {
    max-width: 760px;
    margin: 12px 0 20px;
  }
  .back {
    display: inline-block;
    margin-bottom: 16px;
    color: var(--text-3);
    text-decoration: none;
  }
  .offline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 16px;
    margin-bottom: 20px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-left: 4px solid var(--status-in-progress);
    border-radius: var(--radius-m);
    color: var(--text-2);
  }
  .back:hover {
    color: var(--text);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 24px;
    margin-bottom: 28px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    color: var(--text-3);
  }
  .archived {
    color: var(--status-boots);
  }
  .cta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    flex-shrink: 0;
  }
  section {
    max-width: 760px;
    margin-bottom: 28px;
  }
  h2 {
    font-size: 1rem;
    color: var(--text-2);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
  }
  .assets,
  .links {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .assets li {
    display: grid;
    grid-template-columns: 180px 1fr auto;
    gap: 12px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  .links li {
    padding: 4px 0;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    overflow-wrap: anywhere;
  }
</style>
