/** One hardware channel, e.g. { major: "Analog", minor: "Phones", label: "1L" } */
export interface Port {
  /** channel index on the device; hidden channels leave gaps */
  channel: number;
  major: string;
  minor: string;
  label: string;
}

/** Preamp of a device input; null where the input has no such control */
export interface Preamp {
  channel: number;
  /** dB */
  gain: number | null;
  phantom: boolean | null;
}

/** mic preamp gain in dB */
export interface GainRange {
  min: number;
  max: number;
  step: number;
}

export interface Device {
  idx: number;
  name: string;
  kind: string;
  isDaw: boolean;
  ins: number;
  outs: number;
  inPorts: Port[];
  outPorts: Port[];
  preamps: Preamp[];
  /** set if gain can be edited */
  gainRange: GainRange | null;
}

export interface Strip {
  cluster: number;
  index: number;
  name: string;
  color: number | null;
  width: number;
  /** output source of direct outs/busses, see TAP_LABELS */
  tap: number | null;
}

/** driver channel counts LV1 offers for a DAW */
export const DRIVER_CHANNELS = [4, 8, 16, 32, 48, 64, 96, 128];

export const TAP_LABELS = ["INP", "PRE", "PST", "PSP"];

export interface Src {
  cluster: number;
  index: number;
  channel: number;
}

export interface Dst extends Src {
  /** 0 = Input A, 1 = Input B, -1 for hardware outputs */
  section: number;
}

export interface Route {
  id: number | null;
  src: Src;
  dst: Dst;
  status: number;
}

export interface PatchOp {
  dst: Dst;
  src: Src | null;
}

export interface ShowState {
  path: string;
  snapshot: string;
  devices: Device[];
  inputs: Strip[];
  sources: Strip[];
  routes: Route[];
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  lv1Open: boolean;
}

// cluster_type ids of the LV1 showfile
export const CLUSTER = {
  INPUT: 0,
  GROUP: 1,
  AUX: 2,
  MAIN: 3,
  CENTER: 4,
  MONO: 5,
  MATRIX: 6,
  CUE: 7,
  TALKBACK: 8,
  HW_IN: 10,
  HW_OUT: 11,
} as const;

export const CLUSTER_LABEL: Record<number, string> = {
  0: "Direct Out",
  1: "Group",
  2: "Aux",
  3: "Main",
  4: "Center",
  5: "Mono",
  6: "Matrix",
  7: "Cue",
  8: "Talkback",
};
