//! Reading and writing the patch of a Waves LV1 showfile (.emo, an SQLite database).
//!
//! Modified are the `routes` table, the direct-out source (tap point) of the live mixer state
//! and the preamp settings (gain, 48V) of the I/O devices in the live state. Edits are kept in memory (with undo/redo) and written back in one transaction on
//! save, after a backup of the file was made.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::iom::{self, Preamp};
use crate::layout::{self, GainRange, Port};

// cluster_type ids (see table cluster_type)
pub const INPUT: i64 = 0;
pub const TALKBACK: i64 = 8;
pub const HW_IN: i64 = 10;
pub const HW_OUT: i64 = 11;
pub const MON: i64 = 25;
/// Clusters that can feed a hardware output (direct outs and busses), in display order:
/// the monitor mixes are aux busses too and follow the FX auxes.
const SOURCE_CLUSTERS: [i64; 9] = [0, 1, 2, MON, 3, 4, 5, 6, 7];
/// snapshot_chainer_params.param_id of PARAM_COLOR_VALUE
const PARAM_COLOR: i64 = 592;
/// snapshot_chainer_params.param_id of PARAM_OUTPUT_DIRECT_SOURCE (0 INP, 1 PRE, 2 PST, 3 PSP)
const PARAM_DIRECT_SOURCE: i64 = 379;
pub const TAP_INP: i64 = 0;
#[cfg(test)]
const TAP_PRE: i64 = 1;
const TAP_PST: i64 = 2;
const TAP_PSP: i64 = 3;
/// Driver channel counts LV1 offers for a DAW (SoundGrid driver).
pub const DRIVER_CHANNELS: [i64; 8] = [4, 8, 16, 32, 48, 64, 96, 128];
/// The snapshot holding the live mixer state ("Active").
const ACTIVE_SNAPSHOT: i64 = -1;

pub type Result<T> = std::result::Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// io_bank * 8 + assign, as used in routes.*_cluster_type_index
    pub idx: i64,
    pub name: String,
    pub kind: String,
    pub is_daw: bool,
    pub ins: i64,
    pub outs: i64,
    pub in_ports: Vec<Port>,
    pub out_ports: Vec<Port>,
    /// gain / 48V per input channel, empty if the file has no preamp data for the device
    pub preamps: Vec<Preamp>,
    /// set if gain can be edited
    pub gain_range: Option<GainRange>,
    #[serde(skip)]
    db_id: i64,
    #[serde(skip)]
    model: String,
    #[serde(skip)]
    iom: Option<IomChunk>,
}

/// The IOM chunk (preamp settings) of a device in the live state.
#[derive(Clone)]
struct IomChunk {
    snapshot: i64,
    device_id: i64,
    vendor_id: i64,
    text: String,
}

impl Device {
    fn with_channels(&self, ins: i64, outs: i64) -> Device {
        Device {
            ins,
            outs,
            in_ports: layout::ports(&self.model, self.is_daw, layout::Dir::In, ins),
            out_ports: layout::ports(&self.model, self.is_daw, layout::Dir::Out, outs),
            ..self.clone()
        }
    }
}

/// A mixer object (input channel, bus, ...) with its mono/stereo width.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Strip {
    pub cluster: i64,
    pub index: i64,
    pub name: String,
    pub color: Option<i64>,
    pub width: i64,
    /// direct-out / bus output source (TAP_*), only for output sources
    pub tap: Option<i64>,
    #[serde(skip)]
    obj_id: i64,
    #[serde(skip)]
    num_inputs: i64,
    #[serde(skip)]
    num_outputs: i64,
    /// a mono-to-stereo plugin (4cc ending in X) makes a mono channel stereo after it
    #[serde(skip)]
    mono_to_stereo: bool,
    /// tap value stored in the file (None: not stored, `default_tap` applies)
    #[serde(skip)]
    stored_tap: Option<i64>,
    /// LV1 default of the tap per object type (chainer_params_default_values)
    #[serde(skip)]
    default_tap: i64,
}

