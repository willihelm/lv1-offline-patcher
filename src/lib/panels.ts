// Physical front and back panels of I/O devices, so the stage box view can show each connector where it is on
// the hardware. Channels are 0-based channel indices of the device (see layout.rs); models without a panel here
// are shown as generic connector banks. Layouts are taken from product photos and the manufacturers' user guides.
// Not drawn: DiGiGrid IOC (its line and AES I/O are on D-Sub), DiGiGrid M (no line outputs on the hardware,
// layout.rs has two), and the MADI/DigiLink devices, where one connector carries 64 channels and more.

/** which side of a panel the stage box view shows */
export type BoxFace = "both" | "front" | "back";

/** combo = XLR/TRS combo input, xlr = female for inputs and male for outputs */
export type Conn = "xlr" | "combo" | "jack" | "port";

export interface PanelSocket {
  kind: "socket";
  conn: Conn;
  out: boolean;
  /** one channel, or L/R of a stereo connector */
  channels: number[];
  label: string;
  /** volume knob next to a headphones jack */
  knob?: boolean;
}

/** Parts of the panel that are not patch points, drawn for orientation. */
export interface PanelDeco {
  kind: "fan" | "power" | "psu" | "ethercon" | "rj45" | "bnc" | "dsub" | "status" | "logo";
  text?: string;
}

export type PanelItem = PanelSocket | PanelDeco | { kind: "gap" };

/** A printed section of the panel, filled row by row in `cols` columns. */
export interface PanelBlock {
  title?: string;
  cols: number;
  items: PanelItem[];
  /** vertical position in its row, top by default */
  align?: "start" | "center" | "end";
}

/** Parts of a panel side by side ("spacer" takes up the free width), or `stack`ed from top to bottom. */
export type PanelNode = PanelBlock | "spacer" | { row: PanelNode[] } | { stack: PanelNode[] };

/** a face is one row of parts */
export type PanelFace = PanelNode[];

export interface Panel {
  /** rack units; for desktop devices only the height of the drawing */
  units: number;
  /** no rack ears */
  desktop?: boolean;
  front: PanelFace;
  /** as seen from behind */
  back: PanelFace;
}

const isStereo = (it: PanelItem) => it.kind === "socket" && it.channels.length > 1;

/** width of a stereo connector in connector cells: room for "L"/"R" next to its labels */
export const STEREO_WIDTH = 1.3;

/** Width of each column of a block in connector cells: wider where it has stereo connectors. */
export const colWidths = (b: PanelBlock) =>
  Array.from({ length: b.cols }, (_, i) => (b.items.some((it, j) => j % b.cols === i && isStereo(it)) ? STEREO_WIDTH : 1));

/** Width of a face in connector cells, so the drawing can be scaled to fit. */
export function faceWidth(nodes: PanelNode[]): number {
  const width = (n: PanelNode): number => {
    if (n === "spacer") return 0.5;
    if ("row" in n) return faceWidth(n.row);
    if ("stack" in n) return Math.max(...n.stack.map(width));
    return colWidths(n).reduce((a, b) => a + b, 0) * 1.05 + (n.items.some((it) => it.kind === "fan") ? 1.6 : 0);
  };
  return nodes.reduce((sum, n) => sum + width(n), 0) + (nodes.length - 1) * 0.3;
}

const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => from + i);
const input = (n: number, conn: Conn = "combo"): PanelSocket => ({ kind: "socket", conn, out: false, channels: [n - 1], label: String(n) });
const output = (n: number, conn: Conn = "xlr"): PanelSocket => ({ kind: "socket", conn, out: true, channels: [n - 1], label: String(n) });
const stereo = (ch: number, out: boolean, conn: Conn, label: string, knob = false): PanelSocket => ({
  kind: "socket",
  conn,
  out,
  channels: [ch, ch + 1],
  label,
  knob,
});
const deco = (kind: PanelDeco["kind"], text?: string, align: PanelBlock["align"] = "center"): PanelBlock => ({
  cols: 1,
  items: [{ kind, text }],
  align,
});

