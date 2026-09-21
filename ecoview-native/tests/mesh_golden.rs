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
