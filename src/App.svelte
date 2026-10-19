<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { ask, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import * as api from "./lib/api";
  import PatchGrid from "./lib/PatchGrid.svelte";
  import * as recent from "./lib/recent";
  import { clearRowsOps, deviceGrid, inputGrid, outputGrid, type OutputTab, type PatchView } from "./lib/grid";
  import { CLUSTER, DRIVER_CHANNELS, type Device, type PatchOp, type ShowState } from "./lib/types";

  let show = $state<ShowState | null>(null);
  let view = $state<PatchView>("input");
  let section = $state(0);
  let outputTab = $state<OutputTab>("direct");
  let devIdx = $state(-1);
  let srcDevIdx = $state(-1);
  let cell = $state(32);
  let selected = $state(new Set<number>());
  let hover = $state<{ r: number; c: number } | null>(null);
  let message = $state("");
  let busy = $state(false);
  let dragging = $state(false);
  let recentShows = $state(recent.load());

  const views: [PatchView, string][] = [
    ["input", "Input"],
    ["output", "Output"],
    ["d2d", "Device to\nDevice"],
  ];

  // devices that have channels in the direction the column side of the matrix needs
  const colDevices = $derived(show?.devices.filter((d) => (view === "input" ? d.ins : d.outs) > 0) ?? []);
  const rowDevices = $derived(show?.devices.filter((d) => d.ins > 0) ?? []);
  const device = $derived(colDevices.find((d) => d.idx === devIdx) ?? colDevices[0]);
  const srcDevice = $derived(rowDevices.find((d) => d.idx === srcDevIdx) ?? rowDevices[0]);

  const grid = $derived.by(() => {
    if (!show || !device) return null;
    if (view === "input") return inputGrid(show, device, section);
    if (view === "output") return outputGrid(show, device, outputTab);
    return srcDevice ? deviceGrid(show, srcDevice, device) : null;
  });

  const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const dirName = (p: string) => p.slice(0, p.length - baseName(p).length).replace(/[\\/]$/, "");
  const fileName = $derived(show ? baseName(show.path) : "");
  const status = $derived.by(() => {
    if (!grid || !hover || !device) return "";
    const row = grid.rows[hover.r];
    const col = grid.cols[hover.c];
    if (!row || !col || row.heading !== undefined) return "";
    const rowText = `${row.number}  ${row.label} ${row.side}`.trim();
    const port = [device.name, col.port.minor, col.port.label].filter(Boolean).join(" ");
    if (view === "input") return `${port}  →  ${rowText}${section ? "  (Input B)" : ""}`;
    return `${rowText}  →  ${port}${col.note ? `   (currently: ${col.note})` : ""}`;
  });

  // a new matrix starts with an empty row selection
  $effect(() => {
    void view, section, outputTab, device?.idx, srcDevice?.idx;
    selected = new Set();
  });

  async function run(task: () => Promise<ShowState | void>) {
    busy = true;
    try {
      const s = await task();
      if (s) show = s;
    } catch (e) {
      message = `Error: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function openFile() {
    let path: string | null = "demo";
    if (isTauri()) {
      path = await open({ filters: [{ name: "LV1 Showfile", extensions: ["emo"] }] });
    }
    if (path) await openPath(path);
  }

  async function openPath(path: string) {
    await run(async () => {
      let s;
      try {
        s = await api.openShow(path);
      } catch (e) {
        recentShows = recent.remove(path);
        throw e;
      }
      recentShows = recent.add(path);
      message = s.lv1Open
        ? "Warning: this show seems to be open in LV1. Close it there before saving."
        : `Opened ${path}`;
      return s;
    });
  }

  async function saveFile(saveAs: boolean) {
    if (!show) return;
    let target: string | null = null;
    if (saveAs) {
      if (!isTauri()) return;
      target = await saveDialog({
        defaultPath: show.path.replace(/\.emo$/i, "-patched.emo"),
        filters: [{ name: "LV1 Showfile", extensions: ["emo"] }],
      });
      if (!target) return;
    }
    await run(async () => {
      let res;
      try {
        res = await api.save(target, false);
      } catch (e) {
        if (e !== "LV1_OPEN") throw e;
        const ok = await ask(
          "The showfile has an active -wal file, LV1 probably has it open. Writing now can corrupt the show or be overwritten by LV1.\n\nWrite anyway?",
          { title: "LV1 is running", kind: "warning" },
        );
        if (!ok) return;
        res = await api.save(target, true);
      }
      message = res.backup ? `Saved. Backup: ${res.backup}` : `Saved ${target ?? show!.path}`;
      return res.show;
    });
  }

  const isShowfile = (p: string) => /\.emo$/i.test(p);

  // files dropped from Finder/Explorer only reach us through Tauri's native drag-drop event
  $effect(() => {
    if (!isTauri()) return;
    const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter") dragging = payload.paths.some(isShowfile);
      else if (payload.type === "leave") dragging = false;
      else if (payload.type === "drop") {
        dragging = false;
        const path = payload.paths.find(isShowfile);
        if (!path) message = "Drop a Waves LV1 showfile (.emo) to open it.";
        else if (!busy) openPath(path);
      }
    });
    return () => void unlisten.then((f) => f());
  });

  const apply = (ops: PatchOp[]) => run(() => api.patch(ops));

  function setDriverChannels(d: Device, count: number) {
    const lost = show!.routes.filter(
      (r) =>
        (r.src.cluster === CLUSTER.HW_IN && r.src.index === d.idx && r.src.channel >= count) ||
        (r.dst.cluster === CLUSTER.HW_OUT && r.dst.index === d.idx && r.dst.channel >= count),
    ).length;
    run(async () => {
      const s = await api.setDriverChannels(d.idx, count);
      message = `${d.name}: ${count} driver channels` + (lost ? `, ${lost} patch${lost > 1 ? "es" : ""} removed` : "");
      return s;
    });
  }

  function deleteSelected() {
    if (!grid || !selected.size) return;
    const ops = clearRowsOps(grid, selected);
    if (ops.length) apply(ops);
  }

  function keydown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    const k = e.key.toLowerCase();
    // typing a gain value: Backspace/Delete edit the text, not the patch
    if (e.target instanceof HTMLInputElement && !(mod && (k === "o" || k === "s"))) return;
    if (mod && k === "o") openFile();
    else if (mod && k === "s") saveFile(e.shiftKey);
    else if (mod && k === "z" && show) run(e.shiftKey ? api.redo : api.undo);
    else if (mod && k === "y" && show) run(api.redo);
    else if ((e.key === "Delete" || e.key === "Backspace") && !mod) deleteSelected();
    else return;
    e.preventDefault();
  }

  const mac = /Mac|iPhone|iPad/.test(navigator.userAgent);
  const keys = mac
    ? { undo: "⌘Z", redo: "⇧⌘Z", save: "⌘S", saveAs: "⇧⌘S" }
    : { undo: "Ctrl+Z", redo: "Ctrl+Y", save: "Ctrl+S", saveAs: "Ctrl+Shift+S" };

  const deviceNo = (d: Device) => `${d.idx + 1}.`;
</script>

<svelte:window onkeydown={keydown} />

<main>
  <header class="topbar">
    <div class="brand">LV1 <span>Offline Patcher</span></div>
    <button onclick={openFile} disabled={busy}>Open…</button>
    {#if show}
      <div class="file" title={show.path}>
        <strong>{fileName}{show.dirty ? "*" : ""}</strong>
        <span>{show.snapshot}</span>
      </div>
      <button onclick={() => run(api.undo)} disabled={!show.canUndo || busy} title="Undo ({keys.undo})">Undo</button>
      <button onclick={() => run(api.redo)} disabled={!show.canRedo || busy} title="Redo ({keys.redo})">Redo</button>
      <div class="spacer"></div>
      {#if show.lv1Open}<span class="warn">LV1 has this file open</span>{/if}
      <button class="primary" onclick={() => saveFile(false)} disabled={!show.dirty || busy} title="Save ({keys.save})">
        Save
      </button>
      <button onclick={() => saveFile(true)} disabled={busy} title="Save patched copy ({keys.saveAs})">Save As…</button>
    {/if}
  </header>

  {#if show}
    <section class="selectors">
      <div class="panel views">
        <div class="caption">Select Patch View</div>
        <div class="row">
          {#each views as [v, label]}
            <button class="big" class:active={view === v} onclick={() => (view = v)}>{label}</button>
          {/each}
        </div>
      </div>
      <div class="panel devices">
        <div class="devrow">
          {#each colDevices as d (d.idx)}
            <div class="device" class:daw={d.isDaw} class:active={d.idx === device?.idx}>
              <button class="dsel" title={d.name} onclick={() => (devIdx = d.idx)}>
                <div class="dhead">{deviceNo(d)} {d.name}</div>
                <div class="dbody">
                  <span class="kind">{d.isDaw ? "DAW" : d.kind}</span>
                  {#if !d.isDaw}
                    <span class="io">{view === "input" ? d.inPorts.length + " in" : d.outPorts.length + " out"}</span>
                  {/if}
                </div>
              </button>
              {#if d.isDaw}
                <select
                  class="driver"
                  title="Driver Channels (removes patches on channels that no longer exist)"
                  value={d.ins === d.outs ? d.ins : ""}
                  disabled={busy}
                  onchange={(e) => setDriverChannels(d, +e.currentTarget.value)}
                >
                  {#if d.ins !== d.outs}<option value="" disabled>{d.ins} in / {d.outs} out</option>{/if}
                  {#each DRIVER_CHANNELS as n}<option value={n}>{n} ch</option>{/each}
                </select>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    </section>

    <section class="work">
      <aside class="panel side">
        {#if view === "input"}
          <button class="big" class:active={section === 0} onclick={() => (section = 0)}>Input A</button>
          <button class="big" class:active={section === 1} onclick={() => (section = 1)}>Input B</button>
        {:else if view === "d2d"}
          <div class="caption small">From</div>
          {#each rowDevices as d (d.idx)}
            <button class="srcdev" class:daw={d.isDaw} class:active={d.idx === srcDevice?.idx} title={d.name} onclick={() => (srcDevIdx = d.idx)}>
              {deviceNo(d)} {d.name}
            </button>
          {/each}
        {:else}
          <button class="big" class:active={outputTab === "direct"} onclick={() => (outputTab = "direct")}>Direct</button>
          <button class="big" class:active={outputTab === "busses"} onclick={() => (outputTab = "busses")}>
            Mix{"\n"}Busses
          </button>
        {/if}
      </aside>

      {#snippet tools()}
        <div class="tools">
          <div class="zoom">
            <button class="icon" onclick={() => (cell = Math.max(18, cell - 4))} title="Zoom out">
              <svg class="zoomicon" viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="10" cy="10" r="6.5" /><path d="M15 15l5.5 5.5M7 10h6" />
              </svg>
            </button>
            <button class="icon" onclick={() => (cell = Math.min(48, cell + 4))} title="Zoom in">
              <svg class="zoomicon" viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="10" cy="10" r="6.5" /><path d="M15 15l5.5 5.5M7 10h6M10 7v6" />
              </svg>
            </button>
          </div>
          <div class="where">
            <strong>{view === "input" ? `Input ${section ? "B" : "A"}` : view === "output" ? "Output" : "Device to Device"}</strong>
            <span>
              {view === "input" ? "From Device to Channel" : view === "output" ? "From Channel to Device" : "From Device to Device"}
            </span>
          </div>
          <svg class="direction" viewBox="0 0 32 32" aria-hidden="true">
            {#if view === "input"}
              <path d="M27 4v12a4 4 0 0 1-4 4H6M11 15l-5 5 5 5" />
            {:else}
              <path d="M5 28V16a4 4 0 0 1 4-4h17M21 7l5 5-5 5" />
            {/if}
          </svg>
        </div>
      {/snippet}
      <div class="panel gridpanel">
        {#if grid}
          <div class="gridwrap">
            <PatchGrid
              {grid}
              {cell}
              bind:selected
              bind:hover
              onpatch={apply}
              ontap={(cluster, index, tap) => run(() => api.setTap(cluster, index, tap))}
              onpreamp={(idx, channel, gain, phantom) => run(() => api.setPreamp(idx, channel, gain, phantom))}
              corner={tools}
            />
          </div>
        {:else}
          {@render tools()}
          <div class="empty">No device with channels for this view.</div>
        {/if}
      </div>
    </section>
  {:else}
    <section class="welcome">
      <div class="intro">
        <div class="eyebrow">Waves LV1</div>
        <h1>Offline Patcher</h1>
        <p class="lead">
          Edit the input, output and device-to-device patch of an LV1 showfile without the hardware being online.
        </p>
        <div class="actions">
          <button class="primary" onclick={openFile} disabled={busy}>Open Showfile…</button>
          <span>or drop an <code>.emo</code> file onto this window</span>
        </div>
        <ul class="notes">
          <li>Close the show in LV1 before saving.</li>
          <li>A backup (<code>.bak-&lt;timestamp&gt;</code>) is written before every save.</li>
        </ul>
      </div>
      <div class="panel recent">
        <div class="recenthead">Recent Showfiles</div>
        {#each recentShows as p (p)}
          <div class="recentrow">
            <button class="recentfile" title={p} disabled={busy} onclick={() => openPath(p)}>
              <strong>{baseName(p)}</strong>
              <span>{dirName(p)}</span>
            </button>
            <button class="forget" title="Remove from list" onclick={() => (recentShows = recent.remove(p))}>
              <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
            </button>
          </div>
        {:else}
          <div class="norecent">Showfiles you open will appear here.</div>
        {/each}
      </div>
    </section>
  {/if}

  {#if dragging}
    <div class="dropzone"><div>Drop to open showfile</div></div>
  {/if}

  <footer class="statusbar">
    <span>{status || message}</span>
    {#if grid}<span class="count">{grid.patched.size} patched in view</span>{/if}
  </footer>
</main>

<style>
  main {
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    height: 100vh;
    gap: 8px;
    padding: 8px;
    box-sizing: border-box;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .brand {
    font-weight: 800;
    font-size: 18px;
    margin-right: 8px;
  }
  .brand span {
    font-weight: 400;
    color: var(--text-dim);
  }
  .file {
    display: flex;
    flex-direction: column;
    background: #111;
    border: 1px solid #444;
    border-radius: 6px;
    padding: 3px 12px;
    font-size: 12px;
    min-width: 160px;
  }
  .file span {
    color: var(--text-dim);
  }
  .spacer {
    flex: 1;
  }
  .warn {
    color: #ff5252;
    font-weight: 700;
  }
  .selectors {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px;
    min-width: 0;
  }
  .views {
    padding: 8px 10px;
  }
  .caption {
    text-align: center;
    font-size: 18px;
    font-weight: 600;
    margin-bottom: 8px;
  }
  .caption.small {
    font-size: 13px;
    color: var(--text-dim);
  }
  .views .row {
    display: flex;
    gap: 8px;
  }
  .devices {
    padding: 10px;
    overflow-x: auto;
  }
  /* sized by the cards, not by the scroll area: a horizontal scrollbar must not squeeze them */
  .devrow {
    display: flex;
    gap: 10px;
    width: max-content;
  }
  .device {
    flex: none;
    display: flex;
    flex-direction: column;
    width: 112px;
    overflow: hidden;
    background: #111;
    border: 2px solid transparent;
    border-radius: 6px;
  }
  .device.active {
    border-color: var(--accent);
  }
  .dsel {
    flex: 1;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: none;
    color: var(--text);
    text-align: left;
    display: flex;
    flex-direction: column;
  }
  .driver {
    /* a fixed height instead of the native control, which is taller in WebKit than in Chromium */
    appearance: none;
    -webkit-appearance: none;
    height: 22px;
    margin: 0 6px 6px;
    padding: 0 18px 0 6px;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    line-height: 20px;
    text-align: center;
    text-align-last: center;
    color: var(--text);
    background: #262628
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 6'%3E%3Cpath d='M1 1l4 4 4-4' fill='none' stroke='%23a6a6aa' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
      no-repeat right 6px center / 9px 6px;
    border: 1px solid #555;
    border-radius: 4px;
    cursor: pointer;
  }
  .driver:hover:not(:disabled) {
    border-color: #888;
  }
  .dhead {
    background: #3d8bfd;
    color: #000;
    font-size: 11px;
    font-weight: 700;
    padding: 3px 5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .device.daw .dhead {
    background: #2bd17e;
  }
  .dbody {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 6px 4px;
    gap: 2px;
  }
  .kind {
    font-weight: 800;
    font-size: 15px;
  }
  .io {
    font-size: 11px;
    color: var(--text-dim);
  }
  .work {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 8px;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 8px;
    overflow-y: auto;
  }
  .srcdev {
    flex: none;
    text-align: left;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-left: 4px solid #3d8bfd;
  }
  .srcdev.active {
    border-left-width: 4px;
  }
  .srcdev.daw {
    border-left-color: #2bd17e;
  }
  .gridpanel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    padding: 8px;
  }
  .tools {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-areas: "zoom zoom" "where direction";
    align-items: end;
    row-gap: 4px;
    padding: 4px 8px 6px;
    line-height: 1.2;
  }
  .zoom {
    grid-area: zoom;
    display: flex;
    gap: 8px;
  }
  .zoom button {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: 1.5px solid #8a8a8e;
    border-radius: 7px;
    background: transparent;
  }
  .zoom button:hover {
    border-color: var(--text);
  }
  .zoomicon {
    width: 24px;
    height: 24px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .where {
    grid-area: where;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .where strong {
    font-size: 17px;
  }
  .where span {
    font-size: 13px;
    color: var(--text-dim);
  }
  .direction {
    grid-area: direction;
    width: 30px;
    height: 30px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .gridwrap {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--text-dim);
  }
  .welcome {
    grid-row: 2 / 4;
    align-self: center;
    justify-self: center;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
    gap: 48px;
    align-items: start;
    width: min(960px, 100%);
    padding: 0 32px;
    box-sizing: border-box;
  }
  .eyebrow {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .intro h1 {
    margin: 4px 0 12px;
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .lead {
    margin: 0 0 24px;
    font-size: 15px;
    line-height: 1.5;
    color: var(--text-dim);
  }
  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .actions button {
    padding: 9px 18px;
    font-size: 14px;
  }
  .notes {
    margin: 28px 0 0;
    padding: 14px 0 0 18px;
    border-top: 1px solid #333;
    font-size: 12px;
    line-height: 1.7;
    color: var(--text-dim);
  }
  .recent {
    display: flex;
    flex-direction: column;
    padding: 6px;
    min-width: 0;
  }
  .recenthead {
    padding: 8px 10px 10px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .recentrow {
    display: flex;
    align-items: center;
    border-radius: 5px;
  }
  .recentrow:hover {
    background: #303033;
  }
  .recentfile {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 10px;
    border: 0;
    background: none;
    text-align: left;
  }
  .recentfile strong,
  .recentfile span {
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .recentfile span {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-dim);
  }
  .forget {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin-right: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-dim);
    visibility: hidden;
  }
  .recentrow:hover .forget {
    visibility: visible;
  }
  .forget:hover {
    color: var(--text);
    background: #444;
  }
  .forget svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .norecent {
    padding: 4px 10px 12px;
    font-size: 12px;
    color: var(--text-dim);
  }
  @media (max-width: 720px) {
    .welcome {
      grid-template-columns: 1fr;
      gap: 28px;
    }
  }
  .dropzone {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    padding: 16px;
    background: rgb(0 0 0 / 0.6);
    pointer-events: none;
  }
  .dropzone div {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    border: 3px dashed var(--accent);
    border-radius: 12px;
    font-size: 22px;
    font-weight: 700;
    color: var(--accent);
  }
  .statusbar {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--text-dim);
    padding: 0 4px;
    min-height: 16px;
  }
</style>
