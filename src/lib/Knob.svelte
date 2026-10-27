<script lang="ts">
  // Gain knob: drag up/down (Shift for fine) or arrow keys to turn, double-click or Enter to
  // type a value; the mouse wheel scrolls the list. Turning previews locally and reports one
  // change when it comes to rest, so a whole drag is a single undo step.
  let {
    value,
    min,
    max,
    step = 0.5,
    disabled = false,
    title,
    onchange,
    onedit,
  }: {
    value: number;
    min: number;
    max: number;
    step?: number;
    disabled?: boolean;
    title?: string;
    onchange: (value: number) => void;
    /** asks for a text field to type the value */
    onedit: () => void;
  } = $props();

  /** value while turning, before it is committed */
  let preview = $state<number | null>(null);
  let drag: { y: number; from: number; fine: boolean } | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const shown = $derived(preview ?? value);
  const frac = $derived(max > min ? (shown - min) / (max - min) : 0);
  // 270° sweep from bottom left (135°) clockwise to bottom right
  const angle = $derived(((135 + 270 * frac) * Math.PI) / 180);

  // a new value from the backend replaces the preview
  $effect(() => {
    void value;
    preview = null;
  });

  const snap = (v: number) => Math.min(max, Math.max(min, Math.round(v / step) * step));

  function commit() {
    clearTimeout(timer);
    if (preview === null) return;
    if (preview === value) preview = null;
    else onchange(preview);
  }

  /** turn by `delta` dB and commit once nothing happened for a moment */
  function nudge(delta: number) {
    preview = snap(shown + delta);
    clearTimeout(timer);
    timer = setTimeout(commit, 450);
  }

  function down(e: PointerEvent) {
    if (disabled || e.button !== 0) return;
    // no text selection while dragging; that also skips the default focus, so focus by hand
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.focus();
    el.setPointerCapture(e.pointerId);
    drag = { y: e.clientY, from: shown, fine: e.shiftKey };
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    // the full range over 200 px, a quarter of that speed with Shift
    if (e.shiftKey !== drag.fine) drag = { y: e.clientY, from: shown, fine: e.shiftKey };
    const perPx = (max - min) / (e.shiftKey ? 800 : 200);
    preview = snap(drag.from + (drag.y - e.clientY) * perPx);
  }

  function up() {
    if (!drag) return;
    drag = null;
    commit();
  }

  function key(e: KeyboardEvent) {
    if (disabled) return;
    const big = e.shiftKey ? 6 : step;
    const d = { ArrowUp: big, ArrowRight: big, ArrowDown: -big, ArrowLeft: -big, PageUp: 6, PageDown: -6 }[e.key];
    if (d !== undefined) nudge(d);
    else if (e.key === "Home") nudge(min - shown);
    else if (e.key === "End") nudge(max - shown);
    else if (e.key === "Enter") onedit();
    else return;
    e.preventDefault();
  }

  const at = (a: number, r: number) => `${16 + r * Math.cos(a)} ${16 + r * Math.sin(a)}`;
  // background track: start (135°) to end (45°), the long way round
  const track = `M ${at((135 * Math.PI) / 180, 12)} A 12 12 0 1 1 ${at((45 * Math.PI) / 180, 12)}`;
</script>

<div
  class="knob"
  class:disabled
  class:turning={preview !== null}
  role="slider"
  tabindex={disabled ? -1 : 0}
  aria-label="Gain"
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={shown}
  aria-valuetext="{shown} dB"
  aria-disabled={disabled}
  {title}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
  onblur={commit}
  ondblclick={() => !disabled && onedit()}
>
  <svg viewBox="0 0 32 32" aria-hidden="true">
    <path class="track" d={track} pathLength="100" />
    <path class="value" d={track} pathLength="100" stroke-dasharray="{frac * 100} 100" />
    <circle class="pointer" cx={16 + 12 * Math.cos(angle)} cy={16 + 12 * Math.sin(angle)} r="2.6" />
    <text x="16" y="16">{shown}</text>
  </svg>
</div>

<style>
  .knob {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    cursor: ns-resize;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    outline: none;
  }
  .knob:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .knob.disabled {
    cursor: default;
    opacity: 0.45;
  }
  svg {
    width: 94%;
    height: 94%;
    overflow: visible;
  }
  .track,
  .value {
    fill: none;
    stroke-width: 3;
    stroke-linecap: round;
  }
  .track {
    stroke: #48484c;
  }
  .value {
    stroke: var(--accent);
  }
  .knob.turning .value {
    stroke: #fff;
  }
  .pointer {
    fill: var(--text);
    stroke: var(--panel);
    stroke-width: 1;
  }
  text {
    fill: var(--text);
    font-size: 8.5px;
    font-weight: 700;
    text-anchor: middle;
    dominant-baseline: central;
  }
</style>
