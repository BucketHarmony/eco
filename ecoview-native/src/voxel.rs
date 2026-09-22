//! The voxel world: columns in, padded chunk buffers out.
//!
//! The bundle is a heightfield -- one `ground_h`, one `medium`, one `building_h` per cell -- so the
//! voxels are generated on demand from the columns rather than stored as a 3D array. A 512x512x64
//! world would be 16.7 M voxels; the columns are 0.26 M. An edit changes a column and the chunks that
//! read it are re-filled and re-meshed.

use crate::bundle::{Bundle, Shrub};
use crate::cover::{CellPlant, Cover, SHRUB_HEIGHT_M};
use crate::overlay::PondLevels;
use crate::palette::{BAND_BASE, POND};
use crate::run::RunMeta;
use crate::tree::TreeForm;
use crate::ECO_CELL_M;

/// The mesher's chunk size. 62 voxels padded to 64 is the layout `binary-greedy-meshing` requires.
pub const CS: usize = 62;
/// Padded edge.
pub const CS_P: usize = CS + 2;
/// Padded volume, in voxels.
pub const CS_P3: usize = CS_P * CS_P * CS_P;

/// Voxel ids. 0 is air. 1..=9 are the scene contract's media, in `bundle.media` order, plus one.
///
/// The plant ids from `TRUNK` up are in **precedence order**: where two plants want the same voxel
/// the lower id takes it, because `fill_chunk` writes the first of a bucket's sorted entries and the
/// buckets sort on `(x, z, y, id)`. So wood shows through leaves, leaves through a vine, and a vine
/// through the ground cover it is rooted in.
pub const AIR: u16 = 0;
pub const SOIL: u16 = 1;
pub const BUILDING: u16 = 10;
pub const TRUNK: u16 = 11;
pub const CANOPY: u16 = 12;
/// A climber on a wall. Expression, not simulation: see [`crate::cover`].
pub const VINE: u16 = 13;
/// Ground cover, drawn as fine voxel texture rather than as entities: the simulator has neither a
/// shrub nor a grass entity, only a fraction per 8 m patch.
pub const SHRUB: u16 = 14;
pub const GRASS: u16 = 15;
/// One past the last id, for palette sizing.
pub const ID_COUNT: usize = 16;

/// How many levels of room [`VoxelWorld::open_dig_room`] opens under the site at a time.
///
/// Opening room is a full remesh, so one level per stroke would remesh the world twelve times over a
/// pond; a block of levels makes it once or twice. Eight levels is 4 m on the reference site's 0.5 m
/// lattice, so the whole dig limit is two lifts and never three.
pub const DIG_ROOM_LEVELS: usize = 8;

/// How far below the bundle's zero the viewer digs before a run has told it otherwise, in metres.
///
/// This is `ecosim`'s `[bundle] base_z` default: the layers of soil it puts under the bundle's lowest
/// ground. A run publishes its own in `meta.json` and [`VoxelWorld::read_dig_limit`] adopts it; this
/// is what a site with no run loaded uses, and it is a fallback in exactly the sense shot S4's
/// plantable gate is one -- the simulator owns the number and the viewer only has a guess at it until
/// the simulator has spoken.
pub const DEFAULT_DIG_LIMIT_M: f32 = 8.0;

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
    /// The overlay band of each **ground cell**, or `None` for the surface-type palette.
    ///
    /// An overlay is a set of voxel ids rather than a colour per column, because the mesher merges
    /// faces that share an id: a per-column colour would leave greedy meshing nothing to merge
    /// (palette.rs, `BANDS`). The bands are resampled from the run's 1 m ecology columns onto the
    /// bundle's finer ground grid once, when the overlay or the snapshot changes.
    bands: Option<Vec<u8>>,
    /// How many voxels of standing water sit on each **ground cell**, or `None` for a dry site.
    ///
    /// Shot S5. The run's `water.bin` is on this same grid -- it is the grid the water ran over --
    /// so unlike [`VoxelWorld::bands`] nothing is resampled on the way in. `None` and an all-zero
    /// grid draw the same site; they differ only in what the HUD says about why.
    ponds: Option<Vec<u8>>,
    /// Can ground cover root in each medium code? See [`Plantable`], which says where it came from.
    pub plantable: Plantable,
    /// Bake ambient occlusion into the voxel ids when this world is meshed (shot V6)?
    ///
    /// **Off by default, and the viewer turns it on.** A `VoxelWorld` built by the library alone
    /// therefore meshes to exactly the bytes V5 produced, which is what lets the five golden hashes
    /// from V0 through V4 stand unchanged and prove this shot added a layer over them rather than
    /// moving anything underneath (`mesh::bake_occlusion`).
    pub ao: bool,
    pub chunks: ChunkPos,
    pub levels: usize,
    /// How far the lattice floor has been pushed **below the bundle's own zero**, in metres (S6).
    ///
    /// `ground_h` is in the *lattice* frame, where 0 is the bottom of the voxel grid and nothing can
    /// be negative. A bundle's heights are relative to its own lowest point, so on a low-relief site
    /// that floor is a few centimetres under the lawn and `LowerGround` runs out of ground almost at
    /// once. [`VoxelWorld::open_dig_room`] moves the floor down instead of clamping: every column
    /// rises by the same amount, the lattice gains that many levels, and this records the shift so
    /// every number the world reports outside itself is still in the bundle's frame.
    ///
    /// It is 0 for a world that has not been dug, which is what keeps the mesh goldens from V0 on
    /// standing: `from_bundle` never opens room, only an edit does.
    datum_m: f32,
    /// How far below the bundle's zero this world will go before an edit is refused, in metres.
    ///
    /// **The simulator's number, not the viewer's.** `ecosim` puts `[bundle] base_z` layers of soil
    /// under the bundle's lowest ground and a column's surface layer is `base_z + round(its mean
    /// height)`, so a hole deeper than `base_z` metres is one the exported world cannot represent.
    /// [`VoxelWorld::read_dig_limit`] takes it from a loaded run's `meta.json`; until a run says
    /// otherwise it is [`DEFAULT_DIG_LIMIT_M`], which is that parameter's default.
    dig_limit_m: f32,
    /// Whether [`VoxelWorld::dig_limit_m`] came from a run or is this viewer's own default.
    ///
    /// The refusal the HUD prints names the limit's source, the way shot S4's plantable line does, so
    /// it cannot credit a run that was never loaded or that predates shot S2's `bundle` section.
    dig_limit_from_run: bool,
}