impl Strip {
    /// Number of output channels this strip feeds into the patch.
    fn source_width(&self, tap: i64) -> i64 {
        let w = if self.cluster != INPUT {
            self.num_outputs
        } else if tap != TAP_INP && self.mono_to_stereo {
            2
        } else {
            self.num_inputs
        };
        w.clamp(1, 2)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Src {
    pub cluster: i64,
    pub index: i64,
    pub channel: i64,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dst {
    pub cluster: i64,
    pub index: i64,
    /// 0 = Input A, 1 = Input B (secondary source); -1 for hardware outputs
    pub section: i64,
    pub channel: i64,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Route {
    /// Row id in the file; `None` for routes created in this session.
    pub id: Option<i64>,
    pub src: Src,
    pub dst: Dst,
    pub status: i64,
}

/// Set the source of `dst`; `src: None` removes the patch.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct PatchOp {
    pub dst: Dst,
    pub src: Option<Src>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowView<'a> {
    pub path: String,
    pub snapshot: String,
    pub devices: Vec<Device>,
    pub inputs: &'a [Strip],
    pub sources: Vec<Strip>,
    pub routes: &'a [Route],
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub lv1_open: bool,
}

/// Everything that can be edited (one undo step = one copy of this).
#[derive(Clone, PartialEq)]
struct Edits {
    routes: Vec<Route>,
    /// current tap per (cluster, index) of the output sources
    taps: HashMap<(i64, i64), i64>,
    /// driver channel count (in and out) per DAW device idx, only where changed
    driver_channels: HashMap<i64, i64>,
    /// preamp settings per (device idx, channel), only where changed
    preamps: HashMap<(i64, i64), Preamp>,
}

pub struct Show {
    pub path: PathBuf,
    snapshot: String,
    devices: Vec<Device>,
    inputs: Vec<Strip>,
    sources: Vec<Strip>,
    edits: Edits,
    original_ids: HashSet<i64>,
    undo: Vec<Edits>,
    redo: Vec<Edits>,
}

impl Show {
    pub fn open(path: &Path) -> Result<Show> {
        if !path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }
        // Not read-only: a read-only connection to a WAL database leaves empty -wal/-shm
        // files behind. Nothing is written here; closing removes them again.
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(err)?;
        let mut devices = load_devices(&db).map_err(err)?;
        let (snapshot_id, snapshot) = db
            .query_row(
                "SELECT s.id, s.number, s.name FROM snapshot s
                 JOIN recalled_snapshot r ON r.number = s.number",
                [],
                |r| {
                    let num: i64 = r.get(1)?;
                    let name: Option<String> = r.get(2)?;
                    Ok((Some(r.get::<_, i64>(0)?), format!("{}:{}", num + 1, name.unwrap_or_default())))
                },
            )
            .unwrap_or((None, String::new()));
        // names, colors and taps come from the live state; older files may only have the
        // recalled snapshot
        let has_active: bool = db
            .query_row("SELECT EXISTS(SELECT 1 FROM snapshot_chainer WHERE snapshot_id = ?)", [ACTIVE_SNAPSHOT], |r| r.get(0))
            .map_err(err)?;
        let state_snapshot = if has_active { Some(ACTIVE_SNAPSHOT) } else { snapshot_id };
        if let Some(snap) = state_snapshot {
            load_preamps(&db, snap, &mut devices).map_err(err)?;
        }
        let strips = load_strips(&db, state_snapshot).map_err(err)?;
        let routes = load_routes(&db).map_err(err)?;

        let inputs = strips
            .iter()
            .filter(|s| s.cluster == INPUT || s.cluster == TALKBACK)
            .map(|s| Strip { width: s.num_inputs.clamp(1, 2), tap: None, ..s.clone() })
            .collect();
        let mut sources: Vec<Strip> = strips.into_iter().filter(|s| SOURCE_CLUSTERS.contains(&s.cluster)).collect();
        // stable sort keeps the index order within a cluster
        sources.sort_by_key(|s| SOURCE_CLUSTERS.iter().position(|&c| c == s.cluster));
        let taps = sources
            .iter()
            .map(|s| ((s.cluster, s.index), s.stored_tap.unwrap_or(s.default_tap)))
            .collect();

        Ok(Show {
            path: path.to_path_buf(),
            snapshot,
            devices,
            inputs,
            sources,
            original_ids: routes.iter().filter_map(|r| r.id).collect(),
            edits: Edits { routes, taps, driver_channels: HashMap::new(), preamps: HashMap::new() },
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn view(&self) -> ShowView<'_> {
        let sources = self
            .sources
            .iter()
            .map(|s| {
                let tap = self.tap(s);
                Strip { width: s.source_width(tap), tap: Some(tap), ..s.clone() }
            })
            .collect();
        ShowView {
            path: self.path.display().to_string(),
            snapshot: self.snapshot.clone(),
            devices: self.devices.iter().map(|d| self.device(d)).collect(),
            inputs: &self.inputs,
            sources,
            routes: &self.edits.routes,
            dirty: self.dirty(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            lv1_open: wal_in_use(&self.path),
        }
    }

    /// The device with the edited driver channel count and preamps applied.
    fn device(&self, d: &Device) -> Device {
        let mut d = match self.edits.driver_channels.get(&d.idx) {
            Some(&n) => d.with_channels(n, n),
            None => d.clone(),
        };
        for p in &mut d.preamps {
            if let Some(edit) = self.edits.preamps.get(&(d.idx, p.channel)) {
                *p = edit.clone();
            }
        }
        d
    }

    /// I/O devices whose preamp settings differ from the file, with the edited settings.
    fn changed_preamps(&self) -> impl Iterator<Item = (&Device, Vec<Preamp>)> {
        self.devices.iter().filter_map(|d| {
            let preamps = self.device(d).preamps;
            (preamps != d.preamps).then_some((d, preamps))
        })
    }

    /// DAW devices whose driver channel count differs from the file.
    fn changed_drivers(&self) -> impl Iterator<Item = (&Device, i64)> {
        self.devices.iter().filter_map(|d| {
            let n = *self.edits.driver_channels.get(&d.idx)?;
            (n != d.ins || n != d.outs).then_some((d, n))
        })
    }

    fn tap(&self, s: &Strip) -> i64 {
        self.edits.taps.get(&(s.cluster, s.index)).copied().unwrap_or(s.default_tap)
    }

    /// Sources whose tap differs from the file.
    fn changed_taps(&self) -> impl Iterator<Item = (&Strip, i64)> {
        self.sources.iter().filter_map(|s| {
            let tap = self.tap(s);
            (tap != s.stored_tap.unwrap_or(s.default_tap)).then_some((s, tap))
        })
    }

    pub fn dirty(&self) -> bool {
        let routes = &self.edits.routes;
        routes.iter().any(|r| r.id.is_none())
            || routes.len() != self.original_ids.len()
            || self.changed_taps().next().is_some()
            || self.changed_drivers().next().is_some()
            || self.changed_preamps().next().is_some()
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.edits.clone());
        self.redo.clear();
    }

    /// Apply a batch of patch operations as a single undo step.
    pub fn apply(&mut self, ops: &[PatchOp]) {
        if ops.is_empty() {
            return;
        }
        self.checkpoint();
        let routes = &mut self.edits.routes;
        for op in ops {
            routes.retain(|r| r.dst != op.dst);
            if let Some(src) = op.src {
                // LV1 writes status 1 for the secondary (Input B) source
                let status = if op.dst.section == 1 { 1 } else { 0 };
                routes.push(Route { id: None, src, dst: op.dst, status });
            }
        }
    }

    /// Set the direct-out / bus output source (0 INP, 1 PRE, 2 PST, 3 PSP).
    pub fn set_tap(&mut self, cluster: i64, index: i64, tap: i64) -> Result<()> {
        if !(TAP_INP..=TAP_PSP).contains(&tap) {
            return Err(format!("Invalid source setting {tap}"));
        }
        if !self.sources.iter().any(|s| s.cluster == cluster && s.index == index) {
            return Err(format!("Unknown output source {cluster}/{index}"));
        }
        if self.edits.taps.get(&(cluster, index)) != Some(&tap) {
            self.checkpoint();
            self.edits.taps.insert((cluster, index), tap);
        }
        Ok(())
    }

    /// Set the number of driver channels (inputs and outputs) of a DAW. Patches on channels
    /// that no longer exist are removed in the same undo step.
    pub fn set_driver_channels(&mut self, idx: i64, count: i64) -> Result<()> {
        if !DRIVER_CHANNELS.contains(&count) {
            return Err(format!("Invalid driver channel count {count}"));
        }
        let Some(dev) = self.devices.iter().find(|d| d.idx == idx) else {
            return Err(format!("Unknown device {idx}"));
        };
        if !dev.is_daw {
            return Err(format!("{} is not a DAW", dev.name));
        }
        let cur = self.device(dev);
        if cur.ins == count && cur.outs == count {
            return Ok(());
        }
        self.checkpoint();
        self.edits.driver_channels.insert(idx, count);
        self.edits.routes.retain(|r| {
            let gone_src = r.src.cluster == HW_IN && r.src.index == idx && r.src.channel >= count;
            let gone_dst = r.dst.cluster == HW_OUT && r.dst.index == idx && r.dst.channel >= count;
            !gone_src && !gone_dst
        });
        Ok(())
    }

    /// Set gain (dB, rounded to the device's gain step) and/or 48V of an input of an I/O device.
    pub fn set_preamp(&mut self, idx: i64, channel: i64, gain: Option<f64>, phantom: Option<bool>) -> Result<()> {
        let Some(dev) = self.devices.iter().find(|d| d.idx == idx) else {
            return Err(format!("Unknown device {idx}"));
        };
        let dev = self.device(dev);
        let Some(cur) = dev.preamps.iter().find(|p| p.channel == channel) else {
            return Err(format!("{} input {} has no preamp", dev.name, channel + 1));
        };
        let mut next = cur.clone();
        if let Some(g) = gain {
            let (Some(GainRange { min, max, step }), Some(_)) = (dev.gain_range, cur.gain) else {
                return Err(format!("Gain of {} input {} cannot be edited", dev.name, channel + 1));
            };
            if !g.is_finite() {
                return Err(format!("Invalid gain {g}"));
            }
            // + 0.0 turns -0 into 0
            next.gain = Some(((g / step).round() * step).clamp(min, max) + 0.0);
        }
        if let Some(on) = phantom {
            if cur.phantom.is_none() {
                return Err(format!("{} input {} has no phantom power", dev.name, channel + 1));
            }
            next.phantom = Some(on);
        }
        if next != *cur {
            self.checkpoint();
            self.edits.preamps.insert((idx, channel), next);
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.edits, prev));
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.edits, next));
        }
    }

