//! The CI gate (V0-spike.md, item 7a): a fixed chunk in, a hashed vertex and index buffer out.
//!
//! Greedy meshing is a pure function of the chunk buffer, so this needs no GPU, no window and no
//! engine -- CI runs it with `--no-default-features`, which does not compile Bevy at all. Three
//! chunks: flat ground, a staircase, and one building block.

use ecoview_native::bundle::{Bundle, Tree};
use ecoview_native::mesh::{mesh_chunk, Scratch};
use ecoview_native::palette::surface_palette;
use ecoview_native::voxel::{ChunkPos, VoxelWorld};

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
    // The column's centre, in metres, not its south-west corner.
    assert_eq!((snap.trees[0].x, snap.trees[0].y), (2.5, 3.5));
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

/// The three stages are three different sizes, biggest last, and a sapling has no crown.
#[test]
fn the_three_stages_are_three_sizes() {
    let (sh, _, sr) = Stage::Sapling.shape();
    let (yh, _, yr) = Stage::Young.shape();
    let (mh, _, mr) = Stage::Mature.shape();
    assert!(sh < yh && yh < mh, "{sh} {yh} {mh}");
    assert_eq!(sr, 0.0, "a sapling is a bare trunk");
    assert!(yr < mr);
    assert_eq!(Stage::parse("mature"), Some(Stage::Mature));
    assert_eq!(Stage::parse("seedling"), None);
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

    let tree = Tree {
        x: 8.5,
        y: 8.5,
        height: 3.0,
        crown_radius: 1.5,
        crown_base: 1.0,
    };
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

/// A run directory carrying every field an overlay reads, on an `n` x `n` ecology grid with
/// `patch` x `patch` patches.
///
/// The fields are deliberately non-uniform, because a test on a constant field cannot tell a correct
/// reader from one that returns the first byte: `moisture[c] = 4c`, fertility its complement, and
/// light bright everywhere except column 0.
///
/// `with_params` writes the `params` block the scales are read from. Without it the run is one this
/// viewer has to fall back on its own constants for, which is the other half of what is tested.
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
    std::fs::write(
        dir.join("meta.json"),
        format!(
            r##"{{"format_version":4,"dims":{{"x":{n},"y":{n},"z":{z},"patch":{patch}}},
               "seed":42,"ticks":100,"snapshot_every":100,"snapshots":[0,100],
               "world":{{"name":"test","bundle":true,"ground_cell_m":0.5,
                         "ground_width":{},"ground_depth":{}}},
               "species":[{{"id":0,"name":"grass","kind":"cover","color":"#7cc242"}},
                          {{"id":2,"name":"tree","kind":"tree","color":"#112233",
                            "canopy_color":"#445566"}}]{params}}}"##,
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
    for o in Overlay::ALL.into_iter().filter(|o| o.is_field()) {
        let s = Scale::of(o, m);
        assert!(s.from_meta(), "{}: {}", o.name(), s.source);
    }

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
    let stale = w.set_overlay(Some(&ColumnBands { x: 8, y: 8, bands }));
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
    w.set_overlay(Some(&ColumnBands { x: 8, y: 8, bands }));
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
