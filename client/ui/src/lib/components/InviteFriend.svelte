<script lang="ts">
  // "Invite a friend" on a game page: pick a friend, send them a DM with the game's link.
  import { goto } from "$app/navigation";
  import { auth } from "$lib/account.svelte";
  import { personName } from "$lib/chat.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import { type CommandError, type Person, commands } from "$lib/ipc";

  let { portId, title }: { portId: string; title: string } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let friends = $state<Person[] | null>(null);
  let error = $state<CommandError | null>(null);
  let sending = $state<string | null>(null);
  let filter = $state("");

  const shown = $derived(
    (friends ?? []).filter((f) =>
      `${f.display_name ?? ""} ${f.handle}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );

  async function open() {
    error = null;
    dialog?.showModal();
    if (!friends) {
      const r = await commands.friendsList();
      if (r.status === "ok") friends = r.data;
      else error = r.error;
    }
  }

  async function invite(friend: Person) {
    sending = friend.did;
    error = null;
    const r = await commands.gameInvite(friend.did, portId);
    sending = null;
    if (r.status === "ok") {
      dialog?.close();
      await goto(`/chat/${encodeURIComponent(r.data.id)}`);
    } else {
      error = r.error;
    }
  }
</script>

{#if auth.account?.can_chat}
  <button class="btn btn-secondary" onclick={open}>Invite a friend</button>
{/if}

<dialog bind:this={dialog} aria-labelledby="invite-h">
  <h2 id="invite-h">Invite a friend to play {title}</h2>
  <p class="muted">
    Sends a Bluesky direct message with a link to the game. In Portside it shows as an invite
    with a Play button.
  </p>
  {#if error}<p class="error" role="alert">{error.message}</p>{/if}
  {#if friends === null && !error}
    <p class="muted">Loading friends…</p>
  {:else if friends && friends.length === 0}
    <p class="muted">
      No friends yet. Friends are people you follow on Bluesky who follow you back.
      <a href="/friends" onclick={() => dialog?.close()}>Find people</a>
    </p>
  {:else if friends}
    <label class="visually-hidden" for="invite-filter">Filter friends</label>
    <input id="invite-filter" type="search" placeholder="Filter friends" bind:value={filter} />
    <ul>
      {#each shown as friend (friend.did)}
        <li>
          <Avatar src={friend.avatar} name={personName(friend)} size={32} />
          <span class="name">{personName(friend)} <span class="muted">@{friend.handle}</span></span>
          <button class="btn btn-primary" disabled={sending !== null} onclick={() => invite(friend)}>
            {sending === friend.did ? "Sending…" : "Invite"}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <form method="dialog"><button class="btn btn-secondary">Close</button></form>
</dialog>

<style>
  dialog {
    width: min(520px, 90vw);
    max-height: 80vh;
    background: var(--surface-1);
    color: var(--text);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-m);
    padding: 24px;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.6);
  }
  h2 {
    font-size: 1.1rem;
  }
  input {
    width: 100%;
    padding: 8px 12px;
    margin-bottom: 8px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
  }
  ul {
    list-style: none;
    margin: 0 0 16px;
    padding: 0;
    max-height: 45vh;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error {
    color: var(--status-boots);
  }
</style>
