<script lang="ts">
  import { appUpdate, progressLabel } from "$lib/app-update.svelte";

  // `banner`: the strip at the top of every screen, which can be closed.
  // Otherwise the full panel in Settings.
  let { banner = false }: { banner?: boolean } = $props();

  const status = $derived(appUpdate.status);
  const progress = $derived(status?.progress ?? null);
</script>

{#if status?.available && (!banner || appUpdate.showBanner || progress)}
  <div class:banner role="status">
    <p>
      {#if progress}
        Updating to Portside {status.available.version}…
        {progressLabel(progress.downloaded ?? 0, progress.total)}. Portside restarts when it's done.
      {:else}
        <strong>Portside {status.available.version} is available.</strong>
        Updating restarts the app.
      {/if}
    </p>
    {#if progress}
      <progress
        max={progress.total ?? undefined}
        value={progress.total ? (progress.downloaded ?? 0) : undefined}
      ></progress>
    {:else}
      <div class="actions">
        <button class="btn btn-primary" onclick={() => appUpdate.install()}
          >Update and restart</button
        >
        {#if banner}
          <button class="btn btn-secondary" onclick={() => appUpdate.dismiss()}>Later</button>
        {/if}
      </div>
    {/if}
    {#if appUpdate.installError}
      <p class="error" role="alert">{appUpdate.installError.message}</p>
    {/if}
    {#if !banner && status.available.notes}
      <details>
        <summary>What's new</summary>
        <pre>{status.available.notes}</pre>
      </details>
    {/if}
  </div>
{/if}

<style>
  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 16px;
    margin: -8px 0 24px;
    padding: 12px 16px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius-m);
  }
  .banner p {
    flex: 1 1 320px;
    margin: 0;
  }
  .banner progress {
    flex: 1 1 200px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  div:not(.banner) .actions {
    margin: 12px 0;
  }
  progress {
    width: 100%;
    accent-color: var(--accent);
  }
  .error {
    flex-basis: 100%;
    color: var(--text-2);
  }
  pre {
    white-space: pre-wrap;
    font-family: inherit;
    color: var(--text-2);
  }
</style>
