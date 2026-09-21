# CI probe (shot C1, temporary)

This file exists only to make a commit that touches `ecosim/` and nothing else, so that shot C1 can
prove its CI job filter keeps the **ecosim -> ecoview** edge: an ecosim-only diff must still run the
`ecoview` job, because `scripts/sync-data.sh` copies the simulator's output into `ecoview/public/` and
ecoview's tests render it. Shots G4b and G4c each broke that job from an ecosim-only diff, so a filter
that got this wrong would be worse than no filter at all.

It is deleted in the commit that closes shot C1. See `.github/DECISIONS.md`, "C1".
