//! Greedy meshing: one padded chunk buffer in, one triangle mesh out.
//!
//! This module never mentions Bevy. It is the only thing CI gates (V0-spike.md, CI item 7a): a fixed
//! chunk in, a hashed vertex and index buffer out, on a runner with no GPU.

use crate::palette::PALETTE_LEN;
use crate::sky::AO_LEVELS;
use crate::voxel::{ChunkPos, VoxelWorld, AIR, CS, CS_P, CS_P3};
use binary_greedy_meshing::{Face, Mesher, Quad};

/// Which occlusion level each count of solid neighbours above a voxel falls in, indexed by that
/// count, 0 through 8.
///
/// **What "occluded" means here, exactly**: the eight cells in the layer *directly above* the voxel,
/// and nothing else. Not the coplanar ring -- on flat ground every voxel has eight coplanar
/// neighbours, so counting those would dim a lawn uniformly and dim nothing relative to anything
/// else. Not the layer below, which is solid under every ground voxel there is. The layer above is
/// the one a face looks into, so this darkens exactly the concave places: the foot of a wall, the
/// inside of a step, and the inside of a crown, where a leaf has leaves over it.
///
/// One voxel carries one level for all six of its faces, which is coarser than the per-corner
/// ambient occlusion a mesher that owned its own merge key could do. The merge key here is the
/// voxel id (`binary-greedy-meshing` merges on equal ids), so per-corner values would have to
/// break every merge, and the quad count is already what this costs (DECISIONS.md, V6).
const OCCLUSION_LEVEL: [u16; 9] = [0, 0, 1, 1, 2, 2, 3, 3, 3];

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
    if world.ao {
        bake_occlusion(&mut s.voxels);
    }
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

    // The lattice's own datum comes back out here, so world space stays in the bundle's frame
    // whatever the editor has dug (shot S6, `VoxelWorld::open_dig_room`). It is a whole number of
    // cells, so the coordinates are still exact multiples of the cell size and the goldens below
    // still hash the same bits -- and for a world nobody has dug it is 0 and this line is nothing.
    let origin = [
        (c.x * CS) as f32 * cell_m,
        (c.z * CS) as f32 * cell_m - world.datum_m(),
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

/// Rewrites every interior voxel's id to carry its occlusion level: `id + PALETTE_LEN * level`.
///
/// The padded buffer is exactly the right shape for this. Every interior voxel's eight neighbours
/// in the layer above lie inside the pad, including the ones that belong to the chunk next door, so
/// the level a voxel gets does not depend on which chunk it was meshed in and no seam appears along
/// a chunk boundary. A one-voxel pad is also the whole reach: this measures contact, not a horizon.
///
/// Level 0 leaves the id alone, which is what makes an unoccluded voxel's colour -- and therefore a
/// flat, open site's whole mesh -- identical with ambient occlusion on and off.
///
/// Public so the gate can bake a chunk and read the levels back off it; [`mesh_chunk`] calls it for
/// every chunk of a world whose `ao` is on.
pub fn bake_occlusion(buf: &mut [u16]) {
    const UP: isize = (CS_P * CS_P) as isize;
    const EAST: isize = CS_P as isize;
    const NORTH: isize = 1;
    const RING: [isize; 8] = [
        UP - EAST - NORTH,
        UP - EAST,
        UP - EAST + NORTH,
        UP - NORTH,
        UP + NORTH,
        UP + EAST - NORTH,
        UP + EAST,
        UP + EAST + NORTH,
    ];
    let block = PALETTE_LEN as u16;
    for pz in 1..=CS {
        for px in 1..=CS {
            for py in 1..=CS {
                let i = py + px * CS_P + pz * CS_P * CS_P;
                if buf[i] == AIR {
                    continue;
                }
                let mut solid = 0usize;
                for o in RING {
                    // Rewriting as we go is safe: a level never turns a voxel into air, and this
                    // only ever asks whether a neighbour is air.
                    if buf[(i as isize + o) as usize] != AIR {
                        solid += 1;
                    }
                }
                let level = OCCLUSION_LEVEL[solid];
                debug_assert!((level as usize) < AO_LEVELS);
                buf[i] += block * level;
            }
        }
    }
}
