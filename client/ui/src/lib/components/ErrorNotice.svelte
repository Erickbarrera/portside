<script lang="ts">
  import type { CommandError } from "$lib/ipc";

  let {
    error,
    title = "Something went wrong",
    onretry,
  }: { error: CommandError; title?: string; onretry?: () => void } = $props();
</script>

<div class="notice" role="alert">
  <p><strong>{title}.</strong> {error.message}</p>
  {#if error.code === "E_API_UNREACHABLE"}
    <p class="hint">
      The catalog is downloaded from the address in Settings. Check your connection, or point the
      app at another copy with <code>PORTSIDE_CATALOG_URL</code>.
    </p>
  {/if}
  {#if onretry}
    <button class="btn btn-secondary" onclick={onretry}>Try again</button>
  {/if}
</div>

<style>
  .notice {
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-left: 4px solid var(--status-boots);
    border-radius: var(--radius-m);
    padding: 16px 20px;
  }
  .hint {
    color: var(--text-2);
  }
  code {
    font-family: var(--font-mono);
    background: var(--surface-3);
    padding: 1px 6px;
    border-radius: 4px;
  }
</style>
