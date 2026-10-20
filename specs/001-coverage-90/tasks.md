# Tasks: Coverage Gate Restoration

**Input**: Design documents from `specs/001-coverage-90/`

**Prerequisites**: plan.md (required), spec.md (required)

**Tests**: Tests are the deliverable of this feature. Every phase
adds tests and re-measures.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: The task can run in parallel with other [P] tasks.
- **[US1]**: The task serves user story 1, the only story.

## Phase 1: Artifacts

- [x] T001 [US1] Record the owner clarification in spec.md and
      write plan.md and tasks.md under `specs/001-coverage-90/`

## Phase 2: Small Files First

- [ ] T002 [P] [US1] Add unit tests for the locale resolution
      fallbacks in the test module of `src/i18n.rs`; done when the
      file reaches at least 95% of lines
- [ ] T003 [P] [US1] Add unit tests for the score edge inputs in
      the test module of `src/readiness.rs`; done when the file
      reaches at least 95% of lines
- [ ] T004 [P] [US1] Add unit tests for the context builders on
      empty and extreme data in the test module of `src/render.rs`;
      done when the file reaches at least 95% of lines

## Phase 3: Handler Branches

- [ ] T005 [US1] Add integration tests for the invalid-input and
      unknown-identifier branches of the plan and day routes in
      `tests/routes.rs`
- [ ] T006 [US1] Add integration tests for the invalid-input and
      conflict branches of the piece and competition routes in
      `tests/routes.rs`
- [ ] T007 [US1] Add integration tests for the recording upload
      error branches in `tests/routes.rs`
- [ ] T008 [US1] Add integration tests for the remaining branches
      that the coverage report still shows uncovered; record each
      unreachable defensive branch in plan.md instead of forcing it
- [ ] T009 [US1] Re-measure: `cargo llvm-cov --locked` reports at
      least 90.0% of lines for the whole crate

## Phase 4: Gate and Verification

- [ ] T010 [US1] Raise the gate in `.github/workflows/push.yml`
      from `--fail-under-lines 80` to `--fail-under-lines 90`
- [ ] T011 [US1] Run the full local verification: `cargo test
    --locked`, `cargo llvm-cov --locked --fail-under-lines 90`,
      `cargo fmt --check`, `cargo clippy -- -D warnings`
- [ ] T012 [US1] Confirm the diff holds no non-test change outside
      the gate value

## Phase 5: Pull Request and Convergence

- [ ] T013 [US1] Open the pull request with the spec, the plan, the
      tasks, the tests, and the gate value; the pipeline is green
- [ ] T014 [US1] Converge: compare spec.md against the delivered
      change, close every gap in the artifacts, and mark the
      specification complete

## Dependencies and Execution Order

- Phase 2 tasks run in parallel with each other.
- Phase 3 follows Phase 2, so the measure grows in checked steps.
- T009 gates Phase 4: the gate value moves only after the measure
  reaches the target.
- Phase 5 follows a green local verification.