/// How many of a world's ground columns have a crown over them (shot V7).
///
/// The one number the eye's adaptation is driven by. It is a property of the **world as drawn** --
/// it moves with the snapshot, because the trees do -- and it is deliberately site-wide rather than
/// camera-relative: a camera-relative measure would change the exposure as the viewer flew, which
/// is what a real eye does and what a screenshot must not do (DECISIONS.md, V7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Canopy {
    /// Ground columns in the world.
    pub ground: usize,
    /// How many of them have at least one leaf voxel above them.
    pub covered: usize,
}

impl Canopy {
    /// The covered fraction, 0 on an open site and 1 under a closed canopy. Zero columns is 0.
    pub fn closure(&self) -> f32 {
        if self.ground == 0 {
            0.0
        } else {
            self.covered as f32 / self.ground as f32
        }
    }
}

/// One overlay band per column of the grid the field was read on, as
/// [`crate::overlay::Fields::bands`] produces them.
///
/// `cell_m` says **which grid**: [`crate::ECO_CELL_M`] for the six ecology overlays, which the
/// simulator computes on 1 m columns, and the bundle's own ground cell for shot S5's water, whose
/// file is on the finer grid. Before S5 there was only one answer and it was a constant in
/// [`VoxelWorld::to_col`]; a field on the other grid would have been stretched to twice its size
/// with nothing to say so.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnBands {
    pub x: usize,
    pub y: usize,
    pub cell_m: f32,
    pub bands: Vec<u8>,
}

/// Which media a plant roots in, and where that answer came from.
///
/// The rule is the simulator's: a column is plantable unless its surface is sealed or open water,
/// and `ecosim` publishes the result per medium as `params.medium.<name>.plantable` in the run's
/// `meta.json` (`ecosim/src/world.rs`, `is_plantable`). Shot V4 re-derived the same set from a
/// hard-coded name list -- `concrete`, `asphalt`, `roof`, `water` -- on the grounds that the two
/// projects share no code. They do not, and reading the run does not change that: the run directory
/// **is** the documented interface between them (CLAUDE.md), so shot S4 reads the value instead of
/// copying it. The simulator decides, the viewer expresses.
///
/// The name list stays, as the fallback, because it has to: the viewer opens a bundle with no run at
/// all (`--world` alone, and `--stress`), and a site with nothing to ask still has to draw. Which of
/// the two is in force is named in [`Plantable::source`] and printed on screen, the way shot V3's
/// height curve is -- a fallback that does not say so is indistinguishable from a reading.
#[derive(Debug, Clone, PartialEq)]
pub struct Plantable {
    /// The bundle's medium names, in code order, kept so the gate can be re-read from a run without
    /// the bundle in hand: a `VoxelWorld` already owns its copy of every other column field.
    pub media: Vec<String>,
    /// One flag per medium **code**, in `media` order. Not indexed by voxel id.
    grows: Vec<bool>,
    /// Where the flags came from, in words, for the HUD and for `ecoview.stats`.
    pub source: String,
    /// False if any medium's flag is this viewer's fallback rather than the run's.
    pub from_meta: bool,
}

/// The media shot V4 hard-coded as growing nothing, kept as the no-run fallback only.
const SEALED_NAMES: [&str; 4] = ["concrete", "asphalt", "roof", "water"];

impl Plantable {
    /// The fallback: the V4 name list, for a bundle opened with no run.
    pub fn fallback(media: &[String]) -> Plantable {
        let grows: Vec<bool> = media
            .iter()
            .map(|m| !SEALED_NAMES.contains(&m.as_str()))
            .collect();
        let source = format!(
            "this viewer's fallback name list, because no run is loaded to ask; {}",
            sealed_line(media, &grows)
        );
        Plantable {
            media: media.to_vec(),
            grows,
            source,
            from_meta: false,
        }
    }

    /// The run's own answer, medium by medium, falling back by name for any the run does not carry.
    ///
    /// Per medium rather than all-or-nothing: a run written before a medium existed still says the
    /// truth about the eight it does name, and only the ninth is guessed at. Every guess is named.
    pub fn of(media: &[String], meta: &RunMeta) -> Plantable {
        let rows = &meta.params.medium;
        let mut missing: Vec<&str> = Vec::new();
        let grows: Vec<bool> = media
            .iter()
            .map(|m| match rows.get(m).and_then(|r| r.plantable) {
                Some(p) => p,
                None => {
                    missing.push(m.as_str());
                    !SEALED_NAMES.contains(&m.as_str())
                }
            })
            .collect();
        let sealed = sealed_line(media, &grows);
        let source = if missing.is_empty() {
            format!("the run's meta.json, params.medium.<name>.plantable; {sealed}")
        } else if missing.len() == media.len() {
            // A run that says nothing about any medium is not a source. Every format-1, -2 and -3
            // run is one, and so is any run written before the field existed; naming meta.json
            // first there would credit the run for an answer it never gave.
            format!("this viewer's fallback name list, because the run carries no params.medium; {sealed}")
        } else {
            format!(
                "the run's meta.json, params.medium.<name>.plantable, and this viewer's fallback name list for {}, which it does not carry; {sealed}",
                missing.join(", ")
            )
        };
        Plantable {
            media: media.to_vec(),
            grows,
            source,
            from_meta: missing.is_empty(),
        }
    }

