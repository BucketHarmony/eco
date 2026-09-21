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
