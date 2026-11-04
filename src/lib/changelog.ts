// Release notes in the shape the update dialog draws them. The notes are the changelog
// release-please writes: a `## [1.2.0](compare-url) (date)` heading, `### Features` /
// `### Bug Fixes` and `* subject ([abc1234](commit-url))` items. Read here instead of rendered as
// HTML: the dialog needs headings and items only, and text that never becomes markup can't
// inject any.

export type Block = { kind: "heading" | "item" | "text"; text: string };

/** inline Markdown reduced to its words; commit and issue references dropped */
const plain = (line: string) =>
  line
    .replace(/,?\s*closes\s+\[#\d+\]\([^)]*\)/gi, "")
    .replace(/\s*\(\[[0-9a-f]{7,40}\]\([^)]*\)\)/gi, "")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\*\*|__|`/g, "")
    .trim();

export function parseNotes(notes: string | null): Block[] {
  const blocks: Block[] = [];
  for (const raw of (notes ?? "").split(/\r?\n/)) {
    const line = raw.trim();
    // the version heading: the dialog shows the version itself
    if (!line || /^#{1,2}\s+\[?v?\d+\.\d+/.test(line) || /^(\*\*)?full changelog/i.test(line)) continue;
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    const item = /^[-*+]\s+(.*)$/.exec(line);
    if (heading) blocks.push({ kind: "heading", text: plain(heading[1]) });
    else if (item) blocks.push({ kind: "item", text: plain(item[1]).replace(/^./, (c) => c.toUpperCase()) });
    else blocks.push({ kind: "text", text: plain(line) });
  }
  return blocks.filter((b) => b.text);
}

const SEEN_KEY = "updateSeen";

/** The dialog opens by itself once per version (in the first window that finds it); after
 * "Later" the button in the top bar opens it again. */
export function firstSighting(version: string): boolean {
  try {
    if (localStorage.getItem(SEEN_KEY) === version) return false;
    localStorage.setItem(SEEN_KEY, version);
  } catch {
    // no storage: it opens again at the next start
  }
  return true;
}
