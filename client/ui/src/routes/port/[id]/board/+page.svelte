<script lang="ts">
  import Board from "$lib/components/Board.svelte";

  let { data } = $props();
</script>

<svelte:head><title>{data.title} discussions · Portside</title></svelte:head>

<a class="back" href={`/port/${encodeURIComponent(data.portId)}`}>← {data.title}</a>
<h1>{data.title} discussions</h1>
<Board portId={data.portId} />

<style>
  .back {
    display: inline-block;
    margin-bottom: 12px;
    color: var(--text-2);
    text-decoration: none;
  }
</style>
