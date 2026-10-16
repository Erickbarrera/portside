<script lang="ts">
  // Portside's on-screen keyboard for couch mode (docs/ui.md). Opens when a text field is
  // pressed with a controller (lib/controller.ts sends `portside:keyboard`) and types into it.
  // A controller can't leave it until Done or B.
  import { tick } from "svelte";
  import { type Field, ROWS, backspace, typeText } from "$lib/osk";

  type TextField = HTMLInputElement | HTMLTextAreaElement;

  let target = $state<TextField | null>(null);
  let preview = $state("");
  let shift = $state(false);
  let panel = $state<HTMLElement | null>(null);

  const label = $derived(
    target ? (target.getAttribute("aria-label") ?? target.placeholder ?? "Text") : "",
  );

  $effect(() => {
    const open = (e: Event) => {
      const field = (e as CustomEvent<TextField>).detail;
      target = field;
      preview = field.value;
      shift = false;
      void tick().then(() => panel?.querySelector<HTMLButtonElement>("button")?.focus());
    };
    window.addEventListener("portside:keyboard", open);
    return () => window.removeEventListener("portside:keyboard", open);
  });

  function read(field: TextField): Field {
    const end = field.value.length;
    return { value: field.value, start: field.selectionStart ?? end, end: field.selectionEnd ?? end };
  }

  /** Writes to the field the way typing would, so bound values and search filters follow. */
  function write(next: Field) {
    if (!target) return;
    target.value = next.value;
    target.setSelectionRange(next.start, next.end);
    target.dispatchEvent(new Event("input", { bubbles: true }));
    preview = next.value;
  }

  function key(ch: string) {
    if (!target) return;
    write(typeText(read(target), shift ? ch.toUpperCase() : ch));
    shift = false;
  }

  function close() {
    const field = target;
    target = null;
    field?.focus();
  }

  function enter() {
    const form = target?.form;
    close();
    form?.requestSubmit();
  }
</script>

{#if target}
  <div
    class="osk"
    role="dialog"
    aria-label="On-screen keyboard"
    data-nav-scope
    bind:this={panel}
    oncancel={close}
  >
    <p class="preview"><span class="muted">{label}</span> {preview}<span class="caret" aria-hidden="true"></span></p>
    {#each ROWS as row (row)}
      <div class="row">
        {#each row.split("") as ch (ch)}
          <button class="k" onclick={() => key(ch)}>{shift ? ch.toUpperCase() : ch}</button>
        {/each}
      </div>
    {/each}
    <div class="row">
      <button class="k wide" aria-pressed={shift} onclick={() => (shift = !shift)}>Shift</button>
      <button class="k space" onclick={() => key(" ")}>Space</button>
      <button class="k wide" onclick={() => target && write(backspace(read(target)))}>Delete</button>
      {#if target.form}
        <button class="k wide" onclick={enter}>Enter</button>
      {/if}
      <button class="k wide done" onclick={close}>Done</button>
    </div>
  </div>
{/if}

<style>
  .osk {
    position: fixed;
    left: 50%;
    bottom: 0;
    transform: translateX(-50%);
    z-index: 50;
    width: min(100%, 900px);
    padding: 16px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-bottom: none;
    border-radius: var(--radius-m) var(--radius-m) 0 0;
    box-shadow: 0 -8px 32px rgb(0 0 0 / 50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .preview {
    margin: 0 0 4px;
    padding: 8px 12px;
    background: var(--bg);
    border-radius: var(--radius-s);
    min-height: 2.5em;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .preview .muted {
    margin-right: 8px;
  }
  .caret {
    display: inline-block;
    width: 2px;
    height: 1.1em;
    vertical-align: text-bottom;
    background: var(--accent);
  }
  .row {
    display: flex;
    gap: 6px;
    justify-content: center;
  }
  .k {
    flex: 1;
    min-width: 0;
    padding: 12px 0;
    border-radius: var(--radius-s);
    border: 1px solid var(--border);
    background: var(--surface-3);
    font-weight: 500;
    cursor: pointer;
  }
  .k:hover {
    border-color: var(--border-strong);
  }
  .wide {
    flex: 2;
  }
  .space {
    flex: 5;
  }
  .k[aria-pressed="true"],
  .done {
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
