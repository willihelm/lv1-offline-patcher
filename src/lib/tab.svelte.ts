import type { OutputTab, PatchView } from "./grid";
import type { ShowState } from "./types";

let nextKey = 0;

/** A tab of the window: an opened showfile (`id` addresses its backend session) or, without
 * one, the welcome page. Keeps its own view so switching tabs returns to where you were. */
export class Tab {
  readonly key = ++nextKey;
  id = $state<number | null>(null);
  show = $state<ShowState | null>(null);
  view = $state<PatchView>("input");
  section = $state(0);
  outputTab = $state<OutputTab>("direct");
  devIdx = $state(-1);
  srcDevIdx = $state(-1);
  selected = $state(new Set<number>());
  message = $state("");

  constructor(session?: { id: number; show: ShowState }) {
    if (session) this.load(session);
  }

  load(session: { id: number; show: ShowState }) {
    this.id = session.id;
    this.show = session.show;
  }
}
