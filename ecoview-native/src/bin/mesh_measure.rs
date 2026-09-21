//! `mesh_measure [capitol|stress]`: the mesh numbers for MEASUREMENTS.md, with no engine and no GPU.
//!
//! Reports full-world mesh time single-threaded and across the machine's cores, the single-edit
//! remesh time, and the triangle count, so the frame-rate gate can be read against the mesh path.

use std::time::Instant;

use ecoview_native::mesh::{mesh_chunk, Scratch};
use ecoview_native::palette::surface_palette;
use ecoview_native::voxel::{EditAction, VoxelWorld};
use ecoview_native::{stress_world, Bundle, CAPITOL};

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "capitol".into());
    let t = Instant::now();
    let bundle = if which == "stress" {
        stress_world()
    } else {
        Bundle::load(std::path::Path::new(CAPITOL)).expect("read the committed Capitol bundle")
    };
    let read_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let mut world = VoxelWorld::from_bundle(&bundle);
    let voxelise_ms = t.elapsed().as_secs_f64() * 1000.0;
    let palette = surface_palette();
    let chunks = world.all_chunks();

    let t = Instant::now();
    let mut scratch = Scratch::new();
    let mut tris = 0usize;
    let mut drawn = 0usize;
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for &c in &chunks {
        let m = mesh_chunk(&world, c, &palette, &mut scratch);
        tris += m.indices.len() / 3;
        drawn += usize::from(!m.is_empty());
        for p in &m.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    let serial_ms = t.elapsed().as_secs_f64() * 1000.0;

    let threads = std::thread::available_parallelism().map_or(8, |n| n.get());
    let t = Instant::now();
    std::thread::scope(|s| {
        for part in chunks.chunks(chunks.len().div_ceil(threads)) {
            let (w, p) = (&world, &palette);
            s.spawn(move || {
                let mut scratch = Scratch::new();
                for &c in part {
                    std::hint::black_box(mesh_chunk(w, c, p, &mut scratch));
                }
            });
        }
    });
    let parallel_ms = t.elapsed().as_secs_f64() * 1000.0;

    // One edit in the middle of the site, remeshing only the chunks it touches: the gate's < 10 ms.
    let (cx, cy) = (world.width / 2, world.depth / 2);
    let mut remesh = Vec::new();
    for i in 0..20 {
        let action = if i % 2 == 0 {
            EditAction::RaiseGround
        } else {
            EditAction::LowerGround
        };
        let t = Instant::now();
        let stale = world.apply(cx + i, cy, action);
        let n = stale.len();
        for c in stale {
            std::hint::black_box(mesh_chunk(&world, c, &palette, &mut scratch));
        }
        remesh.push((t.elapsed().as_secs_f64() * 1000.0, n));
    }
    remesh.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let median = remesh[remesh.len() / 2];

    println!(
        "world:            {} {}x{} cells at {} m",
        bundle.name, bundle.width, bundle.depth, bundle.ground_cell_m
    );
    println!(
        "chunks:           {} total, {drawn} with geometry, {tris} triangles",
        chunks.len()
    );
    println!(
        "bounds:           x {:.1}..{:.1}, y {:.1}..{:.1}, z {:.1}..{:.1} m",
        lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
    );
    println!("read:             {read_ms:.0} ms");
    println!("voxelise:         {voxelise_ms:.0} ms");
    println!("mesh 1 thread:    {serial_ms:.0} ms");
    println!("mesh {threads} threads:  {parallel_ms:.0} ms");
    println!(
        "single edit:      median {:.2} ms over 20 edits ({} chunks each), worst {:.2} ms",
        median.0,
        median.1,
        remesh.last().unwrap().0
    );
}
