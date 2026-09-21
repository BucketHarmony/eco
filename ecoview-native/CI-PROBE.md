# CI probe (shot C1, temporary)

This file exists only to make a commit that touches `ecoview-native/` and nothing else, so that shot C1
can prove the narrow path works: with `[ci-narrow]` in the commit message, such a diff runs the
`ecoview-native` job alone and the six ecosim jobs and the `ecoview` job are skipped.

It is deleted in the commit that closes shot C1. See `.github/DECISIONS.md`, "C1".
