# CI decisions

Decisions about `.github/workflows/ci.yml` that the file's own comments are too small to hold, one
section per shot. The three components have their own DECISIONS.md; this covers the pipeline.

## C1: which jobs run, and the lavapipe screenshots

Every push ran all eight jobs whatever it changed, and CI was 42%, 45% and 57% of the wall time of the
three shots before this one. Shot V0a changed one paragraph of markdown and spent about 50 minutes
rendering pictures of a component it had not touched; G4c was red at `ecoview` by 02:29 and still
running at 02:57, same cause.

### Opt-in, not opt-out: `[ci-narrow]`

A new `changes` job computes the push's file list with plain `git diff` and writes three booleans to
`$GITHUB_OUTPUT`; every other job carries `needs: changes` and an `if:` on one of them.

**The full run is the default.** A push whose head commit message says nothing runs all eight jobs; the
narrow run is opted into by putting `[ci-narrow]` in that message. Four other paths also fall back to
the full run: a non-push event, no usable base to diff against (a branch's first push, where
`github.event.before` is all zeros), an empty diff, and any path outside the three component directories.

This direction is the whole design. The opposite scheme — narrow by default, full on request — fails
the first time somebody forgets, and it fails *silently*: a shot gets declared Done on a check that
never ran. Forgetting the marker here costs 30 minutes; forgetting it the other way costs a verdict.

For the same reason the closing push of a shot never carries the marker. `overnight/MASTER.md`'s rule
that GitHub CI green on the shot's commit is part of Done is unchanged, and it now means all eight jobs
on that commit.

### The relation is directed

`scripts/sync-data.sh` copies the simulator's output into `ecoview/public/`, so ecoview's tests and
screenshots run against ecosim's data, and **an ecosim change must run the `ecoview` job**. The reverse
is not true. The table is in the comment above the `changes` job so the next person editing it cannot
miss it. This is not hypothetical: shots G4b and G4c each broke `ecoview` from an ecosim-only diff, and
a filter keyed on the changed directory alone — what most path-filter actions do out of the box — would
have skipped the job that caught both.

### Plain `git diff`, not a marketplace action

`dorny/paths-filter` and friends would do this in fewer lines, but this repo has no third-party actions
beyond the four it already trusts, the directed edge needs a hand-written case either way, and a
supply-chain dependency is a bad trade for fifteen lines of shell.

### The lavapipe screenshots: ten became one

`ecoview-native`'s screenshot step is a measurement and has never been a gate — the only gate in that
job is `mesh golden`, and every step after it carries `continue-on-error: true`. Its answer is already
in `ecoview-native/MEASUREMENTS.md`: 10/10 succeeded, 136–142 s each, every PNG 811,550 bytes. On
b48e7bd the ten took **43m 41s** of a **52m 46s** run, while the job that did all of that shot's actual
work finished in 17m 45s and then waited half an hour.

**And the repetition was not being read.** Those ten runs on b48e7bd came in at **259–267 s each and
815,129 bytes every time** — 1.9× slower than MEASUREMENTS.md still quotes, and a different picture.
That drift sat unread in every run's log for two shots, which argues for the cut rather than against
it: a measurement repeated ten times a push and read zero times a week is not a measurement, it is a
queue. The screenshot this shot kept measured 273 s and 815,129 bytes on run 35565798278, in line with
the ten it replaced. Correcting MEASUREMENTS.md is `ecoview-native`'s work, and is flagged in the backlog.

Of the three options the shot prompt offered, this took **one screenshot as a smoke check**. Limiting
the ten to an `ecoview-native` diff was not enough alone: the closing push of every shot is a full run
by design and would have paid the 44 minutes again every time. Dropping it entirely would have left
nothing watching the headless software-rendered path, the one thing in that component only CI can see —
it needs a Linux box with no GPU, and the developer machine is Windows with an RTX 4090.

What is kept: the path runs end to end, produces a picture and prints its byte count, so a regression
that breaks it turns the log line to FAILED. What is given up: **repetition** — ten runs could show
variance and an intermittent failure, 10/10 rather than 9/10, and one cannot. The paragraph above is
the evidence that the repetition was going unread.

`timeout-minutes` came down from 90 to 30 with the step, as the comment above it said it should: room
for the viewer's release build (8m 21s from a warm cache), one screenshot and the apt install, against
the 13 minutes the job now takes.

### What it saved, measured

