// Turns the show state into the rows/columns of one patch matrix.
// Every cell is a (src, dst) pair: rows carry one side, columns the other.
import { stripColor } from "./colors";
import {
  CLUSTER,
  CLUSTER_LABEL,
  type Device,
  type Dst,
  type PatchOp,
  type Port,
  type GainRange,
  type Preamp,
  type ShowState,
  type Src,
  type Strip,
} from "./types";

export type PatchView = "input" | "output" | "d2d" | "list" | "stagebox";
export type OutputTab = "direct" | "busses";

export interface DataRow {
  heading?: undefined;
  label: string;
  number: string;
  side: string;
  color: string;
  /** port type of hardware rows, e.g. "An Mic/L" */
  kind?: string;
  /** output source setting of direct outs/busses */
  tap?: { cluster: number; index: number; value: number };
  src?: Src;
  dst?: Dst;
  /** patched to something outside the current matrix (e.g. another device) */
  note?: string;
}
export interface HeadingRow {
  heading: string;
}
export type Row = DataRow | HeadingRow;

export interface Col {
  port: Port;
  src?: Src;
  dst?: Dst;
  /** occupied by a source that is not a row of this matrix */
  note?: string;
  /** gain / 48V of the device input */
  preamp?: Preamp;
}

export interface Grid {
  title: string;
  rowTitle: string;
  rows: Row[];
  cols: Col[];
  /** "row:col" of patched cells */
  patched: Set<string>;
  /** device whose input preamps are shown above the columns */
  preamps?: { device: number; gainRange: GainRange | null };
}

const key = (a: { cluster: number; index: number; channel: number }) => `${a.cluster}/${a.index}/${a.channel}`;
const dstKey = (d: Dst) => `${key(d)}/${d.section}`;
const sideLabel = (width: number, ch: number) => (width === 1 ? "M" : ch === 0 ? "L" : "R");

export function deviceLabel(devices: Device[], idx: number) {
  return devices.find((d) => d.idx === idx)?.name ?? `IO ${idx + 1}`;
}

const MAJOR_ABBR: Record<string, string> = { Analog: "An", Digital: "Dg" };

/** e.g. "IOS-XL-1 Phones 1L" */
export function portLabel(devices: Device[], idx: number, channel: number, out: boolean) {
  const dev = devices.find((d) => d.idx === idx);
  const port = (out ? dev?.outPorts : dev?.inPorts)?.find((p) => p.channel === channel);
  return [dev?.name ?? `IO ${idx + 1}`, port?.minor, port?.label ?? channel + 1].filter(Boolean).join(" ");
}

export function srcLabel(show: ShowState, src: Src) {
  if (src.cluster === CLUSTER.HW_IN) return portLabel(show.devices, src.index, src.channel, false);
  const strip = show.sources.find((s) => s.cluster === src.cluster && s.index === src.index);
  const side = strip && strip.width === 2 ? (src.channel === 0 ? " L" : " R") : "";
  return `${strip?.name ?? `${CLUSTER_LABEL[src.cluster]} ${src.index + 1}`}${side}`;
}

function stripRows(strips: Strip[], make: (s: Strip, ch: number) => Partial<DataRow>, headings: boolean): Row[] {
  const rows: Row[] = [];
  let cluster = -1;
  for (const s of strips) {
    if (headings && s.cluster !== cluster) rows.push({ heading: CLUSTER_LABEL[s.cluster] ?? "" });
    cluster = s.cluster;
    for (let ch = 0; ch < s.width; ch++) {
      rows.push({
        label: s.name,
        number: s.cluster === CLUSTER.TALKBACK ? "TB" : String(s.index + 1),
        side: sideLabel(s.width, ch),
        color: stripColor(s.color),
        ...make(s, ch),
      });
    }
  }
  return rows;
}

