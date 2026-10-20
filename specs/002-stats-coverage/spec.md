# Feature Specification: Stats Module Coverage

**Feature Branch**: `feat/stats-coverage`

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "Raise the line coverage of src/stats.rs to at least 97 percent with tests only and no behavior change"

## Clarifications

### Session 2026-10-20

- No clarification was required. The assignment fixes the target
  file, the threshold, and the method: tests only, in the shape of
  feature 001. The measure is the line coverage of `src/stats.rs`
  alone, as the project coverage invocation reports it per file.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - The stats module carries the same proof as the rest (Priority: P1)

The repository owner reads the coverage report. The diary statistics
module, `src/stats.rs`, sits at 94.7% of lines while the crate as a
whole holds 91.8% under a 90% gate. The owner wants the module's own
edge branches — percentile ranks, week aggregation, chart geometry,
and the database aggregations behind the diary page — pinned by
tests, so a green report means the statistics behavior is verified,
not merely compiled.

**Why this priority**: Feature 001 restored the crate gate and left
`stats.rs` as the largest file below 95%. Its uncovered lines are
reachable edge branches, not defensive code. This story closes the
gap; the gate value does not move.

**Independent Test**: Run the coverage invocation on the branch. The
per-file report shows `src/stats.rs` at or above 97.0% of lines,
with the whole crate still at or above the 90% gate.

**Acceptance Scenarios**:

1. **Given** the branch with the new tests, **When** the coverage
   invocation runs, **Then** the report shows at least 97.0% of the
   lines of `src/stats.rs` covered.
2. **Given** the application before and after this change, **When** a
   user exercises the diary page, **Then** the behavior is identical:
   this change adds tests and nothing else.
3. **Given** a line of `src/stats.rs` that resists coverage without a
   non-test change, **When** the implementation reaches it, **Then**
   the plan records the line instead of forcing the coverage.

### Edge Cases

- A line that only a database failure reaches (an error propagation
  out of a query) resists coverage in a suite without doubles. The
  implementation records each such line in the plan.
- The window logic of the coverage rows reads the real calendar.
  Tests seed dates relative to the current day, so the window
  branches hold on any day the suite runs.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The test suite MUST cover at least 97.0% of the lines
  of `src/stats.rs`. The baseline measure on 2026-10-20 is 94.7%
  (504 of 532 lines).
- **FR-002**: The change MUST NOT alter application behavior. The
  diff on non-test code MUST stay empty. New unit tests live inside
  the test module of `src/stats.rs`; integration tests under `tests/`
  only for a branch that the module cannot reach.
- **FR-003**: Non-test code MUST NOT change to satisfy a test. When
  a line resists coverage without a non-test change, the
  implementation MUST record the line in the plan instead of forcing
  the coverage.
- **FR-004**: Every pre-existing test MUST keep passing with its
  expectations unchanged.
- **FR-005**: The whole-crate measure MUST stay at or above the 90%
  gate. The gate value MUST NOT change.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: The coverage report of the final branch shows at least
  97.0% of the lines of `src/stats.rs` covered.
- **SC-002**: The pipeline of the pull request is green.
- **SC-003**: The pull request diff contains no non-test code change.

## Assumptions

- The lines of a file's own test module count in the file measure,
  as cycle 001 observed: new tests raise the numerator and the
  denominator together. The target accounts for that effect.
- The uncovered lines concentrate in reachable branches of pure
  functions and in the query closures of the diary aggregations,
  which an in-memory database fixture exercises without doubles.
