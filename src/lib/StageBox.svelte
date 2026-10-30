<script lang="ts">
  // Read-only view of one device's connectors with what is patched to them, so the stage box can be cabled without
  // a list. Models with a known panel are drawn front and back as on the hardware; others as connector banks.
  import { socketBanks, type ListRow, type Socket as SocketData } from "./grid";
  import { colWidths, faceWidth, panelFor, STEREO_WIDTH, type BoxFace, type Conn, type PanelBlock, type PanelNode, type PanelSocket } from "./panels";
  import Socket from "./Socket.svelte";

  let {
    model,
    inputs,
    outputs,
    part,
    face,
  }: { model: string; inputs: ListRow[]; outputs: ListRow[]; part: "all" | "inputs" | "outputs"; face: BoxFace } = $props();

  const panel = $derived(panelFor(model));
  const faces = $derived(
    panel
      ? [
          { title: "Front", nodes: panel.front, show: face !== "back" },
          { title: "Back", nodes: panel.back, show: face !== "front" },
        ].filter((f) => f.show)
      : [],
  );

  /** connector cells across the wider side; front and back share one scale */
  const width = $derived(panel ? Math.max(faceWidth(panel.front), faceWidth(panel.back), 8) : 0);
  const defaultText: Record<string, string> = { power: "Power", psu: "PSU 24VDC" };

  // list rows are numbered from 1, panel channels from 0
  const byChannel = (rows: ListRow[]) => new Map(rows.map((r) => [r.channel - 1, r]));
  const inRows = $derived(byChannel(inputs));
  const outRows = $derived(byChannel(outputs));

  function socketOf(s: PanelSocket): SocketData {
    const rows = s.out ? outRows : inRows;
    return {
      label: s.label,
      lines: s.channels.map((ch, i) => ({ side: s.channels.length > 1 ? (i ? "R" : "L") : "", patch: rows.get(ch)?.patch ?? [] })),
      phantom: !s.out && s.channels.length === 1 ? (rows.get(s.channels[0])?.preamp?.phantom ?? null) : null,
    };
  }
  /** for the tooltip: "Input 3", "AES Out 1" */
  const typeOf = (b: PanelBlock, s: PanelSocket) =>
    b.title && b.title !== "Input" && b.title !== "Output" ? b.title : s.out ? "Output" : "Input";

  const sections = $derived(
    [
      { title: "Inputs", out: false, banks: socketBanks(inputs), show: part !== "outputs" },
      { title: "Outputs", out: true, banks: socketBanks(outputs), show: part !== "inputs" },
    ].filter((s) => s.show && s.banks.length),
  );
  const connector = (type: string): Conn => (/Phones|Inst/.test(type) ? "jack" : /Mic|Line|AES/.test(type) ? "xlr" : "port");
</script>