export function inputGrid(show: ShowState, device: Device, section: number): Grid {
  const bySrc = new Map(
    show.routes.filter((r) => r.src.cluster === CLUSTER.HW_IN).map((r) => [dstKey(r.dst), r.src] as const),
  );
  const rows = stripRows(
    show.inputs,
    (s, ch) => {
      const dst = { cluster: s.cluster, index: s.index, section, channel: ch };
      const src = bySrc.get(dstKey(dst));
      return { dst, note: src && src.index !== device.idx ? srcLabel(show, src) : undefined };
    },
    false,
  );
  const cols = device.inPorts.map((port) => ({
    port,
    src: { cluster: CLUSTER.HW_IN, index: device.idx, channel: port.channel },
    preamp: device.preamps.find((p) => p.channel === port.channel),
  }));
  const colOf = columnIndex(cols);
  const patched = new Set<string>();
  rows.forEach((row, r) => {
    if (row.heading !== undefined || !row.dst) return;
    const src = bySrc.get(dstKey(row.dst));
    const c = src && src.index === device.idx ? colOf.get(src.channel) : undefined;
    if (c !== undefined) patched.add(`${r}:${c}`);
  });
  const preamps = device.preamps.length ? { device: device.idx, gainRange: device.gainRange } : undefined;
  return { title: device.name, rowTitle: "Channel Name", rows, cols, patched, preamps };
}

/** Matrix with hardware outputs of `device` as columns and the given sources as rows. */
function outputMatrix(show: ShowState, device: Device, rows: Row[], rowTitle: string): Grid {
  const rowBySrc = new Map<string, number>();
  rows.forEach((row, r) => row.heading === undefined && row.src && rowBySrc.set(key(row.src), r));
  const cols: Col[] = device.outPorts.map((port) => ({
    port,
    dst: { cluster: CLUSTER.HW_OUT, index: device.idx, section: -1, channel: port.channel },
  }));
  const colOf = columnIndex(cols);
  const patched = new Set<string>();
  for (const r of show.routes) {
    const c = colOf.get(r.dst.channel);
    if (r.dst.cluster !== CLUSTER.HW_OUT || r.dst.index !== device.idx || c === undefined) continue;
    const row = rowBySrc.get(key(r.src));
    if (row !== undefined) patched.add(`${row}:${c}`);
    else cols[c].note = srcLabel(show, r.src);
  }
  return { title: device.name, rowTitle, rows, cols, patched };
}

/** "direct": channel direct outs, "busses": groups, auxes, mains, matrices, cue */
export function outputGrid(show: ShowState, device: Device, tab: OutputTab): Grid {
  const direct = tab === "direct";
  const rows = stripRows(
    show.sources.filter((s) => (s.cluster === CLUSTER.INPUT) === direct),
    (s, ch) => ({
      src: { cluster: s.cluster, index: s.index, channel: ch },
      tap: s.tap == null ? undefined : { cluster: s.cluster, index: s.index, value: s.tap },
    }),
    !direct,
  );
  return outputMatrix(show, device, rows, "Channel Name");
}

export function deviceGrid(show: ShowState, from: Device, to: Device): Grid {
  const rows: Row[] = from.inPorts.map((port) => ({
    label: from.name,
    number: String(port.channel + 1),
    kind: [MAJOR_ABBR[port.major] ?? port.major, port.minor].filter(Boolean).join("  "),
    side: port.label,
    color: from.isDaw ? "#2bd17e" : "#3d8bfd",
    src: { cluster: CLUSTER.HW_IN, index: from.idx, channel: port.channel },
  }));
  return outputMatrix(show, to, rows, "Device Name");
}

/** device channel -> column position (ports can skip hidden channels) */
function columnIndex(cols: Col[]) {
  return new Map(cols.map((col, c) => [col.port.channel, c]));
}

export function cellOp(grid: Grid, r: number, c: number, on: boolean): PatchOp | null {
  const row = grid.rows[r];
  const col = grid.cols[c];
  if (!row || row.heading !== undefined || !col) return null;
  const dst = (row.dst ?? col.dst)!;
  const src = (row.src ?? col.src)!;
  return { dst, src: on ? src : null };
}

/** Cells of a diagonal drag from (r0,c0) to (r1,c1), skipping heading rows. */
export function diagonal(grid: Grid, r0: number, c0: number, r1: number, c1: number): [number, number][] {
  const dataRows = grid.rows.flatMap((row, i) => (row.heading === undefined ? [i] : []));
  const a = dataRows.indexOf(r0);
  let b = dataRows.findIndex((i) => i >= r1);
  if (a < 0) return [];
  if (b < 0) b = dataRows.length - 1;
  const dr = b - a;
  const dc = c1 - c0;
  const n = Math.min(Math.abs(dr), Math.abs(dc)) + 1;
  const sr = Math.sign(dr) || 1;
  const sc = Math.sign(dc) || 1;
  const cells: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const r = dataRows[a + i * sr];
    const c = c0 + i * sc;
    if (r === undefined || c < 0 || c >= grid.cols.length) break;
    cells.push([r, c]);
  }
  return cells;
}