| run | head | the push | wall clock | runner minutes | jobs run |
|---|---|---|---|---|---|
| 35562043184 | b48e7bd | before this shot | 52.8 min | 85.1 | 8 of 8 |
| 35565798278 | 864329f | `.github/`, full | **17.9 min** | 46.2 | 8 of 8 |
| 35566907615 | e11423e | `ecosim/`, narrow | 24.4 min | 40.6 | 7 of 8 |
| 35566915166 | 339afc0 | `ecoview-native/`, narrow | **13.3 min** | 13.3 | 1 of 8 |
| 35568881905 | acf3175 | `.github/`, full (closing) | 24.7 min | 53.0 | 8 of 8 |

**Part (b) is where the wall clock went.** A full run no longer waits on `ecoview-native`; the long
pole is now `ecoview`, which took 17.8, 24.2 and 24.5 minutes on three runs of the same suite tonight.
So the honest figure is a saving of **28–35 minutes on every full run**, not a fixed 35, and the two
full runs above (17.9 and 24.7) differ from each other only by that job's variance.

Part (a) mostly buys runner minutes. On an ecosim diff it saves no wall clock at all, because `ecoview`
must run and is the long pole — 24.4 minutes against a full run's 17.9, where the filter skipped a job
that had been running in parallel anyway. Where (a) pays is an `ecoview-native` diff: 13.3 minutes
against 17.9, one job instead of eight, which is exactly the V0a case that prompted the row.

### What this shot did not touch

No gate was weakened, removed or made conditional in a way that lets a shot pass without it. The
coverage floor is still 85%, `MAX_DIFF` is still 0.02, no test is skipped, and the eight jobs keep
their names. This changes *when* jobs run, never *what they assert*.

## C2: the `ecoview` job's wall clock, and where its variance actually comes from

The row this shot came from measured the job at 17.8, 24.2, 24.5 and then **24 min 57 s** against its
own 30-minute timeout — 83% — with one outright failure in between: `edit.spec.ts:331` timed out at
3.0 min and the browser context died with it, and the same test passed in 2.8 min on a re-run of an
identical tree. It told this shot to measure before cutting, "because a suite whose slowest test swings
2x is not a suite whose timeout is the problem".

### The measurement: the suite did not get slower, the runner did

Two green runs of the same job, the slow one (35587962014, 24.9 min) and the fast one (35574305358,
16.3 min), with their per-test durations taken from the `list` reporter's own output. 49 tests are
common to both.

| | slow run | fast run | ratio |
|---|---|---|---|
| all 49 common tests | 876.7 s | 529.9 s | **1.65** |
| `perf.spec.ts:92` draw rate, cam=iso | 198 s | 120 s | 1.65 |
| `perf.spec.ts:92` draw rate, cam=top | 132 s | 78 s | 1.69 |
| `edit.spec.ts:331` pick the block under the crosshair | 168 s | 84 s | **2.00** |
| `sim.spec.ts:81` R runs the simulator on the edits | 54.7 s | 34.1 s | 1.60 |
| `edit.spec.ts:241` saves a bundle byte for byte | 46.0 s | 29.6 s | 1.55 |
| `film.spec.ts:36` 10-frame tiled set is byte-identical | 33.7 s | 23.7 s | 1.42 |

Every test in the suite is between 1.42x and 2.00x slower on the slow run, and the whole-job steps move
with them: step 9 542 → 886 s, film 89 → 130, tiled film 243 → 354, screenshots 19 → 26. **A uniform
factor across independent steps is a slower machine, not a slower test.** GitHub's hosted runners vary
in CPU, and everything in this job renders through SwiftShader on that CPU.

So the row's headline fact — one test swinging 1.4, 2.2, 2.8 minutes and then timing out — is that same
1.65x acting on a test that had nowhere to go. It is not flakiness in the test's own logic.

### What was cut: the two films now run beside the job instead of inside it

New job `ecoview-film` runs `film:check` and `film:tiled:check` on its own runner. On the slow run those
two steps were 2.2 and 5.9 minutes, **8.1 of that job's 24.9**, and they assert nothing the rest of the
job asserts: no test depends on a film and no film depends on a test. The predicted worst case for
`ecoview` is therefore **16.9 min, 56% of its timeout**, with the films finishing around 10 on a runner
of their own.

They are still a gate. `film-check.mjs` compares frame 100 of each film against `shots/reference/02`
cropped to `#view`, exactly as before, and a red in the new job is a red run. Nothing was skipped,
shortened or made `continue-on-error`. The job duplicates `ecoview`'s data step verbatim (about 40 s:
the two 20000-tick runs and `sync-data.sh`) and shares its cargo cache key, which is the price of the
split and is paid on a second runner rather than on the critical path.

