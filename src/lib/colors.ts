// LV1 channel color palette (PARAM_COLOR_VALUE index), row-major as shown in LV1's color picker.
const PALETTE = [
  "#189ca4", "#008fff", "#31a300", "#00945f",
  "#6ec7c9", "#4e7386", "#ffb400", "#ff0073",
  "#0066ff", "#00d0ff", "#00feff", "#006e8a",
  "#74a7da", "#00a8af", "#00ee52", "#00ffac",
  "#a048cd", "#ff8e95", "#effa00", "#ff003a",
  "#e7e7e7", "#c40000", "#00ff00", "#a0a0a0",
  "#ff6500", "#ff00ff", "#e58ce2", "#ff4192",
  "#877600", "#a4667e", "#bc2c88", "#8d46ff",
];
const DEFAULT = PALETTE[0];

export const stripColor = (c: number | null) => (c == null ? DEFAULT : (PALETTE[c] ?? DEFAULT));