    /// Does a plant root in medium `code`? Indexed by the bundle's own medium code.
    #[inline]
    pub fn grows(&self, code: u8) -> bool {
        self.grows[code as usize]
    }

    /// The media that grow nothing, in medium-code order.
    pub fn sealed(&self) -> Vec<&str> {
        self.media
            .iter()
            .zip(self.grows.iter())
            .filter(|(_, g)| !**g)
            .map(|(m, _)| m.as_str())
            .collect()
    }
}

/// Moves every plant voxel up `cells` levels, into buckets for the chunk grid it now belongs to.
///
/// A lift changes what chunk a given level is in, so the buckets cannot simply be carried over. The
/// alternative is re-voxelising the whole scene, which would be correct and also throws away the
/// run's trees until the next snapshot is applied; re-bucketing keeps exactly the voxels that were
/// there. Anything pushed past the top is dropped, the way [`VoxelWorld::set_plant`] drops it.
fn shift_plants(
    plants: &[Vec<(u8, u8, u8, u16)>],
    before: ChunkPos,
    after: ChunkPos,
    cells: usize,
) -> Vec<Vec<(u8, u8, u8, u16)>> {
    let mut out = vec![Vec::new(); after.x * after.y * after.z];
    for cz in 0..before.z {
        for cy in 0..before.y {
            for cx in 0..before.x {
                let src = cx + before.x * (cy + before.y * cz);
                for &(lx, lz, ly, id) in &plants[src] {
                    let z = cz * CS + lz as usize + cells;
                    let nz = z / CS;
                    if nz >= after.z {
                        continue;
                    }
                    let dst = cx + after.x * (cy + after.y * nz);
                    out[dst].push((lx, (z % CS) as u8, ly, id));
                }
            }
        }
    }
    for bucket in &mut out {
        bucket.sort_unstable();
    }
    out
}

/// "concrete, asphalt, roof, water grow nothing", or that none of them do.
fn sealed_line(media: &[String], grows: &[bool]) -> String {
    let sealed: Vec<&str> = media
        .iter()
        .zip(grows.iter())
        .filter(|(_, g)| !**g)
        .map(|(m, _)| m.as_str())
        .collect();
    if sealed.is_empty() {
        "every medium on this site grows something".to_string()
    } else {
        format!("{} grow nothing", sealed.join(", "))
    }
}

impl VoxelWorld {
    /// Builds the chunk grid and voxelises the bundle's trees and shrubs into it.
    pub fn from_bundle(b: &Bundle) -> VoxelWorld {
        VoxelWorld::from_bundle_with_headroom(b, 0.0)
    }

