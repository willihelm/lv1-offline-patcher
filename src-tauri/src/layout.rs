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

fn model_layout(model: &str, dir: Dir) -> Option<Vec<Group>> {
    Some(match (model, dir) {
        ("IOS-XL", Dir::In) => vec![("Analog", "Mic/L", numbered(8)), ("Digital", "AES", lr())],
        ("IOS-XL", Dir::Out) => vec![
            ("Analog", "Line", numbered(8)),
            ("Analog", "Phones", stereo_pairs(2)),
            ("Digital", "AES", lr()),
        ],
        ("IOX", Dir::In) => vec![("Analog", "Mic/L", numbered(12))],
        // the data has 16 outputs; 7-8 do not exist on the IOX
        ("IOX", Dir::Out) => vec![("Analog", "Line", numbered(6)), hidden(2), ("Analog", "Phones", stereo_pairs(4))],
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
}
