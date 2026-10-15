// Approximation of the LV1 channel color palette (PARAM_COLOR_VALUE index).
const PALETTE = [
  "#e53935", "#f4511e", "#fb8c00", "#fdd835", "#c0ca33", "#7cb342", "#43a047", "#00897b",
  "#00acc1", "#3cc4f0", "#1e88e5", "#3949ab", "#5e35b1", "#8e24aa", "#d81b60", "#ec407a",
  "#8d6e63", "#9e9e9e", "#607d8b", "#ffb74d", "#aed581", "#4db6ac", "#f58220", "#ba68c8",
  "#f06292", "#ffd54f", "#81c784", "#64b5f6", "#9575cd", "#e57373", "#ff8a65", "#a1887f",
];
const DEFAULT = "#1a9aa6";

export const stripColor = (c: number | null) => (c == null ? DEFAULT : (PALETTE[c] ?? DEFAULT));
