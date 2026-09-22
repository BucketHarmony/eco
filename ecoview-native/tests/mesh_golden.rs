//! The CI gate (V0-spike.md, item 7a): a fixed chunk in, a hashed vertex and index buffer out.
//!
//! Greedy meshing is a pure function of the chunk buffer, so this needs no GPU, no window and no
//! engine -- CI runs it with `--no-default-features`, which does not compile Bevy at all. Three
//! chunks: flat ground, a staircase, and one building block.

use ecoview_native::bundle::{Bundle, Tree};
use ecoview_native::mesh::{mesh_chunk, Scratch};
use ecoview_native::palette::surface_palette;
use ecoview_native::sim::{self, SimJob, SimState};
use ecoview_native::tree::{Life, TreeForm, CROWN_BASE_FRACTION};
use ecoview_native::voxel::{ChunkPos, EditAction, VoxelWorld};

/// A bare bundle of `n` x `n` cells at `cell_m`, all lawn, all at `h` metres.
fn flat(n: usize, cell_m: f32, h: f32) -> Bundle {
    Bundle {
        name: "test".into(),
        size_m: n as f32 * cell_m,
        ground_cell_m: cell_m,
        width: n,
        depth: n,
        media: [
            "soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        ground_h: vec![h; n * n],
        medium: vec![1; n * n],
        building_h: vec![0.0; n * n],
        trees: Vec::new(),
        shrubs: Vec::new(),
        source: String::new(),
        dir: None,
    }
}

fn mesh_one(b: &Bundle) -> (u64, usize, usize) {
    let w = VoxelWorld::from_bundle(b);
    let m = mesh_chunk(
        &w,
        ChunkPos { x: 0, y: 0, z: 0 },
        &surface_palette(),
        &mut Scratch::new(),
    );
    (m.hash(), m.positions.len(), m.indices.len())
}

#[test]
fn golden_flat() {
    // 16 x 16 lawn at 4 m on a 0.5 m lattice: 10 quads -- top, floor, and each of the four walls split
    // in two because the lawn voxel on top of each column is a different id from the soil under it.
    let (hash, verts, indices) = mesh_one(&flat(16, 0.5, 4.0));
    assert_eq!((hash, verts, indices), (0x7c23_841c_633d_3d9d, 40, 60));
}

#[test]
fn golden_stepped() {
    let mut b = flat(16, 0.5, 4.0);
    for y in 0..16 {
        for x in 0..16 {
            b.ground_h[x + 16 * y] = 4.0 + (x / 2) as f32 * 0.5;
            b.medium[x + 16 * y] = if x % 4 == 0 { 6 } else { 1 };
        }
    }
    let (hash, verts, indices) = mesh_one(&b);
    assert_eq!((hash, verts, indices), (0x38b9_47be_35e7_22cd, 256, 384));
}

#[test]
fn golden_building() {
    let mut b = flat(16, 0.5, 4.0);
    for y in 6..9 {
        for x in 6..9 {
            b.building_h[x + 16 * y] = 5.0;
            b.medium[x + 16 * y] = 7; // roof
        }
    }
    let (hash, verts, indices) = mesh_one(&b);
    assert_eq!((hash, verts, indices), (0x85e9_ae35_89ca_2585, 72, 108));
}

/// The cube edge comes from the bundle. A world at 0.25 m is the same shape twice as fine, so its
/// mesh is a different hash and its top face sits at a different height -- a hard-coded 0.5 fails here
/// (V0-spike.md, correction 4; ecoview/DECISIONS.md, "E3 block world").
#[test]
fn cell_size_comes_from_the_bundle() {
    let coarse = mesh_one(&flat(16, 0.5, 4.0));
    let fine = mesh_one(&flat(16, 0.25, 4.0));
    assert_ne!(coarse.0, fine.0);
    let w = VoxelWorld::from_bundle(&flat(16, 0.25, 4.0));
    assert_eq!(w.ground_level(0, 0), 16);
}

/// A tree is a trunk and a canopy blob, so it adds geometry above the ground it stands on.
#[test]
fn a_tree_adds_geometry() {
    let bare = mesh_one(&flat(16, 0.5, 4.0));
    let mut b = flat(16, 0.5, 4.0);
    b.trees.push(Tree {
        x: 4.0,
        y: 4.0,
        height: 6.0,
        crown_radius: 1.5,
        crown_base: 2.0,
    });
    let with = mesh_one(&b);
    assert!(
        with.1 > bare.1 + 100,
        "tree added {} vertices",
        with.1 - bare.1
    );
}

// ---- shot V1: the run directory ----
//
// These live in this file rather than in a `run_dir.rs` beside it because the CI gate runs exactly
// one target -- `cargo test --release --no-default-features --test mesh_golden` -- and adding a second
// one means editing `.github/workflows/ci.yml`, which belongs to a `ci` row and not to a viewer shot.
// Every test below is engine-free for the same reason the ones above are.

use ecoview_native::bundle::Shrub;
use ecoview_native::run::{Run, Stage};

/// Writes a run directory with `ticks` of snapshots every `every`, each holding `trees` trees.
fn write_run(
    dir: &std::path::Path,
    version: u32,
    size_m: usize,
    snaps: &[u64],
    trees: &[(i32, i32, &str)],
) {
    std::fs::create_dir_all(dir).unwrap();
    let list: Vec<String> = snaps.iter().map(|t| t.to_string()).collect();
    std::fs::write(
        dir.join("meta.json"),
        format!(
            r#"{{"format_version":{version},"dims":{{"x":{size_m},"y":{size_m},"z":32,"patch":8}},
               "seed":42,"ticks":{},"snapshot_every":100,"snapshots":[{}],
               "world":{{"name":"test","bundle":true,"ground_cell_m":0.5,
                         "ground_width":{},"ground_depth":{}}}}}"#,
            snaps.last().copied().unwrap_or(0),
            list.join(","),
            size_m * 2,
            size_m * 2,
        ),
    )
    .unwrap();
    for t in snaps {
        let sd = dir.join(format!("snap_{t:06}"));
        std::fs::create_dir_all(&sd).unwrap();
        let ents: Vec<String> = trees
            .iter()
            .enumerate()
            .map(|(i, (x, y, stage))| {
                format!(
                    r#"{{"id":{i},"kind":"tree","x":{x},"y":{y},"z":9,"age":100,"stage":"{stage}","lifespan":500}}"#
                )
            })
            .collect();
        std::fs::write(sd.join("entities.json"), format!("[{}]", ents.join(","))).unwrap();
    }
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ecoview-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn a_run_directory_is_read_and_checked_against_the_bundle() {
    let dir = tmp("ok");
    // 16 cells at 0.5 m is an 8 m site, so the ecology grid is 8 x 8 columns.
    write_run(&dir, 4, 8, &[0, 100, 200], &[(2, 3, "mature")]);
    let run = Run::load(&dir).unwrap();
    assert_eq!(run.snapshot_count(), 3);
    assert_eq!(run.tick_at(2), 200);
    assert_eq!(run.index_of_tick(140), 1, "140 is nearest snapshot 100");
    assert_eq!(
        run.index_of_tick(99_999),
        2,
        "past the end clamps to the last"
    );

    let mut b = flat(16, 0.5, 4.0);
    b.name = "test".into();
    run.check_against(&b).unwrap();

    let snap = run.trees_at(0).unwrap();
    assert_eq!(snap.trees.len(), 1);
    assert_eq!(snap.unknown_stage, 0);
    assert_eq!(
        snap.stages,
        [0, 0, 1],
        "one tree, and the run calls it mature"
    );
    // The column's centre, in metres, not its south-west corner.
    assert_eq!((snap.trees[0].x, snap.trees[0].y), (2.5, 3.5));
    // This run has no field files at all, so the crown light is the named fallback and every crown
    // is in full sun -- said out loud rather than drawn as shade.
    assert_eq!(snap.light, (1.0, 1.0, 1.0));
    assert!(
        snap.light_source.contains("this viewer's fallback"),
        "{}",
        snap.light_source
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A run of the wrong format, or about the wrong site, is refused by name. The viewer would otherwise
/// stand trees in the air over ground they were never computed on, with nothing on screen to say so.
#[test]
fn a_run_that_does_not_fit_the_bundle_is_refused() {
    let old = tmp("v3");
    write_run(&old, 3, 8, &[0], &[]);
    let e = Run::load(&old).unwrap_err().to_string();
    assert!(e.contains("format_version 3"), "{e}");

    let dir = tmp("mismatch");
    write_run(&dir, 4, 8, &[0], &[]);
    let run = Run::load(&dir).unwrap();

    let mut wrong_size = flat(32, 0.5, 4.0);
    wrong_size.name = "test".into();
    let e = run.check_against(&wrong_size).unwrap_err();
    assert!(e.contains("ground cells"), "{e}");

    let mut wrong_cell = flat(16, 0.25, 4.0);
    wrong_cell.name = "test".into();
    let e = run.check_against(&wrong_cell).unwrap_err();
    assert!(
        e.contains("ecology grid") || e.contains("ground cells"),
        "{e}"
    );

    let mut wrong_name = flat(16, 0.5, 4.0);
    wrong_name.name = "somewhere else".into();
    let e = run.check_against(&wrong_name).unwrap_err();
    assert!(e.contains("somewhere else"), "{e}");

    std::fs::remove_dir_all(&old).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A stage the viewer does not know is counted, not guessed at. A new stage in the simulator has to
/// be visible; silently drawing it as a sapling would hide it forever.
#[test]
fn an_unknown_stage_is_counted_and_not_drawn() {
    let dir = tmp("stage");
    write_run(
        &dir,
        4,
        8,
        &[0],
        &[(1, 1, "mature"), (2, 2, "ancient"), (3, 3, "young")],
    );
    let snap = Run::load(&dir).unwrap().trees_at(0).unwrap();
    assert_eq!(snap.trees.len(), 2);
    assert_eq!(snap.unknown_stage, 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The stage is read and ordered, and that is now all it does.
///
/// V1 had a `Stage::shape()` here and this test asserted that the three stages were three sizes.
/// Shot V3 took size away from the stage and gave it to `age` through the run's own curve, so the
/// size half of the claim moved to `age_is_where_a_tree_size_comes_from` and this one keeps the
/// parsing, which is still the part that must not guess.
#[test]
fn a_stage_is_parsed_or_refused() {
    assert_eq!(Stage::parse("mature"), Some(Stage::Mature));
    assert_eq!(Stage::parse("seedling"), None);
    assert_eq!(Stage::Sapling.index(), 0);
    assert!(Stage::Sapling.index() < Stage::Young.index());
    assert!(Stage::Young.index() < Stage::Mature.index());
}

/// Swapping a snapshot's trees changes the world's geometry, and the chunks reported stale are the
/// only ones whose mesh actually moved. This is the claim the timeline rests on: a snapshot change
/// remeshes what the trees touch, not the site.
#[test]
fn set_plants_reports_exactly_the_chunks_whose_mesh_changed() {
    let b = flat(128, 0.5, 4.0);
    let mut w = VoxelWorld::from_bundle(&b);
    let palette = surface_palette();
    let mut scratch = Scratch::new();
    let all = w.all_chunks();
    let before: Vec<u64> = all
        .iter()
        .map(|&c| mesh_chunk(&w, c, &palette, &mut scratch).hash())
        .collect();

    let tree = TreeForm::grown(8.5, 8.5, 3.0, 1.0, 0x51ce);
    let stale = w.set_plants(std::slice::from_ref(&tree), &[]);
    assert!(!stale.is_empty(), "one tree has to make some chunk stale");
    assert!(
        stale.len() < all.len(),
        "and not all of them: {}",
        stale.len()
    );

    let after: Vec<u64> = all
        .iter()
        .map(|&c| mesh_chunk(&w, c, &palette, &mut scratch).hash())
        .collect();
    for (i, c) in all.iter().enumerate() {
        if before[i] != after[i] {
            assert!(
                stale.contains(c),
                "chunk {c:?} changed but was not reported stale"
            );
        }
    }
    assert_ne!(before, after, "the tree has to show up somewhere");

    // Back to nothing: the world returns to exactly the mesh it had before the tree.
    let stale = w.set_plants(&[], &[]);
    assert!(!stale.is_empty());
    let back: Vec<u64> = all
        .iter()
        .map(|&c| mesh_chunk(&w, c, &palette, &mut scratch).hash())
        .collect();
    assert_eq!(before, back, "removing the tree has to undo it exactly");
}

/// `chunk_pos` is the inverse of `chunk_index`, which is what makes the stale set correct.
#[test]
fn chunk_index_and_chunk_pos_are_inverses() {
    let mut b = flat(200, 0.5, 4.0);
    b.shrubs.push(Shrub {
        x: 4.0,
        y: 4.0,
        height: 30.0,
        rx: 1.0,
        ry: 1.0,
        angle: 0.0,
    });
    let w = VoxelWorld::from_bundle(&b);
    assert!(w.chunk_count() > 1);
    for i in 0..w.chunk_count() {
        assert_eq!(w.chunk_index(w.chunk_pos(i)), i);
    }
}

// ---- shot V2: the ecological overlays ----
//
// Same rule as the V1 block above: these live in this file because the CI gate runs exactly one
// target. All of them are engine-free.

use ecoview_native::overlay::{band_of, Scale};
use ecoview_native::palette::{
    linear_rgba, palette, Overlay, BANDS, BAND_BASE, FIRE_BURNT, FIRE_QUIET,
};
use ecoview_native::voxel::{ColumnBands, CANOPY, ID_COUNT, SOIL, TRUNK};
use ecoview_native::ECO_CELL_M;

/// A run directory carrying every field an overlay reads, on an `n` x `n` ecology grid with
/// `patch` x `patch` patches.
///
/// The fields are deliberately non-uniform, because a test on a constant field cannot tell a correct
/// reader from one that returns the first byte: `moisture[c] = 4c`, fertility its complement, and
/// light bright everywhere except column 0.
///
/// `with_params` writes the `params` block the scales are read from **and** the `overlays` array the
/// ramp hues are read from (`ecosim` shot S2). Without it the run is one this viewer has to fall back
/// on its own constants for, in both halves, which is the other half of what is tested.
///
/// The hues written here are deliberately not the ones in `palette.rs`: `#010203` is nowhere in this
/// viewer, so a band that comes out that colour can only have come from the file.
fn write_overlay_run(dir: &std::path::Path, n: usize, patch: usize, with_params: bool) {
    let z = 8usize;
    let cols = n * n;
    let (px, py) = (n / patch, n / patch);
    let patches = px * py;
    std::fs::create_dir_all(dir).unwrap();
    let params = if with_params {
        r#","params":{"grass":{"temp":[0.0,5.0,30.0,35.0]},
                      "shrub":{"temp":[-5.0,0.0,28.0,33.0]},
                      "tree":{"temp":[-3.0,2.0,26.0,31.0]},
                      "disease":{"grazer_threshold":16},
                      "fire":{"duration":3}}"#
    } else {
        ""
    };
    let overlays = if with_params {
        r##","overlays":[{"name":"light","lo":"#010203","hi":"#fdfeff"},
                        {"name":"moisture","lo":"#ffffff","hi":"#1f4fd1"},
                        {"name":"fertility","lo":"#ffffff","hi":"#4a2c12"},
                        {"name":"temperature","lo":"#2040ff","hi":"#ff3020"},
                        {"name":"crowding","lo":"#ffffff","hi":"#d81b9c"},
                        {"name":"fire","lo":"#b3300a","hi":"#ffb020","burnt":"#0a0b0c"},
                        {"name":"traits","lo":"#1f5bff","mid":"#ffffff","hi":"#ff1f1f"}]"##
    } else {
        ""
    };
    std::fs::write(
        dir.join("meta.json"),
        format!(
            r##"{{"format_version":4,"dims":{{"x":{n},"y":{n},"z":{z},"patch":{patch}}},
               "seed":42,"ticks":100,"snapshot_every":100,"snapshots":[0,100],
               "world":{{"name":"test","bundle":true,"ground_cell_m":0.5,
                         "ground_width":{},"ground_depth":{}}},
               "species":[{{"id":0,"name":"grass","kind":"cover","color":"#7cc242"}},
                          {{"id":2,"name":"tree","kind":"tree","color":"#112233",
                            "canopy_color":"#445566"}}]{overlays}{params}}}"##,
            n * 2,
            n * 2,
        ),
    )
    .unwrap();
    // One burnout, between the two snapshots, in the last patch.
    std::fs::write(
        dir.join("events.csv"),
        format!(
            "tick,kind,species,patch_x,patch_y,x,y,cause,detail\n\
             50,burnout,,{},{},0,0,,0\n\
             50,ignition,,0,0,0,0,,0\n",
            px - 1,
            py - 1
        ),
    )
    .unwrap();
    for (i, tick) in [0u64, 100].into_iter().enumerate() {
        let sd = dir.join(format!("snap_{tick:06}"));
        std::fs::create_dir_all(&sd).unwrap();
        std::fs::write(
            sd.join("moisture.bin"),
            (0..cols).map(|c| (c * 4) as u8).collect::<Vec<u8>>(),
        )
        .unwrap();
        std::fs::write(
            sd.join("fertility.bin"),
            (0..cols).map(|c| 255 - (c * 4) as u8).collect::<Vec<u8>>(),
        )
        .unwrap();
        std::fs::write(sd.join("height.bin"), vec![2u8; cols]).unwrap();
        // Shot S5: `water.bin`, ponded depth on the **ground** grid in tenths of a millimetre. The
        // first snapshot is dry, as tick 0 of every run is, and the second holds four cells: one
        // film far too thin to draw, one puddle, one pond a metre deep and one five metres deep.
        // 0.1 mm is deliberately the smallest a u16 can carry, so a reader that rounds it away or
        // treats it as dry is caught.
        let gcells = (n * 2) * (n * 2);
        let mut water = vec![0u16; gcells];
        if i == 1 {
            water[1] = 3; // 0.3 mm: wet ground, not standing water
            water[2] = 500; // 50 mm: a puddle, one voxel on a 0.5 m lattice
            water[3] = 10_000; // 1000 mm
            water[4] = 50_000; // 5000 mm, the corner that never drains
        }
        let raw: Vec<u8> = water.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(sd.join("water.bin"), raw).unwrap();
        // Light is sampled one voxel above the surface, so the value that matters is at z = 3.
        let mut light = vec![0u8; cols * z];
        for c in 0..cols {
            light[c + cols * 3] = if c == 0 { 0 } else { 200 };
            light[c + cols * 2] = 99; // the surface voxel itself, which must never be the sample
        }
        std::fs::write(sd.join("light.bin"), light).unwrap();
        // The second snapshot has two patches alight, with different times left.
        let rows: Vec<String> = (0..patches)
            .map(|p| {
                let burning = if i == 1 && p == 1 {
                    3
                } else if i == 1 && p == 2 {
                    1
                } else {
                    0
                };
                format!(
                    r#"{{"grass":0.5,"shrub":0.1,"detritus":10.0,"temperature":{}.0,
                        "burning_ticks_left":{burning}}}"#,
                    10 + p * 5
                )
            })
            .collect();
        std::fs::write(sd.join("patches.json"), format!("[{}]", rows.join(","))).unwrap();
        // Three grazers in the first patch and one in the last, plus a hunter, which crowding counts
        // no more than it counts the tree.
        //
        // The animals' positions are **floats**, because that is what the simulator writes for
        // anything that moves: `"x":82.0`, and `"x":0.75` between two columns. A reader that takes
        // them as integers fails on the whole file, trees included.
        let last = n as f32 - 0.25;
        std::fs::write(
            sd.join("entities.json"),
            format!(
                r#"[{{"id":0,"kind":"tree","x":1,"y":1,"z":3,"age":9,"stage":"mature","lifespan":99}},
                    {{"id":1,"kind":"grazer","x":0.0,"y":0.0,"z":3,"energy":91.5,"age":1}},
                    {{"id":2,"kind":"grazer","x":1.75,"y":0.25,"z":3,"energy":91.5,"age":1}},
                    {{"id":3,"kind":"grazer","x":0.5,"y":1.5,"z":3,"energy":91.5,"age":1}},
                    {{"id":4,"kind":"grazer","x":{last},"y":{last},"z":3,"energy":91.5,"age":1}},
                    {{"id":5,"kind":"hunter","x":2.5,"y":2.5,"z":3,"energy":91.5,"age":1}}]"#
            ),
        )
        .unwrap();
    }
}

