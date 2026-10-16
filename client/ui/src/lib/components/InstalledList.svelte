<script lang="ts">
  import { page } from "$app/state";
  import Cover from "$lib/components/Cover.svelte";
  import { games } from "$lib/games.svelte";
  import { searchGames, sidebarGames } from "$lib/installed-list";
  import { fraction, installs } from "$lib/installs.svelte";
  import { summaries } from "$lib/summaries.svelte";

  // Steam-style list of installed games in the sidebar: one click to a game's page from
  // anywhere in the app, with what each game is doing right now.
  $effect(() => {
    installs.start();
    games.start();
  });

  let query = $state("");
  const all = $derived(
    sidebarGames(installs.installed, installs.active, games.running, summaries.byId),
  );
  const shown = $derived(searchGames(all, query));
  $effect(() => {
    summaries.want(all.map((g) => g.id));
  });

  /** "38%" while downloading; null while unpacking or when the size is unknown. */
  const percent = (id: string) => {
    const p = installs.active[id] ? fraction(installs.active[id]) : null;
    return p === null ? null : `${Math.round(p * 100)}%`;
  };
  const current = (id: string) => page.url.pathname.startsWith(`/port/${id}`);
</script>

<section class="installed" aria-labelledby="installed-heading">
  <h2 id="installed-heading">
    Installed
    {#if all.length > 0}<span class="count">{all.length}</span>{/if}
  </h2>
  {#if !installs.loaded}
    <!-- Nothing yet: the list appears once the library is read. -->
  {:else if all.length === 0}
    <p class="empty">Games you install show up here.</p>
  {:else}
    <input
      type="search"
      placeholder="Search installed"
      aria-label="Search installed games"
      bind:value={query}
    />
    {#if shown.length === 0}
      <p class="empty">No installed game matches.</p>
    {:else}
      <ul>
        {#each shown as game (game.id)}
          {@const s = summaries.byId[game.id]}
          <li>
            <a
              href={`/port/${game.id}`}
              aria-current={current(game.id) ? "page" : undefined}
              title={game.title}
              class:dim={game.activity === "installing"}
            >
              <Cover url={s?.screenshot_url ?? null} gameKey={s?.game_key ?? game.id} title={game.title} size="icon" />
              <span class="name">{game.name}</span>
              {#if game.activity === "playing"}
                <span class="tag playing">Playing</span>
              {:else if game.activity}
                <!-- Just the percentage where it fits; the word is there for screen readers. -->
                {@const word = game.activity === "installing" ? "Installing" : "Updating"}
                {@const pct = percent(game.id)}
                <span class="tag">
                  {#if pct}<span class="visually-hidden">{word}</span> {pct}{:else}{word}{/if}
                </span>
              {/if}
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  .installed {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 0 12px;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .count {
    font-weight: 500;
    color: var(--text-4);
  }
  input {
    margin: 0 4px;
    padding: 6px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
    font-size: 0.85rem;
  }
  input::placeholder {
    color: var(--text-4);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  a {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-radius: var(--radius-s);
    color: var(--text-2);
    text-decoration: none;
    font-size: 0.9rem;
  }
  a:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  a[aria-current="page"] {
    background: var(--surface-3);
    color: var(--text);
  }
  a.dim .name {
    color: var(--text-4);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    flex-shrink: 0;
    font-size: 0.75rem;
    color: var(--text-3);
  }
  .tag.playing {
    color: var(--status-perfect);
  }
  .empty {
    margin: 0;
    padding: 0 12px;
    font-size: 0.85rem;
    color: var(--text-4);
  }
</style>