/** Ops that remove every patch of the given rows (for the Delete button). */
export function clearRowsOps(grid: Grid, rows: Iterable<number>): PatchOp[] {
  const ops: PatchOp[] = [];
  for (const r of rows) {
    const row = grid.rows[r];
    if (!row || row.heading !== undefined) continue;
    if (row.dst) {
      // input rows have at most one source, possibly on another device
      if (row.note || grid.cols.some((_, c) => grid.patched.has(`${r}:${c}`))) ops.push({ dst: row.dst, src: null });
    } else {
      grid.cols.forEach((col, c) => grid.patched.has(`${r}:${c}`) && ops.push({ dst: col.dst!, src: null }));
    }
  }
  return ops;
}

/**
 * Ops that repatch every channel direct out on `device` straight from the device input that feeds
 * the channel (Input A, else Input B), bypassing the mixer. A mono channel with a stereo direct out
 * sends its one input to both sides. `skipped` lists direct outs whose channel has no input patch.
 */
export function directToDeviceOps(show: ShowState, device: Device) {
  const inputOf = new Map(
    show.routes.filter((r) => r.src.cluster === CLUSTER.HW_IN).map((r) => [dstKey(r.dst), r.src] as const),
  );
  const ops: PatchOp[] = [];
  const skipped: string[] = [];
  for (const r of show.routes) {
    const { src, dst } = r;
    if (dst.cluster !== CLUSTER.HW_OUT || dst.index !== device.idx || src.cluster !== CLUSTER.INPUT) continue;
    const width = show.inputs.find((s) => s.cluster === src.cluster && s.index === src.index)?.width ?? 1;
    const channel = Math.min(src.channel, width - 1);
    const input = [0, 1]
      .map((section) => inputOf.get(dstKey({ cluster: src.cluster, index: src.index, section, channel })))
      .find(Boolean);
    if (input) ops.push({ dst, src: input });
    else skipped.push(srcLabel(show, src));
  }
  return { ops, skipped };
}

/**
 * Reverse of directToDeviceOps: repatches every device input on `device` as the direct out of the
 * channel it feeds (Input A before Input B, else the lowest channel number). For a mono channel with
 * a stereo direct out, the first output gets L and a second one from the same input gets R.
 * `skipped` lists device inputs that feed no channel.
 */
export function deviceToDirectOps(show: ShowState, device: Device) {
  const channelsOf = new Map<string, Dst[]>();
  const inputs = show.routes
    .filter((r) => r.src.cluster === CLUSTER.HW_IN && r.dst.cluster === CLUSTER.INPUT)
    .sort((a, b) => a.dst.section - b.dst.section || a.dst.index - b.dst.index || a.dst.channel - b.dst.channel);
  for (const r of inputs) channelsOf.set(key(r.src), [...(channelsOf.get(key(r.src)) ?? []), r.dst]);
  const width = (strips: Strip[], index: number) =>
    strips.find((s) => s.cluster === CLUSTER.INPUT && s.index === index)?.width ?? 1;
  const ops: PatchOp[] = [];
  const skipped: string[] = [];
  const used = new Set<string>();
  const outs = show.routes
    .filter((r) => r.dst.cluster === CLUSTER.HW_OUT && r.dst.index === device.idx && r.src.cluster === CLUSTER.HW_IN)
    .sort((a, b) => a.dst.channel - b.dst.channel);
  for (const { src, dst } of outs) {
    const ch = channelsOf.get(key(src))?.[0];
    if (!ch) {
      skipped.push(srcLabel(show, src));
      continue;
    }
    let direct = { cluster: CLUSTER.INPUT, index: ch.index, channel: ch.channel };
    // mono input, stereo direct out: L is taken already, so this one becomes R
    if (width(show.inputs, ch.index) === 1 && width(show.sources, ch.index) === 2 && used.has(key(direct)))
      direct = { ...direct, channel: 1 };
    used.add(key(direct));
    ops.push({ dst, src: direct });
  }
  return { ops, skipped };
}

