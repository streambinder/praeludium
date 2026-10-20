# Tasks: Stats Module Coverage

**Input**: Design documents from `specs/002-stats-coverage/`

**Prerequisites**: plan.md (required), spec.md (required)

**Tests**: Tests are the deliverable of this feature. Every phase
adds tests and re-measures.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: The task can run in parallel with other [P] tasks.
- **[US1]**: The task serves user story 1, the only story.

## Phase 1: Artifacts

- [x] T001 [US1] Write spec.md, plan.md, and tasks.md under
      `specs/002-stats-coverage/` with the measured baseline

## Phase 2: Pure Functions

- [x] T002 [P] [US1] Add unit tests for the percentile exact rank
      and the aggregation skips (zero minutes, unparsable dates)
      in the test module of `src/stats.rs`
- [x] T003 [P] [US1] Add unit tests for the week count ordering,
      the empty week count result, and the streak fallbacks in the
      test module of `src/stats.rs`

## Phase 3: Chart Builders

- [x] T004 [US1] Add unit tests for the distribution, week,
      confidence, and consistency charts on empty, single, and
      zero inputs in the test module of `src/stats.rs`

## Phase 4: App Aggregations

- [x] T005 [US1] Add unit tests with an in-memory fixture for the
      piece totals, day totals, audition time rows, coverage rows,
      and diary stats in the test module of `src/stats.rs`
- [x] T006 [US1] Re-measure: the JSON report of `cargo llvm-cov
--locked` shows `src/stats.rs` at or above 97.0% of lines;
      record each line that stays uncovered in plan.md

## Phase 5: Verification

- [x] T007 [US1] Run the full local verification: `cargo test
--locked`, `cargo llvm-cov --locked --fail-under-lines 90`,
      `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`
- [x] T008 [US1] Confirm the diff holds no non-test change

## Phase 6: Pull Request and Convergence

- [x] T009 [US1] Open the pull request with the spec, the plan,
      the tasks, and the tests; the pipeline is green
- [x] T010 [US1] Converge: compare spec.md against the delivered
      change, close every gap in the artifacts, and mark the
      specification complete

## Dependencies and Execution Order

- Phase 2 and Phase 3 run in any order; both precede the measure.
- T006 gates Phase 5: the verification runs on the measured state.
- Phase 6 follows a green local verification.
