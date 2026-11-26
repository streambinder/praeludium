# Implementation Plan: Choice Clusters Count Once at Their Best Member in Readiness

**Cycle**: 012

**Spec**: `specs/012-cluster-readiness/spec.md`

**Branch**: `feat/cluster-readiness-012`

## Phase A: Code

- A-1: Add one shared helper for readiness units.
  It takes the pieces of one audition with their
  links and a preparation map by piece, and it
  returns one unit per choice cluster at the
  maximum member preparation, plus one unit per
  other piece at its own preparation. Rounds play
  no part. A single-member cluster returns that
  piece. A tie goes to the first member in list
  order. Put the helper next to the tile
  computation, where the preparation map already
  lives.
- A-2: Rewrite the 3 mean sites to use the helper:
  `audition_readiness` in `src/handlers.rs`,
  `audition_prep_stats` in `src/prepmap.rs`, and
  the mean inside `stamp_prep_levels` in
  `src/prepmap.rs`. Delete the duplicated mean
  loops. The enrolled and upcoming filter of
  `audition_readiness` stays at the call site.
- A-3: Apply the unit rule in `readiness_coverage`
  in `src/readiness.rs`: a cluster counts as
  touched when any member is touched in the
  window, and the denominator counts units.
- A-4: Apply the unit rule in `readiness_weakest`
  in `src/readiness.rs`: rank units, and show a
  cluster once, as the row of its best member.
- A-5: Change the merged map tile preparation in
  `src/prepmap.rs` from the minimum to the maximum
  member preparation. The level, the summed
  sessions and minutes, and the averaged
  confidence stay as they are.

### Gate A

- The code compiles with no warning.
- No mean over pieces survives in the 3 sites:
  a search for the old loop shape finds nothing.
- No seed file changes in this phase.

## Phase B: Tests

- B-1: Add unit tests for the helper with
  fabricated preparations: the SC-1 case (cluster
  members 0.6, 0.3, 0.0 with singles 0.8 and 0.4
  give mean 0.6); a single-member cluster; a tie
  for the maximum, won by the first member in
  list order; members with different rounds still
  form one unit.
- B-2: Add route tests on a seeded audition with
  one cluster: the headline mean (SC-1), coverage
  3 of 3 through a non-best member (SC-2), the
  weakest list showing the cluster once as its
  best member (SC-3), and the merged tile showing
  the maximum member preparation (SC-4).
- B-3: Run the full local gate: format, clippy
  with warnings denied, all tests, coverage at or
  above 90 percent with the exact CI command.

### Gate B

- The full local gate is green on the exact head.

## Phase C: Deploy and verify

- C-1: Open the implementation PR, one commit per
  phase, CI green and clean.
- C-2: Merge the implementation PR with rebase,
  wait for the master run to go green.
- C-3: Verify live (SC-5): the Santa Cecilia pin
  rises from 45 percent to 51 or 52 percent; the
  San Carlo pin rises from 40 percent to 50 or 51
  percent; coverage on Santa Cecilia reads 9 of
  17; the weakest list of Santa Cecilia no longer
  shows the zero Capriccios. The seed log reads
  inserted 0, updated 0, unchanged, with the
  comparison at 0, 0, 0: no seed change rides this
  deploy.
- C-4: Open the converge PR (spec to Complete,
  tasks checked), merge with rebase on green CI.
