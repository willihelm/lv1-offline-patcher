<script lang="ts">
  // Copy the patch of one device to another: choose the devices and inputs/outputs, then check
  // the preview channel by channel and leave out what should stay as it is.
  import { onMount, untrack } from "svelte";
  import { copyOps, copyPlan, type CopyItem, type CopyPart } from "./grid";
  import PatchEntries from "./PatchEntries.svelte";
  import type { Device, PatchOp, ShowState } from "./types";

  let {
    show,
    to: initialTo,
    onapply,
    onclose,
  }: {
    show: ShowState;
    to: Device;
    onapply: (ops: PatchOp[], from: Device, to: Device) => void;
    onclose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());

  const canCopy = (from: Device, to: Device, part: CopyPart) =>
    part === "inputs" ? from.ins > 0 && to.ins > 0 : from.outs > 0 && to.outs > 0;
  const PARTS: [CopyPart, string][] = [
    ["inputs", "Inputs"],
    ["outputs", "Outputs"],
  ];
  const label = (d: Device) => `${d.idx + 1}. ${d.name}`;

  let toIdx = $state(untrack(() => initialTo.idx));
  const to = $derived(show.devices.find((d) => d.idx === toIdx) ?? initialTo);
  const sources = $derived(show.devices.filter((d) => d.idx !== to.idx && PARTS.some(([p]) => canCopy(d, to, p))));
  let fromIdx = $state(-1);
  const from = $derived(sources.find((d) => d.idx === fromIdx) ?? sources[0]);
  let wanted = $state<Record<CopyPart, boolean>>({ inputs: true, outputs: true });
  const parts = $derived(PARTS.map(([p]) => p).filter((p) => wanted[p] && from && canCopy(from, to, p)));
  let showUnchanged = $state(false);
  /** items left out, by itemKey; reset when the devices change */
  let excluded = $state(new Set<string>());
  $effect(() => {
    void from?.idx, to.idx;
    excluded = new Set();
    anchor = null;
  });

  const itemKey = (i: CopyItem) => `${i.part}:${i.channel}`;
  const items = $derived(from ? copyPlan(show, from, to, parts) : []);
  const changes = (i: CopyItem) => i.missing || copyOps(show, [i]).length > 0;
  const visible = $derived(showUnchanged ? items : items.filter(changes));
  const chosen = $derived(items.filter((i) => !i.missing && !excluded.has(itemKey(i))));
  const ops = $derived(copyOps(show, chosen));
  const hidden = $derived(items.length - visible.length);

  /** last clicked item: start of a Shift-click range, marked in the list */
  let anchor = $state<string | null>(null);
  let hovered = $state<string | null>(null);
  let shift = $state(false);

  /** items from the anchor to `i`, in list order */
  function rangeTo(i: CopyItem) {
    const from = visible.findIndex((v) => itemKey(v) === anchor);
    const at = visible.indexOf(i);
    return from < 0 ? [i] : visible.slice(Math.min(from, at), Math.max(from, at) + 1);
  }
  /** what a Shift-click on the hovered row would change */
  const preview = $derived.by(() => {
    const i = shift && hovered ? visible.find((v) => itemKey(v) === hovered) : undefined;
    return new Set(i ? rangeTo(i).map(itemKey) : []);
  });

  /** check or uncheck an item; with Shift the whole range from the anchor, which takes the anchor's state */
  function toggle(e: MouseEvent, i: CopyItem) {
    const a = visible.find((v) => itemKey(v) === anchor);
    const range = e.shiftKey && a ? rangeTo(i) : [i];
    const on = e.shiftKey && a ? !excluded.has(itemKey(a)) : excluded.has(itemKey(i));
    const next = new Set(excluded);
    for (const r of range) if (!r.missing) on ? next.delete(itemKey(r)) : next.add(itemKey(r));
    excluded = next;
    if (!e.shiftKey || !a) anchor = itemKey(i);
  }

  function toggleAll(part: CopyPart, on: boolean) {
    const next = new Set(excluded);
    for (const i of visible) if (i.part === part) on ? next.delete(itemKey(i)) : next.add(itemKey(i));
    excluded = next;
  }

  const sectionOf = (part: CopyPart) => visible.filter((i) => i.part === part);
  const allOn = (list: CopyItem[]) => list.filter((i) => !i.missing).every((i) => !excluded.has(itemKey(i)));
  const noneOn = (list: CopyItem[]) => list.filter((i) => !i.missing).every((i) => excluded.has(itemKey(i)));

  function apply() {
    if (!from || !ops.length) return;
    onapply(ops, from, to);
    dialog.close();
  }
</script>

<svelte:window onkeydown={(e) => (shift = e.shiftKey)} onkeyup={(e) => (shift = e.shiftKey)} onblur={() => (shift = false)} />

