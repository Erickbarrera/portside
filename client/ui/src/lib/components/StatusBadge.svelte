<script lang="ts">
  import type { PlayableStatus } from "$lib/ipc";
  import { statusLabel } from "$lib/labels";

  let { status, prefix }: { status: PlayableStatus; prefix?: string } = $props();
</script>

<span class="badge s-{status}">
  <span class="dot" aria-hidden="true"></span>
  {#if prefix}<span class="prefix">{prefix}</span>{/if}
  {statusLabel[status]}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    color: var(--text-2);
    font-size: 0.8rem;
    white-space: nowrap;
  }
  .prefix {
    color: var(--text);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .s-perfect {
    --c: var(--status-perfect);
  }
  .s-playable {
    --c: var(--status-playable);
  }
  .s-in_progress {
    --c: var(--status-in-progress);
  }
  .s-boots {
    --c: var(--status-boots);
  }
  .s-not_playable {
    --c: var(--status-not-playable);
  }
</style>