/**
 * Ops for a virtual soundcheck: every output of `device` that carries a channel comes back on the
 * same-numbered input of `device`, patched to Input B of that channel. A channel is carried by its
 * direct out, or by a device input patched straight to the output (device to device) that feeds the
 * channel's Input A. If several outputs carry the same channel side, the lowest one wins; the R side
 * of a mono channel's stereo direct out is ignored. Patches that are already in place are left out.
 * `skipped` lists outputs that carry a channel but have no input of the same number on `device`.
 */
export function returnToInputBOps(show: ShowState, device: Device) {
  const width = (index: number) => show.inputs.find((s) => s.cluster === CLUSTER.INPUT && s.index === index)?.width ?? 1;
  const current = new Map(show.routes.map((r) => [dstKey(r.dst), r.src] as const));
  const ops = new Map<string, PatchOp>();
  const skipped: string[] = [];
  const outs = show.routes
    .filter((r) => r.dst.cluster === CLUSTER.HW_OUT && r.dst.index === device.idx)
    .sort((a, b) => a.dst.channel - b.dst.channel);
  for (const { src, dst } of outs) {
    const channels: Dst[] =
      src.cluster === CLUSTER.INPUT
        ? src.channel < width(src.index)
          ? [{ cluster: CLUSTER.INPUT, index: src.index, section: 1, channel: src.channel }]
          : []
        : src.cluster === CLUSTER.HW_IN
          ? show.routes
              .filter((r) => r.dst.cluster === CLUSTER.INPUT && r.dst.section === 0 && key(r.src) === key(src))
              .map((r) => ({ ...r.dst, section: 1 }))
          : [];
    if (!channels.length) continue;
    if (!device.inPorts.some((p) => p.channel === dst.channel)) {
      skipped.push(portLabel(show.devices, device.idx, dst.channel, true));
      continue;
    }
    const ret: Src = { cluster: CLUSTER.HW_IN, index: device.idx, channel: dst.channel };
    for (const ch of channels) if (!ops.has(dstKey(ch))) ops.set(dstKey(ch), { dst: ch, src: ret });
  }
  const unchanged = (op: PatchOp) => current.get(dstKey(op.dst)) && key(current.get(dstKey(op.dst))!) === key(op.src!);
  return { ops: [...ops.values()].filter((op) => !unchanged(op)), skipped };
}

export type CopyPart = "inputs" | "outputs";

/** One channel of a copy from one device to another, for the preview. */
export interface CopyItem {
  part: CopyPart;
  /** 1-based */
  channel: number;
  port: string;
  /** patch of the channel on the target device now, and after the copy */
  before: ListEntry[];
  after: ListEntry[];
  /** removes the patches of the target channel; applied before all `sets` */
  clears: PatchOp[];
  sets: PatchOp[];
  /** the target device has no such channel */
  missing: boolean;
}

/**
 * Copy of the patch of `from` to `to`, channel by channel. Inputs: everything an input of `from`
 * feeds (channels, Talkback, device outputs) is fed by the same input of `to` instead, and the
 * other patches of that input of `to` are removed. As a destination has one source, this moves the
 * patch away from `from`. Outputs: every output of `to` gets the source of the same output of
 * `from`, or is unpatched. Turn the chosen items into ops with copyOps.
 */
