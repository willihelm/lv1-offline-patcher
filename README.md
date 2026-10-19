# LV1 Offline Patcher

Desktop app for macOS and Windows (Tauri 2 / Rust + Svelte 5) for editing the patch of a Waves LV1 showfile (`.emo`)
without the I/O hardware being online. The UI follows LV1's Patch page:

- **Input** (A/B): device inputs → channels (and Talkback)
- **Output**: Direct (channel direct outs) and Mix Busses (groups, auxes, mains, matrices, cue)
  → device outputs. The Source column sets the output source: INP, PRE, PST or PSP
- **Device to Device**: device inputs → device outputs

Click a cell to patch or unpatch it. Drag diagonally to patch a range; a drag that starts on a
patched cell unpatches instead. Click row names to select them (shift for a range, ⌘/Ctrl to
toggle), then press **Delete**. ⌘Z / ⇧⌘Z undo and redo, ⌘S saves, ⇧⌘S saves a patched copy
(on Windows: Ctrl+Z, Ctrl+Y, Ctrl+S, Ctrl+Shift+S).
Orange text marks patches that come from outside the current matrix (another device or source).

## How it works

The `.emo` file is an SQLite database. The app reads devices, channel names, colors and output
sources from the live mixer state (snapshot `-1`, "Active"). It changes only two things: the
`routes` table and the output source (INP/PRE/PST/PSP, `PARAM_OUTPUT_DIRECT_SOURCE`) in the
live state. Stored snapshots are not touched.

The file only stores channel counts per device, so the port types (Analog Line, Phones, AES, ...)
come from a table per device model in `src-tauri/src/layout.rs`. It covers the SoundGrid
interfaces from Waves' compatibility chart (DiGiGrid, IONIC, StageGrid, MADI/Dante bridges, ...);
only IOS-XL and IOX are verified against real showfiles, unknown models fall back to numbered AES. A mono channel with a
mono-to-stereo plugin (plugin code ending in `X`, e.g. RVerb) has a stereo direct out unless its
source is INP.
All file access happens in the Rust backend (`src-tauri/src/show.rs`). Edits stay in memory until
you save. **Save** first writes a `<file>.bak-<timestamp>` backup, then applies the changes in one transaction and checkpoints the
WAL into the `.emo`. **Close the show in LV1 before saving**: a non-empty `-wal` file means it is
still open, and the app asks before writing anyway.

Channel colors are an approximation of LV1's palette.

## Download

Installers for macOS (universal `.dmg`) and Windows (`-setup.exe` / `.msi`) are on the
[Releases](https://github.com/willihelm/lv1-offline-patcher/releases) page. The builds are not
code-signed: on macOS right-click the app and choose **Open** the first time (or run
`xattr -cr "/Applications/LV1 Offline Patcher.app"`); on Windows click **More info → Run anyway**
in the SmartScreen dialog. Windows needs the WebView2 runtime, which the installer fetches if missing.

## Development

```sh
npm install
npm run tauri dev          # desktop app
npm run tauri build        # release bundle in src-tauri/target/release/bundle
cd src-tauri && cargo test # runs on temp copies of src-tauri/testdata/lv1-test-session.emo
```

Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds macOS and Windows installers
and attaches them to a draft GitHub release.

`npm run dev` also runs the UI in a plain browser with a mock backend. It loads
`public/demo.json`, which you generate from the test session with
`cd src-tauri && cargo test dump_demo -- --ignored`.

`src-tauri/testdata/lv1-test-session.emo` is an empty LV1 session with mock devices: a DAW
(B0S0), an IOS-XL (B0S1), an IOX (B0S2) and a second DAW with 64 driver channels (B0S3), plus a
few named channels and patches.

## License

[MIT](LICENSE). Not affiliated with or endorsed by Waves Audio.
