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
        media: ["soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water"]
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
    assert!(with.1 > bare.1 + 100, "tree added {} vertices", with.1 - bare.1);
}