/// Every number in an overlay's scale comes out of the run's own `meta.json`, and the two colours the
/// simulator owns by name -- the trunk and the canopy -- come out of its species table.
///
/// This is the claim the row was queued on. `#445566` is nowhere in this viewer; `palette` can only
/// produce it by reading the file.
#[test]
fn the_scale_and_the_species_colours_come_from_meta_json() {
    let dir = tmp("scales");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let m = &run.meta;

    // The union of the three species' temperature curves: shrub's -5 and grass's 35.
    let t = Scale::of(Overlay::Temperature, m);
    assert_eq!((t.lo, t.hi), (-5.0, 35.0));
    // Twice the crowding disease threshold, and the fire duration, both as written in params.
    assert_eq!(Scale::of(Overlay::Crowding, m).hi, 32.0);
    assert_eq!(Scale::of(Overlay::Fire, m).hi, 3.0);
    // Light and moisture are fractions because the file format says so, not by a choice made here.
    assert_eq!(Scale::of(Overlay::Light, m).hi, 1.0);
    assert_eq!(Scale::of(Overlay::Moisture, m).hi, 1.0);
    assert_eq!(Scale::of(Overlay::Fertility, m).hi, 255.0);
    // Every ecology overlay's scale is the run's. **Water is the exception, and it is the
    // simulator's to close, not this viewer's**: `meta.json` carries no scale for ponded depth --
    // there is no parameter in the run that says how deep a deep puddle is -- so shot S5 draws it
    // on a ramp of its own and says on screen that it did. That is the fallback machinery working,
    // not a hole in it, and the test asserts the fallback rather than skipping the overlay.
    for o in Overlay::ALL
        .into_iter()
        .filter(|o| o.is_field() && *o != Overlay::Water)
    {
        let s = Scale::of(o, m);
        assert!(s.from_meta(), "{}: {}", o.name(), s.source);
    }
    let w = Scale::of(Overlay::Water, m);
    assert!(!w.from_meta(), "{}", w.source);
    assert!(w.source.contains("ponded depth"), "{}", w.source);
    assert_eq!((w.lo, w.hi), ecoview_native::overlay::WATER_RAMP_MM);

    let p = palette(Overlay::Surface, Some(m));
    assert_eq!(p[TRUNK as usize], linear_rgba("#112233"));
    assert_eq!(p[CANOPY as usize], linear_rgba("#445566"));
    // With no run the viewer's own table stands, which is what the mesh goldens above hash.
    let bare = palette(Overlay::Surface, None);
    assert_eq!(bare[..ID_COUNT], surface_palette()[..]);
    assert_ne!(
        bare[CANOPY as usize], p[CANOPY as usize],
        "the hard-coded canopy and meta.json's disagree -- which is why this is read from the file"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The ramp hues come out of `meta.json` too (`ecosim` shot S2), which is the half V2 could not do.
///
/// `#010203` and `#fdfeff` are nowhere in this viewer: the light ramp can only come out those colours
/// by reading the file. The fire row's `burnt` is read the same way, and it is a *band* rather than a
/// point on the ramp, so it is checked in the palette where it lands.
#[test]
fn the_overlay_ramp_hues_come_from_meta_json() {
    let dir = tmp("ramps");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let m = &run.meta;
    assert_eq!(
        m.overlays.len(),
        7,
        "every overlay the simulator publishes, traits included"
    );

    let r = Overlay::Light.ramp(Some(m));
    assert_eq!((r.lo.as_str(), r.hi.as_str()), ("#010203", "#fdfeff"));
    assert!(r.from_meta(), "{}", r.source);
    for o in Overlay::ALL
        .into_iter()
        .filter(|o| o.is_field() && *o != Overlay::Water)
    {
        assert!(
            o.ramp(Some(m)).from_meta(),
            "{}: {}",
            o.name(),
            o.ramp(Some(m)).source
        );
    }
    // And the same exception, for the same reason: `ecosim` shot S2 published seven rows and none of
    // them is water, because at the time nothing drew water. Shot S5 draws it in hues of its own and
    // names them as its own; publishing an eighth row is a row for the simulator.
    let w = Overlay::Water.ramp(Some(m));
    assert!(!w.from_meta(), "{}", w.source);
    assert!(w.source.contains("overlays.water"), "{}", w.source);
    // The bottom and the top band of the drawn palette are those two colours, linearised.
    let p = palette(Overlay::Light, Some(m));
    assert_eq!(p[ID_COUNT], linear_rgba("#010203"));
    assert_eq!(p[ID_COUNT + BANDS - 1], linear_rgba("#fdfeff"));
    // Fire's burnt band is the run's colour, and the quiet band is still this viewer's: "nothing to
    // show here" is not an ecological quantity (palette.rs).
    let f = palette(Overlay::Fire, Some(m));
    assert_eq!(f[ID_COUNT + FIRE_BURNT as usize], linear_rgba("#0a0b0c"));
    assert_ne!(f[ID_COUNT + FIRE_QUIET as usize], linear_rgba("#0a0b0c"));
    // `Surface` is the bundle's media and has no ramp to own, which it says rather than claiming a
    // fallback it does not use.
    let surf = Overlay::Surface.ramp(Some(m));
    assert!(
        surf.from_meta() && surf.source.contains("media"),
        "{}",
        surf.source
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A run written before shot S2 has no `overlays`, and then the hues are this viewer's copy of the
/// `ecoview` legend -- named as a fallback, the same way a missing scale is.
#[test]
fn a_run_without_overlays_says_the_hues_are_the_viewers() {
    let dir = tmp("noramps");
    write_overlay_run(&dir, 8, 4, false);
    let m = &Run::load(&dir).unwrap().meta;
    assert!(m.overlays.is_empty());
    let r = Overlay::Light.ramp(Some(m));
    assert_eq!(
        (r.lo.as_str(), r.hi.as_str()),
        ("#000000", "#ffffff"),
        "the ecoview legend"
    );
    assert!(!r.from_meta(), "{}", r.source);
    assert!(r.source.contains("overlays.light"), "{}", r.source);
    // The picture is still drawn, with the legend's colours: a missing palette is not a blank screen.
    let p = palette(Overlay::Light, Some(m));
    assert_eq!(p[ID_COUNT + BANDS - 1], linear_rgba("#ffffff"));
    assert_eq!(
        p[ID_COUNT + BANDS - 1],
        palette(Overlay::Light, None)[ID_COUNT + BANDS - 1]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A run whose `meta.json` carries no `params` still draws, and says on the record that the scale is
/// this viewer's guess rather than the simulator's. Substituting a constant silently is the failure
/// this test exists to prevent.
#[test]
fn a_run_without_params_says_whose_numbers_those_are() {
    let dir = tmp("noparams");
    write_overlay_run(&dir, 8, 4, false);
    let m = &Run::load(&dir).unwrap().meta;
    let t = Scale::of(Overlay::Temperature, m);
    assert_eq!((t.lo, t.hi), (0.0, 30.0));
    assert!(!t.from_meta(), "{}", t.source);
    assert!(!Scale::of(Overlay::Crowding, m).from_meta());
    assert!(!Scale::of(Overlay::Fire, m).from_meta());
    // The three that come out of the file format are unaffected: they were never in params.
    assert!(Scale::of(Overlay::Moisture, m).from_meta());
    assert!(Scale::of(Overlay::Light, m).from_meta());
    assert!(Scale::of(Overlay::Fertility, m).from_meta());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// What each overlay reads, at the column or the patch it reads it from.
#[test]
fn every_overlay_reads_its_own_field() {
    let dir = tmp("fields");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let d = run.meta.dims;
    let f = run.fields_at(1).unwrap();

    assert_eq!(f.moisture[5], 20, "moisture[c] = 4c");
    assert_eq!(f.fertility[5], 235);
    // The sample is the voxel *above* the surface. 99 sits in the surface voxel itself and must not
    // appear anywhere: a reader off by one level would return it for every column.
    assert_eq!(f.light[0], 0, "column 0 is shaded");
    assert_eq!(f.light[5], 200);
    assert!(
        !f.light.contains(&99),
        "the surface voxel is not the sample"
    );

    assert_eq!(d.patch_count(), 4);
    assert_eq!(f.temperature, vec![10.0, 15.0, 20.0, 25.0]);
    assert_eq!(
        f.grazers,
        vec![3, 0, 0, 1],
        "counted per patch, and hunters are not grazers"
    );
    assert_eq!(f.burning, vec![0, 3, 1, 0]);
    assert_eq!(
        f.burnt,
        vec![false, false, false, true],
        "one burnout at 50"
    );

    // The first snapshot has no previous tick, so nothing has burnt since one.
    assert_eq!(run.fields_at(0).unwrap().burnt, vec![false; 4]);

    assert_eq!(f.value(Overlay::Moisture, &d, 5, 0), 20.0 / 255.0);
    assert_eq!(f.value(Overlay::Fertility, &d, 5, 0), 235.0);
    assert_eq!(f.value(Overlay::Light, &d, 5, 0), 200.0 / 255.0);
    assert_eq!(f.value(Overlay::Temperature, &d, 5, 0), 15.0);
    assert_eq!(f.value(Overlay::Crowding, &d, 0, 0), 3.0);
    assert_eq!(f.fire_counts(), (2, 1));

    // The same file read for trees. Animals are written at continuous positions, so this is also the
    // regression test for reading `"x":1.75` as a whole column: V1 took both as `i32` and a snapshot
    // with one grazer in it failed entirely, trees and all.
    let t = run.trees_at(1).unwrap();
    assert_eq!(t.trees.len(), 1);
    assert_eq!(t.other_kinds, 5, "four grazers and a hunter");
    assert_eq!((t.trees[0].x, t.trees[0].y), (1.5, 1.5));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A field file of the wrong length is refused by name rather than drawn as far as it goes. Half a
/// moisture map over a whole site is a picture with nothing on it to say which half is real.
#[test]
fn a_short_field_file_is_refused() {
    let dir = tmp("short");
    write_overlay_run(&dir, 8, 4, true);
    std::fs::write(dir.join("snap_000100/moisture.bin"), vec![0u8; 3]).unwrap();
    let e = Run::load(&dir)
        .unwrap()
        .fields_at(1)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("moisture.bin") && e.contains("expected 64"),
        "{e}"
    );

    std::fs::write(dir.join("snap_000100/moisture.bin"), vec![0u8; 64]).unwrap();
    std::fs::write(dir.join("snap_000100/patches.json"), "[]").unwrap();
    let e = Run::load(&dir)
        .unwrap()
        .fields_at(1)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("patches.json") && e.contains("expected 4"),
        "{e}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The ramp is clamped at both ends, and the top of the scale is the last band rather than one past
/// it.
#[test]
fn bands_span_the_scale_and_clamp() {
    let s = Scale {
        lo: 0.0,
        hi: 10.0,
        unit: "",
        source: String::new(),
    };
    assert_eq!(band_of(-5.0, &s), 0);
    assert_eq!(band_of(0.0, &s), 0);
    assert_eq!(band_of(10.0, &s), (BANDS - 1) as u8);
    assert_eq!(band_of(1e9, &s), (BANDS - 1) as u8);
    let mut last = 0u8;
    for i in 0..=100 {
        let b = band_of(i as f32 / 10.0, &s);
        assert!(b >= last, "the ramp has to be monotonic: {b} after {last}");
        last = b;
    }
    // A scale whose ends meet does not divide by zero; everything lands in band 0.
    let meeting = Scale {
        lo: 1.0,
        hi: 1.0,
        unit: "",
        source: String::new(),
    };
    assert_eq!(band_of(1.0, &meeting), 0);
}

/// Fire's bands are three things rather than one ramp: quiet, burnt since the last snapshot, and
/// alight.
#[test]
fn fire_has_a_quiet_band_a_burnt_band_and_a_ramp() {
    let dir = tmp("fire");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let d = run.meta.dims;
    let s = Scale::of(Overlay::Fire, &run.meta);
    let (bands, _) = run.fields_at(1).unwrap().bands(Overlay::Fire, &d, &s);
    // The south-west column of patch p, which is (p % 2, p / 2) patches across an 8-column grid.
    let at = |p: usize| bands[(p % 2) * 4 + d.x * (p / 2) * 4];
    assert_eq!(at(0), FIRE_QUIET, "neither alight nor freshly burnt");
    assert_eq!(at(1), (BANDS - 1) as u8, "3 of 3 ticks left is the top");
    assert_eq!(
        at(2),
        FIRE_BURNT + 1,
        "1 tick left is the bottom of the ramp"
    );
    assert_eq!(at(3), FIRE_BURNT, "burnt out since the previous snapshot");
    // And the palette gives the two categorical bands colours off the orange ramp.
    let p = palette(Overlay::Fire, Some(&run.meta));
    assert_ne!(p[ID_COUNT], p[ID_COUNT + 1]);
    assert_ne!(p[ID_COUNT + 1], p[ID_COUNT + 2]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An overlay covers the ground cells its ecology column covers, and no others. The simulator works
/// at 1 m and the Capitol bundle at 0.5, so one column is 2 x 2 cells: resampling the other way round
/// would claim four times the resolution the simulator has.
#[test]
fn one_ecology_column_covers_its_ground_cells() {
    // 16 cells at 0.5 m is an 8 m site, so 8 x 8 ecology columns.
    let mut w = VoxelWorld::from_bundle(&flat(16, 0.5, 4.0));
    let mut bands = vec![0u8; 64];
    bands[1] = 7; // the column from x = 1 m to x = 2 m
    let stale = w.set_overlay(Some(&ColumnBands {
        x: 8,
        y: 8,
        cell_m: ECO_CELL_M,
        bands,
    }));
    assert!(!stale.is_empty());
    assert!(w.has_overlay());
    let g = w.ground_level(0, 0);
    let band = |x: i32| w.voxel(x, 0, g) - BAND_BASE;
    assert_eq!(
        (band(0), band(1), band(2), band(3), band(4)),
        (0, 0, 7, 7, 0)
    );
    // Under the surface it is still soil: an overlay colours the top of a column and nothing else, so
    // digging under one shows the ground rather than the map.
    assert_eq!(w.voxel(2, 0, g - 1), SOIL);
    // And taking it off puts the media back.
    w.set_overlay(None);
    assert!(!w.has_overlay());
    assert_eq!(w.voxel(2, 0, g), 2, "lawn is medium 1, id 2");
}

/// Scrubbing under an overlay remeshes what the field changed, not the site, and taking the overlay
/// off restores the surface mesh to the bit.
///
/// Switching the overlay *on* is a different matter and is not asserted small here: it changes the
/// top voxel of every column, so every chunk really is stale.
#[test]
fn set_overlay_reports_exactly_the_chunks_whose_mesh_changed() {
    // 128 cells at 0.5 m is a 64 m site: 64 x 64 ecology columns.
    let mut w = VoxelWorld::from_bundle(&flat(128, 0.5, 4.0));
    let pal = palette(Overlay::Moisture, None);
    let mut scratch = Scratch::new();
    let all = w.all_chunks();
    let hashes = |w: &VoxelWorld, s: &mut Scratch| -> Vec<u64> {
        all.iter()
            .map(|&c| mesh_chunk(w, c, &pal, s).hash())
            .collect()
    };
    let surface = hashes(&w, &mut scratch);

    let flat_field = ColumnBands {
        x: 64,
        y: 64,
        cell_m: ECO_CELL_M,
        bands: vec![0u8; 64 * 64],
    };
    assert_eq!(
        w.set_overlay(Some(&flat_field)).len(),
        all.len(),
        "switching an overlay on changes every column"
    );
    let before = hashes(&w, &mut scratch);
    assert_ne!(surface, before);
    // The same field again is not a change, and reports nothing.
    assert!(w.set_overlay(Some(&flat_field)).is_empty());

    let mut moved = flat_field.clone();
    moved.bands[10 + 64 * 10] = 12;
    let stale = w.set_overlay(Some(&moved));
    assert!(
        !stale.is_empty(),
        "a band that moved has to make a chunk stale"
    );
    assert!(
        stale.len() < all.len(),
        "and not the whole site: {} of {}",
        stale.len(),
        all.len()
    );
    let after = hashes(&w, &mut scratch);
    for (i, c) in all.iter().enumerate() {
        if before[i] != after[i] {
            assert!(
                stale.contains(c),
                "chunk {c:?} changed but was not reported stale"
            );
        }
    }
    assert_ne!(before, after, "the band has to show up somewhere");

    assert!(!w.set_overlay(None).is_empty());
    assert_eq!(
        surface,
        hashes(&w, &mut scratch),
        "taking the overlay off has to undo it exactly"
    );
}

/// The golden sibling of the three at the top of this file, under an overlay: a banded chunk hashes
/// to a fixed value, so a change in the band ids, the resampling or the ramp shows up here rather
/// than in a screenshot nobody diffs.
#[test]
fn golden_banded() {
    let mut w = VoxelWorld::from_bundle(&flat(16, 0.5, 4.0));
    // Four-metre stripes of two bands: 13 quads rather than the flat site's 10, because the two
    // band ids do not merge across the stripe.
    let bands: Vec<u8> = (0..64).map(|c| if c % 8 < 4 { 5 } else { 9 }).collect();
    w.set_overlay(Some(&ColumnBands {
        x: 8,
        y: 8,
        cell_m: ECO_CELL_M,
        bands,
    }));
    let m = mesh_chunk(
        &w,
        ChunkPos { x: 0, y: 0, z: 0 },
        &palette(Overlay::Moisture, None),
        &mut Scratch::new(),
    );
    assert_eq!(
        (m.hash(), m.positions.len(), m.indices.len()),
        (0x2461_2572_2ee5_86ae, 52, 78)
    );
}

// ---- shot V3: procedural trees ----
//
// In this file for the reason the V1 and V2 tests are: the CI gate runs exactly one test target.

/// A bundle with room over it for a run's trees, since the chunk grid is sized once at load.
fn tall(n: usize, cell_m: f32, h: f32) -> VoxelWorld {
    VoxelWorld::from_bundle_with_headroom(&flat(n, cell_m, h), 25.0)
}

/// The run that the V3 tree model reads: a real `year_len`, the three tree ages, and the two
/// height breakpoints, so `Life` is the simulator's curve and not this viewer's fallback.
///
/// `light.bin` is deliberately **inverted** between the surface and the sky: the western half of the
/// site is lit at the surface and dark above 10 m, the eastern half the other way round. A reader
/// that sampled the sky over a crown instead of the simulator's own surface voxel would get exactly
/// the opposite answer on both halves, which is what `crown_light_is_the_simulator_own_sample` uses.
fn write_tree_run(dir: &std::path::Path, trees: &[(i32, i32, u32)]) {
    let (n, z) = (16usize, 32usize);
    let cols = n * n;
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("meta.json"),
        format!(
            r#"{{"format_version":4,"dims":{{"x":{n},"y":{n},"z":{z},"patch":8}},
               "seed":42,"ticks":100,"snapshot_every":100,"snapshots":[0],"year_len":4000,
               "world":{{"name":"test","bundle":true,"ground_cell_m":0.5,
                         "ground_width":{},"ground_depth":{}}},
               "params":{{"tree":{{"young_age_years":0.125,"mature_age_years":0.25,
                                   "max_age_years":1.5}},
                          "bundle":{{"tree_mature_height":3.0,"tree_tall_height":20.0,
                                     "tree_tall_age_years":0.75}}}}}}"#,
            n * 2,
            n * 2,
        ),
    )
    .unwrap();
    let sd = dir.join("snap_000000");
    std::fs::create_dir_all(&sd).unwrap();
    std::fs::write(sd.join("height.bin"), vec![2u8; cols]).unwrap();
    let mut light = vec![0u8; cols * z];
    for c in 0..cols {
        let west = c % n < n / 2;
        // The surface is at level 2, so the first voxel of air over it is 3.
        light[c + cols * 3] = if west { 255 } else { 0 };
        for zi in 10..z {
            light[c + cols * zi] = if west { 0 } else { 255 };
        }
    }
    std::fs::write(sd.join("light.bin"), light).unwrap();
    let ents: Vec<String> = trees
        .iter()
        .enumerate()
        .map(|(i, (x, y, age))| {
            format!(
                r#"{{"id":{i},"kind":"tree","x":{x},"y":{y},"z":3,"age":{age},
                     "stage":"mature","lifespan":6000}}"#
            )
        })
        .collect();
    std::fs::write(sd.join("entities.json"), format!("[{}]", ents.join(","))).unwrap();
}

/// The golden: one procedural tree over flat ground, meshed. A change to the branching model, the
/// rasteriser or the lattice moves this hash, and it is taken on Windows and asserted on Linux CI,
/// which is what the model's "no transcendental function" rule is for (`tree.rs`).
#[test]
fn golden_procedural_tree() {
    let mut w = tall(32, 0.5, 4.0);
    w.set_plants(&[TreeForm::grown(8.0, 8.0, 12.0, 0.8, 0x5eed)], &[]);
    let m = mesh_chunk(
        &w,
        ChunkPos { x: 0, y: 0, z: 0 },
        &surface_palette(),
        &mut Scratch::new(),
    );
    assert_eq!(
        (m.hash(), m.positions.len(), m.indices.len()),
        (GOLDEN_TREE, GOLDEN_TREE_VERTS, GOLDEN_TREE_INDICES)
    );
}

const GOLDEN_TREE: u64 = 0x193f_685c_694d_8b75;
const GOLDEN_TREE_VERTS: usize = 1784;
const GOLDEN_TREE_INDICES: usize = 2676;

/// Two trees with the same seed are the same wood, voxel for voxel; two with different seeds are
/// different wood inside the same silhouette. Without the first half, scrubbing the timeline would
/// reshuffle every tree on the site; without the second, a wood would be one tree stamped out.
#[test]
fn the_same_seed_grows_the_same_tree() {
    let one = |seed: u64| {
        let mut w = tall(32, 0.5, 4.0);
        w.set_plants(&[TreeForm::grown(8.0, 8.0, 12.0, 1.0, seed)], &[]);
        let mut v = w.plant_voxels();
        v.sort_unstable();
        v
    };
    let a = one(7);
    let b = one(7);
    let c = one(8);
    assert_eq!(a, b, "the same seed has to give the same tree");
    assert_ne!(a, c, "and a different seed a different one");

    // The seed moves the wood inside the crown; it cannot change how big the tree reads, because
    // the envelope is the allometry's. So both trees fit the same bound: a 12 m tree on 4 m of
    // ground at 0.5 m cells stands on level 9 and is 24 levels tall, and its crown radius is
    // 0.30 x 12 m = 7.2 cells, plus the half cell a leaf cluster is rounded out to.
    let bound = |v: &Vec<(usize, usize, usize, u16)>| {
        let hi = v.iter().map(|t| t.2).max().unwrap() as i64;
        let far = v
            .iter()
            .map(|t| (t.0 as i64 - 16).abs().max((t.1 as i64 - 16).abs()))
            .max()
            .unwrap();
        (hi, far)
    };
    for v in [&a, &c] {
        let (hi, far) = bound(v);
        assert!(hi <= 9 + 24, "a 12 m tree reached level {hi}");
        assert!(far <= 8, "its crown reached {far} cells from the trunk");
    }
}

/// Size comes from `age` through the run's own curve, and the curve is the one the simulator plants
/// scene trees with (`ecosim/src/plants.rs`, `import_age`) read backwards. The round trip below is
/// that claim: a height turned into an age by the simulator's formula comes back as the same height.
#[test]
fn age_is_where_a_tree_size_comes_from() {
    let dir = tmp("tree-age");
    write_tree_run(&dir, &[(4, 4, 3000)]);
    let run = Run::load(&dir).unwrap();
    let life = &run.life;
    assert!(
        life.from_meta,
        "this run states every number: {}",
        life.source
    );
    assert_eq!((life.year_ticks, life.tall_height_m), (4000.0, 20.0));

    // Monotone, through the two breakpoints, flat above the last.
    assert_eq!(life.height_of(0), 0.0);
    assert!(
        (life.height_of(1000) - 3.0).abs() < 1e-4,
        "mature at 0.25 y"
    );
    assert!((life.height_of(3000) - 20.0).abs() < 1e-4, "tall at 0.75 y");
    assert_eq!(
        life.height_of(9999),
        life.height_of(3000),
        "flat above tall"
    );
    let mut last = -1.0;
    for age in (0..6000).step_by(37) {
        let h = life.height_of(age);
        assert!(h >= last, "height fell at age {age}: {h} after {last}");
        last = h;
    }

    // `import_age`, transcribed from its own doc comment, in ticks.
    let import_age = |h: f32| -> u32 {
        let (hm, ht) = (3.0f32, 20.0f32);
        let (mature, tall) = (0.25 * 4000.0, 0.75 * 4000.0);
        let age = if h <= 0.0 {
            0.0
        } else if h < hm {
            mature * h / hm
        } else if h < ht {
            mature + (tall - mature) * (h - hm) / (ht - hm)
        } else {
            tall
        };
        age.round() as u32
    };
    for h in [0.4f32, 1.5, 2.9, 3.0, 7.25, 13.766, 19.9, 20.0] {
        let back = life.height_of(import_age(h));
        assert!(
            (back - h).abs() < 0.02,
            "{h} m became age {} and came back {back} m",
            import_age(h)
        );
    }

    // And the tree the snapshot actually built is that height.
    let snap = run.trees_at(0).unwrap();
    assert_eq!(snap.trees.len(), 1);
    assert!((snap.trees[0].height - 20.0).abs() < 1e-4);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A run that states none of the curve gets `ecosim`'s defaults and **says so**, naming each number
/// it had to stand in for. The reference Capitol run is this case: `params.bundle` is left out of
/// `meta.json` whenever it is at its defaults, so the heights are always the fallback there.
#[test]
fn a_life_with_no_curve_names_its_fallback() {
    let life = Life::default();
    assert!(!life.from_meta);
    for want in ["this viewer's fallback", "year_len", "params.bundle"] {
        assert!(life.source.contains(want), "{}", life.source);
    }
    // The fallback is still `ecosim/params.toml`'s own numbers, not invented ones.
    assert_eq!(
        (life.year_ticks, life.mature_height_m, life.tall_height_m),
        (4000.0, 3.0, 20.0)
    );
    assert_eq!(life.stage_years(), (0.125, 0.25, 1.5));
}

/// A tree's light is the number the **simulator** computed for it: `light.bin` at the first voxel
/// above its own column, which is `ecosim`'s `surface_light` and what its growth curves read.
///
/// Both trees below are 20 m, so both crowns are up in the sky layer; the run's light is inverted
/// between surface and sky, so a viewer that sampled the sky over the crown -- which is what this
/// shot tried first, and measured as 1.00 for every tree on the reference run -- would swap these
/// two answers.
#[test]
fn crown_light_is_the_simulator_own_sample() {
    let dir = tmp("tree-light");
    write_tree_run(&dir, &[(4, 4, 3000), (12, 12, 3000)]);
    let snap = Run::load(&dir).unwrap().trees_at(0).unwrap();
    assert_eq!(snap.trees.len(), 2);
    assert!(
        snap.light_source.contains("light.bin"),
        "{}",
        snap.light_source
    );
    let west = snap.trees.iter().find(|t| t.x < 8.0).unwrap();
    let east = snap.trees.iter().find(|t| t.x > 8.0).unwrap();
    assert!((west.height - 20.0).abs() < 1e-4 && (east.height - 20.0).abs() < 1e-4);
    assert_eq!(west.light, 1.0, "its own column is lit at the surface");
    assert_eq!(east.light, 0.0, "and this one is shaded at the surface");
    assert!(
        west.blob_radius() > east.blob_radius(),
        "light fills a crown"
    );

    // With the light file gone the crowns are drawn in full sun and the fallback is named.
    std::fs::remove_file(dir.join("snap_000000/light.bin")).unwrap();
    let snap = Run::load(&dir).unwrap().trees_at(0).unwrap();
    assert_eq!(snap.light, (1.0, 1.0, 1.0));
    assert!(
        snap.light_source.contains("full sun"),
        "{}",
        snap.light_source
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Light fills the crown and leaves the wood alone. A shaded tree has the same skeleton as a sunlit
/// one -- same seed, same branches -- and fewer leaves on it.
#[test]
fn light_fills_the_crown_and_leaves_the_wood_alone() {
    let shade = TreeForm::grown(8.0, 8.0, 12.0, 0.0, 99);
    let sun = TreeForm::grown(8.0, 8.0, 12.0, 1.0, 99);
    assert_eq!(shade.skeleton(0.5), sun.skeleton(0.5), "same wood");
    assert!(shade.blob_radius() < sun.blob_radius());

    let counts = |t: TreeForm| {
        let mut w = tall(32, 0.5, 4.0);
        w.set_plants(&[t], &[]);
        w.plant_counts()
    };
    let (wood_a, leaf_a) = counts(shade);
    let (wood_b, leaf_b) = counts(sun);
    assert_eq!(wood_a, wood_b, "the light did not move a branch");
    assert!(
        leaf_b > leaf_a,
        "a sunlit crown is fuller: {leaf_b} leaves against {leaf_a}"
    );
}

/// A sapling is a stick and a big tree has branches. The model shows more of itself on a bigger
/// tree, which is how the same code draws both without a stage table.
#[test]
fn a_sapling_is_a_stick_and_a_big_tree_has_branches() {
    let sapling = TreeForm::grown(4.0, 4.0, 0.6, 1.0, 3);
    assert_eq!(sapling.levels(0.5), 0);
    assert_eq!(sapling.skeleton(0.5).len(), 1, "a stick is one segment");

    let big = TreeForm::grown(4.0, 4.0, 18.0, 1.0, 3);
    assert_eq!(big.levels(0.5), 3);
    assert!(
        big.skeleton(0.5).len() > 10,
        "{} limbs",
        big.skeleton(0.5).len()
    );
    // Wood thins outwards: the trunk is the thickest limb and it is the first one.
    let limbs = big.skeleton(0.5);
    assert!(limbs[0].r >= limbs.iter().map(|l| l.r).fold(0.0, f32::max));
    assert!(limbs.iter().all(|l| l.b[1] <= big.height + 1e-4));
}

/// No leaf voxel leaves the crown envelope, and the envelope is the allometry's. This is what keeps
/// a procedural crown honest: however the branches scatter, the silhouette is still the size the
/// simulator's curve says the tree is.
#[test]
fn every_leaf_voxel_is_inside_the_crown_envelope() {
    let cell_m = 0.5;
    let t = TreeForm::grown(8.0, 8.0, 14.0, 1.0, 0xbeef);
    let mut w = tall(32, cell_m, 4.0);
    w.set_plants(&[t], &[]);
    // `flat` is 4 m of ground on a 0.5 m lattice, so the first air level is 9.
    let base = w.ground_level(16, 16) + 1;
    let leaves = w.plant_voxels();
    let mut n = 0;
    for (x, y, z, id) in leaves {
        if id != ecoview_native::voxel::CANOPY {
            continue;
        }
        n += 1;
        let p = [
            (x as i64 - 16) as f32 * cell_m,
            (z as i64 - base as i64) as f32 * cell_m,
            (y as i64 - 16) as f32 * cell_m,
        ];
        assert!(t.in_envelope(p), "leaf at {p:?} is outside the crown");
    }
    assert!(n > 100, "only {n} leaf voxels");
    // And the crown starts where the allometry says, not at the ground.
    assert!((t.crown_base - 14.0 * CROWN_BASE_FRACTION).abs() < 1e-4);
}

/// A run tree taller than the bundle's own tallest survey is not quietly beheaded. The chunk grid is
/// sized once at load, so the viewer passes the run's ceiling in; without it the top of every mature
/// tree on a site of young ones would simply not exist.
#[test]
fn a_run_tree_gets_room_above_the_bundle() {
    let t = TreeForm::grown(8.0, 8.0, 18.0, 1.0, 5);
    let top = |mut w: VoxelWorld| {
        w.set_plants(std::slice::from_ref(&t), &[]);
        let base = w.ground_level(16, 16) + 1;
        let hi = w.plant_voxels().iter().map(|v| v.2).max().unwrap();
        (hi as i32 - base) as f32 * 0.5
    };
    // No headroom: `flat` has no trees, so the grid stops two levels over the ground.
    let clipped = top(VoxelWorld::from_bundle(&flat(32, 0.5, 4.0)));
    let whole = top(tall(32, 0.5, 4.0));
    assert!(clipped < 2.0, "clipped to {clipped} m");
    assert!(whole > 17.0, "kept to {whole} m");
}

// ---------------------------------------------------------------------------------------------
// shot V4: ground cover and vines
//
// Every test below is about **expression**, not ecology. The run owns four numbers -- the grass and
// shrub fraction of a patch, and the moisture and light of a column -- and the viewer owns only
// where a blade stands and how far a climber gets. Nothing here competes, accumulates or feeds back,
// and no vine is an entity in any run. What these tests can therefore check is that the viewer's
// picture *moves with* the run's numbers and is reproducible from them, which is the whole of the
// claim the row makes.
// ---------------------------------------------------------------------------------------------

use ecoview_native::cover::{Cover, VINE_JITTER, VINE_REACH_M};
use ecoview_native::overlay::Fields;
use ecoview_native::run::Dims;
use ecoview_native::voxel::{level_of, GRASS, SHRUB, VINE};

/// The ecology grid over a `flat(n, 0.5, _)` world: 1 m columns, 8-column patches (CLAUDE.md).
fn eco(n: usize) -> Dims {
    Dims {
        x: n / 2,
        y: n / 2,
        z: 8,
        patch: 8,
    }
}

/// A snapshot's fields, uniform over the site, so a test changes one driver at a time.
fn drivers(d: Dims, grass: f32, shrub: f32, moisture: u8, light: u8) -> Fields {
    Fields {
        moisture: vec![moisture; d.columns()],
        fertility: vec![0; d.columns()],
        light: vec![light; d.columns()],
        temperature: vec![0.0; d.patch_count()],
        burning: vec![0; d.patch_count()],
        grazers: vec![0; d.patch_count()],
        burnt: vec![false; d.patch_count()],
        grass: vec![grass; d.patch_count()],
        shrub: vec![shrub; d.patch_count()],
    }
}

/// A flat 32 m site with one square building on it, `h` metres tall.
fn with_building(n: usize, h: f32) -> Bundle {
    let mut b = flat(n, 0.5, 4.0);
    for y in 20..30 {
        for x in 20..30 {
            b.building_h[x + n * y] = h;
            b.medium[x + n * y] = 7; // roof
        }
    }
    b
}

/// The simulator's fractions come out the other side as the fractions of the ground actually drawn.
///
/// This is the one number that would let the viewer lie about the run: if a patch the simulator
/// calls half grass came out fully green, the picture would be saying the site recovered when it
/// did not. The tolerance is the sampling error of 4096 independent draws, not a fudge.
#[test]
fn the_drawn_cover_is_the_fraction_the_run_reported() {
    let n = 64;
    let d = eco(n);
    // With headroom, the way the viewer sizes a world when a run is loaded. Without it the grid
    // stops two levels above the ground and a shrub is clipped to a single voxel -- which is right,
    // but it is not what a run ever draws.
    let mut w = VoxelWorld::from_bundle_with_headroom(&flat(n, 0.5, 4.0), 20.0);
    let c = Cover::of(&drivers(d, 0.5, 0.1, 128, 200), d, 42);
    w.set_scene(&[], &[], Some(&c));
    let (grass, shrub, vine) = w.cover_counts();
    let cells = (n * n) as f32;
    // A blade of grass is one voxel; a shrub is `SHRUB_HEIGHT_M` of them.
    let shrub_voxels = (level_of(1.2, 0.5) + 1) as f32;
    assert!(
        ((grass as f32 / cells) - 0.5).abs() < 0.03,
        "{grass} grass voxels of {cells} cells"
    );
    assert!(
        ((shrub as f32 / shrub_voxels / cells) - 0.1).abs() < 0.02,
        "{shrub} shrub voxels of {cells} cells"
    );
    // No building on this site, so no wall and no climber.
    assert_eq!(vine, 0);
    // And the drivers the viewer reports are the ones it was handed.
    let m = c.means();
    assert!((m.grass - 0.5).abs() < 1e-6 && (m.shrub - 0.1).abs() < 1e-6);
    assert!((m.water - 128.0 / 255.0).abs() < 1e-6);
}

/// Same run, same seed, same blades -- so scrubbing back to a tick is the tick, not a reshuffle.
#[test]
fn the_same_seed_scatters_the_same_cover() {
    let n = 64;
    let d = eco(n);
    let f = drivers(d, 0.4, 0.2, 200, 40);
    let draw = |seed: u64| {
        let mut w = VoxelWorld::from_bundle(&with_building(n, 8.0));
        w.set_scene(&[], &[], Some(&Cover::of(&f, d, seed)));
        w.plant_voxels()
    };
    assert_eq!(draw(42), draw(42));
    assert_ne!(draw(42), draw(43));
}

/// Sealed ground grows nothing, and neither does the wall standing in it.
///
/// This is the only place the viewer decides *whether* a plant is there rather than where. With no
/// run to ask it is the fallback name list that answers, which is what this test drives; the run's
/// own answer is in `the_run_decides_which_media_grow_things` below. It is also what makes an edit
/// legible: pave a lawn in the viewer and its blades and its climbers both go.
#[test]
fn sealed_ground_grows_nothing() {
    let n = 64;
    let d = eco(n);
    let f = drivers(d, 0.9, 0.09, 255, 0);
    let count = |medium: u8| {
        let mut b = with_building(n, 8.0);
        for y in 0..n {
            for x in 0..n {
                if b.building_h[x + n * y] == 0.0 {
                    b.medium[x + n * y] = medium;
                }
            }
        }
        let mut w = VoxelWorld::from_bundle(&b);
        w.set_scene(&[], &[], Some(&Cover::of(&f, d, 7)));
        w.cover_counts()
    };
    let (g, s, v) = count(1); // lawn
    assert!(g > 1000 && s > 100 && v > 100, "lawn grew {g}/{s}/{v}");
    assert_eq!(count(6), (0, 0, 0)); // asphalt
    assert_eq!(count(8), (0, 0, 0)); // open water

    // And it says out loud that this was its own list and not a reading, because no run was open.
    let w = VoxelWorld::from_bundle(&with_building(n, 8.0));
    assert!(!w.plantable.from_meta);
    assert!(
        w.plantable
            .source
            .starts_with("this viewer's fallback name list"),
        "{}",
        w.plantable.source
    );
    assert_eq!(
        w.plantable.sealed(),
        ["concrete", "asphalt", "roof", "water"]
    );
}

/// A vine is drawn on the open side of a wall, from the ground up, and stops at the wall's top.
///
/// It has to be on the *outside*: a plant voxel inside a solid building column would never be
/// drawn, and a climber that overshot the parapet would be a plant standing in mid-air.
#[test]
fn a_vine_climbs_the_outside_of_a_wall_and_stops_at_the_top() {
    let n = 64;
    let d = eco(n);
    // A 2 m wall, low enough that a well-fed vine would clear it if nothing stopped it:
    // `VINE_REACH_M` is 12 m.
    let mut w = VoxelWorld::from_bundle(&with_building(n, 2.0));
    assert!(VINE_REACH_M > 2.0);
    let c = Cover::of(&drivers(d, 0.9, 0.09, 255, 0), d, 11);
    w.set_scene(&[], &[], Some(&c));
    let wall_top = level_of(4.0 + 2.0, 0.5);
    let ground = level_of(4.0, 0.5);
    let vines: Vec<_> = w
        .plant_voxels()
        .into_iter()
        .filter(|v| v.3 == VINE)
        .collect();
    assert!(!vines.is_empty());
    for (x, y, z, _) in &vines {
        // Never inside the building, never below the ground, never above the wall.
        assert!(!(20..30).contains(x) || !(20..30).contains(y), "inside");
        assert!(*z as i32 > ground && *z as i32 <= wall_top, "at level {z}");
        // And always touching the wall: one of the four orthogonal neighbours is the building.
        let touches = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|(dx, dy)| {
                let (nx, ny) = (*x as i64 + dx, *y as i64 + dy);
                (20..30).contains(&nx) && (20..30).contains(&ny)
            });
        assert!(touches, "vine at {x},{y} touches no wall");
    }
    // At this vigour the climb is capped by the wall, so the ragged top is flat against it.
    assert_eq!(
        vines.iter().map(|v| v.2 as i32).max(),
        Some(wall_top),
        "the vine stops short of the parapet"
    );
}

/// Wetter and shadier grows more vine; drier grows less. The viewer's picture moves with the run's
/// numbers, which is the whole of what "driven by the simulator" can mean for something the
/// simulator does not model.
#[test]
fn a_vine_answers_the_run_moisture_and_shade() {
    let n = 64;
    let d = eco(n);
    // A tall wall, so nothing is capped and the count is the vigour.
    let vine_voxels = |grass: f32, moisture: u8, light: u8| {
        let mut w = VoxelWorld::from_bundle(&with_building(n, 30.0));
        let c = Cover::of(&drivers(d, grass, 0.0, moisture, light), d, 5);
        w.set_scene(&[], &[], Some(&c));
        w.cover_counts().2
    };
    let wet = vine_voxels(1.0, 255, 0);
    let dry = vine_voxels(1.0, 20, 0);
    let sunny = vine_voxels(1.0, 255, 255);
    let bare = vine_voxels(0.0, 255, 0);
    assert!(dry < wet, "dry {dry} not under wet {wet}");
    assert!(sunny < wet, "sunny {sunny} not under shaded {wet}");
    // Cover gates the climb: ground the simulator says is bare grows no climber at all.
    assert_eq!(bare, 0);
    // The jitter is a ragged edge, not a second driver: it cannot turn the ordering over.
    assert!(VINE_JITTER < 1.0);
}

/// A field overlay takes the ground cover off and leaves the vines on.
///
/// An overlay is a map of the ground, and a site two thirds under grass would be a map of the
/// grass. No overlay colours a wall, so the climbers stay -- and the moisture and light maps are
/// exactly what explains where they are.
#[test]
fn the_overlay_hides_the_ground_cover_and_keeps_the_vines() {
    let n = 64;
    let d = eco(n);
    let f = drivers(d, 0.8, 0.1, 255, 0);
    let mut w = VoxelWorld::from_bundle(&with_building(n, 20.0));
    let mut c = Cover::of(&f, d, 3);
    w.set_scene(&[], &[], Some(&c));
    let (g0, s0, v0) = w.cover_counts();
    assert!(g0 > 0 && s0 > 0 && v0 > 0);

    c.ground = false;
    w.set_scene(&[], &[], Some(&c));
    assert_eq!(w.cover_counts(), (0, 0, v0));

    // And **V** off is all three gone, with the ground left exactly as it was.
    c.vines = false;
    let stale = w.set_scene(&[], &[], Some(&c));
    assert_eq!(w.cover_counts(), (0, 0, 0));
    assert!(!stale.is_empty(), "taking the cover off remeshes nothing");
}

/// With no cover the world is the one shot V3 left behind, so the row adds a layer rather than
/// changing the site under it.
#[test]
fn no_cover_leaves_the_world_as_v3_drew_it() {
    let t = TreeForm::grown(8.0, 8.0, 12.0, 0.8, 0x5eed);
    let mut a = VoxelWorld::from_bundle(&flat(32, 0.5, 4.0));
    a.set_plants(std::slice::from_ref(&t), &[]);
    let mut b = VoxelWorld::from_bundle(&flat(32, 0.5, 4.0));
    b.set_scene(std::slice::from_ref(&t), &[], None);
    assert_eq!(a.plant_voxels(), b.plant_voxels());
    assert_eq!(b.cover_counts(), (0, 0, 0));
}

/// The cover's two patch fields are read from `patches.json`, off the same pass as the overlays.
#[test]
fn the_cover_reads_its_fractions_from_the_run() {
    let dir = tmp("cover-fields");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let f = run.fields_at(0).unwrap();
    assert_eq!(f.grass.len(), run.meta.dims.patch_count());
    assert!(f.grass.iter().all(|g| (*g - 0.5).abs() < 1e-6));
    assert!(f.shrub.iter().all(|s| (*s - 0.1).abs() < 1e-6));
    // And the seed the scatter hangs on is the run's, not the viewer's.
    assert_eq!(run.meta.seed, 42);
}

/// The golden: one covered chunk in, one hashed mesh out. Grass, shrub and vine are three more
/// voxel ids, so the mesher treats them the way it treats everything else -- and a change to the
/// scatter, the climb or the three colours moves this number.
#[test]
fn golden_cover() {
    let n = 64;
    let d = eco(n);
    let mut w = VoxelWorld::from_bundle(&with_building(n, 6.0));
    w.set_scene(
        &[],
        &[],
        Some(&Cover::of(&drivers(d, 0.5, 0.1, 200, 30), d, 42)),
    );
    let m = mesh_chunk(
        &w,
        ChunkPos { x: 0, y: 0, z: 0 },
        &surface_palette(),
        &mut Scratch::new(),
    );
    assert_eq!(
        (m.hash(), m.positions.len(), m.indices.len()),
        (0x7a35_505d_d651_0006, 23036, 34554),
        "the covered chunk"
    );
    // The three cover ids are all in the palette, and none of them is the canopy's colour: a vine
    // is the viewer's own hue because the run has no climber species to name one (palette.rs).
    let p = surface_palette();
    assert_ne!(p[VINE as usize], p[CANOPY as usize]);
    assert_ne!(p[GRASS as usize], p[SHRUB as usize]);
}

// -------------------------------------------------------------------------------------------
// V5: edit, run, grow, in place.
//
// The round trip is four things, and three of them have no engine in them, so they are gated here:
// write the edited site back out as a world bundle, build the command line, start `ecosim` as a
// **command** and watch it, and read what it wrote. The fourth -- drawing it -- is the viewer's own
// and is measured in MEASUREMENTS.md instead.
//
// Nothing below models any ecology. The edit changes three numbers in a column; every consequence
// of that edit is the simulator's, computed in its own process, and arrives back as files on disk.
// The two projects still share no code and have no IPC (CLAUDE.md).

/// A 16 m site with one raised, repaved column and one column of building.
fn edited_site(n: usize) -> (Bundle, VoxelWorld) {
    let b = flat(n, 0.5, 4.0);
    let mut w = VoxelWorld::from_bundle(&b);
    w.apply(7, 9, EditAction::RaiseGround);
    w.apply(7, 9, EditAction::SetSurface(6)); // asphalt
    w.apply(20, 4, EditAction::RaiseBuilding);
    (b, w)
}

#[test]
fn the_bundle_that_is_written_is_the_site_that_was_edited() {
    let dir = tmp("v5-save");
    let (b, w) = edited_site(32);
    b.save(
        &dir,
        (&w.ground_h, &w.medium, &w.building_h),
        "the mesh golden gate",
    )
    .unwrap();

    // What `ecosim` will read is what the viewer is drawing, cell for cell -- not the bundle as it
    // was loaded. A round trip that ran on the unedited site would be a very convincing lie.
    let back = Bundle::load(&dir).unwrap();
    assert_eq!(back.ground_h, w.ground_h);
    assert_eq!(back.medium, w.medium);
    assert_eq!(back.building_h, w.building_h);
    assert_eq!((back.width, back.depth), (b.width, b.depth));
    assert_eq!(back.ground_cell_m, b.ground_cell_m);
    assert_eq!(back.size_m, b.size_m);
    assert_eq!(back.media, b.media);
    assert_eq!(back.name, b.name);

    // And nothing else moved: two columns were edited, two columns differ.
    let changed = (0..b.width * b.depth)
        .filter(|&i| {
            back.ground_h[i] != b.ground_h[i]
                || back.medium[i] != b.medium[i]
                || back.building_h[i] != b.building_h[i]
        })
        .collect::<Vec<_>>();
    assert_eq!(changed, vec![20 + 32 * 4, 7 + 32 * 9]);
    assert_eq!(back.ground_h[7 + 32 * 9], 4.5, "one cell of ground, raised");
    assert_eq!(back.medium[7 + 32 * 9], 6);
    assert_eq!(back.building_h[20 + 32 * 4], 0.5);

    // The directory is a bundle in its own right: v2, and it says it has been through an editor.
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("bundle.json")).unwrap()).unwrap();
    assert_eq!(meta["format"], "ecosim-world-bundle");
    assert_eq!(meta["version"], 2);
    assert_eq!(meta["edited_by"], "the mesh golden gate");
    assert_eq!(meta["counts"]["pipes"], 0, "a synthetic site has no drains");
    assert_eq!(std::fs::read(dir.join("pipes.json")).unwrap(), b"[]");
}

#[test]
fn the_capitol_comes_out_of_a_save_byte_for_byte() {
    let src = std::path::Path::new(ecoview_native::CAPITOL);
    let b = Bundle::load(src).expect("the committed reference bundle");
    let dir = tmp("v5-capitol");
    b.save(&dir, (&b.ground_h, &b.medium, &b.building_h), "unedited")
        .unwrap();

    // An unedited save is a copy. The three grids are the same bytes, which is the strongest thing
    // that can be said about a float grid that has been through memory and back.
    for f in ["ground_h.f32", "medium.u8", "building_h.f32", "pipes.json"] {
        assert_eq!(
            std::fs::read(src.join(f)).unwrap(),
            std::fs::read(dir.join(f)).unwrap(),
            "{f} changed on the way through the viewer"
        );
    }
    let back = Bundle::load(&dir).unwrap();
    assert_eq!(back.trees.len(), b.trees.len());
    assert_eq!(back.shrubs.len(), b.shrubs.len());
    // The credit travels with the file. The Capitol's medium grid is ODbL and its terrain is USGS
    // LiDAR; an edited copy is still made of those (CLAUDE.md, worlds/capitol/README.md).
    assert_eq!(back.source, b.source);
    assert!(
        back.source.contains("OpenStreetMap") && back.source.contains("USGS"),
        "provenance lost: {:?}",
        back.source
    );
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("bundle.json")).unwrap()).unwrap();
    assert_eq!(meta["counts"]["pipes"], 4, "the four drains came through");
    assert_eq!(meta["counts"]["trees"], b.trees.len());
}

#[test]
fn a_save_refuses_to_write_over_the_site_it_came_from() {
    let dir = tmp("v5-guard");
    let (b, w) = edited_site(32);
    let grids = (&w.ground_h[..], &w.medium[..], &w.building_h[..]);
    b.save(&dir, grids, "first").unwrap();
    let loaded = Bundle::load(&dir).unwrap();

    // The 22 MB Capitol is public data that took a Blender scene to make, and an editor that can
    // overwrite its source by mis-clicking is an editor nobody should run.
    let err = loaded.save(&dir, grids, "over itself").unwrap_err();
    assert!(
        err.to_string().contains("refusing"),
        "wrong refusal: {err}, which should name the directory"
    );
    assert!(err.to_string().contains(&dir.display().to_string()));
    // Somewhere else is fine, and the source is still there afterwards.
    let other = tmp("v5-guard-2");
    loaded.save(&other, grids, "elsewhere").unwrap();
    assert_eq!(Bundle::load(&dir).unwrap().ground_h, w.ground_h);
    assert_eq!(Bundle::load(&other).unwrap().ground_h, w.ground_h);

    // A grid that is not the site's size is refused before anything is written, not half written.
    let short = vec![0.0f32; 4];
    let err = loaded
        .save(
            &tmp("v5-short"),
            (&short, &w.medium, &w.building_h),
            "short",
        )
        .unwrap_err();
    assert!(err.to_string().contains("ground_h"), "{err}");
}

#[test]
fn the_command_line_is_the_one_a_person_would_type() {
    // Copied from `ecoview/scripts/sim-lib.mjs`, not shared with it: the two viewers have no code
    // in common either. If this drifts, a garden run started from the viewer stops being the run a
    // hand-typed command produces, and the two stop being comparable.
    let args = sim::ecosim_args(
        std::path::Path::new("world"),
        std::path::Path::new("run"),
        42,
        2000,
        200,
        None,
    );
    let line = args.join(" ");
    assert!(line.starts_with("run --world world --out run "), "{line}");
    for expected in [
        "--seed 42",
        "--ticks 2000",
        "--snapshot-every 200",
        "--snapshot-state false",
        // MASTER.md's standing rule for every garden run on a bundle world: a rainfall ramp makes
        // no sense on a 256 m photographed site, and animals are not what this project is about.
        "--set animals.enabled=false",
        "--set climate.rain_gradient=0",
    ] {
        assert!(line.contains(expected), "{expected:?} missing from {line}");
    }
    assert!(!line.contains("--params"), "no params file, no flag");

    let with = sim::ecosim_args(
        std::path::Path::new("world"),
        std::path::Path::new("run"),
        1,
        10,
        1,
        Some(std::path::Path::new("p.toml")),
    );
    assert!(with.join(" ").ends_with("--params p.toml"));
}

#[test]
fn ten_snapshots_whatever_the_run_is_worth() {
    assert_eq!(sim::snapshot_every(2000), 200);
    assert_eq!(sim::snapshot_every(sim::MAX_TICKS), 2000);
    assert_eq!(sim::snapshot_every(sim::DEFAULT_TICKS), 200);
    // Short runs still have a timeline to scrub rather than an every-zero-ticks division by zero.
    assert_eq!(sim::snapshot_every(1), 1);
    assert_eq!(sim::snapshot_every(4), 1);
}

#[test]
fn progress_is_read_off_the_disk_and_nothing_else() {
    // The simulator says nothing about its progress, so the only honest source is the snapshots it
    // has actually finished writing. Names that are not `snap_NNNNNN` are ignored, not guessed at.
    let names = [
        "meta.json",
        "series.csv",
        "snap_000000",
        "snap_001200",
        "snap_000100",
        "snap_12",
        "snap_abcdef",
        "snapshot_002000",
    ];
    assert_eq!(sim::tick_of_snapshots(names.iter()), 1200);
    assert_eq!(sim::tick_of_snapshots(std::iter::empty::<&str>()), 0);
}

#[test]
fn the_simulator_is_found_where_the_environment_says() {
    // The only test here that touches the environment, so it cannot race the others.
    let dir = tmp("v5-env");
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("ecosim-somewhere-else");
    std::fs::write(&fake, b"not really a simulator").unwrap();

    std::env::set_var("ECOSIM_BIN", &fake);
    assert_eq!(sim::binary(), fake);
    std::env::remove_var("ECOSIM_BIN");
    let d = sim::binary();
    assert!(
        d.ends_with("ecosim") || d.ends_with("ecosim.exe"),
        "{}",
        d.display()
    );
    assert!(d.starts_with("../ecosim/target/release"), "{}", d.display());

    // A params file that is not there is `None` rather than a path the simulator would reject:
    // `ecosim` looks for `params.toml` beside its working directory, and the viewer's scratch
    // directory is not that, so a round trip either passes the real file or does not run at all.
    std::env::set_var("ECOSIM_PARAMS", dir.join("no-such-params.toml"));
    assert_eq!(sim::params_file(), None);
    let real = dir.join("params.toml");
    std::fs::write(&real, b"[world]\n").unwrap();
    std::env::set_var("ECOSIM_PARAMS", &real);
    assert_eq!(sim::params_file(), Some(real));
    std::env::remove_var("ECOSIM_PARAMS");
}

/// Spawns this test binary as a stand-in for the simulator, and attaches a job to it.
///
/// `ecosim` is not built in the `ecoview-native` CI job -- the gate compiles this crate alone, with
/// `--no-default-features` -- so the lifecycle is exercised against a process that exists wherever
/// the test does. Everything after the spawn is the code a real round trip runs.
fn job_on(dir: &std::path::Path, args: &[&str], ticks: u32) -> SimJob {
    std::fs::create_dir_all(dir).unwrap();
    let exe = std::env::current_exe().unwrap();
    let log = dir.join("ecosim.log");
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let child = sim::spawn(&exe, &owned, dir, &log).unwrap();
    SimJob::attach(child, dir.join("world"), dir.join("run"), log, ticks)
}

fn wait_for(job: &mut SimJob) -> SimState {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let s = job.poll();
        if s != SimState::Running || std::time::Instant::now() > deadline {
            return s;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn a_run_is_polled_until_it_stops_and_then_reports_what_it_did() {
    let dir = tmp("v5-job-ok");
    let mut job = job_on(&dir, &["--list"], 2000);
    assert_eq!(job.state(), SimState::Running);
    assert_eq!(job.every, 200, "ten snapshots over the run");

    // Half a run of snapshots on disk, as the simulator would leave them.
    std::fs::create_dir_all(job.out.join("snap_001000")).unwrap();
    std::fs::create_dir_all(job.out.join("snap_000200")).unwrap();
    let (tick, fraction) = job.progress();
    assert_eq!(tick, 1000);
    assert!((fraction - 0.5).abs() < 1e-6, "{fraction}");

    assert_eq!(wait_for(&mut job), SimState::Done);
    assert_eq!(job.poll(), SimState::Done, "a stopped job is not re-asked");
    assert!(job.elapsed() > std::time::Duration::ZERO);
    // A file, not a pipe: the viewer never reads the output, and an unread pipe fills and stops
    // the writer halfway through a long run with no error anywhere.
    assert!(!std::fs::read_to_string(&job.log).unwrap().is_empty());
}

#[test]
fn a_run_that_fails_says_so_in_the_words_the_process_used() {
    let dir = tmp("v5-job-bad");
    let mut job = job_on(&dir, &["--no-such-flag"], 100);
    match wait_for(&mut job) {
        SimState::Failed(msg) => {
            assert!(msg.contains("exited"), "{msg}");
            // The last line of the log, not "it failed": a viewer that keeps the reason in a file
            // nobody opens is a viewer that wastes an afternoon.
            let last = sim::last_line(&job.log);
            assert!(!last.is_empty());
            assert!(msg.contains(&last), "{msg} does not carry {last:?}");
        }
        other => panic!("a bad command line should fail, not {other:?}"),
    }
    // Nothing was written where a run would go, and the viewer keeps the run it already had.
    assert!(!job.out.join("meta.json").exists());
}

#[test]
fn a_cancelled_run_stops_being_a_run() {
    let dir = tmp("v5-job-cancel");
    let mut job = job_on(&dir, &["--list"], 2000);
    job.cancel();
    assert_eq!(job.state(), SimState::Failed("cancelled".into()));
    assert_eq!(job.poll(), job.state(), "polling does not revive it");
    assert!(job.elapsed() > std::time::Duration::ZERO);
}

#[test]
fn the_crosshair_points_at_the_column_under_it() {
    let b = flat(32, 0.5, 4.0); // a 16 m site, flat at 4 m
    let mut w = VoxelWorld::from_bundle(&b);
    // Straight down from above: the cell the ray is over, in ground cells, not metres.
    assert_eq!(
        w.pick_cell([5.25, 20.0, 7.25], [0.0, -1.0, 0.0], 120.0),
        Some((10, 14))
    );
    // Up is nothing. So is a direction that is not one.
    assert_eq!(
        w.pick_cell([5.25, 20.0, 7.25], [0.0, 1.0, 0.0], 120.0),
        None
    );
    assert_eq!(
        w.pick_cell([5.25, 20.0, 7.25], [0.0, 0.0, 0.0], 120.0),
        None
    );
    // Off the side of the site: the ray passes nothing solid and says so.
    assert_eq!(
        w.pick_cell([-5.0, 20.0, 7.25], [0.0, -1.0, 0.0], 120.0),
        None
    );
    // Out of range is out of range: 20 m up, looking down, with 5 m of reach.
    assert_eq!(w.pick_cell([5.25, 20.0, 7.25], [0.0, -1.0, 0.0], 5.0), None);

    // A wall a metre and a half up. A level ray at 5 m passes over the lawn and stops at the first
    // column of building -- the crosshair edits what it can see, which includes what is built.
    for y in 0..32 {
        for x in 20..24 {
            w.apply(x, y, EditAction::RaiseBuilding);
            w.apply(x, y, EditAction::RaiseBuilding);
            w.apply(x, y, EditAction::RaiseBuilding);
        }
    }
    assert_eq!(w.column(20, 14).unwrap().2, 1.5);
    assert_eq!(
        w.pick_cell([0.1, 5.0, 7.25], [1.0, 0.0, 0.0], 120.0),
        Some((20, 14)),
        "the near face of the wall, not the far side of the site"
    );
}

#[test]
fn undo_puts_back_what_the_edit_took_rather_than_acting_again() {
    let mut w = VoxelWorld::from_bundle(&flat(32, 0.5, 4.0));
    assert_eq!(w.column(5, 5), Some((4.0, 1, 0.0)));
    assert_eq!(w.column(32, 0), None, "outside the site is not a column");
    assert!(w.restore_column(32, 0, (0.0, 0, 0.0)).is_empty());

    // Surface: the action throws the old code away, so only a record of it can put it back.
    let was = w.column(5, 5).unwrap();
    let stale = w.apply(5, 5, EditAction::SetSurface(6));
    assert!(!stale.is_empty(), "the edit leaves its own chunk stale");
    assert_eq!(w.column(5, 5).unwrap().1, 6);
    assert!(!w.restore_column(5, 5, was).is_empty());
    assert_eq!(w.column(5, 5), Some(was));

    // Ground: `LowerGround` clamps at zero, so the opposite action is not an inverse. Eight drops
    // take this column to the floor; the ninth does nothing, and a raise afterwards would leave it
    // half a metre above where it started. The recorded column does not.
    for _ in 0..8 {
        w.apply(0, 0, EditAction::LowerGround);
    }
    let floor = w.column(0, 0).unwrap();
    assert_eq!(floor.0, 0.0);
    w.apply(0, 0, EditAction::LowerGround);
    assert_eq!(w.column(0, 0).unwrap().0, 0.0, "it cannot go below zero");
    w.apply(0, 0, EditAction::RaiseGround);
    assert_eq!(w.column(0, 0).unwrap().0, 0.5, "the opposite over-corrects");
    w.restore_column(0, 0, floor);
    assert_eq!(w.column(0, 0), Some(floor));
}

// -----------------------------------------------------------------------------------------------
// Shot V6: the beauty pass. Ambient occlusion is baked into the voxel ids, so it is the mesher's
// business and belongs in this gate; the sun, the sky and the season are engine-free arithmetic in
// `sky.rs` and are checked here for the same reason -- this is the one test target CI runs.

use ecoview_native::mesh::bake_occlusion;
use ecoview_native::palette::{base_id, PALETTE_LEN};
use ecoview_native::sky::{
    shaded_palette, Clock, Season, SkyState, Sun, AO_SHADE, AXIAL_TILT_DEG, DAYS_PER_YEAR,
    DEFAULT_YEAR_LEN, SEASON_STEPS, SOLSTICE_DAY,
};
use ecoview_native::voxel::{CS_P, CS_P3};

/// The flat site with one building on it, with occlusion baked in or not.
fn occluded(on: bool) -> VoxelWorld {
    let mut w = VoxelWorld::from_bundle(&with_building(64, 6.0));
    w.ao = on;
    w
}

fn mesh_ao(w: &VoxelWorld) -> (u64, usize, usize) {
    let pal = shaded_palette(&palette(Overlay::Surface, None));
    let m = mesh_chunk(w, ChunkPos { x: 0, y: 0, z: 0 }, &pal, &mut Scratch::new());
    (m.hash(), m.positions.len(), m.indices.len())
}

/// The golden: one occluded chunk in, one hashed mesh out. A change to the neighbourhood counted,
/// to the four shade levels, or to how a level is packed into an id moves this number.
#[test]
fn golden_occluded() {
    assert_eq!(
        mesh_ao(&occluded(true)),
        (GOLDEN_AO, GOLDEN_AO_VERTS, GOLDEN_AO_INDICES)
    );
}

const GOLDEN_AO: u64 = 0x677b_50c8_3df8_0b76;
const GOLDEN_AO_VERTS: usize = 156;
const GOLDEN_AO_INDICES: usize = 234;

/// **The proof that this shot added a layer and changed nothing under it.** Occlusion costs quads
/// -- two voxels of one material at different shade no longer merge into one -- and turning it off
/// gives back the V5 mesh to the byte, because level 0 is the palette's first block untouched.
#[test]
fn ambient_occlusion_off_is_the_mesh_of_the_shot_before() {
    let off = mesh_ao(&occluded(false));
    let on = mesh_ao(&occluded(true));
    assert_ne!(off.0, on.0, "occlusion has to change something");
    assert!(
        on.2 > off.2,
        "occlusion splits merges: {} indices on, {} off",
        on.2,
        off.2
    );
    // And the switch is reversible, from a world that has already been meshed occluded.
    let mut w = occluded(true);
    assert_eq!(mesh_ao(&w), on);
    assert_eq!(w.set_ao(false).len(), w.all_chunks().len());
    assert_eq!(mesh_ao(&w), off, "off gives back exactly the V5 mesh");
    assert!(
        w.set_ao(false).is_empty(),
        "switching to what it already is"
    );
}

/// What "occluded" means, on a site whose answer is known by eye: the lawn under open sky is
/// level 0, the strip of ground the building's wall stands on is not, and under the roof is darkest.
#[test]
fn occlusion_darkens_the_foot_of_a_wall_and_leaves_the_lawn_alone() {
    let c = ChunkPos { x: 0, y: 0, z: 0 };
    let mut plain = vec![0u16; CS_P3];
    occluded(true).fill_chunk(c, &mut plain);
    let mut baked = plain.clone();
    bake_occlusion(&mut baked);
    // Occlusion never changes what a voxel is, only how dark it is drawn.
    for (a, b) in plain.iter().zip(&baked) {
        assert_eq!(*a, base_id(*b), "the material under the shade");
    }
    let at = |x: usize, y: usize, z: usize| baked[(y + 1) + (x + 1) * CS_P + (z + 1) * CS_P * CS_P];
    let level = |x: usize, y: usize, z: usize| at(x, y, z) as usize / PALETTE_LEN;
    let top = |x: usize, y: usize| (0..40).filter(|z| at(x, y, *z) != 0).next_back().unwrap();
    // 4 m of ground on a 0.5 m lattice, and the building's 6 m on top of that.
    let (ground, roof) = (top(2, 2), top(25, 25));
    assert!(roof > ground, "the building stands on the lawn");
    assert_eq!(level(2, 2, ground), 0, "open sky is the V5 colour exactly");
    assert_eq!(level(25, 25, roof), 0, "so is the top of the roof");
    // x 19 is the ring of lawn immediately west of the building, which starts at x 20: three of
    // the eight cells over it are wall.
    assert_eq!(level(19, 25, ground), 1, "the foot of a wall is occluded");
    // And inside the building's own volume every one of the eight is.
    assert_eq!(level(25, 25, ground), AO_SHADE.len() - 1, "under a roof");
    // Soil under soil is the darkest there is, and is never meshed: no face of it is exposed.
    assert_eq!(level(2, 2, ground - 1), AO_SHADE.len() - 1);
}

/// The shaded palette is the palette four times over, each block dimmer, the first untouched.
#[test]
fn the_shaded_palette_keeps_its_first_block() {
    let base = palette(Overlay::Moisture, None);
    let pal = shaded_palette(&base);
    assert_eq!(pal.len(), PALETTE_LEN * AO_SHADE.len());
    assert_eq!(&pal[..PALETTE_LEN], &base[..]);
    for level in 1..AO_SHADE.len() {
        for i in 0..PALETTE_LEN {
            let (a, b) = (pal[i], pal[level * PALETTE_LEN + i]);
            for ch in 0..3 {
                assert!(b[ch] <= a[ch] + 1e-6, "level {level} is no brighter");
            }
            assert_eq!(a[3], b[3], "alpha is not a shade");
        }
    }
    assert!(AO_SHADE.windows(2).all(|w| w[1] < w[0]), "monotone");
}

/// A clock on a chosen day of the year. `Clock::of` maps a tick to a day, so this goes the other
/// way to pick one; the hour is the viewer's either way.
fn on_day(day: f32, hour: f32) -> Clock {
    let year_len = 36525u64;
    let f = (day - SOLSTICE_DAY).rem_euclid(DAYS_PER_YEAR) / DAYS_PER_YEAR + 0.25;
    Clock::of(
        Some((f * year_len as f32) as u64 % year_len),
        year_len,
        hour,
    )
}

/// The sun, against what the geometry says. Noon at an equinox is `90 - latitude` above a southern
/// horizon; the two solstices are that plus and minus the axial tilt; midnight is below it.
#[test]
fn the_sun_is_where_the_geometry_says() {
    let lat = 42.7;
    for (day, expect) in [
        (80.0, 90.0 - lat),
        (SOLSTICE_DAY, 90.0 - lat + AXIAL_TILT_DEG),
        (SOLSTICE_DAY + 182.6, 90.0 - lat - AXIAL_TILT_DEG),
    ] {
        let noon = Sun::at(&on_day(day, 12.0), lat);
        assert!(
            (noon.elevation_deg - expect).abs() < 1.5,
            "day {day}: {:.1} deg up, expected about {expect:.1}",
            noon.elevation_deg
        );
        assert!(
            (noon.azimuth_deg - 180.0).abs() < 1.0,
            "north of the tropics the noon sun is due south, got {:.1}",
            noon.azimuth_deg
        );
        assert!(noon.is_up());
    }
    let midnight = Sun::at(&on_day(SOLSTICE_DAY, 0.0), lat);
    assert!(!midnight.is_up(), "{:.1} deg up", midnight.elevation_deg);
    assert_eq!(midnight.illuminance, 0.0);
    // Morning sun in the east, afternoon sun in the west, and the unit vector agrees with both.
    let noon = Sun::at(&on_day(SOLSTICE_DAY, 12.0), lat);
    let morning = Sun::at(&on_day(SOLSTICE_DAY, 8.0), lat);
    let evening = Sun::at(&on_day(SOLSTICE_DAY, 16.0), lat);
    assert!(morning.azimuth_deg < 180.0 && morning.dir[0] > 0.0, "east");
    assert!(evening.azimuth_deg > 180.0 && evening.dir[0] < 0.0, "west");
    assert!(morning.illuminance < noon.illuminance, "and dimmer");
    // A sun near the horizon is redder than one overhead, which is the air mass it came through.
    assert!(Sun::at(&on_day(SOLSTICE_DAY, 6.0), lat).color[2] < noon.color[2]);
    // South of the equator the same day is midwinter and the noon sun stands in the north.
    let south = Sun::at(&on_day(SOLSTICE_DAY, 12.0), -33.9);
    assert!(
        south.azimuth_deg < 5.0 || south.azimuth_deg > 355.0,
        "north"
    );
}

/// **The day of the year is the run's**, and the alignment is the simulator's own temperature peak.
#[test]
fn the_day_of_the_year_comes_from_the_tick() {
    let year = 4000u64;
    // The simulator's warmest tick is a quarter of the way through its year (`abiotic.rs`), and
    // that is the tick this viewer draws as the summer solstice.
    let summer = Clock::of(Some(year / 4), year, 10.0);
    assert!((summer.day - SOLSTICE_DAY).abs() < 0.1, "{}", summer.day);
    assert_eq!(Season::of(summer.day).name, "summer");
    // Which fixes tick 0 as the spring equinox.
    let spring = Clock::of(Some(0), year, 10.0);
    assert!((spring.day - (SOLSTICE_DAY - DAYS_PER_YEAR / 4.0)).abs() < 0.1);
    // A tick is the hours `ecosim/UNITS.md` derives from `year_len`, and the year wraps.
    assert!(
        (spring.tick_hours - 2.1915).abs() < 1e-3,
        "{}",
        spring.tick_hours
    );
    assert_eq!(Clock::of(Some(4 * year), year, 10.0).day, spring.day);
    assert!((Clock::of(Some(6000), year, 10.0).years() - 1.5).abs() < 1e-6);
    // No run, no year: the viewer says whose the date is, and lights the site anyway.
    let none = Clock::of(None, 0, 10.0);
    assert!(!none.from_run && none.day == SOLSTICE_DAY);
    assert!(Clock::of(Some(0), year, 10.0).from_run);
    // A `year_len` of zero is a run that did not state one; the shipped default stands in.
    assert_eq!(none.year_len, DEFAULT_YEAR_LEN);
    assert_eq!(Clock::of(Some(0), year, 25.5).hhmm(), "01:30");
    assert_eq!(Clock::of(Some(0), year, 10.0).month_day(), ("March", 22));
}

/// The season moves the leaves, moves nothing else, and moves them smoothly.
#[test]
fn the_season_turns_the_leaves_and_leaves_the_paving_alone() {
    let base = palette(Overlay::Surface, None);
    let autumn = Season::of(288.0);
    assert_eq!(autumn.name, "autumn");
    let mut tinted = base.clone();
    autumn.tint_palette(&mut tinted);
    // A canopy in October is not the canopy `meta.json` named, and it has gone towards the red.
    let c = CANOPY as usize;
    assert_ne!(tinted[c], base[c]);
    assert!(
        tinted[c][0] > base[c][0] && tinted[c][1] < base[c][1],
        "redder"
    );
    assert_ne!(tinted[GRASS as usize], base[GRASS as usize]);
    // Nothing that is not alive moves, in any season, ever -- including every overlay band.
    for day in [0.0, 90.0, 180.0, 270.0, 364.0] {
        let mut p = palette(Overlay::Moisture, None);
        let before = p.clone();
        Season::of(day).tint_palette(&mut p);
        for id in 0..PALETTE_LEN {
            if ![CANOPY, VINE, SHRUB, GRASS].contains(&(id as u16)) {
                assert_eq!(p[id], before[id], "id {id} is not alive, on day {day}");
            }
        }
    }
    // Summer is the base colour, near enough that a July screenshot is the V5 one.
    let mut july = base.clone();
    Season::of(200.0).tint_palette(&mut july);
    for ch in 0..3 {
        assert!(
            (july[c][ch] - base[c][ch]).abs() < 0.05,
            "summer is the base"
        );
    }
    // And the year is continuous: no two adjacent steps of it jump.
    let mut prev: Option<[f32; 4]> = None;
    for step in 0..SEASON_STEPS {
        let day = (step as f32 + 0.5) * DAYS_PER_YEAR / SEASON_STEPS as f32;
        let s = Season::of(day);
        assert_eq!(s.step(), step, "day {day} is step {step}");
        let mut p = base.clone();
        s.tint_palette(&mut p);
        if let Some(q) = prev {
            for ch in 0..3 {
                assert!((p[c][ch] - q[ch]).abs() < 0.15, "step {step} jumps");
            }
        }
        prev = Some(p[c]);
    }
    // Winter keeps every leaf: leaf fall is geometry, and the simulator's to model, not the
    // viewer's (row G10). Only the colour moves.
    assert_eq!(Season::of(15.0).name, "winter");
    assert_eq!(Season::of(135.0).name, "spring");
}

/// The sky is a mesh like any other: inward-facing, darker at the top than at the horizon by day,
/// and carrying a disc of sun only while the sun is up.
#[test]
fn the_sky_dome_faces_the_camera_inside_it() {
    let (rings, segments) = (12usize, 32usize);
    let ring_verts = (rings + 1) * (segments + 1);
    let noon = SkyState::of(on_day(SOLSTICE_DAY, 12.0), 42.7);
    let d = noon.dome(600.0, rings, segments);
    // The rings, then a fan of sun: one centre and a rim, at the 24 segments the disc is capped to.
    assert_eq!(d.positions.len(), ring_verts + 26);
    assert_eq!(d.indices.len(), rings * segments * 6 + 24 * 3);
    assert_eq!(d.positions.len(), d.normals.len());
    assert_eq!(d.positions.len(), d.colors.len());
    for (p, n) in d.positions.iter().zip(&d.normals) {
        let dot = p[0] * n[0] + p[1] * n[1] + p[2] * n[2];
        assert!(dot < 0.0, "every normal points back at the centre");
    }
    // The disc's centre is where the sun is, and it is brighter than the sky behind it.
    let centre = d.positions[ring_verts];
    let s = noon.sun.dir;
    let along = centre[0] * s[0] + centre[1] * s[1] + centre[2] * s[2];
    assert!(along > 500.0, "the disc sits in the sun's direction");
    assert!(
        d.colors[ring_verts][0] > d.colors[0][0],
        "brighter than sky"
    );
    // Night: no disc, a darker sky, and an ambient level that never reaches zero.
    let night = SkyState::of(on_day(SOLSTICE_DAY, 0.0), 42.7);
    let nd = night.dome(600.0, rings, segments);
    assert_eq!(nd.positions.len(), ring_verts, "no sun to draw");
    assert!(night.zenith[2] < noon.zenith[2], "night is a darker sky");
    assert!(night.ambient > 0.0 && night.ambient < noon.ambient);
    assert!(noon.ambient > 700.0, "noon is at least what V0-V5 lit with");
    // The one line every scripted run prints says whose each half of this is.
    let line = noon.line();
    assert!(line.contains("the hour, the latitude and the hue are the viewer's"));
    assert!(line.contains("the date is the run's"), "{line}");
    assert!(SkyState::of(Clock::of(None, 0, 12.0), 42.7)
        .line()
        .contains("no run loaded"));
    // Remeshing hangs on the season step and on nothing else the sun does.
    assert_eq!(noon.mesh_key(), noon.season.step());
    assert_eq!(night.mesh_key(), noon.mesh_key(), "an hour is not a season");
}

// ---- shot S5: standing water ----
//
// In this file for the reason every block above it is: the CI gate runs exactly one target, and
// adding a second one means editing the workflow, which belongs to a `ci` row rather than a viewer
// shot. All of these are engine-free.
//
// **Nothing here models water.** `water.bin` is ponded depth on the ground grid, written by `ecosim`
// every snapshot since shot G4, and the simulator decided every millimetre in it. What these tests
// pin is the reading of it (the unit, the grid, the refusal of a file of the wrong length), the map
// (dry ground is a band of its own, and the ramp above it is logarithmic) and the drawing (which
// cells stand up as voxels, how many, and that taking the water off gives back the site that was
// under it).

use ecoview_native::overlay::{water_band, PondLevels, POND_MIN_MM, WATER_RAMP_MM};
use ecoview_native::palette::{id_name, POND, POND_HEX, WATER_DRY};
use ecoview_native::voxel::AIR;

/// `water.bin`, read back as ponded depth on the grid it was written on.
#[test]
fn water_bin_is_ponded_depth_in_tenths_of_a_millimetre() {
    let dir = tmp("ponds");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();

    // The grid is the **ground** grid out of `meta.json`'s `world` -- 16 x 16 here -- and not the
    // 8 x 8 ecology grid every other field is on. Reading it as an ecology field would have drawn a
    // quarter of the site and called it the whole.
    let p = run.ponds_at(1).unwrap();
    assert_eq!((p.width, p.depth), (16, 16));
    assert_eq!(p.cells(), 256);
    assert_eq!(p.mm[0], 0.0);
    assert_eq!(p.mm[1], 0.3);
    assert_eq!(p.mm[2], 50.0);
    assert_eq!(p.mm[3], 1000.0);
    assert_eq!(p.mm[4], 5000.0);

    // Four cells hold water and three are deep enough to draw. The mean is over the wet cells, not
    // over the site: a site that is 98% dry has a mean of nearly nothing, which says something
    // about the site and nothing at all about its water.
    let s = p.stats(0.5);
    assert_eq!((s.wet, s.drawn), (4, 3));
    assert_eq!(s.max_mm, 5000.0);
    assert!((s.mean_mm - 6050.3 / 4.0).abs() < 1e-2, "{}", s.mean_mm);
    // 6050.3 mm spread over cells of 0.25 m2 is 1.5126 m3.
    assert!((s.volume_m3 - 1.512_575).abs() < 1e-6, "{}", s.volume_m3);

    // Tick 0 is dry, as tick 0 of every run is: no storm has fallen yet.
    let dry = run.ponds_at(0).unwrap();
    assert_eq!(dry.stats(0.5).wet, 0);
    assert!(dry.mm.iter().all(|d| *d == 0.0));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A `water.bin` of the wrong length is refused by name, the way every other field file is.
#[test]
fn a_short_water_file_is_refused() {
    let dir = tmp("shortwater");
    write_overlay_run(&dir, 8, 4, true);
    std::fs::write(dir.join("snap_000100").join("water.bin"), vec![0u8; 64]).unwrap();
    let run = Run::load(&dir).unwrap();
    let e = run.ponds_at(1).unwrap_err().to_string();
    assert!(e.contains("water.bin"), "{e}");
    assert!(e.contains("512"), "the length it expected: {e}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Dry ground is a band of its own, and every depth above it sits on a log10 ramp.
///
/// The measurement behind that shape is in MEASUREMENTS.md: on `runs/capitol-s42` at tick 10000 the
/// median wet cell holds 10 mm and the deepest holds 5,074, so a linear ramp to the maximum puts
/// 99% of the standing water in the bottom band and paints a flooded site as a dry one.
#[test]
fn the_water_overlay_has_a_dry_band_and_a_logarithmic_ramp() {
    let dir = tmp("waterscale");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let s = Scale::of(Overlay::Water, &run.meta);
    assert_eq!((s.lo, s.hi), WATER_RAMP_MM);

    // Dry is exactly zero, not "below the bottom of the ramp": a tenth of a millimetre is water the
    // simulator put there, and the map says so.
    assert_eq!(water_band(0.0, &s), WATER_DRY);
    assert_eq!(water_band(-1.0, &s), WATER_DRY);
    assert_eq!(water_band(0.1, &s), WATER_DRY + 1);
    assert_eq!(water_band(1.0, &s), WATER_DRY + 1);
    assert_eq!(water_band(10_000.0, &s), BANDS as u8 - 1);
    assert_eq!(
        water_band(99_999.0, &s),
        BANDS as u8 - 1,
        "clamped at the top"
    );

    // Monotone, and a decade of depth is a fixed height on the ramp -- which is what logarithmic
    // means, and what a linear ramp could not give: 1 mm, 10 mm, 100 mm and 1 m are equally far
    // apart. 30 bands over four decades is 7.5 each, so consecutive steps are 7 or 8.
    let b = |mm: f32| water_band(mm, &s) as i32;
    let steps = [b(10.0) - b(1.0), b(100.0) - b(10.0), b(1000.0) - b(100.0)];
    for d in steps {
        assert!((7..=8).contains(&d), "{steps:?}");
    }
    assert_eq!(
        b(10_000.0) - b(1.0),
        30,
        "the whole ramp above the dry band"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The map covers every wet cell; the geometry covers the ones deep enough to stand up.
///
/// This is the one number in the shot that is the **viewer's** rather than the run's, so it is
/// tested rather than only written down: 0.3 mm of water is drawn nowhere and mapped somewhere.
#[test]
fn a_film_of_water_is_mapped_but_not_drawn() {
    let dir = tmp("film");
    write_overlay_run(&dir, 8, 4, true);
    let run = Run::load(&dir).unwrap();
    let p = run.ponds_at(1).unwrap();
    let s = Scale::of(Overlay::Water, &run.meta);
    let (bands, stats) = p.bands(&s);

    assert!(p.mm[1] > 0.0 && p.mm[1] < POND_MIN_MM);
    assert!(bands[1] > WATER_DRY, "the film is on the map");
    assert_eq!(bands[0], WATER_DRY, "and dry ground is not");
    assert!(
        bands[4] > bands[3] && bands[3] > bands[2],
        "deeper is further up"
    );
    // The field's own numbers, over the whole site rather than over the wet cells: this is what the
    // overlay legend reports, and a site with one deep corner has a mean of almost nothing.
    assert_eq!((stats.min, stats.max), (0.0, 5000.0));
    assert!((stats.mean - 6050.3 / 256.0).abs() < 1e-3, "{}", stats.mean);

    let levels = p.levels(0.5);
    assert_eq!(levels.levels[1], 0, "a film is not drawn");
    assert_eq!(
        levels.levels[2], 1,
        "50 mm is one voxel, which is 0.5 m: see the HUD line"
    );
    assert_eq!(levels.levels[3], 2, "1000 mm on a 0.5 m lattice");
    assert_eq!(levels.levels[4], 10, "5000 mm");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Standing water stands on the ground, where the run put it and nowhere else.
#[test]
fn standing_water_stands_on_the_ground_the_run_wetted() {
    // 20 m of headroom so the deepest pond is not clipped by the chunk grid's ceiling.
    let mut w = VoxelWorld::from_bundle_with_headroom(&flat(16, 0.5, 4.0), 20.0);
    let g = w.ground_level(0, 0);
    assert!(!w.has_ponds());

    let mut levels = vec![0u8; 16 * 16];
    levels[2] = 1;
    levels[4] = 10;
    let stale = w.set_ponds(Some(&PondLevels {
        width: 16,
        depth: 16,
        levels,
    }));
    assert!(!stale.is_empty());
    assert!(w.has_ponds());
    assert_eq!(w.pond_counts(), (2, 11));

    // The water sits **on** the ground rather than in it: the column's own top voxel is still the
    // medium that was surveyed there.
    assert_eq!(w.voxel(2, 0, g), 2, "lawn is medium 1, id 2");
    assert_eq!(w.voxel(2, 0, g + 1), POND);
    assert_eq!(w.voxel(2, 0, g + 2), AIR, "one voxel of water, not two");
    assert_eq!(w.voxel(4, 0, g + 10), POND, "ten of them");
    assert_eq!(w.voxel(4, 0, g + 11), AIR);
    // And a dry cell is dry, however wet its neighbour is.
    assert_eq!(w.voxel(3, 0, g + 1), AIR);
    assert_eq!(w.voxel(0, 0, g + 1), AIR);

    // The pond's colour is not the `water` medium's: a reader has to be able to tell water this run
    // ponded from water somebody surveyed (palette.rs, `POND_HEX`).
    let pal = palette(Overlay::Surface, None);
    assert_eq!(pal[POND as usize], linear_rgba(POND_HEX));
    assert_ne!(pal[POND as usize], pal[9], "medium 8, `water`, is id 9");
}

/// Taking the water off gives back exactly the site that was under it, and a change remeshes what
/// the water changed rather than the site.
#[test]
fn set_ponds_reports_exactly_the_chunks_whose_mesh_changed() {
    // 128 cells at 0.5 m is a 64 m site; 20 m of headroom leaves room above the ground for a pond.
    let mut w = VoxelWorld::from_bundle_with_headroom(&flat(128, 0.5, 4.0), 20.0);
    let pal = palette(Overlay::Water, None);
    let mut scratch = Scratch::new();
    let all = w.all_chunks();
    let hashes = |w: &VoxelWorld, s: &mut Scratch| -> Vec<u64> {
        all.iter()
            .map(|&c| mesh_chunk(w, c, &pal, s).hash())
            .collect()
    };
    let dry = hashes(&w, &mut scratch);

    // A dry snapshot is not the same thing as no snapshot, and it draws the same site.
    let none = PondLevels {
        width: 128,
        depth: 128,
        levels: vec![0u8; 128 * 128],
    };
    assert!(
        w.set_ponds(Some(&none)).is_empty(),
        "a dry run changes no voxel"
    );
    assert!(w.has_ponds());
    assert_eq!(hashes(&w, &mut scratch), dry);

    // One puddle, inside the first chunk: one chunk is stale and no other.
    let mut one = none.clone();
    one.levels[10 + 128 * 10] = 1;
    let stale = w.set_ponds(Some(&one));
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert_eq!(stale[0], ChunkPos { x: 0, y: 0, z: 0 });
    assert_ne!(hashes(&w, &mut scratch), dry, "a puddle has to show");

    // And off again, to the bit. This is the claim the shot rests on: water is a layer over the
    // site rather than a change to it.
    assert_eq!(w.set_ponds(None).len(), 1);
    assert!(!w.has_ponds());
    assert_eq!(hashes(&w, &mut scratch), dry);
    assert_eq!(w.pond_counts(), (0, 0));
}

/// The water map is drawn on the **ground** grid, at the resolution of the file it came from.
///
/// The other six overlays are read on the simulator's 1 m columns, and on the Capitol each of those
/// covers 2 x 2 ground cells. Water is not one of them: `water.bin` is per ground cell, because the
/// ground grid is what the water ran over. Before this shot `ColumnBands` had no way to say which
/// grid it was on, so a water field handed to it would have been stretched to twice its size in
/// silence.
#[test]
fn the_water_map_is_not_stretched_from_the_ecology_grid() {
    let mut w = VoxelWorld::from_bundle(&flat(16, 0.5, 4.0));
    let g = w.ground_level(0, 0);
    let mut bands = vec![0u8; 16 * 16];
    bands[1] = 7;
    w.set_overlay(Some(&ColumnBands {
        x: 16,
        y: 16,
        cell_m: 0.5,
        bands,
    }));
    let band = |w: &VoxelWorld, x: i32| w.voxel(x, 0, g) - BAND_BASE;
    assert_eq!(
        (band(&w, 0), band(&w, 1), band(&w, 2)),
        (0, 7, 0),
        "one ground cell, not the two an ecology column covers"
    );

    // The same numbers on the ecology grid cover two cells each, which is what
    // `one_ecology_column_covers_its_ground_cells` pins from the other side.
    let mut bands = vec![0u8; 64];
    bands[1] = 7;
    w.set_overlay(Some(&ColumnBands {
        x: 8,
        y: 8,
        cell_m: ECO_CELL_M,
        bands,
    }));
    assert_eq!((band(&w, 1), band(&w, 2), band(&w, 3)), (0, 7, 7));
}

/// Standing water has an id of its own, past the bands, and that is why every mesh golden hash
/// above still stands.
///
/// The palette is the surface ids, then the 32 bands, then the pond. Putting the pond among the
/// media -- where a new surface id would naturally go -- would have moved `BAND_BASE` by one and
/// changed the colour of every overlay band, and with it the goldens from V0 through V6.
#[test]
fn the_pond_id_sits_past_the_bands() {
    assert_eq!(POND as usize, ID_COUNT + BANDS);
    assert_eq!(PALETTE_LEN, ID_COUNT + BANDS + 1);
    assert_eq!(BAND_BASE as usize, ID_COUNT, "the bands did not move");
    for o in [Overlay::Surface, Overlay::Water, Overlay::Fire] {
        let p = palette(o, None);
        assert_eq!(p.len(), PALETTE_LEN);
        assert_eq!(p[..ID_COUNT], surface_palette()[..], "{}", o.name());
        assert_eq!(p[POND as usize], linear_rgba(POND_HEX));
    }
    // The water overlay's own bands: dry ground is categorical and off the ramp, the way fire's
    // quiet band is, and the ramp above it runs between the two hues the legend names.
    let p = palette(Overlay::Water, None);
    let r = Overlay::Water.ramp(None);
    // The top band is the ramp's far end reached by interpolation, so it is compared as a colour
    // rather than as four bit patterns.
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert_eq!(p[ID_COUNT + WATER_DRY as usize + 1], linear_rgba(&r.lo));
    assert!(close(p[ID_COUNT + BANDS - 1], linear_rgba(&r.hi)));
    assert_ne!(
        p[ID_COUNT + WATER_DRY as usize],
        linear_rgba(&r.lo),
        "dry ground is off the ramp"
    );
    assert_eq!(id_name(POND), "standing water");
}

// ---------------------------------------------------------------------------------------------
// Shot S4: the plantable gate is read from the run, not re-derived from a list of names.
//
// The behaviour these pin has not changed -- the run's `params.medium.<name>.plantable` is false for
// exactly the four media V4 hard-coded, and `the_capitol_run_and_the_name_list_agree` measures that
// on a committed fixture rather than asserting it in prose. What has changed is where the answer
// comes from, so the tests that matter are the ones that make the two disagree: a run that calls
// lawn sealed must strip a lawn, and a run that calls asphalt plantable must grow on it. A viewer
// still reading its own list would pass none of them.

use ecoview_native::run::RunMeta;
use ecoview_native::voxel::Plantable;

/// The nine media of the scene contract, in code order, as every bundle publishes them.
fn media() -> Vec<String> {
    [
        "soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// A `RunMeta` carrying just a `params.medium` table, for driving the gate directly.
fn meta_with_media(rows: &str) -> RunMeta {
    let raw = format!(
        r#"{{"format_version":4,"dims":{{"x":8,"y":8,"z":32,"patch":8}},"seed":1,"ticks":100,
            "snapshot_every":100,"snapshots":[0],"params":{{"medium":{rows}}}}}"#
    );
    serde_json::from_str(&raw).unwrap()
}

/// The run decides, and the viewer follows it even where it contradicts the old name list.
///
/// Both halves matter. A run that calls lawn unplantable has to strip the lawn -- so the gate is not
/// the name list with the run ignored. A run that calls asphalt plantable has to grow on asphalt --
/// so the gate is not the name list *and* the run, intersected. It is the run.
#[test]
fn the_run_decides_which_media_grow_things() {
    let n = 64;
    let d = eco(n);
    let f = drivers(d, 0.9, 0.09, 255, 0);
    let count = |medium: u8, meta: Option<&RunMeta>| {
        let mut b = with_building(n, 8.0);
        for y in 0..n {
            for x in 0..n {
                if b.building_h[x + n * y] == 0.0 {
                    b.medium[x + n * y] = medium;
                }
            }
        }
        let mut w = VoxelWorld::from_bundle(&b);
        if let Some(m) = meta {
            w.read_plantable(m);
        }
        w.set_scene(&[], &[], Some(&Cover::of(&f, d, 7)));
        w.cover_counts()
    };
    // A run that seals the lawn. Everything else keeps the value the run gives it.
    let sealed_lawn = meta_with_media(
        r#"{"soil":{"plantable":true},"lawn":{"plantable":false},"bed":{"plantable":true},
            "mulch":{"plantable":true},"gravel":{"plantable":true},"concrete":{"plantable":false},
            "asphalt":{"plantable":true},"roof":{"plantable":false},"water":{"plantable":false}}"#,
    );
    let free = count(1, None);
    assert!(free.0 > 1000, "the fallback grows a lawn: {free:?}");
    assert_eq!(count(1, Some(&sealed_lawn)), (0, 0, 0), "the run sealed it");
    // The same run calls asphalt plantable, and asphalt then grows exactly what the lawn used to.
    assert_eq!(count(6, Some(&sealed_lawn)), free, "the run opened asphalt");

    // And the gate says whose answer it is, with no fallback left in it.
    let mut w = VoxelWorld::from_bundle(&with_building(n, 8.0));
    w.read_plantable(&sealed_lawn);
    assert!(w.plantable.from_meta);
    assert!(
        w.plantable.source.starts_with("the run's meta.json"),
        "{}",
        w.plantable.source
    );
    assert_eq!(w.plantable.sealed(), ["lawn", "concrete", "roof", "water"]);
}

/// A run that names only some of the media is read for those, and the fallback covers the rest --
/// by name, in the source line, rather than quietly.
///
/// Per medium rather than all-or-nothing, because the alternative throws away eight true answers to
/// avoid one guess. `from_meta` is false, which is the flag the HUD and `ecoview.stats` report.
#[test]
fn a_run_that_names_only_some_media_is_still_read_for_those() {
    let partial = meta_with_media(r#"{"lawn":{"plantable":false},"asphalt":{"plantable":true}}"#);
    let p = Plantable::of(&media(), &partial);
    assert!(!p.grows(1), "the run sealed the lawn");
    assert!(p.grows(6), "the run opened the asphalt");
    assert!(!p.grows(7), "the fallback still seals a roof");
    assert!(p.grows(0), "the fallback still grows soil");
    assert!(!p.from_meta);
    assert!(
        p.source
            .contains("soil, bed, mulch, gravel, concrete, roof, water"),
        "the guessed media are named: {}",
        p.source
    );

    // A row with the key absent is a guess; a row with `plantable` present is not. A run written
    // before the field existed must not read as "everything grows".
    let empty_rows = meta_with_media(r#"{"lawn":{},"asphalt":{}}"#);
    let e = Plantable::of(&media(), &empty_rows);
    assert!(!e.from_meta);
    assert!(e.grows(1) && !e.grows(6), "both fell back to the name list");
}

/// A run with no `params.medium` at all does not get to be called the source.
///
/// Every format-1, -2 and -3 run is one of these, and so is any run written before `plantable`
/// existed. The flags they produce are the same as the fallback's -- there is nothing else they
/// could be -- but the sentence beside them is not: "from the run's meta.json" over an answer the
/// run never gave is the one thing this shot exists to stop, and it would be a worse lie here than
/// the hard-coded list was, because it names a file a reader could go and check.
#[test]
fn a_run_with_no_medium_table_is_not_the_source() {
    let none = meta_with_media("{}");
    let p = Plantable::of(&media(), &none);
    assert!(!p.from_meta);
    assert!(
        p.source.starts_with(
            "this viewer's fallback name list, because the run carries no params.medium"
        ),
        "{}",
        p.source
    );
    // Same flags as the fallback, and the same four media sealed.
    let f = Plantable::fallback(&media());
    assert_eq!(p.sealed(), f.sealed());
    for code in 0..media().len() as u8 {
        assert_eq!(p.grows(code), f.grows(code));
    }
}

/// The claim that this shot changes no picture, measured on a committed run rather than asserted.
///
/// `ecosim/fixtures/capitol-mini` is a real format-4 run of the reference bundle, committed, and its
/// `params.medium` seals exactly the four media shot V4 hard-coded. That is why S4 is a
/// correctness-of-source fix and not a bug fix -- and if a later shot changes a default in
/// `params.toml`, this test goes red and the viewer follows the change instead of drifting from it.
#[test]
fn the_capitol_run_and_the_name_list_agree() {
    let dir = std::path::Path::new("../ecosim/fixtures/capitol-mini");
    let run = Run::load(dir).unwrap();
    let from_run = Plantable::of(&media(), &run.meta);
    let from_names = Plantable::fallback(&media());
    assert!(from_run.from_meta, "the fixture names every medium");
    assert_eq!(
        from_run.sealed(),
        from_names.sealed(),
        "the run's plantable set and the V4 name list still agree"
    );
    assert_eq!(
        from_run.sealed(),
        ["concrete", "asphalt", "roof", "water"],
        "and it is the set V4 wrote down"
    );
    for code in 0..media().len() as u8 {
        assert_eq!(from_run.grows(code), from_names.grows(code));
    }
}
