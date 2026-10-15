<script lang="ts">
  import { crashed, formatPlaytime, games } from "$lib/games.svelte";
  import { fraction, installs } from "$lib/installs.svelte";
  import type { CommandError, Os, PlanSummary } from "$lib/ipc";
  import { archLabel, formatBytes, formatDate, osLabel } from "$lib/labels";
  import ErrorNotice from "./ErrorNotice.svelte";
  import PlayButton from "./PlayButton.svelte";

  let { portId, os }: { portId: string; os: Os } = $props();

  let plan = $state<PlanSummary | null>(null);
  let planError = $state<CommandError | null>(null);
  let planning = $state(false);
  let confirmRemove = $state(false);
  let busy = $state(false);

  const installed = $derived(installs.installed[portId] ?? null);
  const running = $derived(installs.active[portId] ?? null);
  const failure = $derived(installs.failed[portId] ?? null);
  const progress = $derived(running ? fraction(running) : null);
  const playing = $derived(portId in games.running);
  const played = $derived(games.stats[portId]);
  const lastRun = $derived(games.ended[portId]);
  const launchError = $derived(games.failed[portId] ?? null);
  const playedLine = $derived.by(() => {
    const total = formatPlaytime(played?.total_secs);
    if (!total || !played?.last_played) return null;
    const last = formatDate(new Date(played.last_played * 1000).toISOString());
    return `Played ${total.toLowerCase()} · last played ${last}.`;
  });

  const revealLabel: Record<Os, string> = {
    macos: "Show in Finder",
    windows: "Show in Explorer",
    linux: "Show in Files",
    android: "Show files",
  };

  async function check() {
    planning = true;
    planError = null;
    const result = await installs.plan(portId);
    planning = false;
    if (result.status === "ok") plan = result.data;
    else planError = result.error;
  }

  async function confirm() {
    if (!plan) return;
    const chosen = plan;
    plan = null;
    await installs.install(chosen, chosen.unsigned);
  }

  async function remove() {
    busy = true;
    const result = await installs.uninstall(portId);
    busy = false;
    confirmRemove = false;
    if (result.status === "error") planError = result.error;
  }

  function phase(state: NonNullable<typeof running>): string {
    switch (state.state) {
      case "downloading":
        return state.total
          ? `Downloading · ${formatBytes(state.done)} of ${formatBytes(state.total)}`
          : `Downloading · ${formatBytes(state.done)}`;
      case "verifying":
        return "Checking the download…";
      case "unpacking":
        return "Unpacking…";
      default:
        return "Working…";
    }
  }
</script>

<section class="install" aria-label="Install">
  {#if running}
    <div class="row">
      <div class="grow">
        <p class="label" role="status">{phase(running)}</p>
        <div
          class="bar"
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={progress === null ? undefined : Math.round(progress * 100)}
        >
          <span class:indeterminate={progress === null} style:width={progress === null ? undefined : `${progress * 100}%`}></span>
        </div>
      </div>
      {#if running.state === "downloading"}
        <button class="btn btn-secondary" onclick={() => installs.cancel(portId)}>Cancel</button>
      {/if}
    </div>
  {:else if plan}
    <div class="confirm">
      <p>
        <strong>Install {plan.title} {plan.version}?</strong>
        {formatBytes(plan.size)} download of <span class="mono">{plan.asset}</span>
        {#if plan.repo}from <span class="mono">{plan.repo}</span> on GitHub{/if}.
      </p>
      {#if plan.fit === "translated"}
        <p class="note">
          There's no {archLabel.arm64} build, so this {archLabel[plan.arch]} build runs
          {os === "macos" ? "through Rosetta" : "under emulation"}. It may be slower.
        </p>
      {/if}
      {#if plan.verification === "none"}
        <p class="note">This release publishes no checksum, so Portside can't verify the download.</p>
      {/if}
      {#if plan.unsigned}
        <p class="warn">
          This app isn't signed by Apple, so macOS would refuse to open it. Portside will clear that
          check for this app only. Install it only if you trust {plan.repo ?? "its developers"}.
        </p>
      {/if}
      {#if plan.installed_version}
        <p class="note">Replaces the installed version {plan.installed_version}.</p>
      {/if}
      <div class="buttons">
        <button class="btn btn-primary" onclick={confirm}>
          {plan.unsigned ? "Install anyway" : "Install"}
        </button>
        <button class="btn btn-secondary" onclick={() => (plan = null)}>Not now</button>
      </div>
    </div>
  {:else if installed}
    <div class="row">
      <PlayButton {portId} title={installed.title} />
      <div class="grow">
        <p class="label">
          {#if playing}<strong>Playing</strong>{:else}<strong>Installed</strong>{/if} · {installed.version}
        </p>
        <p class="muted small">
          {#if playedLine}{playedLine}{/if}
          {osLabel[installed.os]} · {archLabel[installed.arch]} · {formatBytes(installed.size)} download
          · installed {formatDate(new Date(installed.installed_at * 1000).toISOString())}.
        </p>
      </div>
      <button class="btn btn-secondary" onclick={() => installs.reveal(portId)}>{revealLabel[os]}</button>
      {#if playing}
        <!-- Files can't change under a running game. -->
      {:else if confirmRemove}
        <button class="btn btn-danger" disabled={busy} onclick={remove}>Delete files</button>
        <button class="btn btn-secondary" onclick={() => (confirmRemove = false)}>Keep</button>
      {:else}
        <button class="btn btn-secondary" onclick={() => (confirmRemove = true)}>Uninstall</button>
      {/if}
    </div>
  {:else}
    <div class="row">
      <p class="grow muted">Downloads the latest release from GitHub into your library.</p>
      <button class="btn btn-primary" disabled={planning} onclick={check}>
        {planning ? "Checking…" : "Install"}
      </button>
    </div>
  {/if}

  {#if installed && launchError && launchError.code !== "E_SAVES_CONFLICT"}
    <ErrorNotice error={launchError} title="The game didn't start" onretry={() => games.launch(portId)} />
  {:else if installed && launchError}
    <p class="warn small">Choose which saves to keep under Cloud saves below, then press Play again.</p>
  {:else if installed && crashed(lastRun)}
    <p class="warn small">
      The game quit with error code {lastRun?.code}. Its output from that run is saved in a log.
      <button class="link" onclick={() => games.revealLog(portId)}>Show the log</button>
    </p>
  {/if}

  {#if !running && !plan && (planError ?? failure)}
    <ErrorNotice
      error={(planError ?? failure)!}
      title={planError ? "Can't install this yet" : "The install didn't finish"}
      onretry={planError?.code === "E_NO_BUILD" || planError?.code === "E_NO_MANIFEST" ? undefined : check}
    />
  {/if}
</section>

<style>
  .install {
    max-width: 760px;
    margin-bottom: 28px;
    padding: 16px 20px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
    min-width: 220px;
    margin: 0;
  }
  .label {
    margin: 0 0 6px;
  }
  .small {
    font-size: 0.85rem;
    margin: 0;
  }
  .bar {
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .bar .indeterminate {
    width: 30%;
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .confirm p {
    margin: 0 0 10px;
  }
  .note {
    color: var(--text-2);
  }
  .warn {
    padding: 10px 14px;
    border-left: 4px solid var(--status-in-progress);
    background: var(--surface-2);
    border-radius: var(--radius-s);
  }
  .buttons {
    display: flex;
    gap: 10px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    overflow-wrap: anywhere;
  }
</style>
