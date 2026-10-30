<script lang="ts">
  // One connector of the stage box view: number, 48V LED, the connector itself and a scribble strip with what is
  // patched to it. Sizes follow --u, the width of a connector cell.
  import type { ListEntry, Socket } from "./grid";
  import type { Conn } from "./panels";

  let { socket, conn, out, knob = false, type }: { socket: Socket; conn: Conn; out: boolean; knob?: boolean; type: string } =
    $props();

  /** just the channel name; the full label (with number) where there is no name, e.g. device ports */
  const short = (e: ListEntry) => e.name ?? e.label.replace(/ \(Input A\)$/, "").replace(/ \(Input B\)$/, " (B)");

  const tooltip = $derived.by(() => {
    const text = (p: ListEntry[]) =>
      p.map((e) => (e.channels?.length ? `${e.label} → ${e.channels.map((c) => c.label).join(", ")}` : e.label)).join(", ") || "–";
    const lines = socket.lines.map((l) => `${l.side ? l.side + ": " : ""}${text(l.patch)}`);
    return [`${type} ${socket.label}${socket.phantom ? "  48V" : ""}`, ...lines].join("\n");
  });
</script>

<div class="socket" class:stereo={socket.lines.length > 1} class:unpatched={socket.lines.every((l) => !l.patch.length)} title={tooltip}>
  <div class="top">
    <span class="num">{socket.label}</span>
    {#if socket.phantom !== null}
      <span class="led" class:on={socket.phantom} title={socket.phantom ? "48V on" : "48V off"}></span>
    {/if}
  </div>
  <div class="hw">
    {#if knob}
      <svg class="knob" viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="8.5" class="body" />
        <path d="M10 3v5" class="mark" />
      </svg>
    {/if}
    <svg class="conn {conn}" class:male={out} viewBox="0 0 40 40" aria-hidden="true">
      {#if conn === "xlr" || conn === "combo"}
        <circle cx="20" cy="20" r="17" class="body" />
        <circle cx="20" cy="20" r="12.5" class="face" />
        <circle cx="13.5" cy="16" r={out ? 2.2 : 2.7} class="pin" />
        <circle cx="26.5" cy="16" r={out ? 2.2 : 2.7} class="pin" />
        <circle cx="20" cy="27" r={out ? 2.2 : 2.7} class="pin" />
      {:else if conn === "jack"}
        <circle cx="20" cy="20" r="15" class="body" />
        <circle cx="20" cy="20" r="9" class="face" />
        <circle cx="20" cy="20" r="3.5" class="pin" />
      {:else}
        <rect x="7" y="9" width="26" height="22" rx="3" class="body" />
        <rect x="11" y="13" width="18" height="11" rx="1.5" class="face" />
      {/if}
    </svg>
  </div>
  <div class="lines">
    {#each socket.lines as l}
      <div class="line">
        {#if l.side}<span class="side">{l.side}</span>{/if}
        <div class="patches">
          {#each l.patch as e}
            <span class="entry" style:--c={e.color ?? "transparent"} class:colored={e.color}>
              {short(e)}
              {#each e.channels ?? [] as c}
                <span class="via">→ {short(c)}</span>
              {/each}
            </span>
          {:else}
            <span class="entry none">–</span>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .socket {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--u) * 0.04);
    /* as wide as its cell, which is wider for stereo phones */
    width: 100%;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 5px;
    height: calc(var(--u) * 0.14);
  }
  .num {
    font-size: max(10px, calc(var(--u) * 0.125));
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .unpatched .num {
    color: var(--text-dim);
  }
  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #3a1212;
    border: 1px solid #000;
  }
  .led.on {
    background: #ff2a2a;
    box-shadow: 0 0 6px 1px #ff2a2a;
  }
  .hw {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .conn {
    width: calc(var(--u) * 0.44);
    height: calc(var(--u) * 0.44);
  }
  /* not a patch point: kept in the background like the other panel details */
  .knob {
    width: calc(var(--u) * 0.24);
    height: calc(var(--u) * 0.24);
    opacity: 0.3;
  }
  .knob .body {
    fill: #111;
    stroke: var(--metal);
  }
  .knob .mark {
    stroke: var(--pin);
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .conn .body {
    fill: var(--metal);
  }
  .conn .face {
    fill: var(--hole);
  }
  .conn .pin {
    fill: var(--hole);
    stroke: var(--pin);
    stroke-width: 1;
  }
  .conn.male .pin,
  .conn.jack .pin {
    fill: var(--pin);
    stroke: none;
  }
  .lines {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 3px;
  }
  .side {
    width: 8px;
    font-size: 9px;
    font-weight: 700;
    color: var(--text-dim);
  }
  .patches {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .entry {
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--card);
    font-size: max(9px, calc(var(--u) * 0.115));
    line-height: 1.25;
    overflow: hidden;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    print-color-adjust: exact;
    -webkit-print-color-adjust: exact;
  }
  .stereo .entry {
    -webkit-line-clamp: 1;
    line-clamp: 1;
  }
  .entry.colored {
    border-left: 4px solid var(--c);
  }
  .entry.none {
    color: var(--text-dim);
    text-align: center;
    background: transparent;
  }
  .via {
    display: block;
    white-space: nowrap;
    font-size: 0.9em;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  @media print {
    .entry {
      border: 1px solid #bbb;
    }
    .entry.none {
      border: none;
    }
    .led {
      background: #fff;
      border-color: #999;
    }
    .led.on {
      background: #e00;
      border-color: #e00;
      box-shadow: none;
      print-color-adjust: exact;
      -webkit-print-color-adjust: exact;
    }
    .knob .body {
      fill: #fff;
    }
  }
</style>
