//! Procedural trees: deterministic branching geometry from the run's own numbers.
//!
//! Shot V3. V0 drew a tree as a trunk column under an ellipsoid blob, which is the shape
//! `ecoview` draws on its 1 m voxels; this module replaces it with a branching skeleton and a
//! clustered crown, generated at whatever cube edge the bundle uses. Nothing here is ecology: the
//! simulator decides how tall a tree of a given age is and how much light falls on its crown, and
//! the viewer turns those two numbers into geometry (`overnight/DIRECTION-native-viewer.md`, "the
//! simulator decides, the viewer expresses").
//!
//! **Where each input comes from.** The backlog row asks for geometry from seed, species, age,
//! biomass and light. Four of the five are in the run directory and one is not:
//!
//! | Input | Source |
//! |---|---|
//! | seed | `meta.json`'s `seed`, mixed with the tree's own `id` -- one RNG stream per tree |
//! | species | `params.tree`'s age thresholds, and the species table's two colours (palette.rs) |
//! | age | `entities.json`'s `age` in ticks, over `meta.json`'s `year_len` |
//! | light | `light.bin` at the tree's column, the simulator's own surface sample (run.rs, `CrownLight`) |
//! | **biomass** | **nowhere.** The simulator carries no per-tree mass, diameter or leaf area |
//!
//! So the size of a tree is the **height** its age maps to under the simulator's own age-to-height
//! curve ([`Life`]), and the crown is an allometric function of that height. That curve is the one
//! `ecosim` already uses to plant a scene tree (`ecosim/src/plants.rs`, `import_age`), read
//! backwards; at tick 0 it therefore returns the heights the bundle's survey actually recorded,
//! which is the check `MEASUREMENTS.md` reports. Publishing a per-tree size is an `ecosim` row and
//! not this shot's to make (MASTER.md, component isolation).
//!
//! **No transcendental function appears below.** Branch directions are built by rejection sampling
//! in a cube and normalising, so the only library call in the whole model is `f32::sqrt`, which
//! IEEE-754 makes exact. That is what lets `golden_procedural_tree` hash the same bits on Linux CI
//! as on the Windows machine the value was taken on -- the same problem `ecosim` solved by routing
//! its trigonometry through the `libm` crate (ecosim/DECISIONS.md), solved here by not needing any.
//!
//! Like the rest of the library half this module never mentions Bevy.

use crate::bundle::Tree;
use crate::run::{Params, RunMeta};

/// Crown radius as a fraction of height. **Measured**, not chosen: the mean of that ratio over the
/// 81 surveyed trees in the committed Capitol bundle (`ecosim/worlds/capitol/trees.json`), which is
/// the only set of real crown dimensions either project has.
pub const CROWN_RADIUS_FRACTION: f32 = 0.30;
/// Height of the lowest branch as a fraction of height, measured the same way. A bundle tree never
/// uses either constant -- it has its own surveyed crown -- so both apply exactly to the trees the
/// simulator grew, for which no crown dimension exists anywhere in the run directory.
pub const CROWN_BASE_FRACTION: f32 = 0.37;

/// `ecosim/params.toml`'s own defaults, used only where the run's `meta.json` does not carry the
/// number. `params.bundle` is written to `meta.json` **only when it is not at its defaults**
/// (`ecosim/src/params.rs`, `skip_serializing_if`), so on the reference Capitol run these two
/// heights are always the fallback and [`Life::from_meta`] is false. That is reported on screen
/// rather than hidden, the way V2 reports an overlay scale it had to guess (overlay.rs, `Scale`).
const FALLBACK_YEAR_TICKS: f32 = 4000.0;
const FALLBACK_YOUNG_YEARS: f32 = 0.125;
const FALLBACK_MATURE_YEARS: f32 = 0.25;
const FALLBACK_MAX_YEARS: f32 = 1.5;
const FALLBACK_MATURE_HEIGHT_M: f32 = 3.0;
const FALLBACK_TALL_HEIGHT_M: f32 = 20.0;
const FALLBACK_TALL_YEARS: f32 = 0.75;

