import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PatchOp, ShowState } from "./types";
import * as mock from "./mock";

/** In a plain browser (vite dev without Tauri) a mock backend with demo data is used. */
const call = <T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> =>
  isTauri() ? invoke<T>(cmd, args) : mock.invoke<T>(cmd, args);

/** An opened showfile; `id` addresses it in all commands below. */
export interface Session {
  id: number;
  show: ShowState;
}

/** null if the file is already open: its tab (maybe in another window) is activated instead */
export const openShow = (path: string) => call<Session | null>("open_show", { path });
export const windowSessions = () => call<Session[]>("window_sessions");
export const closeSession = (id: number) => call<void>("close_session", { id });
export const newWindow = () => call<void>("new_window");
/** move a tab into the window with label `target`, or into a new window if null */
export const moveSession = (id: number, target: string | null) => call<void>("move_session", { id, target });

export const patch = (id: number, ops: PatchOp[]) => call<ShowState>("patch", { id, ops });
export const undo = (id: number) => call<ShowState>("undo", { id });
export const redo = (id: number) => call<ShowState>("redo", { id });
export const setTap = (id: number, cluster: number, index: number, tap: number) =>
  call<ShowState>("set_tap", { id, cluster, index, tap });
export const setDriverChannels = (id: number, idx: number, count: number) =>
  call<ShowState>("set_driver_channels", { id, idx, count });
export const setPreamp = (id: number, idx: number, channel: number, gain: number | null, phantom: boolean | null) =>
  call<ShowState>("set_preamp", { id, idx, channel, gain, phantom });
export const save = (id: number, path: string | null, force: boolean) =>
  call<{ show: ShowState; backup: string | null }>("save", { id, path, force });
export const exportText = (path: string, contents: string) => call<void>("export_text", { path, contents });
/** modification time (ms since the epoch) per path, null if the file can't be read */
export const modifiedTimes = (paths: string[]) => call<(number | null)[]>("modified_times", { paths });
/** print the window in landscape; the browser prints with its own dialog */
export const printLandscape = () => (isTauri() ? invoke<void>("print_landscape") : Promise.resolve(window.print()));
