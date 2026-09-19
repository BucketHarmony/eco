//! `state.bin`: the part of a snapshot the other files don't hold exactly, and the restore path that
//! turns a snapshot directory back into a `Sim` that steps identically to the one that wrote it.
//!
//! Layout (version 2), all integers and floats little-endian, no padding:
//!
//! | field | type |
//! |---|---|
//! | magic | `b"ECOSTATE"` |
//! | state version | u32 = 2 |
//! | tick, next_id, hunter_immigrants | 3 × u32 |
//! | RNG seed, stream, word position | [u8; 32], u64, u128 |
//! | deaths this tick | 2 × 5 × u32 (grazers then hunters, `Cause` order) |
//! | moisture, fertility | 2 × 4096 × f32 (column order) |
//! | patches | 64 × (grass, shrub, detritus, temperature) f32 |
//! | trees | u32 count, then per tree: id u32, x u8, y u8, age u32, dry_ticks u32, lifespan u32, alive u8 |
//! | grazers, hunters | each: u32 count, then per animal: id u32, x f32, y f32, energy f32, age u32, cooldown u32, state u8, alive u8 |
//! | grazer grid | per column (4096): u32 length, then that many u32 grazer indices |
//! | fire | total_burnt u32, then 64 × burning_ticks_left u32 (patch order) |
//!
//! Version 2 is version 1 with the fire section appended; nothing before it moved.
//!
//! Entities are stored in `Vec` order, dead ones included, because indices into the Vecs (the trunk
//! index, the grids) and the update order depend on it. Everything else a `Sim` holds is recomputed
//! on load, and the recompute is tested to be bit-identical (DECISIONS.md, "Full-state snapshots").

use crate::animals::{Animal, Kind, State};
use crate::params::Params;
use crate::sim::{offsets_within, Deaths, Patch, Sim, NO_TREE};
use crate::trees::Tree;
use crate::world::{patch_of, World, COLS, PATCHES, ROCK, SOIL, VOXELS, WX, WY, WZ};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::fs;
use std::path::Path;

/// First bytes of every `state.bin`.
pub const MAGIC: &[u8; 8] = b"ECOSTATE";
/// `state.bin` layout version; `decode` rejects any other.
pub const STATE_VERSION: u32 = 2;

const STATES: [State; 6] = [State::Flee, State::Eat, State::Move, State::Wander, State::Rest, State::Hunt];

/// Encode the sim's non-derived state as `state.bin` bytes.
pub fn encode(sim: &Sim) -> Vec<u8> {
    let mut b = Vec::with_capacity(64 * 1024);
    b.extend_from_slice(MAGIC);
    for v in [STATE_VERSION, sim.tick, sim.next_id, sim.hunter_immigrants] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&sim.rng.get_seed());
    b.extend_from_slice(&sim.rng.get_stream().to_le_bytes());
    b.extend_from_slice(&sim.rng.get_word_pos().to_le_bytes());
    for n in sim.deaths.iter().flatten() {
        b.extend_from_slice(&n.to_le_bytes());
    }
    for v in sim.moisture.iter().chain(&sim.fertility) {
        b.extend_from_slice(&v.to_le_bytes());
    }
    for p in &sim.patches {
        for v in [p.grass, p.shrub, p.detritus, p.temperature] {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b.extend_from_slice(&(sim.trees.len() as u32).to_le_bytes());
    for t in &sim.trees {
        b.extend_from_slice(&t.id.to_le_bytes());
        b.extend_from_slice(&[t.x, t.y]);
        for v in [t.age, t.dry_ticks, t.lifespan] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.push(t.alive as u8);
    }
    for group in [&sim.grazers, &sim.hunters] {
        b.extend_from_slice(&(group.len() as u32).to_le_bytes());
        for a in group {
            b.extend_from_slice(&a.id.to_le_bytes());
            for v in [a.x, a.y, a.energy] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            b.extend_from_slice(&a.age.to_le_bytes());
            b.extend_from_slice(&a.cooldown.to_le_bytes());
            b.push(STATES.iter().position(|s| *s == a.state).unwrap_or(0) as u8);
            b.push(a.alive as u8);
        }
    }
    for cell in &sim.grazer_grid {
        b.extend_from_slice(&(cell.len() as u32).to_le_bytes());
        for i in cell {
            b.extend_from_slice(&i.to_le_bytes());
        }
    }
    b.extend_from_slice(&sim.total_burnt.to_le_bytes());
    for p in &sim.patches {
        b.extend_from_slice(&p.burning_ticks_left.to_le_bytes());
    }
    b
}

/// Little-endian reader over `state.bin` bytes; every read fails cleanly on truncation.
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.at..self.at + n).ok_or_else(|| format!("state.bin truncated at byte {}", self.at))?;
        self.at += n;
        Ok(s)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("take returns N bytes"))
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, String> {
        (0..n).map(|_| self.f32()).collect()
    }
    fn flag(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(format!("state.bin: bad flag {v} at byte {}", self.at - 1)),
        }
    }
    /// A count, bounded so a corrupt file can't ask for a huge allocation.
    fn count(&mut self, what: &str) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > 16 * 1024 * 1024 {
            return Err(format!("state.bin: implausible {what} count {n}"));
        }
        Ok(n)
    }
}

