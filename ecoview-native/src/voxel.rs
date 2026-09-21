//! The voxel world: columns in, padded chunk buffers out.
//!
//! The bundle is a heightfield -- one `ground_h`, one `medium`, one `building_h` per cell -- so the
//! voxels are generated on demand from the columns rather than stored as a 3D array. A 512x512x64
//! world would be 16.7 M voxels; the columns are 0.26 M. An edit changes a column and the chunks that
//! read it are re-filled and re-meshed.

use crate::bundle::{Bundle, Shrub, Tree};

/// The mesher's chunk size. 62 voxels padded to 64 is the layout `binary-greedy-meshing` requires.
pub const CS: usize = 62;
/// Padded edge.
pub const CS_P: usize = CS + 2;
/// Padded volume, in voxels.
pub const CS_P3: usize = CS_P * CS_P * CS_P;

/// Voxel ids. 0 is air. 1..=9 are the scene contract's media, in `bundle.media` order, plus one.
pub const AIR: u16 = 0;
pub const SOIL: u16 = 1;
pub const BUILDING: u16 = 10;
pub const TRUNK: u16 = 11;
pub const CANOPY: u16 = 12;
/// One past the last id, for palette sizing.
pub const ID_COUNT: usize = 13;

/// Quantise a height to a lattice level, the way `ecoview` does at draw time
/// (ecoview/DECISIONS.md, "E3 block world"): the data stays continuous, only the display is a lattice.
#[inline]
pub fn level_of(h: f32, cell_m: f32) -> i32 {
    (h / cell_m + 1e-6).floor() as i32
}

/// A single edit, addressed by ground cell. These are the five actions V0 exposes over BRP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    RaiseGround,
    LowerGround,
    SetSurface(u8),
    RaiseBuilding,
    LowerBuilding,
}

impl EditAction {
    /// The BRP wire names. An unknown name is rejected rather than guessed at.
    pub fn parse(name: &str, medium: Option<u8>) -> Option<EditAction> {
        match name {
            "RaiseGround" => Some(EditAction::RaiseGround),
            "LowerGround" => Some(EditAction::LowerGround),
            "RaiseBuilding" => Some(EditAction::RaiseBuilding),
            "LowerBuilding" => Some(EditAction::LowerBuilding),
            "SetSurface" => medium.map(EditAction::SetSurface),
            _ => None,
        }
    }
}

/// A chunk's place in the world grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkPos {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

/// Columns plus the chunk grid over them.
pub struct VoxelWorld {
    pub cell_m: f32,
    pub width: usize,
    pub depth: usize,
    pub ground_h: Vec<f32>,
    pub medium: Vec<u8>,
    pub building_h: Vec<f32>,
    /// Plant voxels, bucketed by chunk so a fill never scans every tree: `(local x, level, local y, id)`.
    plants: Vec<Vec<(u8, u8, u8, u16)>>,
    pub chunks: ChunkPos,
    pub levels: usize,
}

impl VoxelWorld {
    /// Builds the chunk grid and voxelises the bundle's trees and shrubs into it.
    pub fn from_bundle(b: &Bundle) -> VoxelWorld {
        let cell_m = b.ground_cell_m;
        let top = b
            .ground_h
            .iter()
            .zip(b.building_h.iter())
            .map(|(g, bh)| g + bh)
            .fold(0.0f32, f32::max);
        let tallest = b
            .trees
            .iter()
            .map(|t| t.height)
            .chain(b.shrubs.iter().map(|s| s.height))
            .fold(0.0f32, f32::max);
        let levels = (level_of(top + tallest, cell_m) + 2).max(1) as usize;
        let chunks = ChunkPos {
            x: b.width.div_ceil(CS),
            y: b.depth.div_ceil(CS),
            z: levels.div_ceil(CS),
        };
        let mut w = VoxelWorld {
            cell_m,
            width: b.width,
            depth: b.depth,
            ground_h: b.ground_h.clone(),
            medium: b.medium.clone(),
            building_h: b.building_h.clone(),
            plants: vec![Vec::new(); chunks.x * chunks.y * chunks.z],
            chunks,
            levels,
        };
        w.voxelise_plants(&b.trees, &b.shrubs);
        w
    }

