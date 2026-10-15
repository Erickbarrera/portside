<script lang="ts">
  // Shown on social pages when nobody is signed in.
  let { what }: { what: string } = $props();
</script>

<div class="notice">
  <p><strong>Sign in with Bluesky {what}.</strong></p>
  <p class="muted">
    Portside uses your Bluesky account (or any AT Protocol account) for friends, messages and
    discussions. Nothing is stored on a Portside server.
  </p>
  <a class="btn btn-primary" href="/signin">Sign in</a>
</div>

<style>
  .notice {
    max-width: 560px;
    padding: 24px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
  }
</style>
