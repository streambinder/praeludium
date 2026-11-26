# Tasks: Piatti Capriccios as Four Pieces in One Choice Cluster

**Cycle**: 011

**Spec**: `specs/011-piatti-capriccios/spec.md`

**Plan**: `specs/011-piatti-capriccios/plan.md`

## Phase A: Seed

- [ ] T001 Rename piece 34 in `src/seed_data.rs` to work "Capriccio n. 6 (op. 25)" with an empty movement (FR-1, D-1)
- [ ] T002 Add pieces 50, 51, and 52 in `src/seed_data.rs`: nn. 3, 10, and 12, kind solo, difficulty 4, target_bpm none (FR-2, D-2, D-4)
- [ ] T003 Add links (50, 1), (51, 1), and (52, 1) in `src/seed_data.rs`: optional true, cluster `piatti`, round null (FR-3, D-3)
- [ ] T004 Set cluster `piatti` on the link (34, 1) in `src/seed_data.rs` (FR-4)
- [ ] T005 Verify the seed counts by parsing `src/seed_data.rs`: 49 pieces, 100 links, 25 segments, 35 link-segments, 13 rounds (SC-4)

## Phase B: Tests

- [ ] T006 Update the seed-state test in `src/seed.rs` (`fresh_database_holds_the_seed`) to 49 pieces and 100 links, and the count in `seed_addition_through_a_custom_instance` (SC-4)
- [ ] T007 Update the count assertions in `tests/routes.rs` (`seed_has_no_archived_column`) to 49 pieces and 100 links (SC-4)
- [ ] T008 Add a test for the `piatti` cluster: one composite card with four subcards on the Santa Cecilia detail page, and one pool card with four members in the plan, asserted through `app.build_plan` (SC-1, SC-2)
- [ ] T009 Run the full local gate: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo llvm-cov --locked --fail-under-lines 90` (SC-5)

## Phase C: Deploy and verify

- [ ] T010 Open the implementation PR, one commit per phase, CI green and clean (SC-5)
- [ ] T011 Merge the implementation PR with rebase, wait for the master run to go green (SC-5)
- [ ] T012 Verify live: Santa Cecilia detail shows one composite card with four subcards and sixteen single cards; diary page of piece 34 reads "Capriccio n. 6 (op. 25)" with the same five sessions; seed log reads `missing=0 extra=0 different=0` (SC-1, SC-3, SC-4)
- [ ] T013 Open the converge PR (spec to Complete, tasks checked), merge with rebase on green CI
