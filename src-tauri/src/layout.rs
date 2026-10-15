//! Channel layouts of the I/O devices. The showfile only stores channel counts; LV1 knows the
//! port types (analog line, headphones, AES, ...) from the device model, so they are listed here.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Port {
    /// 0-based channel index on the device (as in routes); hidden channels leave gaps
    pub channel: i64,
    /// e.g. "Analog", "Digital"
    pub major: String,
    /// e.g. "Mic/L", "Line", "Phones", "AES"
    pub minor: String,
    /// column label, e.g. "3", "1L", "R"
    pub label: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Dir {
    In,
    Out,
}

/// A run of ports: (major, minor, labels). An empty major hides its channels.
type Group = (&'static str, &'static str, Vec<String>);

/// Channels that exist in the data but not on the hardware.
fn hidden(n: usize) -> Group {
    ("", "", vec![String::new(); n])
}

fn numbered(n: usize) -> Vec<String> {
    (1..=n).map(|i| i.to_string()).collect()
}

fn stereo_pairs(n: usize) -> Vec<String> {
    (1..=n).flat_map(|i| [format!("{i}L"), format!("{i}R")]).collect()
}

fn lr() -> Vec<String> {
    vec!["L".into(), "R".into()]
}

/// Model key: uppercase alphanumerics without a leading brand, so "IOS-XL", "DiGiGrid IOS-XL"
/// and "IONIC 24" / "Ionic24" match the same entry.
fn model_key(model: &str) -> String {
    let key: String = model.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
    for brand in ["DIGIGRID", "WAVES", "DSPRO", "DIRECTOUT", "SOUNDSTUDIO", "JOECO", "PEAVEY"] {
        if let Some(rest) = key.strip_prefix(brand) {
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    key
}

/// Port layouts per model, from the manufacturers' specs (Waves SoundGrid interfaces chart).
/// Order: analog inputs/line outputs first, then headphones, then digital, as on the IOS-XL.
/// Only IOS-XL and IOX are verified against real showfiles.
fn model_layout(model: &str, dir: Dir) -> Option<Vec<Group>> {
    let mic = |n| ("Analog", "Mic/L", numbered(n));
    let line = |n| ("Analog", "Line", numbered(n));
    let phones = |n| ("Analog", "Phones", if n == 1 { lr() } else { stereo_pairs(n) });
    // `n` stereo AES/EBU ports
    let aes = |n| ("Digital", "AES", if n == 1 { lr() } else { stereo_pairs(n) });
    Some(match (model_key(model).as_str(), dir) {
        // DiGiGrid
        ("IOSXL", Dir::In) => vec![mic(8), aes(1)],
        ("IOSXL", Dir::Out) => vec![line(8), phones(2), aes(1)],
        ("IOX", Dir::In) => vec![mic(12)],
        // the data has 16 outputs; 7-8 do not exist on the IOX
        ("IOX", Dir::Out) => vec![line(6), hidden(2), phones(4)],
        ("IOC", Dir::In) => vec![mic(2), line(8), aes(8), ("Digital", "ADAT", numbered(8))],
        ("IOC", Dir::Out) => vec![line(8), phones(2), aes(8), ("Digital", "ADAT", numbered(8))],
        ("D", Dir::In) => vec![mic(2), ("Analog", "Inst", numbered(2))],
        ("D", Dir::Out) => vec![line(4), phones(1)],
        ("M", Dir::In) => vec![mic(1), ("Analog", "Line", vec!["2".into()])],
        ("M", Dir::Out) => vec![line(2), phones(1)],
        ("MGB" | "MGO" | "MGR", _) => vec![("Digital", "MADI", numbered(256))],
        ("DLI" | "DLS", _) => vec![("Digital", "DigiLink", numbered(64))],
        // Waves stageboxes and console
        ("IONIC16", Dir::In) => vec![mic(16), aes(1)],
        ("IONIC16", Dir::Out) => vec![line(12), phones(2), aes(1)],
        ("IONIC24", Dir::In) => vec![mic(24), aes(1)],
        ("IONIC24", Dir::Out) => vec![line(18), phones(2), aes(2)],
        ("LV1CLASSIC", Dir::In) => vec![mic(16), aes(1)],
        ("LV1CLASSIC", Dir::Out) => vec![line(12), aes(2)],
        // third-party stageboxes
        // 1 stereo AES in and out (Waves' chart wrongly lists 2 stereo AES outs)
        ("STAGEGRID1000", Dir::In) => vec![mic(8), aes(1)],
        ("STAGEGRID1000", Dir::Out) => vec![line(4), phones(2), aes(1)],
        ("STAGEGRID4000", Dir::In) => vec![mic(32), aes(4)],
        ("STAGEGRID4000", Dir::Out) => vec![line(16), phones(2), aes(4)],
        ("STG1608", Dir::In) => vec![mic(16)],
        ("STG1608", Dir::Out) => vec![line(8), phones(1)],
        ("STG2412", Dir::In) => vec![mic(24)],
        ("STG2412", Dir::Out) => vec![line(12), phones(1)],
        ("TACTUSSTAGE", Dir::In) => vec![mic(32)],
        ("TACTUSSTAGE", Dir::Out) => vec![line(16), phones(1), aes(2)],
        ("TACTUSFOH", Dir::In) => vec![mic(8), aes(1)],
        ("TACTUSFOH", Dir::Out) => vec![line(8), phones(1)],
        ("BBSG24MP" | "BLACKBOXSOUNDGRIDRECORDER", Dir::In) => vec![mic(24)],
        ("BBSG24MP" | "BLACKBOXSOUNDGRIDRECORDER", Dir::Out) => vec![line(24), phones(2)],
        // network bridges
        ("EXBOXSG", _) => vec![("Digital", "MADI", numbered(128))],
        ("SGMADI", Dir::In) => vec![mic(2), ("Digital", "MADI", numbered(128))],
        ("SGMADI", Dir::Out) => vec![line(2), ("Digital", "MADI", numbered(128))],
        ("WSGBRIDGEFORDANTE" | "DANTEWSGXS", _) => vec![("Digital", "Dante", numbered(64))],
        ("BR1" | "SOUNDGRIDBR1", _) => vec![("Digital", "AVB", numbered(64))],
        _ => return None,
    })
}

/// Ports of a device with `count` channels in direction `dir`.
/// Channels beyond the known layout are numbered in a generic group.
pub fn ports(model: &str, is_daw: bool, dir: Dir, count: i64) -> Vec<Port> {
    let count = count.max(0) as usize;
    let groups = model_layout(model, dir).unwrap_or_else(|| {
        if is_daw {
            vec![("DAW", "SoundGrid", numbered(count))]
        } else {
            // generic I/O (e.g. "Generic 64 IO") is shown as AES by LV1
            vec![("Digital", "AES", numbered(count))]
        }
    });
    let mut ports = Vec::new();
    let mut channel = 0;
    for (major, minor, labels) in groups {
        for label in labels {
            if channel >= count {
                return ports;
            }
            if !major.is_empty() {
                ports.push(Port { channel: channel as i64, major: major.into(), minor: minor.into(), label });
            }
            channel += 1;
        }
    }
    for c in channel..count {
        ports.push(Port { channel: c as i64, major: String::new(), minor: String::new(), label: (c + 1).to_string() });
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models() {
        let out = ports("IOS-XL", false, Dir::Out, 14);
        let labels: Vec<_> = out.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, ["1", "2", "3", "4", "5", "6", "7", "8", "1L", "1R", "2L", "2R", "L", "R"]);
        assert_eq!(out[8].minor, "Phones");
        assert_eq!(out[13].major, "Digital");

        let iox = ports("IOX", false, Dir::Out, 16);
        assert_eq!(iox.len(), 14);
        assert_eq!(iox[5].label, "6");
        assert_eq!((iox[6].label.as_str(), iox[6].channel), ("1L", 8));
        assert_eq!((iox[13].label.as_str(), iox[13].channel), ("4R", 15));

        // more channels than the layout knows -> generic numbering
        let more = ports("IOX", false, Dir::In, 14);
        assert_eq!(more[13].label, "14");
        assert_eq!(more[13].major, "");

        assert_eq!(ports("", true, Dir::In, 32)[31].label, "32");
    }

    #[test]
    fn model_names() {
        assert_eq!(model_key("DiGiGrid IOS-XL"), "IOSXL");
        assert_eq!(model_key("IONIC 24"), "IONIC24");
        assert_eq!(model_key("DSPRO StageGrid 4000"), "STAGEGRID4000");
        assert_eq!(model_key("DiGiGrid D"), "D");

        let ionic = ports("IONIC 24", false, Dir::Out, 26);
        assert_eq!((ionic[17].minor.as_str(), ionic[17].label.as_str()), ("Line", "18"));
        assert_eq!((ionic[18].minor.as_str(), ionic[18].label.as_str()), ("Phones", "1L"));
        assert_eq!((ionic[25].minor.as_str(), ionic[25].label.as_str()), ("AES", "2R"));

        let ioc = ports("IOC", false, Dir::In, 34);
        assert_eq!((ioc[2].minor.as_str(), ioc[2].label.as_str()), ("Line", "1"));
        assert_eq!((ioc[26].minor.as_str(), ioc[26].label.as_str()), ("ADAT", "1"));

        let madi = ports("MGB", false, Dir::In, 64);
        assert_eq!((madi.len(), madi[63].minor.as_str()), (64, "MADI"));
    }
}