export function copyPlan(show: ShowState, from: Device, to: Device, parts: CopyPart[]): CopyItem[] {
  const listFrom = devicePatchList(show, from);
  const listTo = devicePatchList(show, to);
  const has = (ports: Port[], channel: number) => ports.some((p) => p.channel === channel);
  const items: CopyItem[] = [];
  if (parts.includes("inputs"))
    for (const port of from.inPorts) {
      const ch = port.channel;
      const missing = !has(to.inPorts, ch);
      const routes = (idx: number) => show.routes.filter((r) => r.src.cluster === CLUSTER.HW_IN && r.src.index === idx && r.src.channel === ch);
      if (missing && !routes(from.idx).length) continue;
      items.push({
        part: "inputs",
        channel: ch + 1,
        port: port.label,
        before: listTo.inputs.find((r) => r.channel === ch + 1)?.patch ?? [],
        after: listFrom.inputs.find((r) => r.channel === ch + 1)?.patch ?? [],
        clears: missing ? [] : routes(to.idx).map(({ dst }) => ({ dst, src: null })),
        sets: missing ? [] : routes(from.idx).map(({ src, dst }) => ({ dst, src: { ...src, index: to.idx } })),
        missing,
      });
    }
  if (parts.includes("outputs"))
    for (const port of from.outPorts) {
      const ch = port.channel;
      const missing = !has(to.outPorts, ch);
      const after = listFrom.outputs.find((r) => r.channel === ch + 1)?.patch ?? [];
      if (missing && !after.length) continue;
      const src = show.routes.find((r) => r.dst.cluster === CLUSTER.HW_OUT && r.dst.index === from.idx && r.dst.channel === ch)?.src;
      items.push({
        part: "outputs",
        channel: ch + 1,
        port: port.label,
        before: listTo.outputs.find((r) => r.channel === ch + 1)?.patch ?? [],
        after,
        clears: [],
        sets: missing ? [] : [{ dst: { cluster: CLUSTER.HW_OUT, index: to.idx, section: -1, channel: ch }, src: src ?? null }],
        missing,
      });
    }
  return items;
}

/** Ops of the chosen copy items, without the ones that change nothing. */
export function copyOps(show: ShowState, items: CopyItem[]) {
  const ops = new Map<string, PatchOp>();
  for (const op of items.flatMap((i) => i.clears)) ops.set(dstKey(op.dst), op);
  for (const op of items.flatMap((i) => i.sets)) ops.set(dstKey(op.dst), op);
  const current = new Map(show.routes.map((r) => [dstKey(r.dst), r.src] as const));
  const same = (a?: Src | null, b?: Src | null) => (a && b ? key(a) === key(b) : !a && !b);
  return [...ops.values()].filter((op) => !same(current.get(dstKey(op.dst)), op.src));
}

/** e.g. "Ch 1 Kick L", "FX 2 Voc Verb", "Talkback" */
function stripLabel(show: ShowState, a: { cluster: number; index: number; channel: number }, strips: Strip[]) {
  const strip = strips.find((s) => s.cluster === a.cluster && s.index === a.index);
  const prefix = a.cluster === CLUSTER.TALKBACK ? "" : `${a.cluster === CLUSTER.INPUT && strips === show.inputs ? "Ch" : CLUSTER_LABEL[a.cluster]} ${a.index + 1}`;
  const side = strip?.width === 2 ? (a.channel === 0 ? "L" : "R") : "";
  return [prefix, strip?.name ?? (prefix ? "" : CLUSTER_LABEL[a.cluster]), side].filter(Boolean).join(" ");
}

/** e.g. "Kick", "Voc Verb L"; undefined if the strip has no name */
function stripName(a: { cluster: number; index: number; channel: number }, strips: Strip[]) {
  const strip = strips.find((s) => s.cluster === a.cluster && s.index === a.index);
  if (!strip?.name) return undefined;
  return strip.width === 2 ? `${strip.name} ${a.channel === 0 ? "L" : "R"}` : strip.name;
}

export interface ListEntry {
  label: string;
  /** channel or bus name without its number, for the stage box */
  name?: string;
  /** channel color, for channels and busses */
  color?: string;
  /** device to device: the mixer channels the same device input feeds */
  channels?: ListEntry[];
}

export interface ListRow {
  channel: number;
  type: string;
  port: string;
  preamp?: Preamp;
  /** inputs: everything the input feeds; outputs: its source */
  patch: ListEntry[];
}