/// Everything `state.bin` holds.
#[derive(Debug)]
struct Decoded {
    tick: u32,
    next_id: u32,
    hunter_immigrants: u32,
    rng: ChaCha8Rng,
    deaths: Deaths,
    moisture: Vec<f32>,
    fertility: Vec<f32>,
    patches: Vec<Patch>,
    trees: Vec<Tree>,
    grazers: Vec<Animal>,
    hunters: Vec<Animal>,
    grazer_grid: Vec<Vec<u32>>,
    total_burnt: u32,
}

fn decode_animals(r: &mut Reader, kind: Kind) -> Result<Vec<Animal>, String> {
    let n = r.count("animal")?;
    (0..n)
        .map(|_| {
            let id = r.u32()?;
            let (x, y, energy) = (r.f32()?, r.f32()?, r.f32()?);
            let (age, cooldown) = (r.u32()?, r.u32()?);
            let s = r.u8()? as usize;
            let state = *STATES.get(s).ok_or_else(|| format!("state.bin: bad animal state {s}"))?;
            let alive = r.flag()?;
            if !(0.0..WX as f32).contains(&x) || !(0.0..WY as f32).contains(&y) {
                return Err(format!("state.bin: animal {id} off the world at ({x}, {y})"));
            }
            Ok(Animal { id, kind, x, y, energy, age, cooldown, state, alive })
        })
        .collect()
}

fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let mut r = Reader { b: bytes, at: 0 };
    if r.take(8)? != MAGIC {
        return Err("state.bin: bad magic (not an ecosim state file)".into());
    }
    let version = r.u32()?;
    if version != STATE_VERSION {
        return Err(format!("state.bin: state version {version}, this ecosim reads {STATE_VERSION}"));
    }
    let (tick, next_id, hunter_immigrants) = (r.u32()?, r.u32()?, r.u32()?);
    let mut rng = ChaCha8Rng::from_seed(r.arr()?);
    rng.set_stream(u64::from_le_bytes(r.arr()?));
    rng.set_word_pos(u128::from_le_bytes(r.arr()?));
    let mut deaths = Deaths::default();
    for n in deaths.iter_mut().flatten() {
        *n = r.u32()?;
    }
    let (moisture, fertility) = (r.f32s(COLS)?, r.f32s(COLS)?);
    let patches = (0..PATCHES)
        .map(|_| {
            Ok(Patch {
                grass: r.f32()?,
                shrub: r.f32()?,
                detritus: r.f32()?,
                temperature: r.f32()?,
                burning_ticks_left: 0,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let nt = r.count("tree")?;
    let trees = (0..nt)
        .map(|_| {
            let id = r.u32()?;
            let (x, y) = (r.u8()?, r.u8()?);
            let (age, dry_ticks, lifespan) = (r.u32()?, r.u32()?, r.u32()?);
            if x as usize >= WX || y as usize >= WY {
                return Err(format!("state.bin: tree {id} off the world at ({x}, {y})"));
            }
            Ok(Tree { id, x, y, age, dry_ticks, lifespan, alive: r.flag()? })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let grazers = decode_animals(&mut r, Kind::Grazer)?;
    let hunters = decode_animals(&mut r, Kind::Hunter)?;
    let grazer_grid = (0..COLS)
        .map(|_| {
            let n = r.count("grid cell")?;
            (0..n)
                .map(|_| {
                    let i = r.u32()?;
                    match grazers.get(i as usize) {
                        Some(g) if g.alive => Ok(i),
                        _ => Err(format!("state.bin: grazer grid holds {i}, which is not a live grazer")),
                    }
                })
                .collect()
        })
        .collect::<Result<Vec<_>, String>>()?;
    let total_burnt = r.u32()?;
    let mut patches = patches;
    for p in &mut patches {
        p.burning_ticks_left = r.u32()?;
    }
    if r.at != bytes.len() {
        return Err(format!("state.bin: {} trailing bytes", bytes.len() - r.at));
    }
    Ok(Decoded {
        tick,
        next_id,
        hunter_immigrants,
        rng,
        deaths,
        moisture,
        fertility,
        patches,
        trees,
        grazers,
        hunters,
        grazer_grid,
        total_burnt,
    })
}

/// Terrain height per column from a material grid: the topmost soil or rock voxel.
fn ground_from_material(material: &[u8]) -> Vec<u8> {
    (0..COLS)
        .map(|c| (0..WZ).rev().find(|&z| matches!(material[c + COLS * z], SOIL | ROCK)).unwrap_or(0) as u8)
        .collect()
}

impl Sim {
    /// Rebuild a sim from a snapshot directory written with `state.bin`. `params` must be the
    /// params the run was using at that tick (from its `meta.json`).
    ///
    /// Read from the files: light (`light.bin`) and everything in `state.bin`. Recomputed: the
    /// terrain fields from the material grid (and checked against `material.bin` and `height.bin`),
    /// the trunk index, canopy cover, per-patch grazer counts, the hunter grid and the neighbour
    /// offset lists.
    pub fn restore(params: Params, snap_dir: &Path) -> Result<Sim, String> {
        let read = |f: &str, len: usize| -> Result<Vec<u8>, String> {
            let p = snap_dir.join(f);
            let b = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            if len > 0 && b.len() != len {
                return Err(format!("{}: {} bytes, expected {len}", p.display(), b.len()));
            }
            Ok(b)
        };
        let (material, light, height) =
            (read("material.bin", VOXELS)?, read("light.bin", VOXELS)?, read("height.bin", COLS)?);
        let d = decode(&read("state.bin", 0)?)?;
        let mut world = World::from_heights(&ground_from_material(&material), &params);
        if world.material != material || world.height != height {
            return Err(format!(
                "{}: terrain does not rebuild from material.bin with these params (world.* keys changed?)",
                snap_dir.display()
            ));
        }
        world.light = light;
        let mut sim = Sim {
            seek_offsets: offsets_within(params.hunter.seek_radius),
            flee_offsets: offsets_within(params.grazer.flee_radius),
            params,
            rng: d.rng,
            world,
            moisture: d.moisture,
            fertility: d.fertility,
            patches: d.patches,
            trees: d.trees,
            grazers: d.grazers,
            hunters: d.hunters,
            trunk_at: vec![NO_TREE; COLS],
            canopy_cover: vec![false; COLS],
            grazers_in_patch: vec![0; PATCHES],
            grazer_grid: d.grazer_grid,
            hunter_grid: vec![Vec::new(); COLS],
            hunters_in_patch: vec![0; PATCHES],
            tick: d.tick,
            next_id: d.next_id,
            hunter_immigrants: d.hunter_immigrants,
            deaths: d.deaths,
            total_burnt: d.total_burnt,
        };
        sim.recompute_derived();
        Ok(sim)
    }

    /// Recompute every field `state.bin` leaves out from the fields it holds.
    fn recompute_derived(&mut self) {
        for (i, t) in self.trees.iter().enumerate().filter(|(_, t)| t.alive) {
            self.trunk_at[t.col()] = i as u32;
        }
        for c in 0..COLS {
            self.canopy_cover[c] = !self.canopy_z((c % WX) as i32, (c / WX) as i32).is_empty();
        }
        for g in self.grazers.iter().filter(|g| g.alive) {
            self.grazers_in_patch[patch_of(g.x as usize, g.y as usize)] += 1;
        }
        self.rebuild_hunter_grid();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{snapshot_dir_name, write_snapshot};
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn scratch_dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "ecosim-state-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn stepped(seed: u64, ticks: u32) -> Sim {
        let mut sim = Sim::new(Params::load_default(), seed);
        while sim.tick < ticks {
            sim.step();
        }
        sim
    }

    fn restored(sim: &Sim) -> Sim {
        let dir = scratch_dir();
        write_snapshot(sim, &dir, true).unwrap();
        let r = Sim::restore(sim.params.clone(), &dir.join(snapshot_dir_name(sim.tick))).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        r
    }

    /// Every field of the two sims, the recomputed ones included, is bit-identical. The hunter grid
    /// is compared as rebuilt: the sim only reads it after `update_animals` rebuilds it.
    fn assert_same(a: &Sim, b: &Sim) -> Result<(), TestCaseError> {
        prop_assert_eq!(encode(a), encode(b));
        prop_assert!(a.world.material == b.world.material && a.world.light == b.world.light);
        prop_assert!(a.world.height == b.world.height && a.world.ground == b.world.ground);
        prop_assert!(a.world.class == b.world.class && a.world.patch_soil == b.world.patch_soil);
        prop_assert!(a.world.patch_dist == b.world.patch_dist);
        prop_assert!(a.trunk_at == b.trunk_at, "trunk_at");
        prop_assert!(a.canopy_cover == b.canopy_cover, "canopy_cover");
        prop_assert!(a.grazers_in_patch == b.grazers_in_patch, "grazers_in_patch");
        prop_assert!(a.hunters_in_patch == b.hunters_in_patch, "hunters_in_patch");
        prop_assert!(a.seek_offsets == b.seek_offsets && a.flee_offsets == b.flee_offsets);
        let (mut ah, mut bh) = (a.hunter_grid.clone(), b.hunter_grid.clone());
        crate::sim::rebuild_grid(&mut ah, &a.hunters);
        crate::sim::rebuild_grid(&mut bh, &b.hunters);
        prop_assert!(ah == bh, "hunter_grid");
        Ok(())
    }

    /// Restore steps identically: a sim restored from its snapshot at `ticks` equals the original
    /// field for field, and the two stay bit-identical, stats row by stats row, for 500 more ticks.
    fn restore_steps_identically(seed: u64, ticks: u32) -> Result<(), TestCaseError> {
        let mut a = stepped(seed, ticks);
        let mut b = restored(&a);
        assert_same(&a, &b)?;
        for _ in 0..500 {
            a.step();
            b.step();
            prop_assert_eq!(a.stats(), b.stats(), "diverged at tick {}", a.tick);
        }
        assert_same(&a, &b)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(crate::cases(12)))]

        #[test]
        fn prop_restore_steps_identically(seed in any::<u64>(), ticks in 0u32..600) {
            restore_steps_identically(seed, ticks)?;
        }
    }

    /// Tick 50 is the first tree update, and a compaction interval is crossed in the 500 ticks after.
    #[test]
    fn restore_regression_after_first_tree_update() {
        restore_steps_identically(42, 50).unwrap();
    }

    /// Between compactions: dead entities are still in the Vecs and the grazer grid has been reshuffled
    /// by swap-removal, so neither Vec order nor the grid can be rebuilt; both come from state.bin.
    #[test]
    fn restore_regression_with_dead_entities_uncompacted() {
        let a = stepped(7, 1234);
        assert!(a.grazers.iter().any(|g| !g.alive) || a.trees.iter().any(|t| !t.alive));
        restore_steps_identically(7, 1234).unwrap();
    }

    #[test]
    fn decode_rejects_corrupt_files() {
        let good = encode(&stepped(1, 20));
        assert!(decode(&good).is_ok());
        let mut bad_magic = good.clone();
        bad_magic[0] = b'X';
        assert!(decode(&bad_magic).unwrap_err().contains("magic"));
        let mut bad_version = good.clone();
        bad_version[8] = 9;
        assert!(decode(&bad_version).unwrap_err().contains("state version 9"));
        assert!(decode(&good[..good.len() - 1]).unwrap_err().contains("truncated"));
        let mut long = good.clone();
        long.push(0);
        assert!(decode(&long).unwrap_err().contains("trailing"));
    }

    #[test]
    fn restore_rejects_terrain_built_with_other_params() {
        let sim = stepped(1, 10);
        let dir = scratch_dir();
        write_snapshot(&sim, &dir, true).unwrap();
        let mut p = sim.params.clone();
        p.world.soil_depth += 1;
        let err = Sim::restore(p, &dir.join(snapshot_dir_name(10))).err().unwrap();
        assert!(err.contains("terrain does not rebuild"), "{err}");
        fs::remove_file(dir.join(snapshot_dir_name(10)).join("state.bin")).unwrap();
        let err = Sim::restore(sim.params.clone(), &dir.join(snapshot_dir_name(10))).err().unwrap();
        assert!(err.contains("state.bin"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
