# CI decisions

Decisions about `.github/workflows/ci.yml` that the file's own comments are too small to hold. One
section per shot. `ecosim/DECISIONS.md`, `ecoview/DECISIONS.md` and `ecoview-native/DECISIONS.md`
cover the three components; this file covers the pipeline that runs them.

## C1: which jobs run, and the lavapipe screenshots

### The problem

Every push ran all eight jobs whatever it changed. CI was 42%, 45% and 57% of the wall time of the
three shots before this one. Two measured examples: shot V0a changed one paragraph of markdown in
`ecoview-native/MEASUREMENTS.md` and spent about 50 minutes on a step that renders pictures; shot G4c
was red at `ecoview` by 02:29 and its run was still going at 02:57, for the same reason.

### Opt-in, not opt-out: `[ci-narrow]`

A new `changes` job computes the push's file list with `git diff` and writes three booleans to
`$GITHUB_OUTPUT`; every other job carries `needs: changes` and an `if:` on one of them.

**The full run is the default.** A push whose head commit message says nothing runs all eight jobs.
The narrow run is opted into by putting `[ci-narrow]` in that message. Three other paths also fall
back to the full run: a non-push event, no usable commit to diff against (a branch's first push, where
`github.event.before` is all zeros), and any changed path outside the three component directories.

This direction is the whole design. The opposite scheme — narrow by default, full on request — fails
the first time somebody forgets, and it fails silently: a shot gets declared Done on a check that never
ran, and nobody finds out until the check is needed. Forgetting the marker here costs 30 minutes.
Forgetting it the other way round costs a wrong verdict. The shot prompt named this as non-negotiable
and it is right.

For the same reason the closing push of a shot never carries the marker. `overnight/MASTER.md`'s rule
that GitHub CI green on the shot's commit is part of Done is unchanged by this shot, and it now means
all eight jobs on that commit.

### The relation is directed

`scripts/sync-data.sh` copies the simulator's output into `ecoview/public/`, so ecoview's tests and
screenshots run against ecosim's data, and **an ecosim change must run the `ecoview` job**. The reverse
is not true. The table is in the comment above the `changes` job so that the next person editing it
cannot miss it.

This is not hypothetical: shots G4b and G4c each broke `ecoview` from an ecosim-only diff. A filter
keyed on the changed directory alone — which is what most path-filter actions do out of the box — would
have skipped the job that caught both.

### Plain `git diff`, not a marketplace action

`dorny/paths-filter` and friends would do the same work in fewer lines. This repo has no third-party
actions in its workflow beyond the four GitHub and Rust ones it already trusts, adding a supply-chain
dependency to save fifteen lines of shell is a bad trade for a project whose whole verification story
is "one command that exits non-zero", and the directed edge needs a hand-written case either way. The
shot prompt preferred the plain job; having written it, so do I.

### The lavapipe screenshots: ten became one

`ecoview-native`'s screenshot step is a measurement and has never been a gate — the only gate in that
job is `mesh golden`, and every step after it carries `continue-on-error: true`. Its answer is already
recorded in `ecoview-native/MEASUREMENTS.md`: 10/10 succeeded, 136–142 s each, every PNG 811,550 bytes.
Shot V0a reproduced it. On b48e7bd the ten took **43m 41s** of a **52m 46s** run; the job that did all
of that shot's actual work finished in 17m 45s and then waited half an hour.

Of the three options the prompt offered, this shot took **one screenshot as a smoke check**. Cutting it
to an `ecoview-native`-only diff was not enough on its own, because the closing push of every shot is a
full run by design and would have paid the 44 minutes again every time. Dropping it entirely would have
left nothing watching the headless software-rendered path, which is the one thing in that component
that only CI can see — it needs a Linux box with no GPU, and the developer machine is Windows with an
RTX 4090.

What is kept: the path still runs end to end and still produces a picture, so a regression that breaks
it turns the step's log line to FAILED. What is given up: **repetition**. Ten runs measured variance —
136 to 142 seconds, and 10/10 rather than 9/10 — and one run cannot. A timing regression or an
intermittent failure will no longer be visible here. That is the trade, made deliberately: the variance
question has been asked and answered twice, and re-asking it costs 40 minutes of every run.

`timeout-minutes` on that job came down from 90 to 30 with the step, as the comment above it always
said it should. 30 leaves room for the release build of the viewer (8m 21s on b48e7bd, from a warm
cache) plus one screenshot plus the apt install, against the roughly 14 minutes the job now takes.

### What this shot did not touch

No gate was weakened, removed or made conditional in a way that lets a shot pass without it. The
coverage floor is still 85%, `MAX_DIFF` is still 0.02, no test is skipped, and the eight jobs keep
their names. This shot changes *when* jobs run, never *what they assert*.