    /// Write the patch into `target` (the opened file or a copy of it).
    /// Returns the path of the backup that was made, if any.
    pub fn save(&mut self, target: &Path, force: bool) -> Result<Option<PathBuf>> {
        if wal_in_use(&self.path) && !force {
            return Err("LV1_OPEN".into());
        }
        // canonicalize: on Windows the same file can be spelled with different case or separators
        let same_file = target == self.path.as_path()
            || matches!((fs::canonicalize(target), fs::canonicalize(&self.path)), (Ok(a), Ok(b)) if a == b);
        let backup = if same_file {
            Some(backup(&self.path)?)
        } else {
            fs::copy(&self.path, target).map_err(err)?;
            None
        };
        if let Err(e) = self.write(target) {
            if let Some(b) = &backup {
                let _ = fs::remove_file(b);
            }
            return Err(e);
        }
        // reload so that new rows get their ids and undo history starts fresh
        *self = Show::open(target)?;
        Ok(backup)
    }

    fn write(&self, target: &Path) -> Result<()> {
        let mut db = Connection::open(target).map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let routes = &self.edits.routes;
        let kept: HashSet<i64> = routes.iter().filter_map(|r| r.id).collect();
        for id in self.original_ids.difference(&kept) {
            tx.execute("DELETE FROM routes WHERE id = ?", [id]).map_err(err)?;
        }
        for r in routes.iter().filter(|r| r.id.is_none()) {
            // column values as LV1 writes them for hardware patches
            tx.execute(
                "INSERT INTO routes (src_cluster_type, src_cluster_type_index, src_section_type,
                   src_section_index, src_channel_index, src_control_id, src_asgn_type,
                   dst_cluster_type, dst_cluster_type_index, dst_section_type, dst_section_index,
                   dst_channel_index, dst_control_id, dst_asgn_type, status)
                 VALUES (?1, ?2, -1, -1, ?3, -1, -1, ?4, ?5, 0, ?6, ?7, -1, 0, ?8)",
                params![
                    r.src.cluster, r.src.index, r.src.channel,
                    r.dst.cluster, r.dst.index, r.dst.section, r.dst.channel, r.status
                ],
            )
            .map_err(err)?;
        }
        for (strip, tap) in self.changed_taps() {
            tx.execute(
                "INSERT OR REPLACE INTO snapshot_chainer_params (chainer_id, snapshot_id, param_id, param_value)
                 VALUES (?1, ?2, ?3, ?4)",
                params![strip.obj_id, ACTIVE_SNAPSHOT, PARAM_DIRECT_SOURCE, tap as f64],
            )
            .map_err(err)?;
        }
        // LV1 keeps the driver channel count per DAW in the device row and per bank/slot in
        // default_driver_channel_counts (missing in older files)
        let has_defaults: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'default_driver_channel_counts')",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        for (dev, n) in self.changed_drivers() {
            tx.execute(
                "UPDATE device SET in_channel_count = ?1, out_channel_count = ?1 WHERE id = ?2",
                params![n, dev.db_id],
            )
            .map_err(err)?;
            if has_defaults {
                tx.execute(
                    "INSERT INTO default_driver_channel_counts (io_bank, io_slot, in_channel_count, out_channel_count)
                     VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT (io_bank, io_slot) DO UPDATE SET in_channel_count = ?3, out_channel_count = ?3",
                    params![dev.idx / 8, dev.idx % 8, n],
                )
                .map_err(err)?;
            }
        }
        // a new chunk for the live state; triggers on snapshot_iom keep iom_chunks.ref_count and
        // drop the old chunk once no snapshot uses it any more
        for (dev, preamps) in self.changed_preamps() {
            let Some(chunk) = &dev.iom else { continue };
            let text = iom::with_preamps(&chunk.text, &preamps);
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM iom_chunks WHERE iom_chunk = ?1 AND device_id = ?2 AND vendor_id = ?3 LIMIT 1",
                    params![text, chunk.device_id, chunk.vendor_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            let id = match existing {
                Some(id) => id,
                None => {
                    tx.execute(
                        "INSERT INTO iom_chunks (ref_count, device_id, vendor_id, iom_chunk) VALUES (0, ?1, ?2, ?3)",
                        params![chunk.device_id, chunk.vendor_id, text],
                    )
                    .map_err(err)?;
                    tx.last_insert_rowid()
                }
            };
            tx.execute(
                "UPDATE snapshot_iom SET iom_chunk_id = ?1 WHERE snapshot_id = ?2 AND io_bank = ?3 AND io_slot = ?4",
                params![id, chunk.snapshot, dev.idx / 8, dev.idx % 8],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        // the file runs in WAL mode: fold the changes back into the .emo itself
        db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).map_err(err)?;
        Ok(())
    }
}