### What it actually did, measured after the fact — and the prediction was wrong

Run **35599849884** on ea43603, green in all ten checks in 20.1 min and 50.4 runner minutes:
`ecoview` **19.9 min**, `ecoview-film` **7.9 min** (film 81 s, tiled film 289 s).

19.9, not the 16.9 predicted above, **because that runner was the slowest yet measured**: its 49 tests
summed to 1056.7 s against the slow reference run's 876.7 and the fast one's 529.9 — 1.99x the fast
run, where the pair used to build the prediction spanned only 1.65x. The prediction is left standing
above rather than quietly corrected, because the gap between it and this line is the point: **the job's
duration is a property of the machine it lands on, and any single number for it is a sample.**

What the split is worth is the difference, not the total. On this run the two films cost 6.2 min, so
without it `ecoview` would have been about **26.1 min — 87% of its timeout**, worse than the 83% that
opened the row. With it the same job on the same machine sat at **66%**.

The runner that ran it: **4 vCPU, Intel Xeon Platinum 8370C at 2.80 GHz, 16 GB**, from the new
`which runner` step. Four cores is the whole story of this job — SwiftShader rasterises on them, and
`ecosim`'s release build, Chromium and ffmpeg all queue for them.

### What was not done: `workers` stays 1

A second Playwright worker was the row's own first suspect, and the measurement above argues against
it. Contention would slow every test, and `edit.spec.ts:331` measured **168 s of a 180 s budget** on the
slow runner — 93%, with 7% of headroom. Buying throughput by making each test slower is the one trade
this job cannot afford until that budget is fixed, and the fix is inside a frozen component.

### What was done instead: one retry on CI

`retries: process.env.CI ? 1 : 0` in `ecoview/playwright.config.ts`. A runner 7% slower than the slowest
one measured reddens a shot that changed nothing, which is precisely what happened on 35585550769. The
retry turns that into a slower green run, and Playwright prints the test as flaky, so it stays visible
rather than silent. It costs nothing when nothing fails, and it did not fire on 35599849884.

It is worth more than the margin arithmetic suggested when it was written: the section below shows the
expensive test is not always the same test, so the retry covers a roaming cost rather than one known
slow spot.

This is a mitigation and is written down as one. It is **not** a widened gate: no assertion, tolerance
or floor moved, and a test that fails twice still fails.

### Handed up, because it is inside a frozen component

`ecoview` is frozen for features, and its *test bodies* are out of this shot's scope. One thing in them
is wrong and wants a row of its own — and run 35599849884 corrected this shot's first reading of it,
so what follows is the second reading.

**A two-to-three-minute cost lands on one of `edit.spec.ts:300` and `:331`, and not reliably the same
one.** Every other test in the suite scaled by 1.21 between the slow reference run and 35599849884.
Those two swapped places:

| | fast run | slow run | 35599849884 |
|---|---|---|---|
| `edit.spec.ts:300` places and removes one cube with every hotbar slot | 12.8 s | 19.7 s | **192.0 s** |
| `edit.spec.ts:331` picks the block under the crosshair | 84.0 s | 168.0 s | **37.5 s** |
| the pair, together | 96.8 s | 187.7 s | 229.5 s |
| every other test, summed | 433 s | 689 s | 827 s |

So `test.slow()` at line 301 is not simply attached to the wrong test of the two. **It is attached to
only one of the two tests that need it**, and this run is the evidence: `:300` survived 192 s only
because that marker gives it 540 s, while `:331`, with the 180 s the `describe.configure` at line 92
gives the whole block, hit exactly that ceiling on run 35585550769 and reddened a shot that had
changed nothing. Whichever of the pair the cost lands on is the one that decides the build.

The fix is one line — the block's budget, or the marker on both — and it is the real fix for the
intermittent red. A config shot may not make it. What the cost *is* was not chased: it is inside the
editor's first-person path, which both tests drive and nothing else does.

### Also true, and left alone deliberately

`perf.spec.ts:92` is 5.5 min of the remaining 14.8-minute test step, and its own JSON reports
`timeout_ms: 630000` — a 10.5-minute ceiling on a 3.3-minute measurement. It is not near its ceiling and
it is not this shot's problem; it is a measurement that costs what it costs, and cutting it would be an
ecoview decision about what to measure, not a CI decision about where to run it.

### A runner-identity step

`ecoview` now prints `nproc`, the CPU model and the memory before it does anything. The evidence above
took an hour of log archaeology across five runs; the next person gets it in the first ten lines of a
slow job's log.