<div class="stagebox">
  {#if panel}
    <div class="faces" style:--w={width}>
      {#each faces as f (f.title)}
        <section>
          <h3>{f.title}</h3>
          <div class="rack" class:desktop={panel.desktop} style:--units={panel.units}>
            {#if !panel.desktop}<div class="ear"><span></span><span></span></div>{/if}
            <div class="face">
              {#each f.nodes as n}{@render node(n)}{/each}
            </div>
            {#if !panel.desktop}<div class="ear"><span></span><span></span></div>{/if}
          </div>
        </section>
      {/each}
    </div>
  {:else}
    {#each sections as s (s.title)}
      <section>
        <h3>{s.title}</h3>
        <div class="banks">
          {#each s.banks as b (b.type)}
            <div class="bank">
              <div class="type">{b.type || "Other"}</div>
              <div class="sockets" style:--n={Math.min(8, b.sockets.length)} style:--cw={b.sockets.some((k) => k.lines.length > 1) ? STEREO_WIDTH : 1}>
                {#each b.sockets as k, i (i)}
                  <Socket socket={k} conn={connector(b.type)} out={s.out} type={b.type} />
                {/each}
              </div>
            </div>
          {/each}
        </div>
      </section>
    {:else}
      <div class="empty">Nothing to show.</div>
    {/each}
  {/if}
</div>


{#snippet node(n: PanelNode)}
  {#if n === "spacer"}
    <div class="spacer"></div>
  {:else if "row" in n}
    <div class="row">{#each n.row as c}{@render node(c)}{/each}</div>
  {:else if "stack" in n}
    <div class="stack">{#each n.stack as c}{@render node(c)}{/each}</div>
  {:else}
    {@render block(n)}
  {/if}
{/snippet}

{#snippet block(b: PanelBlock)}
  <div class="block" class:inert={!b.items.some((it) => it.kind === "socket")} style:align-self={b.align ?? "start"}>
    {#if b.title}<div class="btitle">{b.title}</div>{/if}
    <div class="bgrid" style:grid-template-columns={colWidths(b).map((w) => `calc(var(--u) * ${w})`).join(" ")}>
      {#each b.items as it}
        {#if it.kind === "socket"}
          <Socket socket={socketOf(it)} conn={it.conn} out={it.out} knob={it.knob} type={typeOf(b, it)} />
        {:else if it.kind === "gap"}
          <div></div>
        {:else if it.kind === "fan"}
          <svg class="deco fan" viewBox="0 0 100 100" aria-hidden="true">
            <defs>
              <pattern id="grille" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <rect width="7" height="7" class="hole" /><path d="M0 0h7M0 0v7" class="wire" />
              </pattern>
            </defs>
            <rect x="2" y="2" width="96" height="96" rx="6" class="frame" />
            <circle cx="50" cy="50" r="44" fill="url(#grille)" />
          </svg>
        {:else if it.kind === "logo"}
          <div class="deco logo">{it.text}</div>
        {:else if it.kind === "status"}
          <div class="deco status"><span class="statusled"></span><span>{it.text ?? "Status"}</span></div>
        {:else}
          <div class="deco">
            {#if it.kind === "power"}
              <svg class="power" viewBox="0 0 40 60" aria-hidden="true">
                <rect x="10" y="4" width="20" height="14" rx="2" class="frame" />
                <rect x="17" y="7" width="6" height="8" rx="1" class="switch" />
                <path d="M6 28h28v18l-6 8H12l-6-8z" class="frame" />
                <path d="M14 36v8M20 36v8M26 36v8" class="wire" />
              </svg>
            {:else if it.kind === "psu"}
              <svg class="small" viewBox="0 0 40 40" aria-hidden="true">
                <circle cx="20" cy="20" r="15" class="frame" />
                <circle cx="15" cy="15" r="2" class="pinlight" /><circle cx="25" cy="15" r="2" class="pinlight" />
                <circle cx="15" cy="25" r="2" class="pinlight" /><circle cx="25" cy="25" r="2" class="pinlight" />
              </svg>
            {:else if it.kind === "ethercon"}
              <svg class="small" viewBox="0 0 40 40" aria-hidden="true">
                <circle cx="20" cy="20" r="17" class="frame" />
                <rect x="11" y="13" width="18" height="14" rx="1.5" class="hole" />
                <rect x="15" y="25" width="10" height="3" class="hole" />
              </svg>
            {:else if it.kind === "rj45"}
              <svg class="small" viewBox="0 0 40 40" aria-hidden="true">
                <rect x="7" y="9" width="26" height="22" rx="2" class="frame" />
                <path d="M11 13h18v12h-5v3h-8v-3h-5z" class="hole" />
              </svg>
            {:else if it.kind === "bnc"}
              <svg class="small" viewBox="0 0 40 40" aria-hidden="true">
                <circle cx="20" cy="20" r="13" class="frame" />
                <circle cx="20" cy="20" r="8" class="hole" />
                <circle cx="20" cy="20" r="2" class="pinlight" />
              </svg>
            {:else if it.kind === "dsub"}
              <svg class="dsub" viewBox="0 0 100 34" aria-hidden="true">
                <path d="M6 4h88l-8 26H14z" class="frame" />
                <path d="M12 8h76l-6 18H18z" class="hole" />
              </svg>
            {/if}
            <span>{it.text ?? defaultText[it.kind] ?? ""}</span>
          </div>
        {/if}
      {/each}
    </div>
  </div>
{/snippet}

<style>
  .stagebox {
    --face: #1e1e20;
    --rack: #19191b;
    --ear: #2c2c2f;
    --metal: #4a4a4e;
    --hole: #0b0b0c;
    --pin: #c9c9cc;
    --card: #303033;
    --line: #444;
    --u: 104px;
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 12px 12px;
    container-type: inline-size;
  }
  h3 {
    margin: 16px 0 8px;
    font-size: 16px;
  }

  /* hardware panels: everything scales with the width (--w connector cells across), so a whole panel fits on
     screen and on paper */
  .faces {
    --u: clamp(54px, calc((100cqw - 90px) / var(--w)), 100px);
  }
  .rack {
    display: flex;
    min-height: calc(var(--units) * var(--u) * 1.15);
    width: max-content;
    min-width: 100%;
    box-sizing: border-box;
    background: var(--rack);
    border: 1px solid var(--line);
    border-radius: 6px;
    break-inside: avoid;
  }
  /* desktop devices and consoles: as high as their connectors need */
  .rack.desktop {
    min-height: 0;
  }
  .ear {
    flex: none;
    width: calc(var(--u) * 0.28);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: center;
    padding: calc(var(--u) * 0.15) 0;
    background: var(--ear);
  }
  .ear:first-child {
    border-radius: 6px 0 0 6px;
  }
  .ear:last-child {
    border-radius: 0 6px 6px 0;
  }
  .ear span {
    width: calc(var(--u) * 0.1);
    height: calc(var(--u) * 0.16);
    border-radius: 999px;
    background: var(--hole);
    opacity: 0.35;
  }
  .face {
    flex: 1;
    display: flex;
    gap: calc(var(--u) * 0.3);
    padding: calc(var(--u) * 0.12) calc(var(--u) * 0.2);
    background: var(--face);
  }
  .spacer {
    flex: 1;
  }
  .row {
    flex: 1;
    display: flex;
    gap: calc(var(--u) * 0.3);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: calc(var(--u) * 0.15);
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .btitle {
    font-size: max(9px, calc(var(--u) * 0.1));
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
    text-align: center;
    border-bottom: 1px solid var(--line);
    padding-bottom: 2px;
  }
  /* same row height in every block, so that rows of neighboring blocks line up as on the hardware */
  .bgrid {
    display: grid;
    grid-auto-rows: minmax(calc(var(--u) * 1.08), auto);
    gap: calc(var(--u) * 0.08) calc(var(--u) * 0.05);
    align-items: start;
  }
  .deco {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    font-size: max(9px, calc(var(--u) * 0.1));
    color: var(--text-dim);
    text-align: center;
    align-self: center;
  }
  /* everything that is not a patch point stays in the background, for orientation only */
  .deco,
  .fan,
  .inert .btitle {
    opacity: 0.22;
  }
  .small {
    width: calc(var(--u) * 0.44);
    height: calc(var(--u) * 0.44);
  }
  .dsub {
    width: calc(var(--u) * 0.9);
    height: calc(var(--u) * 0.31);
  }
  .power {
    width: calc(var(--u) * 0.45);
    height: calc(var(--u) * 0.68);
  }
  /* !important: the column widths are set inline */
  .bgrid:has(.fan) {
    grid-template-columns: auto !important;
    grid-auto-rows: auto;
  }
  .fan {
    width: calc(var(--u) * 2.6);
    height: calc(var(--u) * 2.6);
  }
  .frame {
    fill: var(--metal);
  }
  .hole {
    fill: var(--hole);
  }
  .wire {
    fill: none;
    stroke: var(--metal);
    stroke-width: 1.2;
  }
  .switch {
    fill: var(--hole);
  }
  .pinlight {
    fill: var(--pin);
  }
  .status {
    flex-direction: row;
  }
  .statusled {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-dim);
  }
  .logo {
    font-size: max(11px, calc(var(--u) * 0.16));
    font-weight: 800;
    font-style: italic;
    letter-spacing: 0.04em;
  }

  /* generic connector banks */
  .banks {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
  }
  .bank {
    background: var(--face);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 8px 10px 10px;
    break-inside: avoid;
  }
  .type {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
    margin-bottom: 6px;
  }
  .sockets {
    display: grid;
    /* --cw: stereo connectors are wider, for their L/R labels */
    grid-template-columns: repeat(var(--n), calc(var(--u) * var(--cw)));
    gap: 8px;
  }
  .empty {
    padding: 24px;
    color: var(--text-dim);
  }

  @media print {
    .stagebox {
      --face: #fff;
      --rack: #fff;
      --ear: #eee;
      --metal: #bbb;
      --hole: #fff;
      --pin: #333;
      --card: #fff;
      --line: #999;
      --text-dim: #555;
      --text: #000;
      color: #000;
      overflow: visible;
      padding: 0;
    }
    .ear span {
      border: 1px solid #999;
    }
    section {
      break-inside: avoid;
    }
  }
</style>
