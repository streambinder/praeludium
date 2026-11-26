# Tasks: Choice Clusters Count Once at Their Best Member in Readiness

**Cycle**: 012

**Spec**: `specs/012-cluster-readiness/spec.md`

**Plan**: `specs/012-cluster-readiness/plan.md`

## Phase A: Code

- [x] T001 Add the shared readiness-units helper: one unit per choice cluster at the maximum member preparation, one unit per other piece, rounds ignored, single-member cluster degenerates, tie to the first member in list order (FR-1, FR-2, FR-3, FR-4, D-2)
- [x] T002 Rewrite `audition_readiness` to use the helper and delete its mean loop (FR-5)
- [x] T003 Rewrite `audition_prep_stats` to use the helper and delete its mean loop (FR-5)
- [x] T004 Rewrite the mean inside `stamp_prep_levels` to use the helper and delete its mean loop (FR-5)
- [x] T005 Apply the unit rule in `readiness_coverage`: cluster touched through any member, denominator counts units (FR-6, D-3)
- [x] T006 Apply the unit rule in `readiness_weakest`: cluster once, as the row of its best member (FR-7, D-4)
- [x] T007 Change the merged map tile preparation from the minimum to the maximum member preparation (FR-8, D-5)

## Phase B: Tests

- [x] T008 Add helper unit tests: SC-1 case, single-member cluster, tie to the first member, rounds ignored (SC-1)
- [x] T009 Add route tests: headline mean, coverage 3 of 3 through a non-best member, weakest list with the cluster once, merged tile at the maximum (SC-2, SC-3, SC-4)
- [x] T010 Run the full local gate: format, clippy with warnings denied, all tests, coverage at or above 90 percent with the exact CI command (SC-6)

## Phase C: Deploy and verify

- [x] T011 Open the implementation PR, one commit per phase, CI green and clean (SC-6)
- [x] T012 Merge the implementation PR with rebase, wait for the master run to go green (SC-6)
- [x] T013 Verify live (SC-5): Santa Cecilia pin 51 or 52 percent, San Carlo pin 50 or 51 percent, coverage 9 of 17, weakest list without the zero Capriccios, seed log with no change
- [x] T014 Open the converge PR (spec to Complete, tasks checked), merge with rebase on green CI
