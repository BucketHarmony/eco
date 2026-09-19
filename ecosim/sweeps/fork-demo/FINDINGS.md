# Fork demo: seed 42 with seasonal rain switched off at tick 10000

`ecosim fork runs/s42 --at 10000 --set climate.rain_amp=0 --ticks 10000`. The commands are in `_log.txt`. The whole-run `ecosim stats` output for the parent and the fork is in `stats-s42.txt` and `stats-fork.txt`. Deaths by cause and the ranges over ticks 10001–20000 are in `window.txt`, computed from each `series.csv` by `window.py`. Both runs pass `ecosim check`.

Nothing went extinct: `ecosim stats` reports "first extinction: none" for both runs. What changed is who died and how many. With rain held at its mean, soil moisture stays near saturation: the post-fork minimum of `moisture_mean` is 136, against 62 in the parent's dry seasons. The forest no longer dies back each dry season.
- In the parent, trees fall from 428 to 299 between ticks 13000 and 14000, and from 508 to 163 between 17000 and 18000.
- In the fork, trees stay between 412 and 603 from tick 14000 on, and end at 482, against the parent's 286.

The series first differs at tick 10010, the first soil update after the fork.

Tree death causes aren't recorded. The dry-season crashes are drought deaths (`tree.dry_death_ticks`) by inference, because moisture is the only input the override touches. The denser canopy then shades out the grass: `grass_mean` falls to 0.2 by tick 19000, against 0.5–0.6 in the parent. Grazers follow the grass: their post-fork mean falls from 1044 to 838. Grazer deaths over those 10000 ticks:
- starvation: 7100, against the parent's 8201, because fewer grazers are alive to starve
- predation (`eaten`): 2196, against 2228

Hunters are unaffected: 49 on average in both runs, and 59 of the fork's 61 hunter deaths are old age (parent: 60 of 63). This is the demographic ceiling described in `DECISIONS.md`, "Hunter regulation". So switching off seasonal rain makes this world a closed forest that starves its grazers of grass. The dry season, not the grazers or the hunters, is what keeps the canopy open.
