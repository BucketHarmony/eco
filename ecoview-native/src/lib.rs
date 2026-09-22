//! `ecoview-native`: the V0 spike. A native viewer for an ecosim **world bundle**, read from disk.
//!
//! The crate splits in two on purpose. Everything here -- the bundle loader, the voxel world, the
//! mesher and the edit actions -- is plain Rust with no engine in it, so the mesh golden test that
//! gates CI builds with `--no-default-features` and never compiles Bevy or touches a GPU. The viewer
//! itself is `src/main.rs`, behind the default `viewer` feature.
//!
//! This crate does not depend on `ecosim` and never will inside this shot: the run directory and the
//! world bundle on disk are the only interface between the two projects (CLAUDE.md).

pub mod brp;
pub mod bundle;
pub mod cover;
pub mod mesh;
pub mod overlay;
pub mod palette;
pub mod run;
pub mod sim;
pub mod sky;
pub mod tree;
pub mod voxel;

pub use bundle::Bundle;
pub use cover::Cover;
pub use mesh::{mesh_chunk, ChunkMesh, Scratch};
pub use overlay::{Fields, Scale};
pub use palette::Overlay;
pub use run::Run;
pub use sim::{SimJob, SimState};
pub use sky::{Clock, DaySource, Season, SkyState, Sun};
pub use tree::{Life, TreeForm};
pub use voxel::{ChunkPos, EditAction, VoxelWorld};

/// One ecology column is one metre across, whatever the bundle's ground cell is (CLAUDE.md: "the
/// ecology grid stays at 1 m columns, the ground grid is finer"). The overlays need this to put a
/// column's colour on the ground cells it covers.
pub const ECO_CELL_M: f32 = 1.0;

/// The committed reference bundle, relative to the repo root.
pub const CAPITOL: &str = "../ecosim/worlds/capitol";

/// The synthetic stress world V0 measures against: 512 x 512 cells at **0.25 m**, not the Capitol's
/// 0.5, so a hard-coded cell size fails loudly (V0-spike.md, correction 4).
pub fn stress_world() -> Bundle {
    Bundle::stress(512, 0.25, 200, 20260920)
}