    /// The same, with room above the bundle's own tallest plant for trees a **run** will put here.
    ///
    /// The chunk grid is sized once, at load, and `set_plant` drops anything above it. V1 sized it
    /// from the bundle alone, so a run tree taller than the scene's tallest survey was quietly
    /// beheaded; a caller that knows the run's height ceiling passes it here instead. The Capitol's
    /// tallest survey (23.6 m) already clears the simulator's 20 m curve, so its 324 chunks do not
    /// change -- the headroom is what keeps that from being luck (DECISIONS.md, V3).
    pub fn from_bundle_with_headroom(b: &Bundle, extra_m: f32) -> VoxelWorld {
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
            .fold(extra_m.max(0.0), f32::max);
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
            bands: None,
            ponds: None,
            plantable: Plantable::fallback(&b.media),
            ao: false,
            chunks,
            levels,
            datum_m: 0.0,
            dig_limit_m: DEFAULT_DIG_LIMIT_M,
            dig_limit_from_run: false,
        };
        let trees: Vec<TreeForm> = b.trees.iter().map(TreeForm::measured).collect();
        w.voxelise_plants(&trees, &b.shrubs, None);
        w
    }

    /// Turns trees and shrubs into plant voxels, into whatever the buckets already hold.
    ///
    /// V0 drew a tree as a trunk column under one solid ellipsoid, which is what `ecoview` draws on
    /// its 1 m voxels. Shot V3 replaced the inside of that ellipsoid: the wood is now a branching
    /// skeleton (`tree.rs`, `TreeForm::skeleton`) rasterised segment by segment, and the leaves are
    /// solid clusters at the branch tips. The silhouette is unchanged -- the envelope is still the
    /// allometry's -- so what the viewer says about a tree's size is still exactly what the
    /// simulator's own age-to-height curve says.
    fn voxelise_plants(&mut self, trees: &[TreeForm], shrubs: &[Shrub], cover: Option<&Cover>) {
        if let Some(c) = cover {
            self.voxelise_cover(c);
        }
        let w = self;
        let cell_m = w.cell_m;
        for t in trees {
            let cx = (t.x / cell_m) as i32;
            let cy = (t.y / cell_m) as i32;
            let base = w.ground_level(cx, cy) + 1;
            // One leaf cluster is never thinner than the lattice it is drawn on. Without this floor
            // a small tree's crown is three single voxels on the end of three twigs, which reads as
            // a fence post rather than a tree (MEASUREMENTS.md, V3: the first close-up).
            let leaf_r = t.blob_radius().max(0.9 * cell_m);
            for l in t.skeleton(cell_m) {
                // Half a cell a step: a coarser walk leaves gaps in a limb that runs diagonally,
                // and a finer one only writes the same voxels again.
                let d = [l.b[0] - l.a[0], l.b[1] - l.a[1], l.b[2] - l.a[2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let steps = (len / (0.5 * cell_m)).ceil().max(1.0);
                let n = steps as i32;
                for i in 0..=n {
                    let f = i as f32 / steps;
                    let p = [l.a[0] + d[0] * f, l.a[1] + d[1] * f, l.a[2] + d[2] * f];
                    w.stamp((cx, cy, base), p, l.r, TRUNK, None);
                }
                if l.tip {
                    w.stamp((cx, cy, base), l.b, leaf_r, CANOPY, Some(t));
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

    /// Re-reads the plantable gate from a run's `meta.json`, keeping this world's medium names.
    ///
    /// Called every time a snapshot is applied rather than once at load, because a run can arrive
    /// after the world does: shot E4's round trip grows a run from the edited site and adopts it
    /// mid-session, and a gate read only at startup would still be the fallback's.
    pub fn read_plantable(&mut self, meta: &RunMeta) {
        let next = Plantable::of(&self.plantable.media, meta);
        if next != self.plantable {
            self.plantable = next;
        }
    }

    /// Draws one snapshot's ground cover and its vines, cell by cell over the whole ground grid.
    ///
    /// **This is expression, not ecology** (`overnight/DIRECTION-native-viewer.md`). The simulator
    /// owns how much grass and shrub a patch has and how wet and how shaded its columns are; this
    /// decides only which of the patch's ground cells show it and how far up a wall a climber goes.
    /// Nothing here competes, accumulates or feeds back, and no vine is an entity in any run.
    ///
    /// Both are gated on the surface medium ([`Plantable`], which the run decides and this world
    /// only applies), so paving a lawn in the viewer strips its blades and the vines on the wall
    /// beside it. That gate is the only place the *whether* of a plant is decided here at all, and
    /// since shot S4 even that answer is read from the run rather than re-derived.
    ///
    /// Sweeping every ground cell is O(width x depth) -- 0.26 M on the Capitol -- which is the same
    /// order as the fill that follows it, so the loop is left plain rather than restricted to the
    /// patches with cover in them.
    fn voxelise_cover(&mut self, c: &Cover) {
        if !c.ground && !c.vines {
            return;
        }
        let cell_m = self.cell_m;
        let shrub_top = level_of(SHRUB_HEIGHT_M, cell_m).max(0);
        for gy in 0..self.depth {
            let ey = self.to_col(gy, c.dims.y);
            for gx in 0..self.width {
                let i = gx + self.width * gy;
                if self.building_h[i] > 0.0 || !self.plantable.grows(self.medium[i]) {
                    continue;
                }
                let ex = self.to_col(gx, c.dims.x);
                let base = level_of(self.ground_h[i], cell_m) + 1;
                let (x, y) = (gx as i32, gy as i32);
                if c.vines {
                    // A vine is rooted in this open cell and climbs whatever wall it touches, so one
                    // cell wedged between two buildings grows the same height on both: the drivers
                    // are the ground's, and the ground is what the plant is standing in.
                    let wall = self.tallest_wall(gx, gy);
                    if wall >= base {
                        let top = (base + c.vine_levels(gx, gy, ex, ey, cell_m) - 1).min(wall);
                        for z in base..=top {
                            self.set_plant(x, y, z, VINE);
                        }
                    }
                }
                if !c.ground {
                    continue;
                }
                match c.cell(gx, gy, ex, ey) {
                    Some(CellPlant::Grass) => self.set_plant(x, y, base, GRASS),
                    Some(CellPlant::Shrub) => {
                        for z in base..=base + shrub_top {
                            self.set_plant(x, y, z, SHRUB);
                        }
                    }
                    None => {}
                }
            }
        }
    }

    /// The top level of the tallest building in the four cells orthogonally adjacent to this one,
    /// or `i32::MIN` if none of them has one. A vine climbs the outside of a wall, in the open
    /// column beside it, because the wall's own column is solid and a plant voxel inside it would
    /// never be drawn.
    fn tallest_wall(&self, gx: usize, gy: usize) -> i32 {
        let mut top = i32::MIN;
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (gx as i32 + dx, gy as i32 + dy);
            if nx < 0 || ny < 0 || nx as usize >= self.width || ny as usize >= self.depth {
                continue;
            }
            let j = nx as usize + self.width * ny as usize;
            if self.building_h[j] > 0.0 {
                top = top.max(level_of(self.ground_h[j] + self.building_h[j], self.cell_m));
            }
        }
        top
    }

    /// The ecology column a ground cell's centre falls in, clamped to the field's own grid.
    ///
    /// The one place the viewer crosses between the bundle's ground cells and the simulator's 1 m
    /// columns. Both the overlays and the cover go through it, so neither can claim a resolution
    /// the simulator does not have.
    #[inline]
    fn to_col(&self, g: usize, n: usize) -> usize {
        self.to_grid(g, n, ECO_CELL_M)
    }

    /// The cell of a `cell_m`-metre grid that this ground cell's centre falls in, clamped to it.
    /// With `cell_m` equal to this world's own the map is the identity, which is what shot S5's
    /// water needs: `water.bin` is already on the ground grid and resampling it would be a lie in
    /// either direction.
    #[inline]
    fn to_grid(&self, g: usize, n: usize, cell_m: f32) -> usize {
        let cell_m = if cell_m > 0.0 { cell_m } else { ECO_CELL_M };
        ((((g as f32 + 0.5) * self.cell_m) / cell_m) as usize).min(n.saturating_sub(1))
    }

    /// Replaces every plant voxel in the world with the ones these trees and shrubs make, and returns
    /// the chunks whose mesh is now stale.
    ///
    /// This is what a snapshot change costs: the ground is untouched -- a run never moves it -- so
    /// only the buckets that actually differ, and the neighbours that see them through the one-voxel
    /// pad, are remeshed. A site whose trees all sit near the ground therefore remeshes the ground
    /// chunk layer and nothing above it.
    pub fn set_plants(&mut self, trees: &[TreeForm], shrubs: &[Shrub]) -> Vec<ChunkPos> {
        self.set_scene(trees, shrubs, None)
    }

    /// The same, with one snapshot's ground cover and vines drawn alongside the trees.
    ///
    /// Cover shares the plant buckets rather than getting its own, so a snapshot change diffs once
    /// and a cell that holds both a vine and a blade of grass resolves by id in `fill_chunk`. The
    /// cost is that toggling the cover re-voxelises the trees too; MEASUREMENTS.md, V4 has what that
    /// is worth on the Capitol at tick 20000.
    pub fn set_scene(
        &mut self,
        trees: &[TreeForm],
        shrubs: &[Shrub],
        cover: Option<&Cover>,
    ) -> Vec<ChunkPos> {
        let empty = vec![Vec::new(); self.chunk_count()];
        let before = std::mem::replace(&mut self.plants, empty);
        self.voxelise_plants(trees, shrubs, cover);
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

    /// Every plant voxel in the world, as `(x, y, level, id)` on the ground lattice.
    ///
    /// The buckets hold chunk-local coordinates because that is what a fill needs; this is the only
    /// place that undoes it. Shot V3's tests use it to check that no leaf leaves its tree's crown
    /// envelope, which is the claim that keeps a procedural crown honest about the tree's size.
    pub fn plant_voxels(&self) -> Vec<(usize, usize, usize, u16)> {
        let mut out = Vec::new();
        for (i, bucket) in self.plants.iter().enumerate() {
            let c = self.chunk_pos(i);
            for &(lx, lz, ly, id) in bucket {
                out.push((
                    c.x * CS + lx as usize,
                    c.y * CS + ly as usize,
                    c.z * CS + lz as usize,
                    id,
                ));
            }
        }
        out
    }

    /// How many plant voxels are wood and how many are leaves. What a snapshot's trees cost, for
    /// the HUD and for `MEASUREMENTS.md`, without building the list above.
    pub fn plant_counts(&self) -> (usize, usize) {
        let mut wood = 0;
        let mut leaves = 0;
        for bucket in &self.plants {
            for &(_, _, _, id) in bucket {
                match id {
                    TRUNK => wood += 1,
                    CANOPY => leaves += 1,
                    _ => {}
                }
            }
        }
        (wood, leaves)
    }

    /// How many voxels are grass, shrub and vine. Reported beside every picture that shows them,
    /// because a count of blades is the honest way to say that the blades are a texture: the
    /// simulator counts no grass at all, only a fraction per patch.
    pub fn cover_counts(&self) -> (usize, usize, usize) {
        let (mut grass, mut shrub, mut vine) = (0, 0, 0);
        for bucket in &self.plants {
            for &(_, _, _, id) in bucket {
                match id {
                    GRASS => grass += 1,
                    SHRUB => shrub += 1,
                    VINE => vine += 1,
                    _ => {}
                }
            }
        }
        (grass, shrub, vine)
    }

    /// How much of the ground has a crown over it (shot V7).
    ///
    /// One pass over the plant buckets, counting the ground columns with at least one **leaf**
    /// voxel somewhere above them. Wood is not counted -- a trunk is not a canopy -- and neither is
    /// the ground cover, which sits on the ground rather than over it, nor a vine, which is on a
    /// wall. Buildings are not counted either, and that is a decision rather than an oversight:
    /// ground under a roof is ground nobody can see, so counting it would raise the measure with
    /// surface that never reaches the frame (DECISIONS.md, V7).
    ///
    /// This is O(leaf voxels) -- about 1.8 million on the Capitol at tick 20000, which is a couple
    /// of milliseconds -- so it is taken when the snapshot's plants change and not every frame.
    pub fn canopy(&self) -> Canopy {
        let mut covered = vec![false; self.width * self.depth];
        for (i, bucket) in self.plants.iter().enumerate() {
            let c = self.chunk_pos(i);
            for &(lx, lz, ly, id) in bucket {
                if id != CANOPY {
                    continue;
                }
                let (x, y) = (c.x * CS + lx as usize, c.y * CS + ly as usize);
                if x >= self.width || y >= self.depth {
                    continue;
                }
                let j = x + self.width * y;
                // A leaf is put above the terrain by construction, but a lift can raise the ground
                // into last snapshot's crown, so the level is checked rather than assumed.
                if (c.z * CS + lz as usize) as i32 > level_of(self.ground_h[j], self.cell_m) {
                    covered[j] = true;
                }
            }
        }
        Canopy {
            ground: self.width * self.depth,
            covered: covered.iter().filter(|c| **c).count(),
        }
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

    /// Fills the ball of radius `r_m` metres at `p`, a point in metres relative to the trunk base,
    /// in the plant frame `[east, up, north]`. `at` is that base: ground cell `(x, y)` and the first
    /// level above the terrain.
    ///
    /// `clip` is the tree whose crown envelope a leaf cluster may not leave. Wood passes `None`: a
    /// branch is already pulled back to the envelope wall by the skeleton, and clipping its radius
    /// as well would shave the outer limbs.
    fn stamp(
        &mut self,
        at: (i32, i32, i32),
        p: [f32; 3],
        r_m: f32,
        id: u16,
        clip: Option<&TreeForm>,
    ) {
        let (cx, cy, base) = at;
        let cell_m = self.cell_m;
        // Half a cell is the floor, so a twig thinner than the lattice is still one voxel wide
        // rather than a dotted line.
        let rc = (r_m / cell_m).max(0.5);
        let (fx, fy, fz) = (p[0] / cell_m, p[2] / cell_m, p[1] / cell_m);
        let (ix, iy, iz) = (fx.round() as i32, fy.round() as i32, fz.round() as i32);
        let ri = rc.ceil() as i32;
        for oz in -ri..=ri {
            for oy in -ri..=ri {
                for ox in -ri..=ri {
                    let (gx, gy, gz) = (ix + ox, iy + oy, iz + oz);
                    let (ex, ey, ez) = (gx as f32 - fx, gy as f32 - fy, gz as f32 - fz);
                    if ex * ex + ey * ey + ez * ez > rc * rc {
                        continue;
                    }
                    if let Some(t) = clip {
                        let q = [gx as f32 * cell_m, gz as f32 * cell_m, gy as f32 * cell_m];
                        if !t.in_envelope(q) {
                            continue;
                        }
                    }
                    self.set_plant(cx + gx, cy + gy, base + gz, id);
                }
            }
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
            let top = self.top_solid_at(i);
            if top > g && z <= top {
                return BUILDING;
            }
            // Standing water stands on whatever the column's top solid is, so a pond on a paved
            // yard sits on the paving and one on a roof -- which the simulator does not make, since
            // a roof's depression storage is zero -- would sit on the roof rather than inside it.
            if let Some(p) = &self.ponds {
                let n = p[i] as i32;
                if n > 0 && z <= top + n {
                    return POND;
                }
            }
            return AIR;
        }
        // Only the top voxel carries the surface medium; everything under it is soil, so digging
        // exposes soil instead of painting the cut face with lawn (ecoview/DECISIONS.md, E3).
        if z == g {
            // Under a field overlay the top voxel of every ground column is the band instead, which
            // is what makes the site read as a map. A column under a building is covered anyway, and
            // the soil below is untouched, so switching the overlay off gets the site back exactly.
            return match &self.bands {
                Some(b) => BAND_BASE + b[i] as u16,
                None => self.medium[i] as u16 + 1,
            };
        }
        SOIL
    }

    /// Puts one overlay's bands on the world, or takes the overlay off with `None`, and returns the
    /// chunks whose mesh is now stale.
    ///
    /// The bands arrive per ecology column -- 1 m, the grid the simulator computes on -- and are
    /// resampled at each ground cell's centre, so a 0.5 m bundle draws each column as 2x2 cells and
    /// the overlay never claims a resolution the simulator does not have.
    pub fn set_overlay(&mut self, field: Option<&ColumnBands>) -> Vec<ChunkPos> {
        let next: Option<Vec<u8>> = field.map(|f| {
            let mut v = vec![0u8; self.width * self.depth];
            for gy in 0..self.depth {
                let ey = self.to_grid(gy, f.y, f.cell_m);
                for gx in 0..self.width {
                    let ex = self.to_grid(gx, f.x, f.cell_m);
                    v[gx + self.width * gy] = f.bands.get(ex + f.x * ey).copied().unwrap_or(0);
                }
            }
            v
        });
        if next == self.bands {
            return Vec::new();
        }
        let changed: Vec<usize> = (0..self.width * self.depth)
            .filter(|&i| self.bands.as_ref().map(|b| b[i]) != next.as_ref().map(|b| b[i]))
            .collect();
        self.bands = next;
        let mut stale = vec![false; self.chunk_count()];
        for i in changed {
            let (x, y) = (i % self.width, i / self.width);
            let g = self.ground_level(x as i32, y as i32);
            if g < 0 || g as usize >= self.levels {
                continue;
            }
            let z = g as usize;
            self.mark_stale(&mut stale, x, y, z, z);
        }
        self.stale_list(&stale)
    }

    /// Marks every chunk whose mesh a change to column `(x, y)` over levels `z0..=z1` leaves stale.
    ///
    /// A voxel reaches into a neighbouring chunk's one-voxel pad only when it sits on its own
    /// chunk's boundary layer, so the 26-neighbour sweep is only needed there. Lifted out of
    /// `set_overlay` by shot S5, which needs the same arithmetic over a **range** of levels: a pond
    /// is one voxel deep on a puddle and ten in the corner of the site that never drains.
    fn mark_stale(&self, stale: &mut [bool], x: usize, y: usize, z0: usize, z1: usize) {
        let span = |v: usize, n: usize| -> (usize, usize) {
            let c = v / CS;
            let lo = if v.is_multiple_of(CS) {
                c.saturating_sub(1)
            } else {
                c
            };
            let hi = if v % CS == CS - 1 {
                (c + 1).min(n - 1)
            } else {
                c
            };
            (lo, hi)
        };
        let (x0, x1) = span(x, self.chunks.x);
        let (y0, y1) = span(y, self.chunks.y);
        let lo = z0.min(self.levels - 1);
        let hi = z1.clamp(lo, self.levels - 1);
        let (z0, _) = span(lo, self.chunks.z);
        let (_, z1) = span(hi, self.chunks.z);
        for cz in z0..=z1 {
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    stale[self.chunk_index(ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    })] = true;
                }
            }
        }
    }

    fn stale_list(&self, stale: &[bool]) -> Vec<ChunkPos> {
        (0..self.chunk_count())
            .filter(|&i| stale[i])
            .map(|i| self.chunk_pos(i))
            .collect()
    }

    /// Puts one snapshot's standing water on the world, or takes it off with `None`, and returns the
    /// chunks whose mesh is now stale (shot S5).
    ///
    /// The levels arrive on the **ground grid**, which is the grid `water.bin` is written on and the
    /// grid this world's columns are, so nothing is resampled: [`VoxelWorld::to_grid`] is the
    /// identity here. It is used anyway, so a run whose grid somehow disagreed is clamped rather
    /// than read off the end of its own file.
    pub fn set_ponds(&mut self, ponds: Option<&PondLevels>) -> Vec<ChunkPos> {
        let next: Option<Vec<u8>> = ponds.map(|p| {
            (0..self.width * self.depth)
                .map(|i| {
                    let gx = self.to_grid(i % self.width, p.width, self.cell_m);
                    let gy = self.to_grid(i / self.width, p.depth, self.cell_m);
                    let n = p.levels.get(gx + p.width * gy).copied().unwrap_or(0) as usize;
                    let top = self.top_solid_at(i);
                    if n == 0 || top < 0 {
                        return 0;
                    }
                    // The chunk grid is sized once, at load, and a voxel above its ceiling is
                    // dropped rather than drawn in the wrong chunk -- the same rule `set_plant`
                    // keeps for a tree taller than the grid.
                    let room = self.levels.saturating_sub(top as usize + 1);
                    n.min(room).min(u8::MAX as usize) as u8
                })
                .collect()
        });
        if next == self.ponds {
            return Vec::new();
        }
        let mut stale = vec![false; self.chunk_count()];
        for i in 0..self.width * self.depth {
            let was = self.ponds.as_ref().map_or(0, |v| v[i]);
            let is = next.as_ref().map_or(0, |v| v[i]);
            if was == is {
                continue;
            }
            let top = self.top_solid_at(i);
            if top < 0 || top as usize + 1 >= self.levels {
                continue;
            }
            let (x, y) = (i % self.width, i / self.width);
            let deepest = top as usize + was.max(is) as usize;
            self.mark_stale(&mut stale, x, y, top as usize + 1, deepest);
        }
        self.ponds = next;
        self.stale_list(&stale)
    }

    /// The top solid level of a column: the building's roof if it has one, otherwise the ground.
    #[inline]
    fn top_solid_at(&self, i: usize) -> i32 {
        let g = level_of(self.ground_h[i], self.cell_m);
        if self.building_h[i] > 0.0 {
            level_of(self.ground_h[i] + self.building_h[i], self.cell_m).max(g)
        } else {
            g
        }
    }

    /// Is standing water on the world, and how much: cells with water on them, and voxels of water.
    ///
    /// The cells are the run's answer quantised to the lattice and the voxels are what is drawn;
    /// neither is a depth, which is why the HUD prints the millimetres beside them
    /// ([`crate::overlay::PondStats`]).
    pub fn pond_counts(&self) -> (usize, usize) {
        match &self.ponds {
            Some(p) => (
                p.iter().filter(|n| **n > 0).count(),
                p.iter().map(|n| *n as usize).sum(),
            ),
            None => (0, 0),
        }
    }

    pub fn has_ponds(&self) -> bool {
        self.ponds.is_some()
    }

    /// Is a field overlay on?
    pub fn has_overlay(&self) -> bool {
        self.bands.is_some()
    }

    /// Turns ambient occlusion on or off, and returns the chunks whose mesh is now stale.
    ///
    /// Every chunk, or none: occlusion is a property of the whole world's shading, not of one
    /// column, so there is no cheaper answer than rebuilding. It is a toggle rather than something
    /// the world is built with because the viewer offers it as a key, and because a shot that wants
    /// to measure what it costs has to be able to mesh the same world both ways.
    pub fn set_ao(&mut self, on: bool) -> Vec<ChunkPos> {
        if self.ao == on {
            return Vec::new();
        }
        self.ao = on;
        self.all_chunks()
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
            // Two floors, and the lower of them wins. The lattice cannot go under its own level 0,
            // and the exported world cannot go under the simulator's `base_z`; a caller that wants
            // the first one out of the way calls `open_dig_room` before it gets here.
            EditAction::LowerGround => {
                let floor = (self.datum_m - self.dig_limit_m).max(0.0);
                self.ground_h[i] = (self.ground_h[i] - self.cell_m).max(floor);
            }
            EditAction::SetSurface(m) => self.medium[i] = m,
            EditAction::RaiseBuilding => self.building_h[i] += self.cell_m,
            EditAction::LowerBuilding => {
                self.building_h[i] = (self.building_h[i] - self.cell_m).max(0.0)
            }
        }
        let after = self.column_span(i);
        self.touched(x, y, before, after)
    }

    /// What a column holds: `(ground height, medium code, building height)`.
    ///
    /// The height is **in the bundle's frame**, where zero is the bundle's own zero and a column dug
    /// below it is negative; the lattice's own floor is at `-`[`VoxelWorld::datum_m`] and is not this
    /// caller's business. The crosshair reads this to say what it is pointing at, and an undo entry
    /// is taken from it **before** the edit rather than derived from the action afterwards:
    /// `LowerGround` clamps at the lattice floor and `SetSurface` throws the old code away, so the
    /// opposite action is not an undo. Taking it in the bundle's frame is also what makes an undo
    /// survive a dig: the entry still means the same height after the floor has moved under it.
    pub fn column(&self, x: usize, y: usize) -> Option<(f32, u8, f32)> {
        (x < self.width && y < self.depth).then(|| {
            let i = x + self.width * y;
            (
                self.ground_h[i] - self.datum_m,
                self.medium[i],
                self.building_h[i],
            )
        })
    }

    /// How far the lattice floor sits below the bundle's zero, in metres. 0 until something is dug.
    #[inline]
    pub fn datum_m(&self) -> f32 {
        self.datum_m
    }

    /// How far below the bundle's zero this world will go before an edit is refused, in metres.
    #[inline]
    pub fn dig_limit_m(&self) -> f32 {
        self.dig_limit_m
    }

    /// Whether the dig limit is a run's number or this viewer's default, for the HUD to say which.
    #[inline]
    pub fn dig_limit_from_run(&self) -> bool {
        self.dig_limit_from_run
    }

    /// The ground grid **in the bundle's frame**, which is what gets written back out to disk.
    ///
    /// A dug column is negative here, and that is the point: `ecosim` reads `ground_h.f32` as heights
    /// over the bundle's zero and puts `[bundle] base_z` layers of soil under the lowest of them, so a
    /// hole is a negative number and nothing about the format has to change to carry one. Allocating
    /// a copy is fine -- it happens once, when a run is started, beside writing 1 MB of it to disk.
    pub fn ground_export(&self) -> Vec<f32> {
        if self.datum_m == 0.0 {
            return self.ground_h.clone();
        }
        self.ground_h.iter().map(|g| g - self.datum_m).collect()
    }

    /// The deepest column on the site, in the bundle's frame. Negative once anything has been dug.
    pub fn lowest_ground_m(&self) -> f32 {
        self.ground_h
            .iter()
            .fold(f32::INFINITY, |a, b| a.min(*b))
            .min(f32::MAX)
            - self.datum_m
    }

    /// Takes the dig limit from a run's `meta.json`: `ecosim`'s own `[bundle] base_z`.
    ///
    /// Called wherever [`VoxelWorld::read_plantable`] is, and for the same reason -- a run can arrive
    /// after the world does, and a number read only at startup would still be the fallback's. Shrinking
    /// the limit below what has already been dug does not undo anything; it only stops the next stroke.
    pub fn read_dig_limit(&mut self, meta: &RunMeta) {
        if let Some(m) = meta.base_z_m() {
            self.dig_limit_m = m;
            self.dig_limit_from_run = true;
        }
    }

    /// How much further this column can be lowered before the dig limit stops it, in metres.
    ///
    /// The limit is the simulator's, not the lattice's: the lattice floor moves when it is reached
    /// ([`VoxelWorld::open_dig_room`]), and what does not move is how deep a hole the exported world
    /// can carry. Never negative, and zero means the next `LowerGround` will do nothing.
    pub fn dig_room_m(&self, x: usize, y: usize) -> f32 {
        match self.column(x, y) {
            Some((g, _, _)) => (g + self.dig_limit_m).max(0.0),
            None => 0.0,
        }
    }

    /// Would `LowerGround` here do nothing? True when the column is already at the dig limit.
    pub fn at_dig_limit(&self, x: usize, y: usize) -> bool {
        self.dig_room_m(x, y) < self.cell_m
    }

    /// Opens room under the site so a dig can go below the bundle's zero, if it needs any.
    ///
    /// Returns `true` when the lattice grew, which means **every** mesh in the world is stale: the
    /// ground did not move in world space -- [`crate::mesh::mesh_chunk`] subtracts the datum back out
    /// again -- but every voxel moved in the lattice, so every chunk buffer has to be refilled. The
    /// chunk grid can also gain a layer, and then the caller's own per-chunk bookkeeping is invalid
    /// too; [`VoxelWorld::chunk_count`] says whether it did.
    ///
    /// This is the half of shot S6 that the operator called "not a one-line change". Clamping at zero
    /// was not a wrong line -- it is what a lattice with a floor can honestly do -- so the fix is to
    /// give the lattice somewhere to put the hole rather than to delete the clamp.
    pub fn open_dig_room(&mut self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.depth {
            return false;
        }
        let i = x + self.width * y;
        // Room is needed only when this column is about to walk off the bottom of the lattice, which
        // after the first lift is rare: eight levels is eight more strokes.
        if self.ground_h[i] - self.cell_m >= -1e-6 {
            return false;
        }
        if self.at_dig_limit(x, y) {
            return false;
        }
        let cells = DIG_ROOM_LEVELS;
        let dz = cells as f32 * self.cell_m;
        self.datum_m += dz;
        for g in &mut self.ground_h {
            *g += dz;
        }
        self.levels += cells;
        let before = self.chunks;
        self.chunks.z = self.levels.div_ceil(CS);
        self.plants = shift_plants(&self.plants, before, self.chunks, cells);
        true
    }

    /// Puts a column back the way [`VoxelWorld::column`] found it. Returns the stale chunks.
    pub fn restore_column(&mut self, x: usize, y: usize, was: (f32, u8, f32)) -> Vec<ChunkPos> {
        if x >= self.width || y >= self.depth {
            return Vec::new();
        }
        let i = x + self.width * y;
        let before = self.column_span(i);
        self.ground_h[i] = was.0 + self.datum_m;
        self.medium[i] = was.1;
        self.building_h[i] = was.2;
        let after = self.column_span(i);
        self.touched(x, y, before, after)
    }

    /// The chunks a change to column `(x, y)` between these two spans leaves stale.
    ///
    /// Only the levels between the old and new ground and tops changed; below them it is still
    /// soil, which no edit rewrites. Lowering ground under a building moves the building's base
    /// too, so the span runs from the lower of the two ground levels to the higher of the two tops.
    fn touched(&self, x: usize, y: usize, before: (i32, i32), after: (i32, i32)) -> Vec<ChunkPos> {
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

    /// The ground cell a ray first meets, marched from `origin` along `dir` for at most `max_m`
    /// metres. `None` when it never touches anything solid inside the site.
    ///
    /// A ray march rather than a proper DDA: the world is a heightfield, so the test at each sample
    /// is one comparison against that column's own top, and a quarter-cell step over 120 m is under
    /// a thousand of them -- far cheaper than the remesh the hit is about to cause. A coarser step
    /// would slip through a wall seen edge-on; a finer one would only find the same cell again.
    ///
    /// Plants are not in it. The crosshair edits ground, paving and buildings, and a tree is
    /// something the simulator planted: pointing through a canopy at the lawn under it is what a
    /// gardener means, and it is the only reading that stays true when that tree grows a tick later.
    pub fn pick_cell(&self, origin: [f32; 3], dir: [f32; 3], max_m: f32) -> Option<(usize, usize)> {
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if !len.is_finite() || len <= 0.0 {
            return None;
        }
        let d = [dir[0] / len, dir[1] / len, dir[2] / len];
        let step = 0.25 * self.cell_m;
        let steps = (max_m / step).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f32 * step;
            let p = [
                origin[0] + d[0] * t,
                origin[1] + d[1] * t,
                origin[2] + d[2] * t,
            ];
            if p[0] < 0.0 || p[2] < 0.0 {
                continue;
            }
            let (x, y) = ((p[0] / self.cell_m) as usize, (p[2] / self.cell_m) as usize);
            if x >= self.width || y >= self.depth {
                continue;
            }
            let i = x + self.width * y;
            // World space is the bundle's frame, so the lattice's own datum comes back out here.
            if p[1] <= self.ground_h[i] - self.datum_m + self.building_h[i] {
                return Some((x, y));
            }
        }
        None
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