/** Patch list of `device`: every input with what it feeds, every output with its source. */
export function devicePatchList(show: ShowState, device: Device) {
  const type = (p: Port) => [p.major, p.minor].filter(Boolean).join(" ");
  const color = (strips: Strip[], a: { cluster: number; index: number }) => {
    const strip = strips.find((s) => s.cluster === a.cluster && s.index === a.index);
    return strip && stripColor(strip.color);
  };
  const dstEntry = (d: Dst): ListEntry =>
    d.cluster === CLUSTER.HW_OUT
      ? { label: portLabel(show.devices, d.index, d.channel, true) }
      : {
          label: `${stripLabel(show, d, show.inputs)}${d.cluster === CLUSTER.INPUT ? ` (Input ${d.section === 1 ? "B" : "A"})` : ""}`,
          name: stripName(d, show.inputs)?.concat(d.cluster === CLUSTER.INPUT && d.section === 1 ? " (B)" : ""),
          color: color(show.inputs, d),
        };
  const channelsOf = (s: Src) =>
    show.routes
      .filter((r) => r.dst.cluster !== CLUSTER.HW_OUT && key(r.src) === key(s))
      .sort((a, b) => a.dst.section - b.dst.section || a.dst.cluster - b.dst.cluster || a.dst.index - b.dst.index)
      .map((r) => dstEntry(r.dst));
  const srcEntry = (s: Src): ListEntry =>
    s.cluster === CLUSTER.HW_IN
      ? { label: portLabel(show.devices, s.index, s.channel, false), channels: channelsOf(s) }
      : { label: stripLabel(show, s, show.sources), name: stripName(s, show.sources), color: color(show.sources, s) };
  const inputs: ListRow[] = device.inPorts.map((port) => ({
    channel: port.channel + 1,
    type: type(port),
    port: port.label,
    preamp: device.preamps.find((p) => p.channel === port.channel),
    patch: show.routes
      .filter((r) => r.src.cluster === CLUSTER.HW_IN && r.src.index === device.idx && r.src.channel === port.channel)
      .map((r) => dstEntry(r.dst)),
  }));
  const outputs: ListRow[] = device.outPorts.map((port) => {
    const src = show.routes.find(
      (r) => r.dst.cluster === CLUSTER.HW_OUT && r.dst.index === device.idx && r.dst.channel === port.channel,
    )?.src;
    return { channel: port.channel + 1, type: type(port), port: port.label, patch: src ? [srcEntry(src)] : [] };
  });
  return { inputs, outputs };
}

/** One connector on the stage box: a mono port, or a stereo pair (phones, AES) on one connector. */
export interface Socket {
  label: string;
  /** one line per channel of the connector; `side` is "L"/"R" for pairs */
  lines: { side: string; patch: ListEntry[] }[];
  /** 48V of a mic input, null if the input has no phantom power */
  phantom: boolean | null;
}

/** Ports of one type in a row, e.g. the "Analog Mic/L" inputs. */
export interface SocketBank {
  type: string;
  sockets: Socket[];
}

/** Rows of the patch list as connectors grouped by port type, for the stage box view. */
export function socketBanks(rows: ListRow[]): SocketBank[] {
  const banks: SocketBank[] = [];
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    let bank = banks[banks.length - 1];
    if (bank?.type !== r.type) banks.push((bank = { type: r.type, sockets: [] }));
    const next = rows[i + 1];
    const pair = r.port.match(/^(\d*)L$/);
    if (pair && next?.type === r.type && next.port === `${pair[1]}R`) {
      bank.sockets.push({
        label: pair[1] || "1",
        lines: [
          { side: "L", patch: r.patch },
          { side: "R", patch: next.patch },
        ],
        phantom: null,
      });
      i++;
    } else {
      bank.sockets.push({ label: r.port, lines: [{ side: "", patch: r.patch }], phantom: r.preamp?.phantom ?? null });
    }
  }
  return banks;
}

/** Patch list of `device` as CSV (semicolon separated with decimal comma, for Excel/Numbers). */
export function devicePatchCsv(show: ShowState, device: Device) {
  const cell = (v: string) => (/[;"\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v);
  const { inputs, outputs } = devicePatchList(show, device);
  const line = (dir: string, r: ListRow) => {
    const gain = r.preamp?.gain == null ? "" : String(r.preamp.gain).replace(".", ",");
    const phantom = r.preamp?.phantom == null ? "" : r.preamp.phantom ? "on" : "off";
    const patch = r.patch.map((e) => (e.channels?.length ? `${e.label} → ${e.channels.map((c) => c.label).join(" / ")}` : e.label));
    return [dir, String(r.channel), r.type, r.port, gain, phantom, patch.join(", ")];
  };
  const lines = [
    ["Direction", "Channel", "Type", "Port", "Gain (dB)", "48V", "Patch"],
    ...inputs.map((r) => line("In", r)),
    ...outputs.map((r) => line("Out", r)),
  ];
  // BOM so that Excel reads umlauts as UTF-8
  return "\uFEFF" + lines.map((l) => l.map(cell).join(";")).join("\r\n") + "\r\n";
}
