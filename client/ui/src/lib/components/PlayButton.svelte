<script lang="ts">
  import { games } from "$lib/games.svelte";

  let { portId, title }: { portId: string; title: string } = $props();

  let starting = $state(false);
  const live = $derived(games.running[portId] ?? null);

  async function play() {
    starting = true;
    await games.launch(portId);
    starting = false;
  }
</script>

{#if live?.state === "stopping"}
  <button class="btn btn-secondary" disabled>Stopping…</button>
{:else if live}
  <button class="btn btn-secondary" aria-label={`Stop ${title}`} onclick={() => games.stop(portId)}>
    Stop
  </button>
{:else}
  <button class="btn btn-primary" aria-label={`Play ${title}`} disabled={starting} onclick={play}>
    {starting ? "Starting…" : "Play"}
  </button>
{/if}
