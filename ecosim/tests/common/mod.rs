//! Helpers shared by the integration test files.

use std::path::Path;

/// `--set` overrides that put the default params back on the 64×64×32 world without rain gradient
/// or slope (shot 15): the world every manifest, fixture and test fact before shot 15 was made on.
pub const SQUARE: [&str; 3] = ["world.width=64", "climate.rain_gradient=0", "world.slope_bias=0"];

/// [`SQUARE`] followed by `extra`, as `--set` strings.
pub fn square_set(extra: &[&str]) -> Vec<String> {
    SQUARE.iter().chain(extra).map(|s| s.to_string()).collect()
}

/// The cheap world for behavioural tests (shot G4b): the 64x64x32 world at the default west-east
/// rain gradient. It is [`SQUARE`] plus the gradient, and the gradient is the difference that
/// matters: at the corrected 800 mm a year, a flat world watered evenly no longer keeps a tree
/// alive, because a tree column is charged its own transpiration on top of its patch's
/// evapotranspiration (`sweeps/shotG4b/FINDINGS.md`). A forced-extinction test has to force one
/// mechanism in a world that is otherwise healthy, so those tests run here and only the byte
/// fixtures stay on [`SQUARE`].
pub const SMALL: [&str; 2] = ["world.width=64", "world.slope_bias=0"];

/// [`SMALL`] followed by `extra`, as `--set` strings.
pub fn small_set(extra: &[&str]) -> Vec<String> {
    SMALL.iter().chain(extra).map(|s| s.to_string()).collect()
}

/// The crate's `params.toml` on the square world.
pub fn square() -> ecosim::Params {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("params.toml");
    ecosim::Params::load_with(&path, &square_set(&[])).unwrap()
}

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
    let p = square();
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

/// Put a run back on the pre-G5 soil: the nutrient tier off, and `climate.decay_k` at the 6.0 the
/// file held before shot G5 corrected it. Two things and not one, because the tier's switch does not
/// cover the decay rate -- the wrong rate was a fact about litter, not about nutrients
/// (`ecosim/TUNING.md`, shot G5). Every manifest and fixture cut from before G5 needs both to be
/// reproducible.
pub fn pre_g5(p: &mut ecosim::Params) {
    p.npk.enabled = false;
    p.climate.decay_k = 6.0;
}

/// [`without_npk`] and then [`without_water`], the cut a pre-G5 run with the water tier off needs.
/// Order matters: both cut the last six columns of `series.csv`, and the nutrient six are outside
/// the water six.
pub fn without_npk_and_water(name: &Path, bytes: Vec<u8>) -> Vec<u8> {
    without_water(name, without_npk(name, bytes))
}

/// A run-directory file of a run with `npk.enabled=false` as the pre-G5 ecosim wrote it: the six
/// nutrient columns cut from `series.csv`, each asserted to be 0. Nothing else changes, because with
/// the nutrient tier off no pool is allocated, `npk.bin` is not written and `state.bin` keeps its
/// pre-G5 layout. The run still has to be made with `climate.decay_k` set back to its pre-G5 6.0:
/// the tier's switch does not cover that, because the wrong decay rate was a fact about litter and
/// not about nutrients (`ecosim/TUNING.md`, shot G5).
///
/// The two pipe columns shot G6 put after the nutrient six are cut first ([`without_pipes`]).
pub fn without_npk(name: &Path, bytes: Vec<u8>) -> Vec<u8> {
    let bytes = without_pipes(name, bytes);
    match name.file_name().and_then(|n| n.to_str()) {
        Some("series.csv") => {
            let text = String::from_utf8(bytes).unwrap();
            let mut out = String::with_capacity(text.len());
            for (i, line) in text.lines().enumerate() {
                let f: Vec<&str> = line.split(',').collect();
                let (keep, npk) = f.split_at(f.len() - 6);
                if i == 0 {
                    assert_eq!(npk[0], "soil_n", "series.csv header");
                } else {
                    assert!(npk.iter().all(|v| *v == "0.0000"), "nutrients with the tier off: {line}");
                }
                out.push_str(&keep.join(","));
                out.push('\n');
            }
            out.into_bytes()
        }
        _ => bytes,
    }
}

/// A run-directory file of a run with `hydro.enabled=false` as the pre-G4 ecosim wrote it: the six
/// water columns cut from `series.csv`, each asserted to be 0. Nothing else changes, because with
/// the water tier off no snapshot file is added and `state.bin` keeps its pre-G4 layout.
pub fn without_water(name: &Path, bytes: Vec<u8>) -> Vec<u8> {
    match name.file_name().and_then(|n| n.to_str()) {
        Some("series.csv") => {
            let text = String::from_utf8(bytes).unwrap();
            let mut out = String::with_capacity(text.len());
            for (i, line) in text.lines().enumerate() {
                let f: Vec<&str> = line.split(',').collect();
                let (keep, water) = f.split_at(f.len() - 6);
                if i == 0 {
                    assert_eq!(water[0], "rain_mm", "series.csv header");
                } else {
                    assert!(water.iter().all(|v| *v == "0.0000"), "water with the tier off: {line}");
                }
                out.push_str(&keep.join(","));
                out.push('\n');
            }
            out.into_bytes()
        }
        _ => bytes,
    }
}

/// A run-directory file of a run without storm drains as the pre-G6 ecosim wrote it: the two pipe
/// columns cut from `series.csv`, each asserted to be 0. Nothing else changes: a world with no
/// pipes, or `pipes.capacity_scale=0`, allocates nothing, logs no `pipe` event and keeps
/// `state.bin` as it was.
pub fn without_pipes(name: &Path, bytes: Vec<u8>) -> Vec<u8> {
    match name.file_name().and_then(|n| n.to_str()) {
        Some("series.csv") => {
            let text = String::from_utf8(bytes).unwrap();
            let mut out = String::with_capacity(text.len());
            for (i, line) in text.lines().enumerate() {
                let f: Vec<&str> = line.split(',').collect();
                let (keep, pipes) = f.split_at(f.len() - 2);
                if i == 0 {
                    assert_eq!(pipes[0], "pipe_in_mm", "series.csv header");
                } else {
                    assert!(pipes.iter().all(|v| *v == "0.0000"), "pipes with no drain: {line}");
                }
                out.push_str(&keep.join(","));
                out.push('\n');
            }
            out.into_bytes()
        }
        _ => bytes,
    }
}