/// A non-empty -wal file means LV1 (or another process) has the show open.
pub fn wal_in_use(path: &Path) -> bool {
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    fs::metadata(PathBuf::from(wal)).map(|m| m.len() > 0).unwrap_or(false)
}

fn backup(path: &Path) -> Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base = format!("{}.bak-{}", path.display(), stamp);
    let mut dst = PathBuf::from(&base);
    let mut n = 1;
    while dst.exists() {
        n += 1;
        dst = PathBuf::from(format!("{base}-{n}"));
    }
    fs::copy(path, &dst).map_err(err)?;
    Ok(dst)
}

fn load_devices(db: &Connection) -> rusqlite::Result<Vec<Device>> {
    let mut names = HashMap::new();
    let mut stmt = db.prepare("SELECT mac, gender, name FROM device_name")?;
    for row in stmt.query_map([], |r| Ok(((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?), r.get::<_, String>(2)?)))? {
        let (k, v) = row?;
        names.insert(k, v);
    }
    let mut stmt = db.prepare(
        "SELECT device_gender, io_bank, assign, description, mac, in_channel_count,
                out_channel_count, max_in_channel_count, max_out_channel_count, id
         FROM device WHERE io_bank >= 0 ORDER BY io_bank, assign",
    )?;
    let devices = stmt
        .query_map([], |r| {
            let gender: i64 = r.get(0)?;
            let bank: i64 = r.get(1)?;
            let slot: i64 = r.get(2)?;
            let desc: Option<String> = r.get(3)?;
            let mac: i64 = r.get(4)?;
            let (i, o, mi, mo): (i64, i64, i64, i64) = (r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?);
            let is_daw = gender == 1;
            let model = desc.unwrap_or_default();
            let ins = if mi > 0 { mi } else { i }.max(0);
            let outs = if mo > 0 { mo } else { o }.max(0);
            Ok(Device {
                idx: bank * 8 + slot,
                name: names.get(&(mac, gender)).cloned().unwrap_or(format!("B{bank}S{slot}")),
                kind: if model.is_empty() { if is_daw { "DAW" } else { "I/O" }.into() } else { model.clone() },
                is_daw,
                ins,
                outs,
                in_ports: layout::ports(&model, is_daw, layout::Dir::In, ins),
                out_ports: layout::ports(&model, is_daw, layout::Dir::Out, outs),
                preamps: Vec::new(),
                gain_range: None,
                db_id: r.get(9)?,
                model,
                iom: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(devices)
}

/// Preamp settings of the I/O devices in `snapshot`. A chunk is only used if it was written
/// for the device model in that slot.
fn load_preamps(db: &Connection, snapshot: i64, devices: &mut [Device]) -> rusqlite::Result<()> {
    let mut stmt = db.prepare(
        "SELECT c.device_id, c.vendor_id, c.iom_chunk
         FROM snapshot_iom s
         JOIN iom_chunks c ON c.id = s.iom_chunk_id
         JOIN device_iobox b ON b.device_id = c.device_id AND b.vendor_id = c.vendor_id
         JOIN device d ON d.id = b.id AND d.io_bank = s.io_bank AND d.assign = s.io_slot
         WHERE s.snapshot_id = ?1 AND d.id = ?2",
    )?;
    for d in devices.iter_mut().filter(|d| !d.is_daw) {
        let chunk = stmt
            .query_row(params![snapshot, d.db_id], |r| {
                Ok(IomChunk { snapshot, device_id: r.get(0)?, vendor_id: r.get(1)?, text: r.get::<_, Option<String>>(2)?.unwrap_or_default() })
            })
            .optional()?;
        if let Some(chunk) = chunk {
            d.preamps = iom::preamps(&chunk.text);
            d.gain_range = layout::gain_range(&d.model).filter(|_| d.preamps.iter().any(|p| p.gain.is_some()));
            d.iom = Some(chunk);
        }
    }
    Ok(())
}

/// All active mixer objects.
fn load_strips(db: &Connection, snapshot: Option<i64>) -> rusqlite::Result<Vec<Strip>> {
    // LV1 stores the tap only where it differs from the per-type default (PST in all files seen)
    let mut default_taps: HashMap<i64, i64> = HashMap::new();
    let has_defaults: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chainer_params_default_values')",
        [],
        |r| r.get(0),
    )?;
    if has_defaults {
        let mut stmt = db.prepare("SELECT obj_type, param_value FROM chainer_params_default_values WHERE param_id = ?")?;
        for row in stmt.query_map([PARAM_DIRECT_SOURCE], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)? as i64)))? {
            let (obj_type, tap) = row?;
            default_taps.insert(obj_type, tap);
        }
    }
    let mut stmt = db.prepare(
        "SELECT o.obj_type, o.obj_index, COALESCE(sc.name, o.name), ch.num_inputs, ch.num_outputs,
                (SELECT p.param_value FROM snapshot_chainer_params p
                  WHERE p.chainer_id = o.id AND p.snapshot_id = ?1 AND p.param_id = ?2),
                (SELECT p.param_value FROM snapshot_chainer_params p
                  WHERE p.chainer_id = o.id AND p.snapshot_id = ?1 AND p.param_id = ?3),
                EXISTS(SELECT 1 FROM plug WHERE plug.chainer_id = o.id AND plug.plugin_4cc LIKE '%X'),
                o.id
         FROM object o
         JOIN chainer ch ON ch.obj_id = o.id
         LEFT JOIN snapshot_chainer sc ON sc.chainer_id = o.id AND sc.snapshot_id = ?1
         WHERE ch.num_inputs >= 0
         ORDER BY o.obj_type, o.obj_index",
    )?;
    let rows = stmt
        .query_map(params![snapshot.unwrap_or(-1), PARAM_COLOR, PARAM_DIRECT_SOURCE], |r| {
            let cluster: i64 = r.get(0)?;
            let index: i64 = r.get(1)?;
            let name: Option<String> = r.get(2)?;
            let color: Option<f64> = r.get(5)?;
            let tap: Option<f64> = r.get(6)?;
            Ok(Strip {
                cluster,
                index,
                name: name.filter(|n| !n.is_empty()).unwrap_or_else(|| default_name(cluster, index)),
                color: color.map(|c| c as i64),
                width: 1,
                tap: None,
                obj_id: r.get(8)?,
                num_inputs: r.get(3)?,
                num_outputs: r.get(4)?,
                mono_to_stereo: r.get(7)?,
                stored_tap: tap.map(|t| t as i64),
                default_tap: default_taps.get(&cluster).copied().unwrap_or(TAP_PST),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn default_name(cluster: i64, index: i64) -> String {
    let label = match cluster {
        0 => "Ch",
        1 => "Group",
        2 => "FX",
        3 => "Main",
        4 => "Center",
        5 => "Mono",
        6 => "Matrix",
        7 => "Cue",
        8 => "Talkback",
        MON => "Mon",
        _ => "Obj",
    };
    format!("{label} {}", index + 1)
}

fn load_routes(db: &Connection) -> rusqlite::Result<Vec<Route>> {
    let mut stmt = db.prepare(
        "SELECT id, src_cluster_type, src_cluster_type_index, src_channel_index, dst_cluster_type,
                dst_cluster_type_index, dst_section_index, dst_channel_index, status
         FROM routes ORDER BY id",
    )?;
    let routes = stmt
        .query_map([], |r| {
            Ok(Route {
                id: Some(r.get(0)?),
                src: Src { cluster: r.get(1)?, index: r.get(2)?, channel: r.get(3)? },
                dst: Dst { cluster: r.get(4)?, index: r.get(5)?, section: r.get(6)?, channel: r.get(7)? },
                status: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary copy of testdata/lv1-test-session.emo: an empty LV1 session with mock
    /// devices (B0S0 "Test DAW", B0S1 "Stage IOS-XL", B0S2 "Stage IOX", B0S3 "Record DAW"
    /// with 64 driver channels), channels Kick/Snare/Vox and 8 patches.
    fn sample() -> PathBuf {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/lv1-test-session.emo");
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("lv1-offline-patcher-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dst = dir.join("session.emo");
        fs::copy(src, &dst).unwrap();
        dst
    }

    fn dev(show: &Show, name: &str) -> Device {
        show.view().devices.into_iter().find(|d| d.name == name).unwrap()
    }

    #[test]
    fn devices() {
        let path = sample();
        let show = Show::open(&path).unwrap();
        let names: Vec<_> = show.devices.iter().map(|d| (d.idx, d.name.as_str(), d.is_daw)).collect();
        assert_eq!(
            names,
            [(0, "Test DAW", true), (1, "Stage IOS-XL", false), (2, "Stage IOX", false), (3, "Record DAW", true)]
        );
        let xl = dev(&show, "Stage IOS-XL");
        assert_eq!((xl.kind.as_str(), xl.in_ports.len(), xl.out_ports.len()), ("IOS-XL", 10, 14));
        let iox = dev(&show, "Stage IOX");
        assert_eq!((iox.ins, iox.outs), (12, 16));
        assert_eq!((dev(&show, "Record DAW").ins, dev(&show, "Test DAW").outs), (64, 32));
        let names: Vec<_> = show.inputs.iter().take(4).map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Kick", "Snare", "Vox", "Channel 4"]);
        assert_eq!(show.edits.routes.len(), 8);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn roundtrip() {
        let path = sample();
        let mut show = Show::open(&path).unwrap();
        assert!(!path.with_extension("emo-wal").exists(), "open must not leave a -wal file");

        // patch IOX in 3 -> Ch 5, unpatch Snare
        let dst = Dst { cluster: INPUT, index: 4, section: 0, channel: 0 };
        let src = Src { cluster: HW_IN, index: 2, channel: 2 };
        let snare = Dst { cluster: INPUT, index: 1, section: 0, channel: 0 };
        show.apply(&[PatchOp { dst, src: Some(src) }, PatchOp { dst: snare, src: None }]);
        assert!(show.dirty());
        show.undo();
        assert!(!show.dirty());
        show.redo();

        let backup = show.save(&path, false).unwrap().unwrap();
        assert!(backup.exists());
        let reread = Show::open(&path).unwrap();
        let r = reread.edits.routes.iter().find(|r| r.dst == dst).unwrap();
        assert_eq!(r.src, src);
        assert!(r.id.is_some());
        assert!(!reread.edits.routes.iter().any(|r| r.dst == snare));
        // Input B of Vox is untouched
        let vox_b = Dst { cluster: INPUT, index: 2, section: 1, channel: 0 };
        let r = reread.edits.routes.iter().find(|r| r.dst == vox_b).unwrap();
        assert_eq!((r.src, r.status), (Src { cluster: HW_IN, index: 0, channel: 0 }, 1));
        assert_eq!(reread.edits.routes.len(), 8);
        assert!(!wal_in_use(&path));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn taps() {
        let path = sample();
        let mut show = Show::open(&path).unwrap();
        let src = |show: &Show, cluster: i64, idx: i64| {
            show.view().sources.into_iter().find(|s| s.cluster == cluster && s.index == idx).unwrap()
        };
        // nothing stored: the LV1 default (chainer_params_default_values) is PST
        let kick = src(&show, INPUT, 0);
        assert_eq!((kick.name.as_str(), kick.tap, kick.width), ("Kick", Some(TAP_PST), 1));
        assert_eq!(src(&show, 3, 0).tap, Some(TAP_PST));

        show.set_tap(INPUT, 0, TAP_PSP).unwrap();
        assert!(show.dirty());
        show.undo();
        assert!(!show.dirty());
        show.redo();
        assert!(show.set_tap(INPUT, 0, 7).is_err());
        assert!(show.set_tap(HW_IN, 0, TAP_PRE).is_err());

        show.save(&path, false).unwrap();
        let reread = Show::open(&path).unwrap();
        assert_eq!(src(&reread, INPUT, 0).tap, Some(TAP_PSP));
        assert_eq!(src(&reread, INPUT, 1).tap, Some(TAP_PST));
        assert!(!reread.dirty());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn driver_channels() {
        let path = sample();
        let mut show = Show::open(&path).unwrap();
        let rec = dev(&show, "Record DAW");
        assert_eq!((rec.is_daw, rec.ins, rec.outs), (true, 64, 64));
        let idx = rec.idx;
        // a patch on a channel that disappears when shrinking
        let dst = Dst { cluster: INPUT, index: 40, section: 0, channel: 0 };
        show.apply(&[PatchOp { dst, src: Some(Src { cluster: HW_IN, index: idx, channel: 60 }) }]);

        assert!(show.set_driver_channels(idx, 33).is_err());
        assert!(show.set_driver_channels(1, 32).is_err()); // IOS-XL is no DAW
        show.set_driver_channels(idx, 32).unwrap();
        let rec = dev(&show, "Record DAW");
        assert_eq!((rec.ins, rec.outs, rec.in_ports.len()), (32, 32, 32));
        assert!(!show.edits.routes.iter().any(|r| r.dst == dst));
        // patches on remaining channels stay
        assert_eq!(show.edits.routes.iter().filter(|r| r.dst.cluster == HW_OUT && r.dst.index == idx).count(), 2);
        show.undo();
        assert!(show.edits.routes.iter().any(|r| r.dst == dst));
        assert_eq!(dev(&show, "Record DAW").ins, 64);
        show.redo();

        show.save(&path, false).unwrap();
        let reread = Show::open(&path).unwrap();
        assert_eq!(dev(&reread, "Record DAW").outs, 32);
        assert!(!reread.dirty());
        let db = Connection::open(&path).unwrap();
        let defaults: (i64, i64) = db
            .query_row(
                "SELECT in_channel_count, out_channel_count FROM default_driver_channel_counts
                 WHERE io_bank = ?1 AND io_slot = ?2",
                [idx / 8, idx % 8],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(defaults, (32, 32));
        drop(db);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn preamps() {
        let path = sample();
        let mut show = Show::open(&path).unwrap();
        let xl = dev(&show, "Stage IOS-XL");
        assert_eq!(xl.gain_range, Some(GainRange { min: 0.0, max: 59.5, step: 0.5 }));
        assert_eq!(xl.preamps.len(), 8);
        assert_eq!(xl.preamps[1], Preamp { channel: 1, gain: Some(36.0), phantom: Some(true) });
        assert_eq!(dev(&show, "Stage IOX").preamps.len(), 12);
        assert!(dev(&show, "Test DAW").preamps.is_empty());

        assert!(show.set_preamp(xl.idx, 8, Some(10.0), None).is_err()); // AES input
        assert!(show.set_preamp(0, 0, None, Some(true)).is_err()); // DAW
        show.set_preamp(xl.idx, 1, Some(36.0), Some(true)).unwrap();
        assert!(!show.dirty() && show.undo.is_empty());
        show.set_preamp(xl.idx, 0, Some(12.3), Some(true)).unwrap();
        show.set_preamp(xl.idx, 1, Some(80.0), None).unwrap();
        let p = &dev(&show, "Stage IOS-XL").preamps;
        assert_eq!((p[0].gain, p[0].phantom, p[1].gain), (Some(12.5), Some(true), Some(59.5)));
        show.undo();
        show.undo();
        assert!(!show.dirty());
        show.redo();
        show.redo();

        let db = Connection::open(&path).unwrap();
        let chunks: i64 = db.query_row("SELECT count(*) FROM iom_chunks", [], |r| r.get(0)).unwrap();
        drop(db);
        show.save(&path, false).unwrap();
        let reread = Show::open(&path).unwrap();
        assert!(!reread.dirty());
        let p = &dev(&reread, "Stage IOS-XL").preamps;
        assert_eq!((p[0].gain, p[0].phantom, p[1].gain, p[2].gain), (Some(12.5), Some(true), Some(59.5), Some(30.0)));
        // the old chunk was only used by the live state: replaced, not duplicated
        let db = Connection::open(&path).unwrap();
        let (n, refs): (i64, i64) = db
            .query_row("SELECT count(*), sum(ref_count) FROM iom_chunks WHERE id > 0", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        let total: i64 = db.query_row("SELECT count(*) FROM iom_chunks", [], |r| r.get(0)).unwrap();
        assert_eq!((total, n, refs), (chunks, 2, 2));
        drop(db);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    /// Writes the UI state of the test session to public/demo.json for the browser mock:
    /// `cargo test dump_demo -- --ignored`
    #[test]
    #[ignore]
    fn dump_demo() {
        let path = sample();
        let show = Show::open(&path).unwrap();
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/demo.json");
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::write(out, serde_json::to_string(&show.view()).unwrap()).unwrap();
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
