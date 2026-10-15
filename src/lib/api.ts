import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PatchOp, ShowState } from "./types";
import * as mock from "./mock";

/** In a plain browser (vite dev without Tauri) a mock backend with demo data is used. */
const call = <T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> =>
  isTauri() ? invoke<T>(cmd, args) : mock.invoke<T>(cmd, args);

export const openShow = (path: string) => call<ShowState>("open_show", { path });
export const patch = (ops: PatchOp[]) => call<ShowState>("patch", { ops });
export const undo = () => call<ShowState>("undo");
export const redo = () => call<ShowState>("redo");
export const setTap = (cluster: number, index: number, tap: number) =>
  call<ShowState>("set_tap", { cluster, index, tap });
export const setDriverChannels = (idx: number, count: number) =>
  call<ShowState>("set_driver_channels", { idx, count });
export const save = (path: string | null, force: boolean) =>
  call<{ show: ShowState; backup: string | null }>("save", { path, force });
