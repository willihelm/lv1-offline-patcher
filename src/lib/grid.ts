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

export type PatchView = "input" | "output" | "d2d";
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

export type CopyPart = "inputs" | "outputs" | "both";

/**
 * Ops that give `to` the patch of `from`, channel by channel. Inputs: every destination fed by an
 * input of `from` (channels, Talkback, device outputs) gets the same input of `to` instead, and the
 * other patches of those inputs of `to` are removed. As a destination has one source, this moves
 * the patch away from `from`. Outputs: every output of `to` gets the source of the same output of
 * `from`, or is unpatched. `skipped` counts patches of `from` on channels `to` doesn't have.
 */
export function copyPatchOps(show: ShowState, from: Device, to: Device, part: CopyPart) {
  const ops = new Map<string, PatchOp>();
  let skipped = 0;
  const current = new Map(show.routes.map((r) => [dstKey(r.dst), r.src] as const));
  const has = (ports: Port[], channel: number) => ports.some((p) => p.channel === channel);
  if (part !== "outputs") {
    const shared = (ch: number) => has(from.inPorts, ch) && has(to.inPorts, ch);
    for (const { src, dst } of show.routes)
      if (src.cluster === CLUSTER.HW_IN && src.index === to.idx && shared(src.channel)) ops.set(dstKey(dst), { dst, src: null });
    for (const { src, dst } of show.routes) {
      if (src.cluster !== CLUSTER.HW_IN || src.index !== from.idx) continue;
      if (has(to.inPorts, src.channel)) ops.set(dstKey(dst), { dst, src: { ...src, index: to.idx } });
      else skipped++;
    }
  }
  if (part !== "inputs") {
    for (const { dst } of show.routes)
      if (dst.cluster === CLUSTER.HW_OUT && dst.index === from.idx && !has(to.outPorts, dst.channel)) skipped++;
    for (const port of to.outPorts) {
      if (!has(from.outPorts, port.channel)) continue;
      const dst = { cluster: CLUSTER.HW_OUT, index: to.idx, section: -1, channel: port.channel };
      ops.set(dstKey(dst), { dst, src: current.get(dstKey({ ...dst, index: from.idx })) ?? null });
    }
  }
  // leave out ops that change nothing
  const same = (a?: Src | null, b?: Src | null) => (a && b ? key(a) === key(b) : !a && !b);
  return { ops: [...ops.values()].filter((op) => !same(current.get(dstKey(op.dst)), op.src)), skipped };
}
