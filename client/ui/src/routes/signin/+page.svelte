<script lang="ts">
  import { goto } from "$app/navigation";
  import { auth } from "$lib/account.svelte";
  import type { CommandError } from "$lib/ipc";

  let handle = $state("");
  let error = $state<CommandError | null>(null);

  async function signIn(withHandle: string) {
    error = null;
    const failed = await auth.signIn(withHandle);
    if (!failed) await goto("/friends");
    else if (failed.code !== "E_CANCELLED") error = failed;
  }
</script>

<svelte:head><title>Sign in · Portside</title></svelte:head>

<div class="wrap">
  <h1>Sign in</h1>
  {#if auth.account}
    <p>You're signed in as <strong>@{auth.account.handle}</strong>.</p>
    <a class="btn btn-secondary" href="/friends">Go to Friends</a>
  {:else if auth.signingIn}
    <p role="status">
      <strong>Finish signing in in your browser.</strong> Approve Portside there, then come back
      here.
    </p>
    <button class="btn btn-secondary" onclick={() => auth.cancel()}>Cancel</button>
  {:else}
    <p class="muted">
      Portside uses your Bluesky account for friends, messages and game discussions. You sign in
      on Bluesky's own page; Portside never sees your password.
    </p>
    <button class="btn btn-primary" onclick={() => signIn("")}>Sign in with Bluesky</button>

    <form
      class="other"
      onsubmit={(e) => {
        e.preventDefault();
        if (handle.trim()) void signIn(handle);
      }}
    >
      <label for="handle">Account on another AT Protocol host</label>
      <div class="row">
        <input
          id="handle"
          type="text"
          autocomplete="username"
          autocapitalize="none"
          spellcheck="false"
          placeholder="name.example.com"
          bind:value={handle}
        />
        <button class="btn btn-secondary" disabled={!handle.trim()}>Continue</button>
      </div>
      <p class="muted small">Your handle works too, for example name.bsky.social.</p>
    </form>

    {#if error}<p class="error" role="alert">{error.message}</p>{/if}

    <p class="muted small">
      Portside asks to post, follow and use your direct messages. Discussion posts and follows are
      public on Bluesky. Messages are Bluesky direct messages and show in the Bluesky app too.
    </p>
    <a href="/library">Continue without signing in</a>
  {/if}
</div>

<style>
  .wrap {
    max-width: 520px;
  }
  .other {
    margin: 28px 0 16px;
    padding-top: 20px;
    border-top: 1px solid var(--border);
  }
  label {
    display: block;
    margin-bottom: 6px;
    font-weight: 500;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  input {
    flex: 1;
    padding: 9px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
  }
  .small {
    font-size: 0.875rem;
    margin-top: 6px;
  }
  .error {
    color: var(--status-boots);
  }
</style>