/// The tree species' life history, and the age-to-height curve that follows from it.
///
/// The curve is piecewise linear and monotone through (0 m, age 0), (`mature_height_m`,
/// `mature_years`) and (`tall_height_m`, `tall_years`), flat above the last -- the exact shape of
/// `ecosim`'s `import_age`, inverted. Flat above the last breakpoint means a tree older than
/// `tall_years` stops growing at `tall_height_m`; that is the simulator's statement about the
/// species, not a cap this viewer chose, and it is why two trees 3,000 ticks apart in age can be
/// the same height.
#[derive(Debug, Clone, PartialEq)]
pub struct Life {
    pub year_ticks: f32,
    pub young_years: f32,
    pub mature_years: f32,
    pub max_years: f32,
    pub mature_height_m: f32,
    pub tall_height_m: f32,
    pub tall_years: f32,
    /// Where the numbers above came from, in words, for the HUD and for `ecoview.stats`.
    pub source: String,
    /// False if any of the three height-curve numbers is this viewer's fallback.
    pub from_meta: bool,
}

impl Default for Life {
    fn default() -> Life {
        Life::of(&RunMeta::default())
    }
}

impl Life {
    /// Reads the curve out of a run's `meta.json`, naming every number it could not find.
    pub fn of(meta: &RunMeta) -> Life {
        let p: &Params = &meta.params;
        let t = &p.tree;
        let b = &p.bundle;
        let heights = (
            b.tree_mature_height,
            b.tree_tall_height,
            b.tree_tall_age_years,
        );
        let from_meta = heights.0.is_some() && heights.1.is_some() && heights.2.is_some();
        let mut missing: Vec<&str> = Vec::new();
        if meta.year_len == 0 {
            missing.push("year_len");
        }
        if t.mature_age_years.is_none() {
            missing.push("params.tree ages");
        }
        if !from_meta {
            missing.push("params.bundle (the height curve's breakpoints)");
        }
        let source = if missing.is_empty() {
            "params.tree ages and params.bundle heights, the simulator's own age-to-height map"
                .to_string()
        } else {
            format!(
                "this viewer's fallback: meta.json has no {}; ecosim's params.toml defaults stand in",
                missing.join(" and ")
            )
        };
        Life {
            year_ticks: if meta.year_len > 0 {
                meta.year_len as f32
            } else {
                FALLBACK_YEAR_TICKS
            },
            young_years: t.young_age_years.unwrap_or(FALLBACK_YOUNG_YEARS),
            mature_years: t.mature_age_years.unwrap_or(FALLBACK_MATURE_YEARS),
            max_years: t.max_age_years.unwrap_or(FALLBACK_MAX_YEARS),
            mature_height_m: heights.0.unwrap_or(FALLBACK_MATURE_HEIGHT_M),
            tall_height_m: heights.1.unwrap_or(FALLBACK_TALL_HEIGHT_M),
            tall_years: heights.2.unwrap_or(FALLBACK_TALL_YEARS),
            source,
            from_meta,
        }
    }

    /// The height, in metres, of a tree `age_ticks` old.
    pub fn height_of(&self, age_ticks: u32) -> f32 {
        let years = age_ticks as f32 / self.year_ticks.max(1.0);
        let am = self.mature_years.max(1e-6);
        let at = self.tall_years.max(am);
        let (hm, ht) = (self.mature_height_m, self.tall_height_m);
        if years <= 0.0 {
            0.0
        } else if years < am {
            hm * years / am
        } else if years < at {
            hm + (ht - hm) * (years - am) / (at - am)
        } else {
            ht
        }
    }

    /// The three age thresholds in years, for reporting.
    pub fn stage_years(&self) -> (f32, f32, f32) {
        (self.young_years, self.mature_years, self.max_years)
    }
}

/// One tree's envelope and the seed that fills it. Metres throughout, `x`/`y` from the world's
/// south-west corner; the base level comes from the viewer's own terrain column, never from the
/// simulator's 1 m surface level (DECISIONS.md, "V1 the simulator owns the tree").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeForm {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub crown_base: f32,
    pub crown_radius: f32,
    /// Fraction of full sun at the crown's centre, from `light.bin`. 1.0 when no run is loaded.
    pub light: f32,
    pub seed: u64,
}

/// One piece of wood: a segment between two points, in metres relative to the trunk's base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limb {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub r: f32,
    /// True for a limb with no children: a leaf cluster hangs off its far end.
    pub tip: bool,
}

