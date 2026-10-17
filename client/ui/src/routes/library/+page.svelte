<script lang="ts">
  import Cover from "$lib/components/Cover.svelte";
  import PlayButton from "$lib/components/PlayButton.svelte";
  import { byRecent, crashed, formatPlaytime, games } from "$lib/games.svelte";
  import { fraction, installs } from "$lib/installs.svelte";
  import { formatDate } from "$lib/labels";
  import { summaries as saved } from "$lib/summaries.svelte";

  $effect(() => {
    installs.start();
    games.start();
  });

  const day = (unix: number) => formatDate(new Date(unix * 1000).toISOString());

  type Order = "recent" | "title";
  const ORDER_KEY = "portside.libraryOrder";
  let order = $state<Order>(savedOrder());
  function savedOrder(): Order {
    try {
      return localStorage.getItem(ORDER_KEY) === "title" ? "title" : "recent";
    } catch {
      return "recent";
    }
  }
  function setOrder(value: string) {
    order = value === "title" ? "title" : "recent";
    try {
      localStorage.setItem(ORDER_KEY, order);
    } catch {
      // Private window or blocked storage: the order just isn't remembered.
    }
  }

  // A game playing now counts as played this moment.
  const lastPlayed = (id: string) =>
    id in games.running ? Number.MAX_SAFE_INTEGER : (games.stats[id]?.last_played ?? null);
  const list = $derived(
    Object.values(installs.installed).sort((a, b) =>
      order === "recent" ? byRecent(a, b, lastPlayed) : a.title.localeCompare(b.title),
    ),
  );
  // Installs still running for games that aren't installed yet.
  const pending = $derived(Object.keys(installs.active).filter((id) => !(id in installs.installed)));

  // Saved catalog summaries, for screenshots and game keys (works offline).
  const summaries = $derived(saved.byId);
  $effect(() => {
    saved.want([...list.map((g) => g.port_id), ...pending]);
  });

  const percent = (id: string) => {
    const p = fraction(installs.active[id]);
    return p === null ? "…" : ` · ${Math.round(p * 100)}%`;
  };
</script>

<svelte:head><title>Library · Portside</title></svelte:head>

<header class="top">
  <h1>Library</h1>
  {#if list.length > 1}
    <label>
      <span class="visually-hidden">Sort</span>
      <select value={order} onchange={(e) => setOrder(e.currentTarget.value)}>
        <option value="recent">Recently played</option>
        <option value="title">Title</option>
      </select>
    </label>
  {/if}
</header>

{#if installs.loaded && list.length === 0 && pending.length === 0}
  <div class="empty">
    <p><strong>Nothing installed yet.</strong></p>
    <p class="muted">Games you install show up here. Find one in the catalog and press Install.</p>
    <a class="btn btn-primary" href="/discover">Browse the catalog</a>
  </div>
{:else}
  <ul class="games">
    {#each pending as id (id)}
      {@const s = summaries[id]}
      <li>
        <a class="cover" href={`/port/${id}`} tabindex="-1" aria-hidden="true">
          <Cover url={s?.screenshot_url ?? null} gameKey={s?.game_key ?? id} title={s?.title ?? id} />
        </a>
        <div class="body">
          <a class="title" href={`/port/${id}`}>{s?.title ?? id}</a>
          <span class="muted small">Installing{percent(id)}</span>
        </div>
      </li>
    {/each}
    {#each list as game (game.port_id)}
      {@const s = summaries[game.port_id]}
      {@const updating = installs.active[game.port_id]}
      {@const played = games.stats[game.port_id]}
      <li>
        <a class="cover" href={`/port/${game.port_id}`} tabindex="-1" aria-hidden="true">
          <Cover url={s?.screenshot_url ?? null} gameKey={s?.game_key ?? game.port_id} title={game.title} />
        </a>
        <div class="body">
          <a class="title" href={`/port/${game.port_id}`}>{game.title}</a>
          <span class="muted small">
            {#if updating}
              Updating{percent(game.port_id)}
            {:else if game.port_id in games.running}
              Playing now · {game.version}
            {:else if crashed(games.ended[game.port_id])}
              <span class="warn">Quit with error code {games.ended[game.port_id]?.code}</span> · see its
              game page for the log
            {:else if played?.last_played && formatPlaytime(played.total_secs)}
              {formatPlaytime(played.total_secs)} played · last played {day(played.last_played)}
            {:else}
              Not played yet · installed {day(game.installed_at)}
            {/if}
          </span>
          {#if !updating}
            <div class="actions"><PlayButton portId={game.port_id} title={game.title} /></div>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 0.5em;
  }
  .top h1 {
    margin: 0;
  }
  select {
    height: 38px;
    padding: 0 36px 0 12px;
    background-color: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath d='M1 1.5 6 6.5l5-5' fill='none' stroke='%23aeb6c4' stroke-width='1.6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 12px center;
  }
  .empty {
    max-width: 520px;
    margin-top: 24px;
    padding: 28px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
  }
  .games {
    list-style: none;
    margin: 24px 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 16px;
  }
  .games li {
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    overflow: hidden;
  }
  .cover {
    display: block;
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 14px 14px;
    min-width: 0;
  }
  .small {
    font-size: 0.85rem;
  }
  .warn {
    color: var(--status-in-progress);
  }
  .title {
    font-weight: 600;
    text-decoration: none;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title:hover {
    color: var(--accent);
  }
  .actions {
    margin-top: auto;
    padding-top: 10px;
  }
</style>
