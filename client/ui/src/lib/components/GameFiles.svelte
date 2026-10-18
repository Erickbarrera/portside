<script lang="ts">
  // Files from the player's own copy of the game, on an installed game's page
  // (docs/specs/manifest-format.md, "game_files"). Hidden for ports that need none.
  import { installs } from "$lib/installs.svelte";
  import { type CommandError, type Found, type GameFilesStatus, type Store, commands } from "$lib/ipc";
  import { games } from "$lib/games.svelte";

  let { portId }: { portId: string } = $props();

  let status = $state<GameFilesStatus | null>(null);
  let busy = $state(false);
  let error = $state<CommandError | null>(null);

  const storeLabel: Record<Store, string> = {
    steam: "Steam",
    gog: "GOG",
    epic: "the Epic Games Store",
    folder: "a folder you chose",
  };

  // Look again after each install or update, which copies the files.
  const version = $derived(installs.installed[portId]?.version ?? null);
  const playing = $derived(portId in games.running);

  $effect(() => {
    const id = portId;
    void version;
    error = null;
    void commands.gameFilesStatus(id).then((r) => {
      if (id === portId && r.status === "ok") status = r.data;
    });
  });

  async function copy(from: Found) {
    busy = true;
    error = null;
    const result = await commands.gameFilesCopy(portId, from.store, from.dir);
    busy = false;
    if (result.status === "error") error = result.error;
    else status = result.data;
  }

  async function chooseFolder() {
    if (!status) return;
    const dir = await commands.savesPickFolder(`Where your copy of ${status.label} is`);
    if (dir) await copy({ store: "folder", dir });
  }

  const others = $derived(
    status?.found.filter((f) => !(status?.ready && status.source?.dir === f.dir)) ?? [],
  );
</script>

{#if status}
  <section class="files" aria-labelledby="files-h">
    <h2 id="files-h">Game files</h2>
    {#if busy}
      <p class="muted" role="status">Copying the files from your copy of {status.label}…</p>
    {:else if status.ready}
      <p>
        Uses the files from your copy of {status.label}{#if status.source}, copied from
          {storeLabel[status.source.store]}{/if}.
      </p>
    {:else if status.found.length > 0}
      <p>
        This game needs the files from your own copy of {status.label}. Portside found it in
        {storeLabel[status.found[0].store]}.
      </p>
    {:else}
      <p>
        This game needs the files from your own copy of {status.label}
        (<span class="mono">{status.files.join(", ")}</span>). Portside didn't find it in Steam, GOG or
        the Epic Games Store on this computer. Install it there, or choose the folder it's in.
      </p>
    {/if}
    {#if !busy}
      <div class="actions">
        {#each others as f (f.dir)}
          <button class="btn btn-secondary" disabled={playing} onclick={() => copy(f)}>
            Copy from {storeLabel[f.store]}
          </button>
        {/each}
        <button class={status.ready ? "link small" : "btn btn-secondary"} disabled={playing} onclick={chooseFolder}>
          {status.ready ? "Use a different folder" : "Choose folder"}
        </button>
      </div>
    {/if}
    {#if error}
      <p class="warn small" role="alert">{error.message}</p>
    {/if}
  </section>
{/if}

<style>
  .files {
    max-width: 760px;
    margin-bottom: 28px;
  }
  h2 {
    font-size: 1rem;
    color: var(--text-2);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 0.85rem;
  }
  .small {
    font-size: 0.9rem;
  }
  .warn {
    color: var(--status-boots);
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