/// Mixes two numbers into an independent stream. The run's seed and a tree's id give every tree its
/// own branching without any two trees sharing a sequence.
pub fn mix(a: u64, b: u64) -> u64 {
    let mut z = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// SplitMix64. Deterministic, seedable and not the simulator's: nothing here has to agree with
/// `ecosim`'s `ChaCha8Rng`, only with itself, and this one is four lines (V0 made the same call for
/// the stress world's `Lcg`).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0..1, exact in binary: a 24-bit integer over 2^24, so the value is the same on any platform.
    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }

    /// A direction roughly uniform on the sphere, by rejection in a cube. No trigonometry, so the
    /// bits are the same everywhere (see the module note).
    fn direction(&mut self) -> [f32; 3] {
        for _ in 0..16 {
            let v = [
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            ];
            let d2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            if (0.05..=1.0).contains(&d2) {
                let k = 1.0 / d2.sqrt();
                return [v[0] * k, v[1] * k, v[2] * k];
            }
        }
        [0.0, 1.0, 0.0]
    }
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let d2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if d2 <= 1e-12 {
        return [0.0, 1.0, 0.0];
    }
    let k = 1.0 / d2.sqrt();
    [v[0] * k, v[1] * k, v[2] * k]
}

/// How much of the parent's direction a child keeps, how far the random direction pulls it, and how
/// hard the whole thing is lifted towards the sky. Tuned by looking at the pictures in
/// `MEASUREMENTS.md`: less lift and the crown sags into the trunk, more and every tree is a broom.
const KEEP: f32 = 0.55;
const SPREAD: f32 = 0.80;
const LIFT: f32 = 0.18;
/// How far out a branch tip may sit, as a fraction of the crown envelope's wall. Under 1 so a leaf
/// cluster centred on a tip is mostly *inside* the crown rather than mostly clipped off it.
const TIP_FRAC: f32 = 0.78;

impl TreeForm {
    /// A tree the scene surveyed: its own measured envelope, and a seed from where it stands, so
    /// the same bundle always grows the same wood (there is no id in `trees.json` to mix in).
    pub fn measured(t: &Tree) -> TreeForm {
        let height = t.height.max(0.0);
        TreeForm {
            x: t.x,
            y: t.y,
            height,
            crown_base: t.crown_base.clamp(0.0, height),
            crown_radius: t.crown_radius.max(0.0),
            light: 1.0,
            seed: mix(t.x.to_bits() as u64, t.y.to_bits() as u64),
        }
    }

    /// A tree the simulator grew: the height its age maps to, the crown the allometry gives that
    /// height, and the light its crown actually receives.
    pub fn grown(x: f32, y: f32, height: f32, light: f32, seed: u64) -> TreeForm {
        let height = height.max(0.0);
        TreeForm {
            x,
            y,
            height,
            crown_base: height * CROWN_BASE_FRACTION,
            crown_radius: height * CROWN_RADIUS_FRACTION,
            light: light.clamp(0.0, 1.0),
            seed,
        }
    }

    /// How full the crown is inside its envelope, from the light at its centre. **This is the one
    /// mapping in the model that is expression rather than measurement**: `light.bin` is the
    /// simulator's and the envelope is allometry, but how densely a shaded crown fills that
    /// envelope is a choice this viewer makes. A crown in deep shade is open enough to see the
    /// branches through; one in full sun is closed.
    pub fn leafiness(&self) -> f32 {
        0.55 + 0.45 * self.light.clamp(0.0, 1.0)
    }

    /// The radius of one leaf cluster. Clusters are solid rather than noise-thinned on purpose:
    /// scattering single leaf voxels is the worst case for greedy meshing -- an isolated voxel is
    /// six quads that merge with nothing -- and that cost lands on every frame, not only on the one
    /// that builds it (MEASUREMENTS.md, "What the branches cost").
    pub fn blob_radius(&self) -> f32 {
        self.crown_radius * (0.22 + 0.26 * self.leafiness())
    }

    /// How many times the crown branches. A tree with no room for a crown is a stick, which is what
    /// a sapling is; past that the depth follows height, so the model shows more of itself on a
    /// bigger tree rather than on a finer lattice.
    pub fn levels(&self, cell_m: f32) -> u8 {
        let depth_cells = (self.height - self.crown_base) / cell_m.max(1e-3);
        if self.height < 2.5 || self.crown_radius <= 0.0 || depth_cells < 2.0 {
            return 0;
        }
        if self.height < 5.0 {
            1
        } else if self.height < 10.0 {
            2
        } else {
            3
        }
    }

