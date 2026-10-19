//! Preamp settings of an I/O device, kept by LV1 as a YAML text ("IOM chunk") per device and
//! snapshot (tables iom_chunks / snapshot_iom). Example (IOS-XL):
//!
//! ```text
//! IOS-XL:
//!   line_in_phantom_map:
//!     0: 0
//!     1: 1
//!   control_gain_map:
//!     0: 16
//!     1: 31.5
//! ```
//!
//! Keys are the device input channels. The text is edited line by line so that everything else
//! stays exactly as LV1 wrote it.

use std::collections::BTreeMap;

use serde::Serialize;

const PHANTOM_MAP: &str = "line_in_phantom_map";
const GAIN_MAP: &str = "control_gain_map";

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Preamp {
    /// device input channel (as in routes)
    pub channel: i64,
    /// dB; `None` if the input has no gain control
    pub gain: Option<f64>,
    /// 48V; `None` if the input has no phantom power
    pub phantom: Option<bool>,
}

/// Entries `key: value` of the map `name` (indented 2, entries indented 4).
fn entries<'a>(text: &'a str, name: &str) -> Vec<(usize, i64, &'a str)> {
    let header = format!("  {name}:");
    let mut lines = text.split('\n').enumerate();
    if !lines.any(|(_, l)| l.trim_end() == header) {
        return Vec::new();
    }
    lines
        .take_while(|(_, l)| l.starts_with("    "))
        .filter_map(|(i, l)| {
            let (k, v) = l.trim().split_once(':')?;
            Some((i, k.trim().parse().ok()?, v.trim()))
        })
        .collect()
}

pub fn preamps(text: &str) -> Vec<Preamp> {
    let mut out: BTreeMap<i64, Preamp> = BTreeMap::new();
    fn at(out: &mut BTreeMap<i64, Preamp>, channel: i64) -> &mut Preamp {
        out.entry(channel).or_insert(Preamp { channel, gain: None, phantom: None })
    }
    for (_, ch, v) in entries(text, GAIN_MAP) {
        if let Ok(g) = v.parse::<f64>() {
            at(&mut out, ch).gain = Some(g);
        }
    }
    for (_, ch, v) in entries(text, PHANTOM_MAP) {
        if let Ok(p) = v.parse::<i64>() {
            at(&mut out, ch).phantom = Some(p != 0);
        }
    }
    out.into_values().collect()
}

/// `text` with the values of `preamps` written into it (entries that exist in the text only).
pub fn with_preamps(text: &str, preamps: &[Preamp]) -> String {
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    let find = |ch| preamps.iter().find(|p| p.channel == ch);
    for (i, ch, _) in entries(text, GAIN_MAP) {
        if let Some(g) = find(ch).and_then(|p| p.gain) {
            // LV1 writes whole numbers without decimals ("16", "31.5"), as does f64's Display
            lines[i] = format!("    {ch}: {g}");
        }
    }
    for (i, ch, _) in entries(text, PHANTOM_MAP) {
        if let Some(on) = find(ch).and_then(|p| p.phantom) {
            lines[i] = format!("    {ch}: {}", on as i64);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const XL: &str = "common:\n  vendor_id: 2\n  device_id: 15\nIOS-XL:\n  clip_indication_threshold: 3\n  line_in_phantom_map:\n    0: 0\n    1: 1\n  line_out_headroom_map:\n    47: 0\n  control_gain_map:\n    0: 16\n    1: 31.5\n  control_src_map:\n    20:\n      is_src_enabled: false\n      e_src_value: 1\n";

    #[test]
    fn parse() {
        assert_eq!(
            preamps(XL),
            [
                Preamp { channel: 0, gain: Some(16.0), phantom: Some(false) },
                Preamp { channel: 1, gain: Some(31.5), phantom: Some(true) },
            ]
        );
        assert!(preamps("common:\n  vendor_id: 0\n").is_empty());
    }

    #[test]
    fn write() {
        let p = [
            Preamp { channel: 0, gain: Some(42.5), phantom: Some(true) },
            Preamp { channel: 1, gain: Some(0.0), phantom: Some(true) },
        ];
        let out = with_preamps(XL, &p);
        assert_eq!(preamps(&out), p);
        assert_eq!(out, XL.replace("    0: 0\n    1: 1", "    0: 1\n    1: 1").replace("0: 16\n    1: 31.5", "0: 42.5\n    1: 0"));
        assert_eq!(with_preamps(XL, &preamps(XL)), XL);
    }
}
