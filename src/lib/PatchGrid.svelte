<script lang="ts">
  import type { Snippet } from "svelte";
  import { cellOp, diagonal, type Grid } from "./grid";
  import Knob from "./Knob.svelte";
  import { TAP_LABELS, type PatchOp } from "./types";

  let {
    grid,
    cell,
    selected = $bindable(new Set<number>()),
    hover = $bindable(null),
    onpatch,
    ontap,
    onpreamp,
    corner,
  }: {
    grid: Grid;
    cell: number;
    selected?: Set<number>;
    hover?: { r: number; c: number } | null;
    onpatch: (ops: PatchOp[]) => void;
    ontap: (cluster: number, index: number, tap: number) => void;
    /** null leaves the value unchanged */
    onpreamp: (device: number, channel: number, gain: number | null, phantom: boolean | null) => void;
    /** Rendered in the empty top-left corner above the row titles. */
    corner?: Snippet;
  } = $props();

  const hasTaps = $derived(grid.rows.some((row) => row.heading === undefined && row.tap));

  /** column whose gain is being typed in */
  let editing = $state<number | null>(null);

  function commitGain(c: number, input: HTMLInputElement) {
    if (editing !== c) return;
    editing = null;
    const gain = parseFloat(input.value.replace(",", "."));
    const p = grid.cols[c]?.preamp;
    if (grid.preamps && p && Number.isFinite(gain) && gain !== p.gain) onpreamp(grid.preamps.device, p.channel, gain, null);
  }

  function gainKey(c: number, e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      commitGain(c, e.currentTarget as HTMLInputElement);
      // Tab moves on to the next input with a gain control
      if (e.key === "Tab") {
        const step = e.shiftKey ? -1 : 1;
        let n = c + step;
        while (n >= 0 && n < grid.cols.length && grid.cols[n].preamp?.gain == null) n += step;
        if (n >= 0 && n < grid.cols.length) editing = n;
      }
    } else if (e.key === "Escape") {
      editing = null;
    }
  }

  const focusSelect = (el: HTMLInputElement) => {
    el.focus();
    el.select();
  };

  let matrix: HTMLDivElement;
  let drag = $state<{ r0: number; c0: number; r1: number; c1: number; erase: boolean } | null>(null);
  let anchor = -1;

  const preview = $derived(drag ? diagonal(grid, drag.r0, drag.c0, drag.r1, drag.c1) : []);
  const patchedCells = $derived(
    [...grid.patched].map((k) => k.split(":").map(Number) as [number, number]),
  );
  const fontSize = $derived(Math.max(10, Math.min(15, cell * 0.45)));

  /** Consecutive columns sharing the same key, merged into one header cell like in LV1. */
  function spans(keyOf: (c: number) => string) {
    const out: { text: string; n: number }[] = [];
    let prev: string | null = null;
    grid.cols.forEach((_, c) => {
      const k = keyOf(c);
      if (k === prev) out[out.length - 1].n++;
      else out.push({ text: k.split("\u0000").pop()!, n: 1 });
      prev = k;
    });
    return out;
  }
  const majors = $derived(spans((c) => grid.cols[c].port.major));
  const minors = $derived(spans((c) => `${grid.cols[c].port.major}\u0000${grid.cols[c].port.minor}`));

  function cellAt(e: PointerEvent) {
    const rect = matrix.getBoundingClientRect();
    const c = Math.floor((e.clientX - rect.left) / cell);
    const r = Math.floor((e.clientY - rect.top) / cell);
    if (r < 0 || c < 0 || r >= grid.rows.length || c >= grid.cols.length) return null;
    return { r, c };
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    const p = cellAt(e);
    if (!p || grid.rows[p.r].heading !== undefined) return;
    matrix.setPointerCapture(e.pointerId);
    drag = { r0: p.r, c0: p.c, r1: p.r, c1: p.c, erase: grid.patched.has(`${p.r}:${p.c}`) };
  }

  function move(e: PointerEvent) {
    const p = cellAt(e);
    hover = p;
    if (drag && p) drag = { ...drag, r1: p.r, c1: p.c };
  }

  function up() {
    if (!drag) return;
    const { erase } = drag;
    const ops = preview
      // erase mode only touches patched cells, patch mode only empty ones
      .filter(([r, c]) => grid.patched.has(`${r}:${c}`) === erase)
      .map(([r, c]) => cellOp(grid, r, c, !erase))
      .filter((op): op is PatchOp => op !== null);
    drag = null;
    if (ops.length) onpatch(ops);
  }

  function selectRow(r: number, e: MouseEvent) {
    const next = new Set(e.metaKey || e.ctrlKey ? selected : []);
    if (e.shiftKey && anchor >= 0) {
      for (let i = Math.min(anchor, r); i <= Math.max(anchor, r); i++)
        if (grid.rows[i].heading === undefined) next.add(i);
    } else if (next.has(r)) {
      next.delete(r);
    } else {
      next.add(r);
      anchor = r;
    }
    selected = next;
  }