// Waves IONIC 24 (3U). Inputs: mic 1-24 = ch 0-23, AES = 24/25. Outputs: line 1-18 = ch 0-17,
// phones 1/2 = 18/19 and 20/21, AES out 1/2 = 22/23 and 24/25.
const IONIC24: Panel = {
  units: 3,
  front: [
    { title: "Input", cols: 8, items: range(1, 24).map((n) => input(n)) },
    {
      title: "Output",
      cols: 6,
     
      items: [
        ...range(1, 4).map((n) => output(n)),
        { kind: "status" },
        stereo(18, true, "jack", "1", true),
        ...range(5, 8).map((n) => output(n)),
        { kind: "logo", text: "IONIC 24" },
        stereo(20, true, "jack", "2", true),
        ...range(9, 14).map((n) => output(n)),
      ],
    },
  ],
  back: [
    deco("power"),
    { title: "Output", cols: 2, items: [...range(15, 18).map((n) => output(n)), { kind: "psu" }] },
    deco("fan"),
    "spacer",
    { title: "SoundGrid", cols: 1, align: "end", items: [{ kind: "ethercon" }] },
    {
      title: "AES",
      cols: 3,
      align: "end",
      items: [stereo(22, true, "xlr", "Out 1"), stereo(24, false, "xlr", "In"), stereo(24, true, "xlr", "Out 2")],
    },
  ],
};

// Waves IONIC 16 (2U). Inputs: mic 1-16 = ch 0-15, AES = 16/17. Outputs: line 1-12 = ch 0-11,
// phones 1/2 = 12/13 and 14/15, AES out = 16/17.
const IONIC16: Panel = {
  units: 2,
  front: [
    { title: "Input", cols: 8, items: range(1, 16).map((n) => input(n)) },
    {
      title: "Output",
      cols: 6,
     
      items: [
        ...range(1, 4).map((n) => output(n)),
        { kind: "status" },
        stereo(12, true, "jack", "1", true),
        ...range(5, 8).map((n) => output(n)),
        { kind: "logo", text: "IONIC 16" },
        stereo(14, true, "jack", "2", true),
      ],
    },
  ],
  back: [
    deco("power"),
    { title: "Output", cols: 3, items: [{ kind: "gap" }, output(9), output(10), { kind: "psu" }, output(11), output(12)] },
    deco("fan"),
    "spacer",
    { title: "SoundGrid", cols: 1, align: "end", items: [{ kind: "ethercon" }] },
    { title: "AES", cols: 2, align: "end", items: [stereo(16, false, "xlr", "In"), stereo(16, true, "xlr", "Out")] },
  ],
};

