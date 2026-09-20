//! Headless, deterministic voxel ecology simulator (SAD 1).
#![warn(missing_docs)]

pub mod abiotic;
pub mod animals;
pub mod bundle;
pub mod check;
pub mod events;
pub mod fire;
pub mod heredity;
pub mod hydro;
pub mod output;
pub mod params;
pub mod plants;
pub mod producers;
pub mod profile;
pub mod sim;
pub mod state;
pub mod sweep;
pub mod trees;
pub mod world;

pub use params::Params;
pub use sim::{Sim, StatsRow};

/// Proptest case count for a property: `n` normally, a quarter of it (at least 2) under
/// `cargo llvm-cov`, whose instrumentation makes each case several times slower.
#[cfg(test)]
pub(crate) const fn cases(n: u32) -> u32 {
    if cfg!(coverage) {
        if n / 4 > 2 {
            n / 4
        } else {
            2
        }
    } else {
        n
    }
}
