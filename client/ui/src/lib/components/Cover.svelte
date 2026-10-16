<script lang="ts">
  import { coverBackground, coverTitle } from "$lib/cover";

  // A game's screenshot, or a generated cover with its title when there is none or the
  // image doesn't load. Screenshots are linked from upstream, never copied (policy rule 6).
  let {
    url,
    gameKey,
    title,
    size = "card",
  }: { url: string | null; gameKey: string; title: string; size?: "card" | "hero" | "thumb" | "icon" } =
    $props();

  let broken = $state(false);
  $effect(() => {
    url;
    broken = false;
  });
  const showImage = $derived(!!url && !broken);
</script>

<div class="cover {size}" class:band={size === "hero" && !showImage} style:background={showImage ? undefined : coverBackground(gameKey)}>
  {#if showImage}
    <img src={url} alt={size === "icon" ? "" : `Screenshot of ${title}`} loading="lazy" onerror={() => (broken = true)} />
  {:else if size === "card"}
    <span class="name" aria-hidden="true">{coverTitle(title)}</span>
  {/if}
</div>

<style>
  .cover {
    position: relative;
    overflow: hidden;
    background-color: var(--surface-2);
    display: flex;
    align-items: flex-end;
  }
  .card {
    aspect-ratio: 16 / 9;
  }
  .hero {
    aspect-ratio: 16 / 9;
    border-radius: var(--radius-m);
    border: 1px solid var(--border);
  }
  .thumb {
    width: 96px;
    aspect-ratio: 16 / 9;
    border-radius: var(--radius-s);
    flex-shrink: 0;
  }
  /* The sidebar's installed list: a small square crop. */
  .icon {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    flex-shrink: 0;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .name {
    padding: 12px 14px;
    font-weight: 700;
    line-height: 1.15;
    color: rgb(255 255 255 / 0.92);
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.4);
    overflow-wrap: anywhere;
  }
  .card .name {
    font-size: 1.05rem;
  }
  /* No screenshot on the game page: a strip of the generated colors under the title. */
  .hero.band {
    aspect-ratio: auto;
    height: 96px;
  }
</style>
