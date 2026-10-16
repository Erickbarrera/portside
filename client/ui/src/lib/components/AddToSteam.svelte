<script lang="ts">
  // "Add to Steam" on an installed game's page (client/docs/platforms.md). Hidden when Steam
  // isn't on this computer.
  import { type CommandError, type SteamStatus, commands } from "$lib/ipc";
  import ErrorNotice from "./ErrorNotice.svelte";

  let { portId, title }: { portId: string; title: string } = $props();

  let status = $state<SteamStatus | null>(null);
  let confirmRestart = $state(false);
  let adding = $state(false);
  let error = $state<CommandError | null>(null);

  $effect(() => {
    const id = portId;
    status = null;
    error = null;
    confirmRestart = false;
    void commands.steamStatus(id).then((s) => {
      if (id === portId) status = s;
    });
  });

  async function add(restartSteam: boolean) {
    if (status?.running && !restartSteam) {
      confirmRestart = true;
      return;
    }
    confirmRestart = false;
    adding = true;
    error = null;
    const result = await commands.steamAdd(portId, restartSteam);
    adding = false;
    if (result.status === "error") error = result.error;
    status = await commands.steamStatus(portId);
  }
</script>

{#if status?.found}
  <div class="steam">
    {#if status.added}
      <p class="muted small">In your Steam library: play it from Steam, including Game Mode and Big Picture.</p>
    {:else if adding}
      <p class="muted small">{status.running ? "Restarting Steam to add it…" : "Adding to Steam…"}</p>
    {:else if confirmRestart && status.started_by_steam}
      <p class="small grow">
        Steam has to be closed to add games, and Steam started Portside. Open Portside on its own (on a Steam
        Deck, in Desktop Mode) to add it.
      </p>
      <button class="btn btn-secondary" onclick={() => (confirmRestart = false)}>OK</button>
    {:else if confirmRestart}
      <p class="small grow">Steam closes for a moment to add {title}, then opens again.</p>
      <button class="btn btn-primary" onclick={() => add(true)}>Restart Steam and add</button>
      <button class="btn btn-secondary" onclick={() => (confirmRestart = false)}>Not now</button>
    {:else}
      <p class="muted small grow">Play it from Steam's library, including Game Mode and Big Picture.</p>
      <button class="btn btn-secondary" onclick={() => add(false)}>Add to Steam</button>
    {/if}
  </div>
  {#if error}
    <ErrorNotice {error} title="Couldn't add it to Steam" onretry={() => add(false)} />
  {/if}
{/if}

<style>
  .steam {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }
  .steam p {
    margin: 0;
  }
  .grow {
    flex: 1;
    min-width: 200px;
  }
  .small {
    font-size: 0.9rem;
  }
</style>
