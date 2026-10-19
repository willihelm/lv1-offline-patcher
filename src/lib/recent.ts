// Recently opened showfiles, persisted in the webview's localStorage (most recent first).

const KEY = "recentShows";
const MAX = 8;

export function load(): string[] {
  try {
    const list = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((p) => typeof p === "string") : [];
  } catch {
    return [];
  }
}

function store(list: string[]): string[] {
  localStorage.setItem(KEY, JSON.stringify(list));
  return list;
}

export const add = (path: string) => store([path, ...load().filter((p) => p !== path)].slice(0, MAX));
export const remove = (path: string) => store(load().filter((p) => p !== path));