    /// Turns trees and shrubs into plant voxels, into whatever the buckets already hold.
    fn voxelise_plants(&mut self, trees: &[Tree], shrubs: &[Shrub]) {
        let w = self;
        let cell_m = w.cell_m;
        for t in trees {
            let cx = (t.x / cell_m) as i32;
            let cy = (t.y / cell_m) as i32;
            let base = w.ground_level(cx, cy) + 1;
            let top = base + level_of(t.height.max(0.0), cell_m);
            let crown_lo = base + level_of(t.crown_base.max(0.0), cell_m);
            let r = (t.crown_radius / cell_m).max(0.5);
            for z in base..=top {
                w.set_plant(cx, cy, z, TRUNK);
            }
            // The same trunk-and-blob solid `ecoview` draws: an ellipsoid from the crown base to the
            // tip. A zero crown radius means no crown at all, which is how a sapling from a run
            // directory is a bare trunk (run.rs, `Stage::shape`).
            if t.crown_radius > 0.0 {
                let mid = (crown_lo + top) as f32 * 0.5;
                let half = ((top - crown_lo) as f32 * 0.5).max(1.0);
                let ri = r.ceil() as i32;
                for z in crown_lo..=top {
                    for dy in -ri..=ri {
                        for dx in -ri..=ri {
                            let fz = (z as f32 - mid) / half;
                            let fr = (((dx * dx + dy * dy) as f32).sqrt()) / r;
                            if fr * fr + fz * fz <= 1.0 {
                                w.set_plant(cx + dx, cy + dy, z, CANOPY);
                            }
                        }
                    }
                }
            }
        }
        for s in shrubs {
            let cx = (s.x / cell_m) as i32;
            let cy = (s.y / cell_m) as i32;
            let base = w.ground_level(cx, cy) + 1;
            let top = base + level_of(s.height.max(0.0), cell_m);
            let (rx, ry) = ((s.rx / cell_m).max(0.5), (s.ry / cell_m).max(0.5));
            let (ca, sa) = (s.angle.cos(), s.angle.sin());
            let ri = rx.max(ry).ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (fx, fy) = (dx as f32, dy as f32);
                    let u = (fx * ca + fy * sa) / rx;
                    let v = (-fx * sa + fy * ca) / ry;
                    if u * u + v * v <= 1.0 {
                        for z in base..=top {
                            w.set_plant(cx + dx, cy + dy, z, CANOPY);
                        }
                    }
                }
            }
        }
        for bucket in &mut w.plants {
            bucket.sort_unstable();
            bucket.dedup();
        }
    }

    /// Replaces every plant voxel in the world with the ones these trees and shrubs make, and returns
    /// the chunks whose mesh is now stale.
    ///
    /// This is what a snapshot change costs: the ground is untouched -- a run never moves it -- so
    /// only the buckets that actually differ, and the neighbours that see them through the one-voxel
    /// pad, are remeshed. A site whose trees all sit near the ground therefore remeshes the ground
    /// chunk layer and nothing above it.
    pub fn set_plants(&mut self, trees: &[Tree], shrubs: &[Shrub]) -> Vec<ChunkPos> {
        let empty = vec![Vec::new(); self.chunk_count()];
        let before = std::mem::replace(&mut self.plants, empty);
        self.voxelise_plants(trees, shrubs);
        let mut stale = vec![false; self.chunk_count()];
        let changed: Vec<usize> = before
            .iter()
            .zip(self.plants.iter())
            .enumerate()
            .filter(|(_, (was, is))| was != is)
            .map(|(i, _)| i)
            .collect();
        for i in changed {
            // A bucket's voxels reach one cell into each of the 26 neighbouring chunks, so their
            // meshes are stale too (`fill_chunk` reads the neighbours' buckets into its pad).
            let c = self.chunk_pos(i);
            for dz in -1..=1i32 {
                for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        let (nx, ny, nz) = (c.x as i32 + dx, c.y as i32 + dy, c.z as i32 + dz);
                        if nx < 0 || ny < 0 || nz < 0 {
                            continue;
                        }
                        let n = ChunkPos {
                            x: nx as usize,
                            y: ny as usize,
                            z: nz as usize,
                        };
                        if n.x < self.chunks.x && n.y < self.chunks.y && n.z < self.chunks.z {
                            stale[self.chunk_index(n)] = true;
                        }
                    }
                }
            }
        }
        (0..self.chunk_count())
            .filter(|&i| stale[i])
            .map(|i| self.chunk_pos(i))
            .collect()
    }

    /// The inverse of [`VoxelWorld::chunk_index`].
    #[inline]
    pub fn chunk_pos(&self, i: usize) -> ChunkPos {
        ChunkPos {
            x: i % self.chunks.x,
            y: (i / self.chunks.x) % self.chunks.y,
            z: i / (self.chunks.x * self.chunks.y),
        }
    }

    fn set_plant(&mut self, x: i32, y: i32, z: i32, id: u16) {
        if x < 0 || y < 0 || z < 0 || x as usize >= self.width || y as usize >= self.depth {
            return;
        }
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if z >= self.levels {
            return;
        }
        let c = self.chunk_index(ChunkPos {
            x: x / CS,
            y: y / CS,
            z: z / CS,
        });
        self.plants[c].push(((x % CS) as u8, (z % CS) as u8, (y % CS) as u8, id));
    }

    #[inline]
    pub fn chunk_index(&self, c: ChunkPos) -> usize {
        c.x + self.chunks.x * (c.y + self.chunks.y * c.z)
    }

    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.chunks.x * self.chunks.y * self.chunks.z
    }

    /// Every chunk of the world, in index order.
    pub fn all_chunks(&self) -> Vec<ChunkPos> {
        let mut v = Vec::with_capacity(self.chunk_count());
        for z in 0..self.chunks.z {
            for y in 0..self.chunks.y {
                for x in 0..self.chunks.x {
                    v.push(ChunkPos { x, y, z });
                }
            }
        }
        v
    }

    /// The top solid level of a ground column. Off the grid the world ends, so the caller sees air.
    #[inline]
    pub fn ground_level(&self, x: i32, y: i32) -> i32 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.depth {
            return -1;
        }
        level_of(
            self.ground_h[x as usize + self.width * y as usize],
            self.cell_m,
        )
    }

    /// The voxel at a world lattice position; air outside the world.
    #[inline]
    pub fn voxel(&self, x: i32, y: i32, z: i32) -> u16 {
        if x < 0 || y < 0 || z < 0 || x as usize >= self.width || y as usize >= self.depth {
            return AIR;
        }
        let i = x as usize + self.width * y as usize;
        let g = level_of(self.ground_h[i], self.cell_m);
        if z > g {
            let bh = self.building_h[i];
            if bh > 0.0 && z <= level_of(self.ground_h[i] + bh, self.cell_m) {
                return BUILDING;
            }
            return AIR;
        }
        // Only the top voxel carries the surface medium; everything under it is soil, so digging
        // exposes soil instead of painting the cut face with lawn (ecoview/DECISIONS.md, E3).
        if z == g {
            return self.medium[i] as u16 + 1;
        }
        SOIL
    }

    /// Fills the mesher's padded 64^3 buffer for one chunk. The buffer's axes are the mesher's own:
    /// stride 1 is north, stride CS_P is east, stride CS_P^2 is up.
    pub fn fill_chunk(&self, c: ChunkPos, buf: &mut [u16]) {
        debug_assert_eq!(buf.len(), CS_P3);
        buf.fill(AIR);
        let (ox, oy, oz) = ((c.x * CS) as i32, (c.y * CS) as i32, (c.z * CS) as i32);
        for pz in 0..CS_P as i32 {
            let wz = oz + pz - 1;
            for px in 0..CS_P as i32 {
                let wx = ox + px - 1;
                for py in 0..CS_P as i32 {
                    let v = self.voxel(wx, oy + py - 1, wz);
                    if v != AIR {
                        buf[(py as usize) + (px as usize) * CS_P + (pz as usize) * CS_P * CS_P] = v;
                    }
                }
            }
        }
        // Plants sit on top of the columns, including in the one-voxel pad from the 26 neighbours.
        for dz in -1..=1i32 {
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (nx, ny, nz) = (c.x as i32 + dx, c.y as i32 + dy, c.z as i32 + dz);
                    if nx < 0 || ny < 0 || nz < 0 {
                        continue;
                    }
                    let n = ChunkPos {
                        x: nx as usize,
                        y: ny as usize,
                        z: nz as usize,
                    };
                    if n.x >= self.chunks.x || n.y >= self.chunks.y || n.z >= self.chunks.z {
                        continue;
                    }
                    for &(lx, lz, ly, id) in &self.plants[self.chunk_index(n)] {
                        let px = lx as i32 + dx * CS as i32 + 1;
                        let py = ly as i32 + dy * CS as i32 + 1;
                        let pz = lz as i32 + dz * CS as i32 + 1;
                        if (0..CS_P as i32).contains(&px)
                            && (0..CS_P as i32).contains(&py)
                            && (0..CS_P as i32).contains(&pz)
                        {
                            let i =
                                (py as usize) + (px as usize) * CS_P + (pz as usize) * CS_P * CS_P;
                            if buf[i] == AIR {
                                buf[i] = id;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Applies one edit and returns the chunks whose meshes are now stale. A column on a chunk seam
    /// also appears in its neighbour's one-voxel pad, so the neighbour is stale too.
    pub fn apply(&mut self, x: usize, y: usize, action: EditAction) -> Vec<ChunkPos> {
        if x >= self.width || y >= self.depth {
            return Vec::new();
        }
        let i = x + self.width * y;
        let before = self.column_span(i);
        match action {
            EditAction::RaiseGround => self.ground_h[i] += self.cell_m,
            EditAction::LowerGround => self.ground_h[i] = (self.ground_h[i] - self.cell_m).max(0.0),
            EditAction::SetSurface(m) => self.medium[i] = m,
            EditAction::RaiseBuilding => self.building_h[i] += self.cell_m,
            EditAction::LowerBuilding => {
                self.building_h[i] = (self.building_h[i] - self.cell_m).max(0.0)
            }
        }
        let after = self.column_span(i);
        // Only the levels between the old and new ground and tops changed; below them it is still
        // soil, which no edit rewrites. Lowering ground under a building moves the building's base
        // too, so the span runs from the lower of the two ground levels to the higher of the two tops.
        let lo = (before.0.min(after.0).max(0) as usize).min(self.levels - 1);
        let hi = (before.1.max(after.1).max(0) as usize).min(self.levels - 1);
        let mut out = Vec::new();
        for cz in (lo / CS)..=(hi / CS) {
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let nx = (x as i32 + dx).clamp(0, self.width as i32 - 1) as usize / CS;
                    let ny = (y as i32 + dy).clamp(0, self.depth as i32 - 1) as usize / CS;
                    let p = ChunkPos {
                        x: nx,
                        y: ny,
                        z: cz,
                    };
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
        out
    }

    /// A column's ground level and its highest solid level, used to find the chunks an edit touches.
    fn column_span(&self, i: usize) -> (i32, i32) {
        let g = level_of(self.ground_h[i], self.cell_m);
        if self.building_h[i] > 0.0 {
            (
                g,
                level_of(self.ground_h[i] + self.building_h[i], self.cell_m).max(g),
            )
        } else {
            (g, g)
        }
    }
}
