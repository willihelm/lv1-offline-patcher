<script lang="ts">
  // Read-only list of everything patched on one device: inputs with what they feed, outputs with their source.
  import type { ListRow } from "./grid";
  import PatchEntries from "./PatchEntries.svelte";

  let { inputs, outputs, part }: { inputs: ListRow[]; outputs: ListRow[]; part: "all" | "inputs" | "outputs" } = $props();

  const sections = $derived(
    [
      { title: "Inputs", rows: inputs, preamps: inputs.some((r) => r.preamp), show: part !== "outputs", patch: "Patched to" },
      { title: "Outputs", rows: outputs, preamps: false, show: part !== "inputs", patch: "Source" },
    ].filter((s) => s.show && s.rows.length),
  );
</script>

<div class="list">
  {#each sections as s (s.title)}
    <h3>
      {s.title}
      <span>{s.rows.filter((r) => r.patch.length).length} of {s.rows.length} patched</span>
    </h3>
    <table>
      <thead>
        <tr>
          <th class="num">Ch</th>
          <th>Type</th>
          <th>Port</th>
          {#if s.preamps}<th class="num">Gain</th><th>48V</th>{/if}
          <th class="patch">{s.patch}</th>
        </tr>
      </thead>
      <tbody>
        {#each s.rows as r (r.channel)}
          <tr class:unpatched={!r.patch.length}>
            <td class="num">{r.channel}</td>
            <td class="dim">{r.type}</td>
            <td>{r.port}</td>
            {#if s.preamps}
              <td class="num">{r.preamp?.gain == null ? "" : `${r.preamp.gain} dB`}</td>
              <td>{#if r.preamp?.phantom}<span class="phantom">48V</span>{/if}</td>
            {/if}
            <td class="patch">
              <PatchEntries entries={r.patch} />
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <div class="empty">Nothing to list.</div>
  {/each}
</div>

<style>
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 12px 12px;
  }
  h3 {
    margin: 16px 0 4px;
    font-size: 16px;
  }
  h3 span {
    margin-left: 10px;
    font-size: 12px;
    font-weight: 400;
    color: var(--text-dim);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  thead th {
    position: sticky;
    top: 0;
    background: var(--panel);
    border-bottom: 1px solid #444;
    text-align: left;
    font-weight: 600;
    color: var(--text-dim);
    padding: 6px 8px;
    white-space: nowrap;
  }
  td {
    padding: 5px 8px;
    border-bottom: 1px solid #333;
    white-space: nowrap;
  }
  tbody tr:hover {
    background: #2f2f32;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    width: 1%;
  }
  .patch {
    width: 100%;
    white-space: normal;
  }
  .dim,
  .unpatched td {
    color: var(--text-dim);
  }
  .phantom {
    font-size: 11px;
    font-weight: 700;
    color: #fff;
    background: #c62828;
    border-radius: 3px;
    padding: 1px 4px;
  }
  .empty {
    padding: 24px;
    color: var(--text-dim);
  }
</style>
