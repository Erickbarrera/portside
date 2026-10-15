<script lang="ts">
  // A game invite inside a chat: the game, and Play or a way to get it.
  import Cover from "$lib/components/Cover.svelte";
  import PlayButton from "$lib/components/PlayButton.svelte";
  import { installs } from "$lib/installs.svelte";
  import { type PortSummary, commands } from "$lib/ipc";

  let { portId, fromMe, senderName }: { portId: string; fromMe: boolean; senderName: string } =
    $props();

  let port = $state<PortSummary | null>(null);

  $effect(() => {
    void installs.start();
    void commands.catalogSummaries([portId]).then((r) => {
      port = r.status === "ok" ? (r.data[0] ?? null) : null;
    });
  });
</script>

<div class="invite">
  {#if port}
    <div class="thumb">
      <Cover url={port.screenshot_url} gameKey={port.game_key} title={port.title} size="thumb" />
    </div>
  {/if}
  <div class="body">
    <p class="label">{fromMe ? "You sent an invite" : `${senderName} invited you to play`}</p>
    <p class="title">{port?.title ?? portId}</p>
    <div class="actions">
      {#if portId in installs.installed}
        <PlayButton {portId} title={port?.title ?? portId} />
      {/if}
      <a class="btn btn-secondary" href={`/port/${encodeURIComponent(portId)}`}>
        {portId in installs.installed ? "View game" : "Get the game"}
      </a>
    </div>
  </div>
</div>

<style>
  .invite {
    display: flex;
    gap: 12px;
    padding: 12px;
    margin-top: 6px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-m);
    max-width: 420px;
  }
  .thumb {
    width: 96px;
    flex: none;
  }
  .label {
    margin: 0;
    color: var(--text-3);
    font-size: 0.85rem;
  }
  .title {
    margin: 0 0 8px;
    font-weight: 600;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>
