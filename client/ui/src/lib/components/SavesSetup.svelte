<script lang="ts">
  import { commands, type CommandError, type DeviceRecord } from "$lib/ipc";
  import { formatAgo, osLabel } from "$lib/labels";
  import { saves } from "$lib/saves.svelte";
  import { onMount } from "svelte";

  let busy = $state(false);
  let error = $state<CommandError | null>(null);
  let devices = $state<DeviceRecord[] | null>(null);
  let confirmOff = $state(false);

  const status = $derived(saves.status);

  onMount(() => {
    void saves.start().then(loadDevices);
  });

  async function loadDevices() {
    if (!saves.status?.enabled) return;
    const result = await commands.savesDevices();
    devices = result.status === "ok" ? result.data : null;
  }

  async function finish(job: Promise<{ status: "ok"; data: import("$lib/ipc").SavesStatus } | { status: "error"; error: CommandError }>) {
    busy = true;
    error = null;
    const result = await job;
    busy = false;
    if (result.status === "error") {
      error = result.error;
      return;
    }
    saves.set(result.data);
    confirmOff = false;
    await loadDevices();
  }

  async function pick() {
    const path = await commands.savesPickFolder("Choose a folder your cloud app keeps in sync");
    if (path) await finish(commands.savesUseFolder(path));
  }
</script>

<section id="saves" aria-labelledby="saves-h">
  <h2 id="saves-h">Cloud saves</h2>
  {#if !status}
    <p class="muted">Checking…</p>
  {:else if status.enabled}
    <dl>
      <dt>Saves folder</dt>
      <dd class="mono">{status.location}</dd>
      <dt>This device</dt>
      <dd>{status.device_name}</dd>
    </dl>
    {#if status.problem}
      <p class="warn">{status.problem}</p>
    {/if}
    <p class="muted">
      Games sync before they start and after you quit. Your saves are kept as normal files in the
      <span class="mono">Portside Saves</span> folder, so you can back them up or copy them by hand.
    </p>
    {#if devices && devices.length > 0}
      <h3>Devices using these saves</h3>
      <ul class="devices">
        {#each devices as d (d.id)}
          <li>
            <span>{d.name}</span>
            <span class="muted">{osLabel[d.os]} · last synced {formatAgo(Date.parse(d.last_sync) / 1000)}</span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if confirmOff}
      <p>
        Stop syncing on this device? Your saves stay on this computer and in the folder, and you can
        turn it back on any time.
      </p>
      <button class="btn btn-danger" disabled={busy} onclick={() => finish(commands.savesTurnOff())}>Turn off</button>
      <button class="btn btn-secondary" onclick={() => (confirmOff = false)}>Keep syncing</button>
    {:else}
      <button class="btn btn-secondary" onclick={() => (confirmOff = true)}>Turn off on this device</button>
    {/if}
  {:else}
    <p class="muted">
      Keep your saves in step on all your computers. Portside stores them in a folder your own cloud
      app syncs: iCloud Drive, Dropbox, OneDrive, Google Drive or Syncthing. Nothing goes to a
      Portside server. Choose the same folder on each computer.
    </p>
    {#if status.suggestions.length > 0}
      <ul class="choices">
        {#each status.suggestions as s (s.path)}
          <li>
            <button class="choice" disabled={busy} onclick={() => finish(commands.savesUseFolder(s.path))}>
              <span class="label">{s.label}</span>
              <span class="mono muted small">{s.path}</span>
              {#if s.has_saves}<span class="tag">Has Portside saves</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    <button class="btn btn-secondary" disabled={busy} onclick={pick}>
      {status.suggestions.length > 0 ? "Choose another folder…" : "Choose a folder…"}
    </button>
  {/if}
  {#if error}
    <p class="warn" role="alert">{error.message}</p>
  {/if}
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
  h3 {
    font-size: 0.95rem;
    margin: 16px 0 8px;
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
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: 0.85rem;
  }
  .warn {
    color: var(--status-boots);
  }
  .devices,
  .choices {
    list-style: none;
    margin: 0 0 16px;
    padding: 0;
  }
  .devices li {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  .choices li {
    margin-bottom: 8px;
  }
  .choice {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
    width: 100%;
    padding: 12px 16px;
    text-align: left;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
    cursor: pointer;
    font: inherit;
  }
  .choice:hover {
    border-color: var(--accent);
  }
  .label {
    font-weight: 600;
  }
  .tag {
    margin-left: auto;
    font-size: 0.8rem;
    color: var(--status-perfect);
  }
</style>
