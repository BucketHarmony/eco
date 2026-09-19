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

/// Bytes of `state.bin` before the tree count, all fixed-size: magic, version, 3 counters, RNG,
/// deaths, moisture and fertility, patches.
const STATE_HEAD_BYTES: usize = 8 + 4 + 12 + 32 + 8 + 16 + 40 + 2 * 4096 * 4 + 64 * 16;
/// Bytes per tree and per animal record in `state.bin`.
const STATE_TREE_BYTES: usize = 19;
const STATE_ANIMAL_BYTES: usize = 26;

/// The default traits of each species, formatted as `series.csv` and `entities.json` write them:
/// (series means, entities.json fields).
fn default_traits() -> [([f32; 3], String); 2] {
    let p = ecosim::Params::load_default();
    [ecosim::animals::Kind::Grazer, ecosim::animals::Kind::Hunter].map(|k| {
        let t = p.default_traits(k);
        let json = format!(
            ",\"energy_cost_mult\":{:?},\"flee_distance\":{:?},\"repro_threshold\":{:?}}}",
            t.energy_cost_mult, t.flee_distance, t.repro_threshold
        );
        (t.as_array(), json)
    })
}

/// A run-directory file of a run with `heredity.mutation=0` (and the default floors of 0) as the
/// pre-trait ecosim wrote it: the 12 trait columns cut from `series.csv`, the three trait fields
/// cut from every `entities.json` animal, and `state.bin` back to layout version 2 without its traits
/// section. Every cut value is asserted to be the species default (or 0 for a species with no live
/// animals), with standard deviation 0.
pub fn without_traits(name: &Path, bytes: Vec<u8>) -> Vec<u8> {
    let defaults = default_traits();
    match name.file_name().and_then(|n| n.to_str()) {
        Some("series.csv") => {
            let text = String::from_utf8(bytes).unwrap();
            let mut out = String::with_capacity(text.len());
            for (i, line) in text.lines().enumerate() {
                let f: Vec<&str> = line.split(',').collect();
                let (keep, traits) = f.split_at(f.len() - 12);
                if i == 0 {
                    assert_eq!(traits[0], "grazer_energy_cost_mult_mean", "series.csv header");
                } else {
                    for (k, (d, _)) in defaults.iter().enumerate() {
                        let got: Vec<&str> = traits[6 * k..6 * k + 6].to_vec();
                        let want: Vec<String> = d.iter().flat_map(|v| [format!("{v:.4}"), "0.0000".into()]).collect();
                        assert!(got == want || got.iter().all(|v| *v == "0.0000"), "traits vary at mutation 0: {line}");
                    }
                }
                out.push_str(&keep.join(","));
                out.push('\n');
            }
            out.into_bytes()
        }
        Some("entities.json") => {
            let mut text = String::from_utf8(bytes).unwrap();
            for (_, json) in &defaults {
                text = text.replace(json.as_str(), "}");
            }
            assert!(!text.contains("energy_cost_mult"), "an animal with non-default traits at mutation 0");
            text.into_bytes()
        }
        Some("state.bin") => {
            let mut b = bytes;
            assert_eq!(b[8..12], 3u32.to_le_bytes(), "state.bin layout version");
            let u = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
            let mut at = STATE_HEAD_BYTES;
            at += 4 + u(&b, at) * STATE_TREE_BYTES;
            let grazers = u(&b, at);
            at += 4 + grazers * STATE_ANIMAL_BYTES;
            let hunters = u(&b, at);
            at += 4 + hunters * STATE_ANIMAL_BYTES;
            for _ in 0..4096 {
                at += 4 + u(&b, at) * 4;
            }
            at += STATE_FIRE_BYTES;
            assert_eq!(b.len() - at, (grazers + hunters) * 12, "traits section length");
            for (n, rec) in b[at..].chunks(12).enumerate() {
                let v: Vec<f32> = rec.chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
                assert_eq!(v, defaults[(n >= grazers) as usize].0, "state.bin traits of animal {n}");
            }
            b[8..12].copy_from_slice(&2u32.to_le_bytes());
            b.truncate(at);
            b
        }
        _ => bytes,
    }
}