</script>

<div
  class="scroller"
  style:--cell="{cell}px"
  style:--font="{fontSize}px"
  style:--rows={grid.rows.length}
  style:--cols={grid.cols.length}
  style:--pre-h={grid.preamps ? `calc(30px + ${cell}px)` : "0px"}
>
  <div class="canvas">
    <div class="corner">
      {@render corner?.()}
      {#if grid.preamps}
        <div class="prelabels">
          <div class="prelabel">48V</div>
          <div class="prelabel knoblabel">Gain</div>
        </div>
      {/if}
      <div class="corner-titles">
        <span class="corner-title">{grid.rowTitle}</span>
        {#if hasTaps}<span class="tapcol">Source</span>{/if}
        <span class="ms">{grid.rowTitle === "Channel Name" ? "M>S" : ""}</span>
      </div>
    </div>

    <div class="colhead">
      <div class="title">{grid.title}</div>
      <div class="groups">
        {#each majors as g}<div class="group" style:width="calc({g.n} * var(--cell))">{g.text}</div>{/each}
      </div>
      <div class="groups">
        {#each minors as g}<div class="group" style:width="calc({g.n} * var(--cell))">{g.text}</div>{/each}
      </div>
      {#if grid.preamps}
        {@const { device, gainRange } = grid.preamps}
        <div class="prerow phantoms">
          {#each grid.cols as col}
            {@const p = col.preamp}
            {#if p?.phantom != null}
              <button
                class="phantom"
                class:on={p.phantom}
                title="48V phantom power {p.phantom ? 'on' : 'off'}"
                onclick={() => onpreamp(device, p.channel, null, !p.phantom)}
              >48V</button>
            {:else}
              <div class="precell"></div>
            {/if}
          {/each}
        </div>
        <div class="prerow knobs">
          {#each grid.cols as col, c}
            {@const p = col.preamp}
            {#if p?.gain != null && gainRange && editing === c}
              <input
                class="gain-input"
                type="number"
                step={gainRange.step}
                min={gainRange.min}
                max={gainRange.max}
                value={p.gain}
                use:focusSelect
                onkeydown={(e) => gainKey(c, e)}
                onblur={(e) => commitGain(c, e.currentTarget)}
              />
            {:else if p?.gain != null}
              <div class="gain">
                <Knob
                  value={p.gain}
                  min={gainRange?.min ?? 0}
                  max={gainRange?.max ?? Math.max(p.gain, 1)}
                  step={gainRange?.step}
                  disabled={!gainRange}
                  title={gainRange
                    ? `Gain ${p.gain} dB (${gainRange.min} to ${gainRange.max} dB). Drag or scroll to turn, Shift for fine, double-click to type`
                    : `Gain ${p.gain} dB (range of this device unknown, not editable)`}
                  onchange={(gain) => onpreamp(device, p.channel, gain, null)}
                  onedit={() => (editing = c)}
                />
              </div>
            {:else}
              <div class="precell"></div>
            {/if}
          {/each}
        </div>
      {/if}
      <div class="numbers">
        {#each grid.cols as col, c}
          <div
            class="num"
            class:hot={hover?.c === c}
            class:occupied={col.note}
            title={col.note ? `In use: ${col.note}` : undefined}
          >
            {col.port.label}
          </div>
        {/each}
      </div>
    </div>

    <div class="rowhead">
      {#each grid.rows as row, r}
        {#if row.heading !== undefined}
          <div class="heading">{row.heading}</div>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="label"
            class:hot={hover?.r === r}
            class:sel={selected.has(r)}
            role="option"
            tabindex="-1"
            aria-selected={selected.has(r)}
            onclick={(e) => selectRow(r, e)}
          >
            <span class="swatch" style:background={row.color}>{row.number}</span>
            <span class="name">{row.label}</span>
            {#if row.kind}<span class="kind">{row.kind}</span>{/if}
            {#if row.note}
              {@const [, head, tail] = row.note.match(/^(.*?)(\s*\S+)$/) ?? ["", row.note, ""]}
              <span class="note" title="Patched from {row.note}"><span class="note-head">{head}</span><span class="note-tail">{tail}</span></span>
            {/if}
            {#if row.tap}
              {@const tap = row.tap}
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
              <span class="tapcol" onclick={(e) => e.stopPropagation()}>
                <select
                  class="tap tap{tap.value}"
                  value={tap.value}
                  onchange={(e) => ontap(tap.cluster, tap.index, Number(e.currentTarget.value))}
                  title="Output source"
                >
                  {#each TAP_LABELS as label, v}<option value={v}>{label}</option>{/each}
                </select>
              </span>
            {/if}
            <span class="side">{row.side}</span>
          </div>
        {/if}
      {/each}
    </div>

    <div
      class="matrix"
      bind:this={matrix}
      role="grid"
      tabindex="-1"
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={() => (drag = null)}
      onpointerleave={() => !drag && (hover = null)}
    >
      {#each grid.rows as row, r}
        {#if row.heading !== undefined}
          <div class="band heading-band" style:top="{r * cell}px"></div>
        {:else if selected.has(r)}
          <div class="band sel-band" style:top="{r * cell}px"></div>
        {/if}
      {/each}
      {#if hover && !drag}
        <div class="band hover-row" style:top="{hover.r * cell}px"></div>
        <div class="hover-col" style:left="{hover.c * cell}px"></div>
      {/if}
      {#each patchedCells as [r, c] (`${r}:${c}`)}
        <div class="cell on" style:top="{r * cell}px" style:left="{c * cell}px"></div>
      {/each}
      {#each preview as [r, c]}
        <div
          class="cell preview"
          class:erase={drag?.erase}
          style:top="{r * cell}px"
          style:left="{c * cell}px"
        ></div>
      {/each}
      {#if hover && !drag}
        <div class="cell cursor" style:top="{hover.r * cell}px" style:left="{hover.c * cell}px"></div>
      {/if}
    </div>
  </div>
</div>

<style>
  .scroller {
    --label-w: 330px;
    --head-h: calc(112px + var(--pre-h));
    overflow: auto;
    height: 100%;
    background: var(--panel);
    border-radius: 6px;
  }
  .canvas {
    display: grid;
    grid-template-columns: var(--label-w) calc(var(--cols) * var(--cell));
    grid-template-rows: var(--head-h) calc(var(--rows) * var(--cell));
    width: max-content;
  }
  .corner {
    position: sticky;
    top: 0;
    left: 0;
    z-index: 3;
    background: var(--panel);
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }
  .corner-titles {
    display: flex;
    align-items: flex-end;
    flex: none;
    /* the corner is exactly as high as the tools (84px) plus this */
    height: 28px;
    box-sizing: border-box;
    padding: 0 0 8px 40px;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-dim);
  }
  .colhead {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--panel);
  }
  .title {
    height: 30px;
    box-sizing: border-box;
    line-height: 30px;
    padding-left: 8px;
    font-weight: 700;
    background: #3a3a3d;
    border-bottom: 1px solid #222;
    position: sticky;
    left: var(--label-w);
    white-space: nowrap;
  }
  .groups {
    display: flex;
    height: 24px;
  }
  .group {
    flex: none;
    box-sizing: border-box;
    padding-left: 6px;
    line-height: 24px;
    font-size: 12px;
    font-weight: 600;
    background: #3a3a3d;
    border-right: 1px solid #222;
    border-bottom: 1px solid #222;
    white-space: nowrap;
    overflow: hidden;
  }
  .numbers {
    display: flex;
    height: 34px;
  }
  .prelabels {
    flex: none;
    /* the titles (28px) are lower than the column numbers (34px): line the labels up with their rows */
    margin-bottom: 6px;
  }
  .prelabel {
    user-select: none;
    -webkit-user-select: none;
    height: 24px;
    line-height: 24px;
    padding-right: 10px;
    text-align: right;
    font-size: 11px;
    font-weight: 700;
    color: var(--text-dim);
  }
  .prerow {
    display: flex;
    height: 24px;
    user-select: none;
    -webkit-user-select: none;
  }
  /* the corner needs a bit more height than the two rows for the tools above their labels */
  .prerow.phantoms {
    margin-top: 6px;
  }
  .precell,
  .phantom,
  .gain,
  .gain-input {
    width: var(--cell);
    height: 24px;
    flex: none;
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    border: 0;
    border-right: 1px solid #222;
    border-bottom: 1px solid #222;
    border-radius: 0;
    background: #2a2a2c;
    font: inherit;
    font-size: min(11px, calc(var(--cell) * 0.34));
    font-weight: 700;
    text-align: center;
  }
  .phantom {
    color: #6a6a6e;
    cursor: pointer;
  }
  .phantom.on {
    background: #c62828;
    color: #fff;
  }
  .prerow.knobs,
  .knoblabel {
    height: var(--cell);
  }
  .knoblabel {
    line-height: var(--cell);
  }
  .knobs .precell,
  .knobs .gain,
  .knobs .gain-input {
    height: var(--cell);
  }
  .phantom:hover {
    border-color: #222;
    outline: 1px solid #aaa;
    outline-offset: -1px;
  }
  .gain-input {
    /* wide enough to type in at small zoom levels */
    width: max(var(--cell), 44px);
    position: relative;
    z-index: 1;
    margin-right: min(0px, calc(var(--cell) - 44px));
    background: #111;
    color: var(--text);
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    appearance: textfield;
    -moz-appearance: textfield;
  }
  .gain-input::-webkit-inner-spin-button,
  .gain-input::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  .num {
    width: var(--cell);
    box-sizing: border-box;
    flex: none;
    display: grid;
    place-items: center;
    font-size: var(--font);
    font-weight: 600;
    background: #333336;
    border-right: 1px solid #222;
  }
  .num.hot {
    background: #5a5a5e;
  }
  .num.occupied {
    color: #f5a623;
  }
  .rowhead {
    position: sticky;
    left: 0;
    z-index: 1;
    background: var(--panel);
  }
  .heading,
  .label {
    height: var(--cell);
    box-sizing: border-box;
    display: flex;
    align-items: center;
    font-size: var(--font);
  }
  .heading {
    padding-left: 10px;
    font-weight: 700;
    color: var(--accent);
    background: #1d1d1f;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    font-size: 11px;
  }
  .label {
    background: #3a3a3d;
    border-bottom: 1px solid #222;
    cursor: pointer;
    user-select: none;
  }
  .label.hot {
    background: #4a4a4e;
  }
  .label.sel {
    background: #2d5d73;
  }
  .swatch {
    width: 32px;
    align-self: stretch;
    flex: none;
    display: grid;
    place-items: center;
    font-size: 10px;
    font-weight: 700;
    color: rgb(0 0 0 / 0.6);
  }
  .name {
    flex: 1;
    padding-left: 8px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kind {
    font-size: 12px;
    font-weight: 600;
    white-space: pre;
    padding: 0 8px;
    color: var(--text-dim);
  }
  .note {
    display: flex;
    min-width: 0;
    font-size: 11px;
    color: #f5a623;
    max-width: 110px;
    white-space: pre;
    padding: 0 6px;
  }
  .note-head {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note-tail {
    flex: none;
  }
  .corner-title {
    flex: 1;
  }
  .corner .ms {
    width: 40px;
    text-align: center;
  }
  .tapcol {
    width: 76px;
    flex: none;
    text-align: center;
    align-self: stretch;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }
  .label .tapcol {
    align-items: stretch;
    border-left: 1px solid #222;
  }
  .tap {
    appearance: none;
    width: 100%;
    border: none;
    border-radius: 0;
    font: inherit;
    font-weight: 700;
    font-size: 12px;
    text-align: center;
    text-align-last: center;
    color: #fff;
    cursor: pointer;
  }
  /* INP, PRE, PST, PSP */
  .tap0 {
    background: #2e7d32;
  }
  .tap1 {
    background: #6a2cd8;
  }
  .tap2 {
    background: #f58220;
  }
  .tap3 {
    background: #c62828;
  }
  .side {
    width: 40px;
    flex: none;
    text-align: center;
    font-weight: 600;
    border-left: 1px solid #222;
    align-self: stretch;
    display: grid;
    place-items: center;
  }
  .matrix {
    position: relative;
    background-color: #141415;
    background-image:
      linear-gradient(to right, #2c2c2e 1px, transparent 1px),
      linear-gradient(to bottom, #2c2c2e 1px, transparent 1px);
    background-size: var(--cell) var(--cell);
    cursor: crosshair;
    touch-action: none;
  }
  .band {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--cell);
    pointer-events: none;
  }
  .heading-band {
    background: #1d1d1f;
  }
  .sel-band {
    background: rgb(56 200 240 / 0.08);
  }
  .hover-row {
    background: rgb(255 255 255 / 0.1);
  }
  .hover-col {
    position: absolute;
    top: 0;
    bottom: 0;
    width: var(--cell);
    background: rgb(255 255 255 / 0.1);
    pointer-events: none;
  }
  .cell {
    position: absolute;
    width: calc(var(--cell) - 1px);
    height: calc(var(--cell) - 1px);
    margin: 1px 0 0 1px;
    pointer-events: none;
  }
  .cell.on {
    background: var(--accent);
  }
  .cell.preview {
    background: rgb(56 200 240 / 0.45);
    outline: 1px solid var(--accent);
  }
  .cell.preview.erase {
    background: rgb(229 57 53 / 0.5);
    outline-color: #e53935;
  }
  .cell.cursor {
    outline: 2px solid #ddd;
    outline-offset: -2px;
  }
</style>
