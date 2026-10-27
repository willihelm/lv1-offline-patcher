<script lang="ts">
  // Patch entries of one port as colored chips; device to device sources also show the mixer channels they feed.
  import type { ListEntry } from "./grid";

  let { entries, empty = "–" }: { entries: ListEntry[]; empty?: string } = $props();
</script>

{#each entries as e}
  <span class="entry" style:--c={e.color ?? "transparent"} class:colored={e.color}>{e.label}</span>
  {#if e.channels?.length}
    <span class="via" title="Device to device: this device input feeds these mixer channels">→</span>
    {#each e.channels as c}
      <span class="entry colored" style:--c={c.color ?? "transparent"}>{c.label}</span>
    {/each}
  {/if}
{:else}
  <span class="dim">{empty}</span>
{/each}

<style>
  .entry {
    display: inline-block;
    margin: 1px 6px 1px 0;
    padding: 1px 8px;
    border-radius: 4px;
    background: #333;
  }
  .entry.colored {
    border-left: 4px solid var(--c);
  }
  .via {
    margin-right: 6px;
    color: var(--text-dim);
  }
  .dim {
    color: var(--text-dim);
  }
</style>