<dialog bind:this={dialog} onclose={onclose}>
  <header>
    <h2>Copy Patch from Device</h2>
    <div class="devices">
      <label>
        From
        <select value={from?.idx} onchange={(e) => (fromIdx = +e.currentTarget.value)}>
          {#each sources as d (d.idx)}<option value={d.idx}>{label(d)}</option>{/each}
        </select>
      </label>
      <span class="arrow">→</span>
      <label>
        To
        <select bind:value={toIdx}>
          {#each show.devices.filter((d) => d.ins + d.outs > 0) as d (d.idx)}<option value={d.idx}>{label(d)}</option>{/each}
        </select>
      </label>
    </div>
    <div class="options">
      {#each PARTS as [p, text]}
        <label class:off={!from || !canCopy(from, to, p)}>
          <input type="checkbox" bind:checked={wanted[p]} disabled={!from || !canCopy(from, to, p)} />
          {text}
        </label>
      {/each}
      <label class="right"><input type="checkbox" bind:checked={showUnchanged} /> Show unchanged channels</label>
    </div>
    {#if from && parts.includes("inputs")}
      <p class="note">
        A channel has only one source, so copied inputs move: {from.name} loses the patches that {to.name} takes over.
      </p>
    {/if}
  </header>

  <div class="preview">
    {#if !from}
      <div class="empty">No other device to copy from.</div>
    {:else}
      {#each PARTS as [part, text]}
        {@const list = sectionOf(part)}
        {#if parts.includes(part)}
          <table>
            <thead>
              <tr>
                <th class="check">
                  <input
                    type="checkbox"
                    checked={list.length > 0 && allOn(list)}
                    indeterminate={!allOn(list) && !noneOn(list)}
                    disabled={!list.length}
                    onchange={(e) => toggleAll(part, e.currentTarget.checked)}
                  />
                </th>
                <th class="num">{text}</th>
                <th>Port</th>
                <th>{to.name} now</th>
                <th>After copy</th>
              </tr>
            </thead>
            <tbody>
              {#each list as i (itemKey(i))}
                <tr
                  class="item"
                  class:excluded={i.missing || excluded.has(itemKey(i))}
                  class:same={!changes(i)}
                  class:anchor={anchor === itemKey(i)}
                  class:inrange={preview.has(itemKey(i))}
                  title={anchor === itemKey(i) ? "Start of a Shift-click range" : undefined}
                  onclick={(e) => toggle(e, i)}
                  onmouseenter={(e) => ((hovered = itemKey(i)), (shift = e.shiftKey))}
                  onmouseleave={() => (hovered = null)}
                >
                  <td class="check">
                    <!-- no preventDefault here: the browser would undo the checked state after Svelte has set it -->
                    <input
                      type="checkbox"
                      checked={!i.missing && !excluded.has(itemKey(i))}
                      disabled={i.missing}
                      onclick={(e) => {
                        e.stopPropagation();
                        toggle(e, i);
                        // a Shift-click can leave this box as it was, but the browser has flipped it already
                        e.currentTarget.checked = !excluded.has(itemKey(i));
                      }}
                    />
                  </td>
                  <td class="num">{i.channel}</td>
                  <td>{i.port}</td>
                  <td><PatchEntries entries={i.before} /></td>
                  <td>
                    {#if i.missing}
                      <span class="dim">{to.name} has no {part === "inputs" ? "input" : "output"} {i.channel}</span>
                    {:else}
                      <PatchEntries entries={i.after} empty="unpatched" />
                    {/if}
                  </td>
                </tr>
              {:else}
                <tr><td colspan="5" class="dim center">No changes to the {text.toLowerCase()}.</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/each}
    {/if}
  </div>

  <footer>
    <span class="dim">
      {ops.length} patch change{ops.length === 1 ? "" : "s"}{hidden && !showUnchanged ? ` · ${hidden} unchanged channel${hidden === 1 ? "" : "s"} hidden` : ""}
      · Shift-click sets the range from the marked row to the same state
    </span>
    <div class="spacer"></div>
    <button onclick={() => dialog.close()}>Cancel</button>
    <button class="primary" onclick={apply} disabled={!ops.length}>Copy</button>
  </footer>
</dialog>

<style>
  dialog {
    width: min(960px, 92vw);
    height: min(720px, 88vh);
    padding: 0;
    border: 1px solid #444;
    border-radius: 10px;
    background: var(--panel);
    color: var(--text);
    flex-direction: column;
  }
  dialog[open] {
    display: flex;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  header {
    padding: 16px 20px 8px;
    border-bottom: 1px solid #3a3a3c;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 18px;
  }
  .devices {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .devices label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  select {
    font: inherit;
    color: var(--text);
    background: #111;
    border: 1px solid #555;
    border-radius: 6px;
    padding: 5px 8px;
    min-width: 200px;
  }
  .arrow {
    color: var(--text-dim);
    font-size: 18px;
  }
  .options {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-top: 12px;
  }
  .options label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .options .off {
    opacity: 0.4;
  }
  .options .right {
    margin-left: auto;
    color: var(--text-dim);
  }
  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: #f0b429;
  }
  .preview {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 20px 12px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    margin-top: 12px;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--panel);
    text-align: left;
    color: var(--text-dim);
    font-weight: 600;
    padding: 6px 8px;
    border-bottom: 1px solid #444;
    white-space: nowrap;
  }
  td {
    padding: 4px 8px;
    border-bottom: 1px solid #333;
  }
  .check {
    width: 1%;
  }
  .num {
    width: 1%;
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  tr.item {
    cursor: pointer;
    user-select: none;
    -webkit-user-select: none;
  }
  tr.item:hover {
    background: #2f2f32;
  }
  /* the anchor of a Shift-click range, and the rows a Shift-click would change */
  tr.anchor td:first-child {
    box-shadow: inset 3px 0 0 var(--accent);
  }
  tr.anchor {
    background: rgb(56 200 240 / 12%);
  }
  tr.inrange,
  tr.inrange:hover {
    background: rgb(56 200 240 / 22%);
  }
  tr.excluded td:not(.check) {
    opacity: 0.4;
  }
  tr.same td:not(.check) {
    color: var(--text-dim);
  }
  .dim {
    color: var(--text-dim);
  }
  .center {
    text-align: center;
    padding: 12px;
  }
  .empty {
    padding: 40px;
    text-align: center;
    color: var(--text-dim);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 20px;
    border-top: 1px solid #3a3a3c;
    font-size: 13px;
  }
  .spacer {
    flex: 1;
  }
</style>
