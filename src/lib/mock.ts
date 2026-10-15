// Browser-only stand-in for the Rust backend, used for UI development via `npm run dev`.
// Loads /demo.json (a ShowState dump, see README) and mimics patch/undo/redo.
import type { PatchOp, Route, ShowState } from "./types";

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
      show = await res.json();
      undoStack = [];
      redoStack = [];
      break;
    }
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
    case "save":
      undoStack = [];
      redoStack = [];
      return { show: state(), backup: null } as T;
  }
  return state() as T;
}
