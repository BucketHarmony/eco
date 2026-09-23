# Shot G6 — storm drains

The prompt is `overnight/shots/G6-drain-pipes.md`. The Capitol's four pipes (`worlds/capitol/pipes.json`)
now take water. Each inlet sits on asphalt. During the storm pass it captures what arrives at its
ground cell, up to `capacity_m3h × tick hours × pipes.capacity_scale`, and hands it to its outlet.
All four outlets are on the crop edge, so everything the pipes take leaves the world.

**The Capitol's pipes are illustrative, not real drains.** Every entry in `pipes.json` says
`"illustrative": true`. They are four points on the site's streets, placed when the scene was built,
each given a round 50 m³/h. They are not the City of Lansing's storm sewer, and nothing below
describes how the real grounds drain. What it describes is what this mechanism does on a real site's
ground.

Runs: `sweep.sh` covers `pipes.capacity_scale` at 0, 0.25, 0.5, 1, 2 and 4, which is the prompt's
grid. None of those values binds, so 0.001, 0.005 and 0.02 were added. That makes 27 runs: the
Capitol, seeds 1–3, 20000 ticks, with the two overrides every bundle run carries (animals off,
rainfall flat). Wall time was 89 s for the prompt's 18 cells and 43 s for the 9 added ones, run
together on one machine. `analyse.py` writes `scan.csv` and the tables below.

## Event-log causes

Tree deaths by cause, summed over seeds 1–3. Animals are off, so trees are the only deaths.

| scale | burnt | crowded | drought | old_age |
|---|---|---|---|---|
| 0 | 7752 | 19795 | 18670 | 731 |
| 0.001 | 7752 | 19795 | 18670 | 731 |
| 0.005 | 7752 | 19795 | 18670 | 731 |
| 0.02 | 7752 | 19795 | 18670 | 731 |
| 0.25 – 4 | 7752 | 19795 | 18670 | 731 |

**The drains change no tree's fate.** Every tree death happens at the same tick, from the same cause,
at every scale. At seed 1, scale 0 against scale 1, every `entities.json` in all 21 snapshots is
byte-identical. The inlets are on asphalt, and the water they take would have run over asphalt and
lawn toward the crop edge. No trunk column is near enough to them for its soil water to cross the
drought line. The only new rows in the log are the drains' own. There are 4 `pipe` rows per storm,
2404, 2716 and 2588 on the three seeds, one per pipe per storm.

## Capture against capacity

Means over seeds 1–3. Water volumes are in m³ over the whole run, and ponding and waterlogging are
averaged over the 20001 series rows.

| scale | captured m³ | overflow m³ | overflowing pipe-storms | ponded mm, mean | ponded mm, max | waterlogged share |
|---|---|---|---|---|---|---|
| 0 | 0 | 0 | 0 | 2.7862 | 6.1153 | 0 |
| 0.001 | 24.9 | 48.5 | 144 | 2.7848 | 6.1130 | 0 |
| 0.005 | 54.1 | 19.3 | 32 | 2.7843 | 6.1127 | 0 |
| 0.02 | 70.1 | 3.3 | 3.3 | 2.7838 | 6.1127 | 0 |
| 0.25 – 4 | 73.4 | 0 | 0 | 2.7837 | 6.1117 | 0 |

- **Capacity never binds at the shipped scale.** A tick is 2.19 h, so one pipe at 50 m³/h can take
  110 m³ in a storm. The largest single storm any pipe saw is 5.4 m³, at `pipe_1` on seed 1. The
  median is 2 litres. Below about 0.05 the pipe fills during the big storms and the rest overflows
  downhill. At 0.001 two thirds of what reaches the inlets goes on over the ground. Scales 0.25 and 4
  give identical run directories, apart from `meta.json`, which records the scale. `ecosim diff`
  was run on seeds 1 and 3.
- **The pipes are a rounding error in the site's water.** Rain is 3568 mm on seed 1. Surface flow
  over the edge is 849 mm at scale 0 and 848 mm at scale 1. The drains take 1.12 mm of it, a mean
  over the 256 m square, because each inlet catches only the few square metres of street that drain
  to its one ground cell. Most of that, 65 of 73 m³, is `pipe_1`. The other three catch about 1, 1
  and 2.5 m³ in five years.
- **Ponding and waterlogging.** Mean ponded depth falls by 0.09% between scale 0 and full capture.
  The waterlogged share is 0 on every row of every run with or without drains: the Capitol's
  plantable ground never stays saturated for the waterlogging clock's duration. What the drains do
  to ponding is local. Water that would have pooled in the street's low cells downhill of an inlet
  leaves instead, and a site-wide mean dilutes that almost to nothing.

## Nitrogen and phosphorus: through the pipes against over the edge

These are grams over the whole run, means over seeds 1–3. They come from the nutrient ledger's
running outflow in the final `state.bin`, which counts both routes out, split with the pipe rows'
N and P.

| scale | N down the pipes | N over the edge | P down the pipes | P over the edge |
|---|---|---|---|---|
| 0 | 0 | 16261 | 0 | 247 690 |
| 0.001 | 29 | 16232 | 241 | 247 450 |
| 0.005 | 69 | 16192 | 699 | 246 990 |
| 0.02 | 94 | 16167 | 1027 | 246 660 |
| 0.25 – 4 | 99 | 16163 | 1105 | 246 590 |

The pipes carry 0.6% of the nitrogen and 0.45% of the phosphorus that leaves the site. The totals
leaving are unchanged to within 0.01%: pipe plus edge at full capture equals edge alone at scale 0
(16262 g against 16261 g of N, and 247 695 g against 247 690 g of P). That
is what an edge outlet should do. It changes the route, not the amount. That share is higher than the
water's, 0.13% of edge outflow, so the water that reaches an inlet carries more load per litre
than the site's mean runoff. This was not traced further.

## Lawn cover near the inlets

| scale | grass in the four inlet patches | surface moisture within 3 m of an inlet (0–255) |
|---|---|---|
| 0 | 0.5495 | 89.73 |
| 0.001 | 0.5499 | 89.59 |
| 0.005 – 4 | 0.5500 | 89.58 |

Both are averaged over the snapshots from tick 12000 to 20000. Lawn cover in the inlet patches does
not measurably change, +0.0005. Surface moisture near an inlet falls by 0.15 of 255. The water a pipe
takes was already on asphalt and mostly headed off the site, so the lawn beside the street loses very
little of it.

## What this says

On the Capitol the drains are too small in catchment to matter, and too large in capacity to fill.
Each inlet is one 0.5 m ground cell on a street, and the flow graph gives it only what runs to that
cell. The mechanism does what the prompt asks:

- it captures up to capacity and lets the rest overflow;
- it carries the load in the water's share;
- it empties over the edge or onto the ground downhill;
- it keeps both balances exact.

The synthetic tests in `src/hydro.rs` exercise the regime the Capitol never reaches: a V valley
that fills its pipe and then ponds, and an interior outlet feeding the next depression. A drain
network that matters to the Capitol's ecology would need a scene with real inlets, each draining a
real catchment. That is a scene question, not a sim question, and nothing here tunes around it:
`capacity_scale` stays at 1.
