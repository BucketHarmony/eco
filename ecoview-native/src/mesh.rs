//! Greedy meshing: one padded chunk buffer in, one triangle mesh out.
//!
//! This module never mentions Bevy. It is the only thing CI gates (V0-spike.md, CI item 7a): a fixed
//! chunk in, a hashed vertex and index buffer out, on a runner with no GPU.

use crate::voxel::{ChunkPos, VoxelWorld, CS, CS_P3};
use binary_greedy_meshing::{Face, Mesher, Quad};

/// A chunk's mesh in the viewer's own frame: X east, Y up, Z north, metres from the world origin.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ChunkMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl ChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// FNV-1a over every buffer, as the golden value. Floats are hashed by their bits, which is exact:
    /// the mesher's coordinates are small integers scaled by the cell size, so there is no rounding to
    /// disagree about between platforms.
    pub fn hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h ^= x as u64;
                h = h.wrapping_mul(0x1000_0000_01b3);
            }
        };
        for p in &self.positions {
            for v in p {
                eat(&v.to_bits().to_le_bytes());
            }
        }
        for n in &self.normals {
            for v in n {
                eat(&v.to_bits().to_le_bytes());
            }
        }
        for c in &self.colors {
            for v in c {
                eat(&v.to_bits().to_le_bytes());
            }
        }
        for i in &self.indices {
            eat(&i.to_le_bytes());
        }
        h
    }
}

/// Reusable scratch: the padded voxel buffer, the two masks and the mesher's own internal buffers.
/// One per worker thread; allocating these per chunk would dominate the meshing time.
pub struct Scratch {
    pub voxels: Vec<u16>,
    mesher: Mesher<CS>,
    opaque: Vec<u64>,
    transparent: Vec<u64>,
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch {
            voxels: vec![0u16; CS_P3],
            mesher: Mesher::<CS>::new(),
            opaque: vec![0u64; Mesher::<CS>::CS_P2],
            transparent: vec![0u64; Mesher::<CS>::CS_P2],
        }
    }
}

/// Fills and meshes one chunk. `palette` gives a colour per voxel id; an id past its end is drawn as
/// its last entry rather than panicking, which is what the surface-only palette does with an overlay
/// band it was not built for.
pub fn mesh_chunk(
    world: &VoxelWorld,
    c: ChunkPos,
    palette: &[[f32; 4]],
    s: &mut Scratch,
) -> ChunkMesh {
    world.fill_chunk(c, &mut s.voxels);
    let Scratch {
        voxels,
        mesher,
        opaque,
        transparent,
    } = s;
    let cell_m = world.cell_m;
    // Every voxel here is opaque: V0 has one overlay and no water or glass (V0-spike.md, constraints).
    opaque.fill(0);
    transparent.fill(0);
    for (i, v) in voxels.iter().enumerate() {
        if *v != 0 {
            opaque[i / crate::voxel::CS_P] |= 1 << (i % crate::voxel::CS_P);
        }
    }
    mesher.clear();
    mesher.fast_mesh(voxels, opaque, transparent);

    let origin = [
        (c.x * CS) as f32 * cell_m,
        (c.z * CS) as f32 * cell_m,
        (c.y * CS) as f32 * cell_m,
    ];
    let mut m = ChunkMesh::default();
    for face in 0..6u8 {
        let f = Face::from(face);
        let n = f.n();
        let normal = [n[0] as f32, n[1] as f32, n[2] as f32];
        let quads: Vec<Quad> = mesher.quads[face as usize].clone();
        for quad in quads {
            let color = palette[(quad.voxel_id() as usize).min(palette.len() - 1)];
            let base = m.positions.len() as u32;
            for v in f.vertices_packed(quad) {
                let p = v.xyz();
                m.positions.push([
                    origin[0] + p[0] as f32 * cell_m,
                    origin[1] + p[1] as f32 * cell_m,
                    origin[2] + p[2] as f32 * cell_m,
                ]);
                m.normals.push(normal);
                m.colors.push(color);
            }
            m.indices
                .extend_from_slice(&[base + 2, base, base + 1, base + 1, base + 3, base + 2]);
        }
    }
    m
}