// Waves eMotion LV1 Classic console. All I/O is on the back except the headphones on the front edge (one stereo
// output on a 1/8" and a 1/4" jack). Inputs: mic 1-16 = ch 0-15, AES = 16/17. Outputs: line 1-12 = ch 0-11,
// phones = 12/13, AES out 1/2 = 14/15 and 16/17.
const LV1CLASSIC: Panel = {
  units: 3,
  desktop: true,
  front: [
    "spacer",
    { title: "Phones", cols: 2, items: [stereo(12, true, "jack", '1/8"', true), stereo(12, true, "jack", '1/4"')] },
    deco("logo", "LV1 Classic"),
  ],
  back: [
    {
      stack: [
        { row: [deco("logo", "LV1 Classic"), "spacer", { title: "AC Input", cols: 2, items: [{ kind: "power", text: "1" }, { kind: "power", text: "2" }] }] },
        {
          row: [
            {
              stack: [
                { title: "Input", cols: 8, items: range(1, 16).map((n) => input(n)) },
                {
                  row: [
                    deco("rj45", "Network"),
                    {
                      title: "SoundGrid",
                      cols: 3,
                      items: [{ kind: "rj45", text: "3/4" }, { kind: "ethercon", text: "1" }, { kind: "ethercon", text: "2" }],
                    },
                  ],
                },
              ],
            },
            {
              stack: [
                { title: "Output", cols: 6, items: range(1, 12).map((n) => output(n)) },
                {
                  row: [
                    "spacer",
                    deco("status"),
                    {
                      title: "AES",
                      cols: 3,
                      items: [stereo(16, false, "xlr", "In"), stereo(14, true, "xlr", "Out 1"), stereo(16, true, "xlr", "Out 2")],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    },
  ],
};

// DiGiGrid IOS-XL (2U). Inputs: mic 1-8 = ch 0-7, AES (or S/PDIF, set in software) = 8/9. Outputs: line 1-8 = ch 0-7
// (TRS, also on the "Alt Line Out" D-Sub), AES out = 8/9, phones 1/2 = 10/11 and 12/13. MIDI, USB and the S/PDIF
// RCA jacks are left out.
const IOSXL: Panel = {
  units: 2,
  front: [
    { title: "Phones", cols: 2, items: [stereo(10, true, "jack", "1", true), stereo(12, true, "jack", "2", true)] },
    "spacer",
    deco("logo", "IOS XL"),
    deco("status", "Net"),
    "spacer",
    deco("logo", "DiGiGrid"),
  ],
  back: [
    deco("power", undefined, "end"),
    { title: "SoundGrid Network", cols: 4, align: "end", items: range(1, 4).map((n) => ({ kind: "rj45", text: String(n) })) },
    { stack: [{ title: "Alt Line Out 1-8", cols: 1, items: [{ kind: "dsub" }] }, { title: "Word Clock", cols: 2, items: [{ kind: "bnc" }, { kind: "bnc" }] }] },
    "spacer",
    {
      stack: [
        { title: "Mic/Line Input", cols: 8, items: range(1, 8).map((n) => input(n)) },
        {
          row: [
            "spacer",
            { title: "AES", cols: 2, items: [stereo(8, false, "xlr", "In"), stereo(8, true, "xlr", "Out")] },
            { title: "Line Out", cols: 4, items: [1, 3, 5, 7, 2, 4, 6, 8].map((n) => output(n, "jack")) },
          ],
        },
      ],
    },
  ],
};

// DiGiGrid IOX (1U). Inputs: mic 1-12 = ch 0-11. Outputs: line 1-6 = ch 0-5 (6/7 do not exist),
// phones 1-4 = 8/9, 10/11, 12/13, 14/15.
const IOX: Panel = {
  units: 1,
  front: [
    { title: "Phones", cols: 4, items: range(1, 4).map((n) => stereo(6 + 2 * n, true, "jack", String(n), true)) },
    deco("logo", "IOX"),
    deco("status", "Net"),
    "spacer",
    deco("logo", "DiGiGrid"),
  ],
  back: [
    deco("power"),
    { stack: [{ title: "Word Clock", cols: 2, items: [{ kind: "bnc" }, { kind: "bnc" }] }, { title: "SoundGrid", cols: 2, items: [{ kind: "rj45", text: "1" }, { kind: "rj45", text: "2" }] }] },
    { title: "Line Out", cols: 3, items: [1, 3, 5, 2, 4, 6].map((n) => output(n, "jack")) },
    { title: "Mic/Line Input", cols: 12, items: range(1, 12).map((n) => input(n)) },
  ],
};

// DiGiGrid D (desktop; the controls are on top). Inputs: mic 1-2 = ch 0-1 (XLR), instrument "3"/"4" = ch 2-3 (TRS).
// Outputs: "Mon" 1-2 = ch 0-1, "Line" 3-4 = ch 2-3 (TRS), phones = 4/5.
const D: Panel = {
  units: 1,
  desktop: true,
  front: [deco("logo", "DiGiGrid"), "spacer", { title: "Phones", cols: 1, items: [stereo(4, true, "jack", "")] }],
  back: [
    { title: "Mon", cols: 1, items: [output(1, "jack"), output(2, "jack")] },
    { title: "Line", cols: 1, items: [output(3, "jack"), output(4, "jack")] },
    { cols: 2, align: "center", items: [{ kind: "psu", text: "12V DC" }, { kind: "rj45", text: "PoE" }] },
    deco("logo", "D"),
    "spacer",
    { title: "Inst", cols: 1, items: [input(3, "jack"), input(4, "jack")] },
    { title: "Mic", cols: 2, items: [input(2, "xlr"), input(1, "xlr")] },
  ],
};

// DSPRO StageGrid 1000 (1U). Inputs: mic 1-8 = ch 0-7 (XLR), AES "Sync 1-2" = 8/9. Outputs: line 1-4 = ch 0-3,
// phones 1/2 = 4/5 and 6/7, AES out = 8/9.
const STAGEGRID1000: Panel = {
  units: 1,
  front: [
    deco("logo", "StageGrid 1000"),
    deco("status", "Net / Pwr"),
    { title: "Phones", cols: 2, items: [stereo(4, true, "jack", "1", true), stereo(6, true, "jack", "2", true)] },
    "spacer",
    { title: "Inputs", cols: 8, items: range(1, 8).map((n) => input(n, "xlr")) },
  ],
  back: [
    deco("power"),
    { title: "Outputs", cols: 4, items: range(1, 4).map((n) => output(n)) },
    { title: "AES/EBU", cols: 2, items: [stereo(8, false, "xlr", "Sync 1-2"), stereo(8, true, "xlr", "1-2")] },
    "spacer",
    deco("status", "Link"),
    { title: "SoundGrid", cols: 1, items: [{ kind: "ethercon" }] },
    deco("logo", "dspro"),
  ],
};

// DSPRO StageGrid 4000 (4U, all connectors on the front). Inputs: mic 1-32 = ch 0-31 (XLR), AES "Sync 1-2" ...
// "7-8" = 32/33 ... 38/39. Outputs: line 1-16 = ch 0-15, phones 1/2 = 16/17 and 18/19, AES out = 20/21 ... 26/27.
const STAGEGRID4000: Panel = {
  units: 4,
  front: [
    {
      stack: [
        {
          row: [
            { title: "Inputs", cols: 8, items: range(1, 32).map((n) => input(n, "xlr")) },
            { title: "Outputs", cols: 4, items: range(1, 16).map((n) => output(n)) },
            {
              title: "AES/EBU",
              cols: 2,
              items: range(0, 3).flatMap((i) => [
                stereo(32 + 2 * i, false, "xlr", `Sync ${2 * i + 1}-${2 * i + 2}`),
                stereo(20 + 2 * i, true, "xlr", `${2 * i + 1}-${2 * i + 2}`),
              ]),
            },
            { title: "Phones", cols: 1, items: [stereo(16, true, "jack", "1", true), stereo(18, true, "jack", "2", true)] },
          ],
        },
        {
          row: [
            "spacer",
            { title: "Word Clock", cols: 2, items: [{ kind: "bnc", text: "In" }, { kind: "bnc", text: "Out" }] },
            deco("status", "Net / Link"),
            { title: "SoundGrid", cols: 1, items: [{ kind: "ethercon" }] },
            deco("status", "Power A / B"),
            "spacer",
            deco("logo", "StageGrid 4000"),
          ],
        },
      ],
    },
  ],
  back: [
    deco("logo", "StageGrid 4000"),
    "spacer",
    { stack: [deco("fan"), { cols: 2, items: [{ kind: "power", text: "Power A" }, { kind: "power", text: "Power B" }] }] },
    "spacer",
  ],
};

/** Same normalization as model_key in layout.rs: "Waves IONIC 24" and "Ionic24" are "IONIC24". */
function modelKey(model: string) {
  const key = model.replace(/[^a-z0-9]/gi, "").toUpperCase();
  for (const brand of ["DIGIGRID", "WAVES", "DSPRO", "DIRECTOUT", "SOUNDSTUDIO", "JOECO", "PEAVEY"])
    if (key.startsWith(brand) && key.length > brand.length) return key.slice(brand.length);
  return key;
}

const PANELS: Record<string, Panel> = {
  IONIC24,
  IONIC16,
  LV1CLASSIC,
  // offline devices are named by model_by_id in layout.rs
  CLASSIC: LV1CLASSIC,
  IOSXL,
  IOX,
  D,
  STAGEGRID1000,
  STAGEGRID4000,
};

export const panelFor = (model: string): Panel | undefined => PANELS[modelKey(model)];
