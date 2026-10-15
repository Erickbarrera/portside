<script lang="ts">
  import { games } from "$lib/games.svelte";
  import { commands, type CommandError, type Os, type SnapshotInfo } from "$lib/ipc";
  import { formatAgo, formatBytes, osLabel } from "$lib/labels";
  import { describeFiles, saveStatusLabel, saves } from "$lib/saves.svelte";

  let { portId, title, os }: { portId: string; title: string; os: Os } = $props();

  let busy = $state(false);
  let error = $state<CommandError | null>(null);

  const game = $derived(saves.games[portId] ?? null);
  const playing = $derived(portId in games.running);
  const blocked = $derived(games.failed[portId]?.code === "E_SAVES_CONFLICT");

  $effect(() => {
    void saves.start().then(() => saves.load(portId));
  });

  async function run(job: () => Promise<{ status: "ok" } | { status: "error"; error: CommandError }>) {
    busy = true;
    error = null;
    const result = await job();
    busy = false;
    if (result.status === "error") error = result.error;
  }

  async function chooseFolder() {
    const path = await commands.savesPickFolder(`Where ${title} keeps its saves`);
    if (path) await run(() => saves.setFolder(portId, path));
  }

  async function keep(side: SnapshotInfo) {
    await run(() => saves.keep(portId, side.id));
    if (blocked && saves.games[portId]?.status !== "conflict") await games.launch(portId);
  }

  function sideTitle(side: SnapshotInfo): string {
    return side.this_device ? "This device" : `${side.device_name} (${osLabel[side.os]})`;
  }
</script>

<section class="saves" aria-labelledby="saves-h">
  <h2 id="saves-h">Cloud saves</h2>
  {#if !game}
    <p class="muted">Checking…</p>
  {:else if game.status === "off"}
    <p class="muted">
      Cloud saves are off. Turn them on to keep this game's saves in step on all your computers.
    </p>
    <a class="btn btn-secondary" href="/settings#saves">Set up cloud saves</a>
  {:else if game.status === "no_folder"}
    <p class="muted">
      Portside doesn't know where {title} keeps its saves on {osLabel[os]}. If you know, choose the
      folder and Portside will sync everything in it.
    </p>
    <button class="btn btn-secondary" disabled={busy} onclick={chooseFolder}>Choose save folder</button>
  {:else}
    <div class="row">
      <div class="grow">
        <p class="state" class:warn={game.status === "conflict" || game.status === "error" || game.status === "unavailable"}>
          {saveStatusLabel[game.status]}
          {#if game.last_sync && game.status === "synced"}
            <span class="muted">· {formatAgo(game.last_sync)}</span>
          {/if}
        </p>
        {#if game.folder}
          <p class="folder muted">
            <span class="mono">{game.folder}</span>
            {#if game.custom_folder}<span>(chosen by you)</span>{/if}
          </p>
        {/if}
      </div>
      <button
        class="btn btn-secondary"
        disabled={busy || playing || game.status === "syncing"}
        onclick={() => run(() => saves.sync(portId))}
      >
        {game.status === "syncing" ? "Syncing…" : "Sync now"}
      </button>
    </div>

    {#if game.message && game.status !== "conflict"}
      <p class="warn small">{game.message}</p>
    {/if}

    {#if game.status === "conflict"}
      <div class="conflict" role="alert">
        <p>
          <strong>Your saves for {title} differ between devices.</strong> Choose which to keep. The
          other copy stays in your cloud folder's history, and this device's current saves are copied
          aside before anything is replaced.
        </p>
        <div class="sides">
          {#each game.conflict as side (side.id)}
            <div class="side">
              <p class="who">{sideTitle(side)}</p>
              <p class="muted small">Saved {formatAgo(Date.parse(side.created_at) / 1000)}</p>
              <p class="muted small">
                {describeFiles(side.file_count, formatBytes(side.size))}
                {#if side.port_version}· {side.port_version}{/if}
              </p>
              <button class="btn btn-primary" disabled={busy || playing} onclick={() => keep(side)}>
                Keep these
              </button>
            </div>
          {/each}
        </div>
        {#if blocked}
          <button class="link small" disabled={busy} onclick={() => games.launch(portId, true)}>
            Play without syncing (sync again when you quit)
          </button>
        {/if}
      </div>
    {/if}

    <div class="actions">
      {#if game.folder}
        <button class="link small" onclick={() => run(() => saves.reveal(portId))}>Show save folder</button>
      {/if}
      <button class="link small" disabled={busy} onclick={chooseFolder}>Use a different folder</button>
      {#if game.custom_folder}
        <button class="link small" disabled={busy} onclick={() => run(() => saves.setFolder(portId, null))}>
          Use the game's usual folder
        </button>
      {/if}
    </div>
  {/if}
  {#if error}
    <p class="warn small" role="alert">{error.message}</p>
  {/if}
</section>

<style>
  .saves {
    max-width: 760px;
    margin-bottom: 28px;
  }
  h2 {
    font-size: 1rem;
    color: var(--text-2);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .state {
    margin: 0 0 4px;
    font-weight: 600;
  }
  .folder {
    margin: 0;
    font-size: 0.85rem;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .warn {
    color: var(--status-boots);
  }
  .small {
    font-size: 0.85rem;
  }
  .conflict {
    margin-top: 16px;
    padding: 16px 20px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-left: 4px solid var(--status-in-progress);
    border-radius: var(--radius-m);
  }
  .sides {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
    margin: 12px 0;
  }
  .side {
    padding: 14px 16px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
  }
  .side p {
    margin: 0 0 4px;
  }
  .side .btn {
    margin-top: 8px;
  }
  .who {
    font-weight: 600;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-top: 12px;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
  }
  .link:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
