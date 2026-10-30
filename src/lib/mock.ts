// Browser-only stand-in for the Rust backend, used for UI development via `npm run dev`.
// Loads /demo.json (a ShowState dump, see README) and mimics patch/undo/redo.
// Each "open" creates a new session with its own copy, so several tabs can be tried out.
import type { PatchOp, Route, ShowState } from "./types";

interface MockSession {
  show: ShowState;
  undoStack: Route[][];
  redoStack: Route[][];
}

const sessions = new Map<number, MockSession>();
let nextId = 0;
let show: ShowState | null = null;
let undoStack: Route[][] = [];
let redoStack: Route[][] = [];

const sameDst = (a: Route["dst"], b: Route["dst"]) =>
  a.cluster === b.cluster && a.index === b.index && a.section === b.section && a.channel === b.channel;

function state(): ShowState {
  if (!show) throw "No showfile opened";
  return { ...show, canUndo: undoStack.length > 0, canRedo: redoStack.length > 0, dirty: undoStack.length > 0 };
}

export async function invoke<T>(cmd: string, args: Record<string, any>): Promise<T> {
  switch (cmd) {
    case "open_show": {
      const res = await fetch("/demo.json");
      if (!res.ok) throw "Mock backend: public/demo.json not found";
      const id = ++nextId;
      const s: ShowState = await res.json();
      s.path = s.path.replace(/(\.emo)?$/i, ` ${id}$1`);
      sessions.set(id, { show: s, undoStack: [], redoStack: [] });
      return { id, show: { ...s, canUndo: false, canRedo: false, dirty: false } } as T;
    }
    case "window_sessions":
      return [] as T;
    case "close_session":
      sessions.delete(args.id);
      return undefined as T;
    case "new_window":
      window.open(location.href);
      return undefined as T;
    case "move_session":
      throw "Moving tabs between windows needs the desktop app";
    case "modified_times":
      return args.paths.map(() => null) as T;
  }
  // the session's state lives in the module variables while a command runs
  const session = sessions.get(args.id);
  if (!session) throw "Showfile is not open";
  ({ show, undoStack, redoStack } = session);
  try {
    return run<T>(cmd, args);
  } finally {
    Object.assign(session, { undoStack, redoStack });
  }
}

function run<T>(cmd: string, args: Record<string, any>): T {
  switch (cmd) {
    case "patch": {
      const s = state();
      undoStack.push(s.routes);
      redoStack = [];
      let routes = s.routes;
      for (const op of args.ops as PatchOp[]) {
        routes = routes.filter((r) => !sameDst(r.dst, op.dst));
        if (op.src) routes = [...routes, { id: null, src: op.src, dst: op.dst, status: op.dst.section === 1 ? 1 : 0 }];
      }
      show!.routes = routes;
      break;
    }
    case "undo":
      if (undoStack.length) {
        redoStack.push(show!.routes);
        show!.routes = undoStack.pop()!;
      }
      break;
    case "redo":
      if (redoStack.length) {
        undoStack.push(show!.routes);
        show!.routes = redoStack.pop()!;
      }
      break;
    case "set_tap": {
      const s = show!.sources.find((x) => x.cluster === args.cluster && x.index === args.index);
      if (s) s.tap = args.tap;
      break;
    }
    case "set_driver_channels": {
      const d = show!.devices.find((x) => x.idx === args.idx);
      if (!d) break;
      const n: number = args.count;
      const ports = Array.from({ length: n }, (_, c) => ({ channel: c, major: "DAW", minor: "SoundGrid", label: `${c + 1}` }));
      Object.assign(d, { ins: n, outs: n, inPorts: ports, outPorts: ports });
      show!.routes = show!.routes.filter(
        (r) =>
          !(r.src.cluster === 10 && r.src.index === d.idx && r.src.channel >= n) &&
          !(r.dst.cluster === 11 && r.dst.index === d.idx && r.dst.channel >= n),
      );
      break;
    }
    case "set_preamp": {
      const d = show!.devices.find((x) => x.idx === args.idx);
      const p = d?.preamps.find((x) => x.channel === args.channel);
      if (!d || !p) throw "No preamp";
      if (args.gain != null && d.gainRange) {
        const { min, max, step } = d.gainRange;
        p.gain = Math.min(max, Math.max(min, Math.round(args.gain / step) * step));
      }
      if (args.phantom != null) p.phantom = args.phantom;
      break;
    }
    case "save":
      undoStack = [];
      redoStack = [];
      return { show: state(), backup: null } as T;
  }
  return state() as T;
}
