//! Helpers shared by the integration test files.

use std::path::Path;

/// Bytes of the fire section `state.bin` version 2 appends: total_burnt, then 64 burning counters.
const STATE_FIRE_BYTES: usize = 4 + 64 * 4;

/// A run-directory file of a run with `fire.base_rate=0` as the pre-fire ecosim wrote it: the two
/// fire columns cut from `series.csv`, the always-0 `burning_ticks_left` cut from `patches.json`, and
/// `state.bin` back to layout version 1 without its fire section. Other files are unchanged.
pub fn without_fire(name: &Path, bytes: Vec<u8>) -> Vec<u8> {
    match name.file_name().and_then(|n| n.to_str()) {
        Some("series.csv") => {
            let text = String::from_utf8(bytes).unwrap();
            let mut out = String::with_capacity(text.len());
            for line in text.lines() {
                let mut it = line.rsplitn(3, ',');
                let (burnt, burning, rest) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
                assert!(burnt == "total_burnt" || (burnt == "0" && burning == "0"), "fire in a rate-0 run: {line}");
                out.push_str(rest);
                out.push('\n');
            }
            out.into_bytes()
        }
        Some("patches.json") => {
            let text = String::from_utf8(bytes).unwrap();
            let cut = text.replace(",\"burning_ticks_left\":0}", "}");
            assert!(!cut.contains("burning_ticks_left"), "a patch burns in a rate-0 run");
            cut.into_bytes()
        }
        Some("state.bin") => {
            let mut b = bytes;
            assert_eq!(b[8..12], 2u32.to_le_bytes(), "state.bin layout version");
            assert!(b[b.len() - STATE_FIRE_BYTES..].iter().all(|&x| x == 0), "fire state in a rate-0 run");
            b[8..12].copy_from_slice(&1u32.to_le_bytes());
            b.truncate(b.len() - STATE_FIRE_BYTES);
            b
        }
        _ => bytes,
    }
}
