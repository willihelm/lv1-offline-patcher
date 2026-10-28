<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { LogicalPosition } from "@tauri-apps/api/dpi";
  import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
  import { getAllWebviewWindows, getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { ask, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import * as api from "./lib/api";
  import PatchGrid from "./lib/PatchGrid.svelte";
  import PatchList from "./lib/PatchList.svelte";
  import CopyDialog from "./lib/CopyDialog.svelte";
  import * as recent from "./lib/recent";
  import {
    clearRowsOps,
    deviceGrid,
    devicePatchCsv,
    devicePatchList,
    deviceToDirectOps,
    directToDeviceOps,
    returnToInputBOps,
    inputGrid,
    outputGrid,
    type PatchView,
  } from "./lib/grid";
  import { Tab } from "./lib/tab.svelte";
  import { CLUSTER, DRIVER_CHANNELS, type Device, type PatchOp, type ShowState } from "./lib/types";

  let tabs = $state<Tab[]>([new Tab()]);
  let activeKey = $state(tabs[0].key);
  const tab = $derived(tabs.find((t) => t.key === activeKey) ?? tabs[0]);
  const show = $derived(tab.show);
  let cell = $state(32);
  let hover = $state<{ r: number; c: number } | null>(null);
  let busy = $state(false);
  let dragging = $state(false);
  let recentShows = $state(recent.load());

  const views: [PatchView, string][] = [
    ["input", "Input"],
    ["output", "Output"],
    ["d2d", "Device to\nDevice"],
    ["list", "List"],
  ];

  // devices that have channels in the direction the column side of the matrix needs (the list shows both)
  const colDevices = $derived(
    show?.devices.filter((d) => (tab.view === "list" ? d.ins + d.outs : tab.view === "input" ? d.ins : d.outs) > 0) ?? [],
  );
  const rowDevices = $derived(show?.devices.filter((d) => d.ins > 0) ?? []);
  const device = $derived(colDevices.find((d) => d.idx === tab.devIdx) ?? colDevices[0]);
  const srcDevice = $derived(rowDevices.find((d) => d.idx === tab.srcDevIdx) ?? rowDevices[0]);

  const grid = $derived.by(() => {
    if (!show || !device || tab.view === "list") return null;
    if (tab.view === "input") return inputGrid(show, device, tab.section);
    if (tab.view === "output") return outputGrid(show, device, tab.outputTab);
    return srcDevice ? deviceGrid(show, srcDevice, device) : null;
  });

  const patchList = $derived(show && device && tab.view === "list" ? devicePatchList(show, device) : null);

  const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const dirName = (p: string) => p.slice(0, p.length - baseName(p).length).replace(/[\\/]$/, "");
  const fileName = $derived(show ? baseName(show.path) : "");
  const tabTitle = (t: Tab) => (t.show ? baseName(t.show.path) : "New Tab");
  const status = $derived.by(() => {
    if (!grid || !hover || !device) return "";
    const row = grid.rows[hover.r];
    const col = grid.cols[hover.c];
    if (!row || !col || row.heading !== undefined) return "";
    const rowText = `${row.number}  ${row.label} ${row.side}`.trim();
    const port = [device.name, col.port.minor, col.port.label].filter(Boolean).join(" ");
    if (tab.view === "input") return `${port}  →  ${rowText}${tab.section ? "  (Input B)" : ""}`;
    return `${rowText}  →  ${port}${col.note ? `   (currently: ${col.note})` : ""}`;
  });

  // a new matrix starts with an empty row selection; switching tabs keeps the tab's selection
  let selectionKey = 0;
  $effect(() => {
    void tab.view, tab.section, tab.outputTab, device?.idx, srcDevice?.idx;
    if (selectionKey === tab.key) tab.selected = new Set();
    selectionKey = tab.key;
  });

  $effect(() => {
    const title = show ? `${fileName}${show.dirty ? " •" : ""} — LV1 Offline Patcher` : "LV1 Offline Patcher";
    document.title = title;
    if (isTauri()) getCurrentWebviewWindow().setTitle(title);
  });

  /** Run a backend command for tab `t`; errors end up in that tab's status bar. */
  async function run(t: Tab, task: () => Promise<ShowState | void>) {
    busy = true;
    try {
      const s = await task();
      if (s) t.show = s;
    } catch (e) {
      t.message = `Error: ${e}`;
    } finally {
      busy = false;
    }
  }

  /** run a command on the session of the active tab */
  const runShow = (task: (id: number) => Promise<ShowState>) => {
    const t = tab;
    if (t.id !== null) run(t, () => task(t.id!));
  };

  function addTab(t = new Tab()) {
    tabs.push(t);
    activeKey = t.key;
    return t;
  }

  /** add a session moved or opened into this window, unless it is here already */
  function addSession(s: api.Session) {
    const existing = tabs.find((t) => t.id === s.id);
    if (existing) return (activeKey = existing.key);
    // an untouched window gives its welcome tab to the session
    if (tabs.length === 1 && !tabs[0].show) tabs[0].load(s);
    else addTab(new Tab(s));
    activeKey = tabs.find((t) => t.id === s.id)!.key;
  }

  function removeTab(t: Tab) {
    const i = tabs.indexOf(t);
    if (i < 0) return;
    tabs.splice(i, 1);
    if (!tabs.length) {
      // like a browser: closing the last tab closes the window
      if (isTauri()) getCurrentWebviewWindow().destroy();
      tabs.push(new Tab());
    }
    if (activeKey === t.key) activeKey = tabs[Math.min(i, tabs.length - 1)].key;
  }

  const confirm = (text: string, okLabel: string) =>
    isTauri() ? ask(text, { title: "Unsaved changes", kind: "warning", okLabel }) : Promise.resolve(window.confirm(text));

  async function closeTab(t: Tab) {
    if (t.show?.dirty) {
      activeKey = t.key;
      const ok = await confirm(`"${tabTitle(t)}" has unsaved changes. Close it anyway?`, "Close Tab");
      if (!ok) return;
    }
    if (t.id !== null) api.closeSession(t.id).catch(() => {});
    removeTab(t);
  }

  function closeWindow() {
    if (isTauri()) getCurrentWebviewWindow().close();
    else window.close();
  }

  function cycleTab(step: number) {
    const i = tabs.indexOf(tab);
    activeKey = tabs[(i + step + tabs.length) % tabs.length].key;
  }

  async function openFile() {
    let paths: string[] | null = ["demo"];
    if (isTauri()) {
      paths = await open({ multiple: true, filters: [{ name: "LV1 Showfile", extensions: ["emo"] }] });
    }
    for (const p of paths ?? []) await openPath(p);
  }

  /** open into the active tab if it is a welcome page, otherwise into a new tab */
  async function openPath(path: string) {
    const t = tab;
    await run(t, async () => {
      let s;
      try {
        s = await api.openShow(path);
      } catch (e) {
        recentShows = recent.remove(path);
        throw e;
      }
      recentShows = recent.add(path);
      if (!s) return; // already open, its tab got activated
      const target = t.show || !tabs.includes(t) ? addTab() : t;
      target.load(s);
      activeKey = target.key;
      target.message = s.show.lv1Open
        ? "Warning: this show seems to be open in LV1. Close it there before saving."
        : `Opened ${path}`;
    });
  }

  async function saveFile(saveAs: boolean) {
    const t = tab;
    if (!t.show || t.id === null) return;
    const id = t.id;
    let target: string | null = null;
    if (saveAs) {
      if (!isTauri()) return;
      target = await saveDialog({
        defaultPath: t.show.path.replace(/\.emo$/i, "-patched.emo"),
        filters: [{ name: "LV1 Showfile", extensions: ["emo"] }],
      });
      if (!target) return;
    }
    await run(t, async () => {
      let res;
      try {
        res = await api.save(id, target, false);
      } catch (e) {
        if (e !== "LV1_OPEN") throw e;
        const ok = await ask(
          "The showfile has an active -wal file, LV1 probably has it open. Writing now can corrupt the show or be overwritten by LV1.\n\nWrite anyway?",
          { title: "LV1 is running", kind: "warning" },
        );
        if (!ok) return;
        res = await api.save(id, target, true);
      }
      t.message = res.backup ? `Saved. Backup: ${res.backup}` : `Saved ${target ?? t.show!.path}`;
      return res.show;
    });
  }

  function command(c: string) {
    if (c === "new-tab") addTab();
    else if (c === "new-window") api.newWindow().catch((e) => (tab.message = `Error: ${e}`));
    else if (c === "open") openFile();
    else if (c === "close-tab") closeTab(tab);
    else if (c === "close-window") closeWindow();
    else if (c === "save") saveFile(false);
    else if (c === "save-as") saveFile(true);
    else if (c === "next-tab") cycleTab(1);
    else if (c === "prev-tab") cycleTab(-1);
  }

  const isShowfile = (p: string) => /\.emo$/i.test(p);

  onMount(() => {
    if (!isTauri()) return;
    const win = getCurrentWebviewWindow();
    const unlisten = [
      // files dropped from Finder/Explorer only reach us through Tauri's native drag-drop event
      getCurrentWebview().onDragDropEvent(async ({ payload }) => {
        if (payload.type === "enter") dragging = payload.paths.some(isShowfile);
        else if (payload.type === "leave") dragging = false;
        else if (payload.type === "drop") {
          dragging = false;
          const paths = payload.paths.filter(isShowfile);
          if (!paths.length) tab.message = "Drop a Waves LV1 showfile (.emo) to open it.";
          else if (!busy) for (const p of paths) await openPath(p);
        }
      }),
      win.listen<string>("menu", (e) => command(e.payload)),
      win.listen<number>("activate-session", (e) => {
        const t = tabs.find((t) => t.id === e.payload);
        if (t) activeKey = t.key;
      }),
      win.listen<api.Session>("session-added", (e) => addSession(e.payload)),
      win.onCloseRequested(async (e) => {
        const dirty = tabs.filter((t) => t.show?.dirty);
        if (!dirty.length) return;
        const names = dirty.map((t) => `"${tabTitle(t)}"`).join(", ");
        if (!(await confirm(`${names} ${dirty.length > 1 ? "have" : "has"} unsaved changes. Close the window anyway?`, "Close Window")))
          e.preventDefault();
      }),
    ];
    // tabs this window was created with (a tab dragged out of another window)
    api.windowSessions().then((list) => list.forEach(addSession));
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  const apply = (ops: PatchOp[]) => runShow((id) => api.patch(id, ops));

  function setDriverChannels(d: Device, count: number) {
    const t = tab;
    const lost = show!.routes.filter(
      (r) =>
        (r.src.cluster === CLUSTER.HW_IN && r.src.index === d.idx && r.src.channel >= count) ||
        (r.dst.cluster === CLUSTER.HW_OUT && r.dst.index === d.idx && r.dst.channel >= count),
    ).length;
    run(t, async () => {
      const s = await api.setDriverChannels(t.id!, d.idx, count);
      t.message = `${d.name}: ${count} driver channels` + (lost ? `, ${lost} patch${lost > 1 ? "es" : ""} removed` : "");
      return s;
    });
  }

  function deleteSelected() {
    if (!grid || !tab.selected.size) return;
    const ops = clearRowsOps(grid, tab.selected);
    if (ops.length) apply(ops);
  }

  const conversions = [
    {
      label: "Convert DO → D2D",
      ops: directToDeviceOps,
      none: "no direct outs to convert",
      done: (n: number) => `${n} direct out${n > 1 ? "s" : ""} converted to device to device`,
      skip: "no input patch",
    },
    {
      label: "Convert D2D → DO",
      ops: deviceToDirectOps,
      none: "no device to device patches to convert",
      done: (n: number) => `${n} device to device patch${n > 1 ? "es" : ""} converted to direct outs`,
      skip: "feeds no channel",
    },
    {
      label: "Return Outputs to Input B",
      ops: returnToInputBOps,
      none: "no outputs to return to Input B",
      done: (n: number) => `${n} channel${n > 1 ? "s" : ""} patched to return on Input B`,
      skip: "no input with the same number",
    },
  ];

  /** repatch the outputs of `d` (direct outs from/to the device inputs that feed their channels), or return them to Input B */
  function convert(d: Device, conv: (typeof conversions)[number]) {
    const t = tab;
    const { ops, skipped } = conv.ops(show!, d);
    const rest = skipped.length ? `; ${skipped.length} skipped, ${conv.skip}: ${skipped.join(", ")}` : "";
    if (!ops.length) {
      t.message = `${d.name}: ${conv.none}${rest}`;
      return;
    }
    run(t, async () => {
      const s = await api.patch(t.id!, ops);
      t.message = `${d.name}: ${conv.done(ops.length)}${rest}`;
      return s;
    });
  }

  /** device the copy dialog copies to, if open */
  let copyTo = $state<Device | null>(null);

  function applyCopy(ops: PatchOp[], from: Device, to: Device) {
    const t = tab;
    run(t, async () => {
      const s = await api.patch(t.id!, ops);
      t.message = `${to.name}: patch copied from ${from.name}, ${ops.length} change${ops.length > 1 ? "s" : ""}`;
      return s;
    });
  }

  /** save the patch list of `d` as CSV */
  async function exportPatchList(d: Device) {
    const t = tab;
    const csv = devicePatchCsv(show!, d);
    const name = `${baseName(t.show!.path).replace(/\.emo$/i, "")} - ${d.name.replace(/[\\/:*?"<>|]/g, "_")}.csv`;
    if (!isTauri()) {
      const a = document.createElement("a");
      a.href = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
      a.download = name;
      a.click();
      return;
    }
    const path = await saveDialog({
      defaultPath: `${dirName(t.show!.path)}/${name}`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      await api.exportText(path, csv);
      t.message = `${d.name}: patch list exported to ${path}`;
    } catch (e) {
      t.message = `Error: ${e}`;
    }
  }

  /** all functions of a device: driver channels, conversions, copy patch; below the button `at` or at the mouse */
  async function deviceMenu(e: MouseEvent, d: Device, at?: HTMLElement) {
    e.preventDefault();
    if (busy || !show) return;
    if (!isTauri()) return browserDeviceMenu(d);
    const item = (text: string, action: () => void, enabled = true) => MenuItem.new({ text, action, enabled });
    const separator = () => PredefinedMenuItem.new({ item: "Separator" });
    const items: (MenuItem | Submenu | CheckMenuItem | PredefinedMenuItem)[] = [
      await item(`${deviceNo(d)} ${d.name}`, () => {}, false),
    ];
    if (d.isDaw)
      items.push(
        await Submenu.new({
          text: "Driver Channels",
          items: await Promise.all(
            DRIVER_CHANNELS.map((n) =>
              CheckMenuItem.new({
                text: `${n} ch`,
                checked: d.ins === n && d.outs === n,
                action: () => setDriverChannels(d, n),
              }),
            ),
          ),
        }),
      );
    if (d.outs) {
      items.push(await separator());
      for (const c of conversions) items.push(await item(c.label, () => convert(d, c)));
    }
    items.push(
      await separator(),
      await item("Export Patch List…", () => exportPatchList(d)),
      await item("Copy Patch from Device…", () => (copyTo = d), show.devices.length > 1),
    );
    const menu = await Menu.new({ items });
    const r = at?.getBoundingClientRect();
    await menu.popup(r && new LogicalPosition(r.left, r.bottom + 4));
  }

  /** stand-in for the native menu in a plain browser */
  function browserDeviceMenu(d: Device) {
    if (window.confirm(`${d.name}: Copy Patch from Device…?`)) return void (copyTo = d);
    if (window.confirm(`${d.name}: Export Patch List?`)) return exportPatchList(d);
    if (d.isDaw) {
      const n = Number(window.prompt(`${d.name}: Driver Channels (${DRIVER_CHANNELS.join(", ")}), empty to skip`));
      if (DRIVER_CHANNELS.includes(n)) return setDriverChannels(d, n);
    }
    const conv = d.outs ? conversions.find((c) => window.confirm(`${d.name}: ${c.label}?`)) : undefined;
    if (conv) return convert(d, conv);
  }

  const mac = /Mac|iPhone|iPad/.test(navigator.userAgent);

  function keydown(e: KeyboardEvent) {
    if (copyTo) return; // the copy dialog handles its own keys
    const mod = e.metaKey || e.ctrlKey;
    const k = e.key.toLowerCase();
    // file and tab shortcuts; in the macOS app the menu handles them and sends a "menu" event
    const menuKeys: Record<string, string> = { t: "new-tab", n: "new-window", o: "open", w: "close-tab", s: "save" };
    if (mod && !e.altKey && menuKeys[k] && !(mac && isTauri())) {
      command(e.shiftKey ? ({ w: "close-window", s: "save-as" } as Record<string, string>)[k] ?? "" : menuKeys[k]);
    } else if (e.ctrlKey && e.key === "Tab") cycleTab(e.shiftKey ? -1 : 1);
    else if (e.ctrlKey && !mac && (e.key === "PageDown" || e.key === "PageUp")) cycleTab(e.key === "PageDown" ? 1 : -1);
    else if (mod && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      activeKey = (e.key === "9" ? tabs[tabs.length - 1] : tabs[+e.key - 1] ?? tab).key;
    }
    // typing a gain value: Backspace/Delete edit the text, not the patch
    else if (e.target instanceof HTMLInputElement) return;
    else if (mod && k === "z" && show) runShow(e.shiftKey ? api.redo : api.undo);
    else if (mod && k === "y" && show) runShow(api.redo);
    else if ((e.key === "Delete" || e.key === "Backspace") && !mod) deleteSelected();
    else return;
    e.preventDefault();
  }

  const keys = mac
    ? { undo: "⌘Z", redo: "⇧⌘Z", save: "⌘S", saveAs: "⇧⌘S", newTab: "⌘T", closeTab: "⌘W" }
    : { undo: "Ctrl+Z", redo: "Ctrl+Y", save: "Ctrl+S", saveAs: "Ctrl+Shift+S", newTab: "Ctrl+T", closeTab: "Ctrl+W" };

  const deviceNo = (d: Device) => `${d.idx + 1}.`;

  // Reordering by drag. Move/up are watched on the window, not the tab: the tab's element is
  // moved in the DOM while dragging, which would drop a pointer capture on it.
  let tabbar = $state<HTMLElement>();
  let draggedKey = $state<number | null>(null);

  function tabPointerDown(e: PointerEvent, t: Tab) {
    if (e.button !== 0) return;
    activeKey = t.key;
    if ((e.target as Element).closest("button")) return;
    const startX = e.clientX;
    const move = (ev: PointerEvent) => {
      if (draggedKey === null && Math.abs(ev.clientX - startX) < 5) return;
      draggedKey = t.key;
      const others = [...tabbar!.querySelectorAll<HTMLElement>(".tab")].filter((el) => el.dataset.key !== `${t.key}`);
      const index = others.filter((el) => {
        const r = el.getBoundingClientRect();
        return r.left + r.width / 2 < ev.clientX;
      }).length;
      const from = tabs.indexOf(t);
      if (from >= 0 && from !== index) {
        tabs.splice(from, 1);
        tabs.splice(index, 0, t);
      }
    };
    const up = () => {
      draggedKey = null;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }

  function moveTab(t: Tab, step: number) {
    const i = tabs.indexOf(t);
    const j = i + step;
    if (j < 0 || j >= tabs.length) return;
    tabs.splice(i, 1);
    tabs.splice(j, 0, t);
  }

  /** move a tab's session into another window (`null`: a new one) */
  async function moveToWindow(t: Tab, target: string | null) {
    if (t.id === null) return;
    try {
      await api.moveSession(t.id, target);
      removeTab(t);
    } catch (e) {
      t.message = `Error: ${e}`;
    }
  }

  /** native menu of a tab: order, move to another window, close */
  async function tabMenu(e: MouseEvent, t: Tab) {
    e.preventDefault();
    if (!isTauri()) return;
    const self = getCurrentWebviewWindow().label;
    const others = await Promise.all(
      (await getAllWebviewWindows()).filter((w) => w.label !== self).map(async (w) => ({ label: w.label, title: await w.title() })),
    );
    const i = tabs.indexOf(t);
    const hasShow = t.id !== null;
    const item = (text: string, action: () => void, enabled = true) => MenuItem.new({ text, action, enabled });
    const menu = await Menu.new({
      items: [
        await item("Move Tab Left", () => moveTab(t, -1), i > 0),
        await item("Move Tab Right", () => moveTab(t, 1), i < tabs.length - 1),
        await PredefinedMenuItem.new({ item: "Separator" }),
        await item("Move to New Window", () => moveToWindow(t, null), hasShow && tabs.length > 1),
        ...(await Promise.all(others.map((w) => item(`Move to “${w.title}”`, () => moveToWindow(t, w.label), hasShow)))),
        await PredefinedMenuItem.new({ item: "Separator" }),
        await item("Close Tab", () => closeTab(t)),
      ],
    });
    await menu.popup();
  }
</script>

<svelte:window onkeydown={keydown} onstorage={() => (recentShows = recent.load())} />

<main>
  <nav class="tabbar" class:dragging={draggedKey !== null} bind:this={tabbar}>
    {#each tabs as t (t.key)}
      <div
        class="tab"
        class:active={t.key === tab.key}
        role="tab"
        tabindex="-1"
        aria-selected={t.key === tab.key}
        title={t.show?.path}
        class:dragged={t.key === draggedKey}
        data-key={t.key}
        onpointerdown={(e) => tabPointerDown(e, t)}
        ondragstart={(e) => e.preventDefault()}
        onauxclick={(e) => e.button === 1 && closeTab(t)}
        oncontextmenu={(e) => tabMenu(e, t)}
      >
        <span class="tabtitle">{tabTitle(t)}</span>
        {#if t.show?.dirty}<span class="dirty" aria-label="unsaved changes"></span>{/if}
        {#if isTauri()}
          <button class="tabmenu" title="Move Tab…" onclick={(e) => tabMenu(e, t)}>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 5.5v-3h3M2.5 2.5l4 4M7 3h6.5v10.5H3V9" /></svg>
          </button>
        {/if}
        <button class="tabclose" title="Close Tab ({keys.closeTab})" onclick={() => closeTab(t)}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
        </button>
      </div>
    {/each}
    <button class="newtab" title="New Tab ({keys.newTab})" onclick={() => addTab()}>
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3v10M3 8h10" /></svg>
    </button>
  </nav>

  <header class="topbar">
    <div class="brand">LV1 <span>Offline Patcher</span></div>
    <button onclick={openFile} disabled={busy}>Open…</button>
    {#if show}
      <div class="file" title={show.path}>
        <strong>{fileName}{show.dirty ? "*" : ""}</strong>
        <span>{show.snapshot}</span>
      </div>
      <button onclick={() => runShow(api.undo)} disabled={!show.canUndo || busy} title="Undo ({keys.undo})">Undo</button>
      <button onclick={() => runShow(api.redo)} disabled={!show.canRedo || busy} title="Redo ({keys.redo})">Redo</button>
      {#if device}
        <button
          class="devmenu"
          disabled={busy}
          title={`Functions of ${device.name} (or right-click a device):\ndriver channels, export patch list, copy patch,\nConvert DO → D2D: Direct Outs to Device to Device\nConvert D2D → DO: Device to Device to Direct Outs`}
          onclick={(e) => deviceMenu(e, device, e.currentTarget)}
        >
          {deviceNo(device)} {device.name}
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" /></svg>
        </button>
      {/if}
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
            <button class="big" class:active={tab.view === v} onclick={() => (tab.view = v)}>{label}</button>
          {/each}
        </div>
      </div>
      <div class="panel devices">
        <div class="devrow">
          {#each colDevices as d (d.idx)}
            <div class="device" class:daw={d.isDaw} class:active={d.idx === device?.idx}>
              <button
                class="dsel"
                title={`${d.name}\nRight-click for device functions\n(DO = Direct Out, D2D = Device to Device)`}
                onclick={() => (tab.devIdx = d.idx)}
                oncontextmenu={(e) => deviceMenu(e, d)}
              >
                <div class="dhead">{deviceNo(d)} {d.name}</div>
                <div class="dbody">
                  <span class="kind">{d.isDaw ? "DAW" : d.kind}</span>
                  {#if d.isDaw}
                    <span class="io">{d.ins === d.outs ? `${d.ins} ch` : `${d.ins} in / ${d.outs} out`}</span>
                  {:else}
                    <span class="io">
                      {tab.view === "list"
                        ? `${d.inPorts.length} in / ${d.outPorts.length} out`
                        : tab.view === "input"
                          ? d.inPorts.length + " in"
                          : d.outPorts.length + " out"}
                    </span>
                  {/if}
                </div>
              </button>
            </div>
          {/each}
        </div>
      </div>
    </section>

    <section class="work">
      <aside class="panel side">
        {#if tab.view === "input"}
          <button class="big" class:active={tab.section === 0} onclick={() => (tab.section = 0)}>Input A</button>
          <button class="big" class:active={tab.section === 1} onclick={() => (tab.section = 1)}>Input B</button>
        {:else if tab.view === "list"}
          {#each [["all", "All"], ["inputs", "Inputs"], ["outputs", "Outputs"]] as const as [p, label]}
            <button class="big" class:active={tab.listPart === p} onclick={() => (tab.listPart = p)}>{label}</button>
          {/each}
        {:else if tab.view === "d2d"}
          <div class="caption small">From</div>
          {#each rowDevices as d (d.idx)}
            <button class="srcdev" class:daw={d.isDaw} class:active={d.idx === srcDevice?.idx} title={d.name} onclick={() => (tab.srcDevIdx = d.idx)}>
              {deviceNo(d)} {d.name}
            </button>
          {/each}
        {:else}
          <button class="big" class:active={tab.outputTab === "direct"} onclick={() => (tab.outputTab = "direct")}>Direct</button>
          <button class="big" class:active={tab.outputTab === "busses"} onclick={() => (tab.outputTab = "busses")}>
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
            <strong>{tab.view === "input" ? `Input ${tab.section ? "B" : "A"}` : tab.view === "output" ? "Output" : "Device to Device"}</strong>
            <span>
              {tab.view === "input" ? "From Device to Channel" : tab.view === "output" ? "From Channel to Device" : "From Device to Device"}
            </span>
          </div>
          <svg class="direction" viewBox="0 0 32 32" aria-hidden="true">
            {#if tab.view === "input"}
              <path d="M27 4v12a4 4 0 0 1-4 4H6M11 15l-5 5 5 5" />
            {:else}
              <path d="M5 28V16a4 4 0 0 1 4-4h17M21 7l5 5-5 5" />
            {/if}
          </svg>
        </div>
      {/snippet}
      <div class="panel gridpanel">
        {#if patchList && device}
          <div class="listhead">
            <div class="where">
              <strong>{deviceNo(device)} {device.name}</strong>
              <span>Patch list of the device</span>
            </div>
            <button onclick={() => exportPatchList(device)} disabled={busy}>Export CSV…</button>
          </div>
          <PatchList inputs={patchList.inputs} outputs={patchList.outputs} part={tab.listPart} />
        {:else if grid}
          <div class="gridwrap">
            <PatchGrid
              {grid}
              {cell}
              bind:selected={tab.selected}
              bind:hover
              onpatch={apply}
              ontap={(cluster, index, tap) => runShow((id) => api.setTap(id, cluster, index, tap))}
              onpreamp={(idx, channel, gain, phantom) => runShow((id) => api.setPreamp(id, idx, channel, gain, phantom))}
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
    <span>{status || tab.message}</span>
    {#if grid}<span class="count">{grid.patched.size} patched in view</span>{/if}
  </footer>
  {#if copyTo && show}
    <CopyDialog {show} to={copyTo} onapply={applyCopy} onclose={() => (copyTo = null)} />
  {/if}
</main>

<style>
  main {
    display: grid;
    grid-template-rows: auto auto auto 1fr auto;
    height: 100vh;
    gap: 8px;
    padding: 8px;
    box-sizing: border-box;
  }
  .tabbar {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    margin: -8px -8px 0;
    padding: 6px 8px 0;
    background: #111;
    border-bottom: 1px solid #333;
    min-width: 0;
    user-select: none;
    -webkit-user-select: none;
  }
  .tab {
    flex: 0 1 220px;
    min-width: 72px;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 6px 0 12px;
    box-sizing: border-box;
    border-radius: 8px 8px 0 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-dim);
    cursor: default;
  }
  .tab:hover {
    background: #1f1f21;
  }
  .tabbar.dragging .tab {
    cursor: grabbing;
  }
  .tab.dragged {
    background: #2c2c2f;
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
  .tab.active {
    background: var(--bg);
    color: var(--text);
    box-shadow: inset 0 2px var(--accent);
  }
  .tabtitle {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dirty {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }
  .tabmenu,
  .tabclose,
  .newtab {
    flex: none;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-dim);
  }
  .tabmenu {
    visibility: hidden;
  }
  .tab:hover .tabmenu,
  .tab.active .tabmenu {
    visibility: visible;
  }
  .tabmenu,
  .tabclose {
    width: 20px;
    height: 20px;
    border-radius: 4px;
  }
  .newtab {
    width: 28px;
    height: 28px;
    margin: 0 0 2px 4px;
    border-radius: 6px;
  }
  .tabmenu:hover,
  .tabclose:hover,
  .newtab:hover {
    color: var(--text);
    background: #3a3a3d;
  }
  .tabmenu svg,
  .tabclose svg,
  .newtab svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }
  .newtab svg {
    width: 12px;
    height: 12px;
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
  .devmenu {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 260px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .devmenu svg {
    flex: none;
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
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
  .listhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 12px 0;
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
    grid-row: 3 / 5;
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
