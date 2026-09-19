//! Headless, deterministic voxel ecology simulator (SAD 1).

pub mod abiotic;
pub mod animals;
pub mod check;
pub mod output;
pub mod params;
pub mod producers;
pub mod sim;
pub mod sweep;
pub mod trees;
pub mod world;

pub use params::Params;
pub use sim::{Sim, StatsRow};