    /// The wood, as segments in metres relative to the trunk's base. Deterministic in `seed`.
    pub fn skeleton(&self, cell_m: f32) -> Vec<Limb> {
        let mut out = Vec::new();
        if self.height <= 0.0 {
            return out;
        }
        let mut rng = Rng::new(self.seed);
        let levels = self.levels(cell_m);
        let r0 = (0.02 * self.height).max(0.6 * cell_m);
        // The trunk runs a quarter of the way into the crown and leans a little, so a stand of
        // trees is not a stand of plumb lines.
        let trunk_top = if levels == 0 {
            self.height
        } else {
            self.crown_base + 0.25 * (self.height - self.crown_base)
        };
        let lean = 0.04 * self.height;
        let top = [rng.range(-lean, lean), trunk_top, rng.range(-lean, lean)];
        out.push(Limb {
            a: [0.0, 0.0, 0.0],
            b: top,
            r: r0,
            tip: levels == 0,
        });
        if levels == 0 {
            return out;
        }
        // Breadth-first, so the RNG sequence a tree gets is fixed by the tree and not by the order
        // a recursive walk would happen to visit its branches in.
        let mut open = vec![(top, norm(top), (self.height - trunk_top) * 0.9, r0, 0u8)];
        let mut next = Vec::new();
        while !open.is_empty() {
            for &(at, dir, len, r, depth) in &open {
                let kids = 2 + usize::from(rng.unit() < 0.45);
                for _ in 0..kids {
                    let rd = rng.direction();
                    let mut d = norm([
                        dir[0] * KEEP + rd[0] * SPREAD,
                        dir[1] * KEEP + rd[1] * SPREAD + LIFT,
                        dir[2] * KEEP + rd[2] * SPREAD,
                    ]);
                    // Nothing grows back into the ground.
                    if d[1] < -0.25 {
                        d = norm([d[0], -0.25, d[2]]);
                    }
                    let l = len * rng.range(0.58, 0.78);
                    let at_b = [at[0] + d[0] * l, at[1] + d[1] * l, at[2] + d[2] * l];
                    // The envelope is the allometry's, so a branch that would leave it is pulled
                    // back into it -- in the envelope's own coordinates, not radially. V3 clamped
                    // x and z to the crown radius and y to the tree's height, and every tree over
                    // 10 m came out a bare post: three generations of branch each rising about
                    // three quarters of its length land all 27 tips at the apex, where the
                    // ellipsoid is a point, so `in_envelope` clipped their leaf clusters away to
                    // nothing (MEASUREMENTS.md, V3: what the first close-up showed).
                    let b = self.pull_into_envelope(at_b, TIP_FRAC);
                    let child_depth = depth + 1;
                    let tip = child_depth >= levels || l < 1.2 * cell_m;
                    out.push(Limb {
                        a: at,
                        b,
                        r: (r * 0.62).max(0.5 * cell_m),
                        tip,
                    });
                    if !tip {
                        next.push((b, d, l, r * 0.62, child_depth));
                    }
                }
            }
            std::mem::swap(&mut open, &mut next);
            next.clear();
        }
        out
    }

    /// Moves a point, in metres relative to the trunk's base, to `frac` of the way out to the crown
    /// envelope's wall if it lies further out than that. The scaling is in the envelope's own
    /// coordinates, so a point near the apex is pulled *down* and one near the equator is pulled
    /// *in*, which is what keeps the branch tips spread over the whole crown.
    pub fn pull_into_envelope(&self, p: [f32; 3], frac: f32) -> [f32; 3] {
        let half = 0.5 * (self.height - self.crown_base);
        if half <= 0.0 || self.crown_radius <= 0.0 {
            return [p[0], p[1].clamp(0.0, self.height), p[2]];
        }
        let mid = self.crown_base + half;
        let (u, v, w) = (
            p[0] / self.crown_radius,
            (p[1] - mid) / half,
            p[2] / self.crown_radius,
        );
        let d = (u * u + v * v + w * w).sqrt();
        if d <= frac || d < 1e-6 {
            return p;
        }
        let k = frac / d;
        [p[0] * k, mid + (p[1] - mid) * k, p[2] * k]
    }

    /// Is a point, in metres relative to the trunk's base, inside the crown envelope? The envelope
    /// is the ellipsoid from `crown_base` to `height` with the crown's radius, which is the
    /// silhouette V0 and `ecoview` drew; what V3 changed is what fills it.
    pub fn in_envelope(&self, p: [f32; 3]) -> bool {
        let half = 0.5 * (self.height - self.crown_base);
        if half <= 0.0 || self.crown_radius <= 0.0 {
            return false;
        }
        let mid = self.crown_base + half;
        let fr = (p[0] * p[0] + p[2] * p[2]).sqrt() / self.crown_radius;
        let fz = (p[1] - mid) / half;
        fr * fr + fz * fz <= 1.0
    }
}
