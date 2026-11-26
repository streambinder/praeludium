# Implementation Plan: Piatti Capriccios as Four Pieces in One Choice Cluster

**Cycle**: 011

**Spec**: `specs/011-piatti-capriccios/spec.md`

**Branch**: `feat/piatti-capriccios-011`

## Phase A: Seed

- A-1: Rename piece 34 in `src/seed_data.rs` to
  work "Capriccio n. 6 (op. 25)" with an empty
  movement. Keep composer, kind, difficulty,
  and target_bpm.
- A-2: Add pieces 50, 51, and 52 in
  `src/seed_data.rs`: nn. 3, 10, and 12, kind
  solo, difficulty 4, target_bpm none.
- A-3: Add links (50, 1), (51, 1), and (52, 1)
  in `src/seed_data.rs`: optional true, cluster
  `piatti`, round null, excerpt and note empty.
- A-4: Set cluster `piatti` on the link (34, 1)
  in `src/seed_data.rs`. Keep optional true.
- A-5: Verify the seed counts by parsing
  `src/seed_data.rs`: 49 pieces, 100 links, 25
  segments, 35 link-segments, 13 rounds.

### Gate A

- The seed parse shows the counts of A-5.
- No other file changes in this phase.

## Phase B: Tests

- B-1: Update the seed-state test in
  `src/seed.rs` (`fresh_database_holds_the_seed`)
  to 49 pieces and 100 links. Update the count
  in `seed_addition_through_a_custom_instance`.
- B-2: Update the count assertions in
  `tests/routes.rs` (`seed_has_no_archived_column`)
  to 49 pieces and 100 links.
- B-3: Add a test that the Santa Cecilia detail
  page shows one composite card with four
  subcards for the `piatti` cluster, and that
  the plan holds one pool card with the four
  members, following `san_carlo_two_choice_clusters`.
  Assert the membership through `app.build_plan`,
  as in cycle 010, because the daily top 8 may
  not surface the card.
- B-4: Run the full local gate: `cargo fmt
--all`, `cargo clippy --all-targets -- -D
warnings`, `cargo test --all-targets`, and
  the exact CI coverage command `cargo llvm-cov
--locked --fail-under-lines 90`.

### Gate B

- The gate of B-4 is green.
- The new test asserts the four members and
  the cluster value.

## Phase C: Deploy and verify

- C-1: Open the implementation PR with one
  commit per phase (one commit when small).
  The PR title is the subject of the head commit.
- C-2: Merge the PR with rebase only on green
  CI and a clean state.
- C-3: Wait for the master run to go green
  (lint, rust, Docker, deploy). No manual data
  step: the deploy applies the seed change.
- C-4: Verify live on lobster: the Santa
  Cecilia detail page shows one composite card
  with four subcards and sixteen single cards;
  the diary page of piece 34 reads "Capriccio
  n. 6 (op. 25)" with the same five sessions;
  the seed log reads `missing=0 extra=0
different=0`.
- C-5: Open the converge PR: the spec moves to
  Complete, the tasks are checked, the PR
  merges with rebase on green CI.

### Gate C

- The live pages show SC-1 and SC-3.
- The seed comparison log reads 0, 0, 0.
- The converge PR merges and the master run
  after it is green.
