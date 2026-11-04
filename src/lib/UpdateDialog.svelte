<script lang="ts">
  // What a newer release brings, and the button that installs it. Opens by itself once per
  // release, from the "Update to …" button in the top bar, and from "Check for Updates…".
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { installUpdate, type AvailableUpdate } from "./api";
  import { parseNotes } from "./changelog";

  let { update, onclose }: { update: AvailableUpdate; onclose: () => void } = $props();

  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());

  const blocks = $derived(parseNotes(update.notes));
  let installing = $state(false);
  let error = $state<string | null>(null);
  /** 0–1 while the download's length is known */
  let progress = $state<number | null>(null);

  async function install() {
    installing = true;
    error = null;
    progress = null;
    const unlisten = await listen<{ received: number; total: number | null }>("update-progress", ({ payload }) => {
      progress = payload.total ? Math.min(1, payload.received / payload.total) : null;
    });
    try {
      // resolves only by failing: on success the app restarts
      await installUpdate();
    } catch (e) {
      error = String(e);
    } finally {
      unlisten();
      installing = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  onclose={onclose}
  oncancel={(e) => installing && e.preventDefault()}
  aria-labelledby="update-title"
>
  <header>
    <span class="eyebrow">Update available</span>
    <h2 id="update-title">LV1 Offline Patcher {update.version}</h2>
    <span class="dim">You have {update.current}</span>
  </header>

  <div class="notes">
    {#each blocks as b, i (i)}
      {#if b.kind === "heading"}
        <h3>{b.text}</h3>
      {:else}
        <p class:item={b.kind === "item"}>{b.text}</p>
      {/if}
    {:else}
      <p class="dim">This release has no notes.</p>
    {/each}
  </div>

  <footer>
    {#if installing}
      <div class="progress">
        <div class="bar"><span class:unknown={progress === null} style:width="{(progress ?? 1) * 100}%"></span></div>
        <span class="dim">{progress === null ? "Downloading…" : `Downloading ${Math.round(progress * 100)} %`}</span>
      </div>
    {:else if error}
      <span class="error">{error}</span>
    {:else}
      <span class="dim">Installing restarts the app.</span>
    {/if}
    <div class="actions">
      <button onclick={() => dialog.close()} disabled={installing}>Later</button>
      <button class="primary" onclick={install} disabled={installing}>{error ? "Try Again" : "Install and Restart"}</button>
    </div>
  </footer>
</dialog>

<style>
  dialog {
    width: min(520px, 92vw);
    max-height: min(600px, 88vh);
    padding: 0;
    border: 1px solid #444;
    border-radius: 10px;
    background: var(--panel);
    color: var(--text);
    flex-direction: column;
  }
  dialog[open] {
    display: flex;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  header {
    padding: 16px 20px 12px;
    border-bottom: 1px solid #3a3a3c;
  }
  .eyebrow {
    display: block;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
  }
  h2 {
    margin: 2px 0;
    font-size: 18px;
  }
  .dim {
    font-size: 12px;
    color: var(--text-dim);
  }
  .notes {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 20px 14px;
    font-size: 13px;
    line-height: 1.45;
  }
  h3 {
    margin: 14px 0 6px;
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  p {
    margin: 4px 0;
  }
  .item {
    position: relative;
    padding-left: 16px;
  }
  .item::before {
    content: "";
    position: absolute;
    left: 3px;
    top: 0.55em;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent);
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    padding: 12px 20px;
    border-top: 1px solid #3a3a3c;
  }
  .error {
    font-size: 12px;
    color: #ff6b6b;
  }
  .progress {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    min-width: 160px;
  }
  .bar {
    height: 5px;
    overflow: hidden;
    background: var(--bg);
    border-radius: 3px;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .bar .unknown {
    animation: breathe 1.2s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.3;
    }
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
</style>
