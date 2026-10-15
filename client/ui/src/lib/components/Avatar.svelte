<script lang="ts">
  // A round avatar from Bluesky, or initials when there is none or it fails to load.
  let {
    src,
    name,
    size = 36,
  }: { src: string | null | undefined; name: string; size?: number } = $props();

  let failed = $state(false);
  const initials = $derived(
    name
      .replace(/^@/, "")
      .split(/[\s.]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase() ?? "")
      .join(""),
  );
</script>

{#if src && !failed}
  <img
    class="avatar"
    {src}
    alt=""
    width={size}
    height={size}
    style:width="{size}px"
    style:height="{size}px"
    onerror={() => (failed = true)}
  />
{:else}
  <span
    class="avatar initials"
    aria-hidden="true"
    style:width="{size}px"
    style:height="{size}px"
    style:font-size="{Math.round(size * 0.4)}px">{initials}</span
  >
{/if}

<style>
  .avatar {
    border-radius: 50%;
    flex: none;
    object-fit: cover;
    background: var(--surface-3);
  }
  .initials {
    display: inline-grid;
    place-items: center;
    color: var(--text-2);
    font-weight: 600;
  }
</style>
