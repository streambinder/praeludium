# Feature Specification: Coverage Gate Restoration

**Feature Branch**: `feat/coverage-90`

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "Raise line coverage to at least 90 percent and restore the CI coverage gate to 90"

## Clarifications

### Session 2026-10-20

- Q: Does the 90% measure include `src/main.rs`, the binary entrypoint,
  which sits at 0%? → A: Counted. The owner decided on 2026-10-20: the
  measure covers the whole crate, entrypoint included. The target needs
  at least 366 additional covered lines, and the implementation reaches
  the target through the handler, i18n, readiness, and render branches.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - The coverage gate enforces the standard again (Priority: P1)

The repository owner merges a change. The pipeline measures line
coverage. The pipeline fails the change when coverage falls below
90%, and passes the change when coverage stays at or above 90%. The
owner reads the failure as a quality signal, not as noise: the gate
matches the project standard again, so a green pipeline means the
standard holds.

**Why this priority**: The gate sits at 80 since the Rust rewrite,
below the 90% minimum of the project standard. Every merge since the
rewrite relied on a weaker gate than the standard prescribes. This
story restores the enforcement; no other story matters if the gate
does not move.

**Independent Test**: Run the pipeline on a branch that adds no tests.
The coverage step reports the current measure, near 83.4%. Raise the
gate value to 90 in the same run: the step fails. Add the new tests:
the step passes.

**Acceptance Scenarios**:

1. **Given** the branch with the new tests, **When** the pipeline
   measures coverage, **Then** the report shows at least 90.0% of lines
   covered and the coverage step passes with the gate set to 90.
2. **Given** a change that removes or weakens a new test, **When** the
   pipeline measures coverage below 90%, **Then** the coverage step
   fails and the merge is blocked.
3. **Given** the application before and after this change, **When** a
   user exercises any page or flow, **Then** the behavior is identical:
   this change adds tests and one gate value, nothing else.

### Edge Cases

- A handler branch that no request can reach (a defensive branch)
  resists coverage through the router. The implementation records each
  such branch in the plan instead of forcing coverage through a
  non-test change.
- A branch that only a startup failure reaches (an unreadable database
  file, a port already bound) belongs to the entrypoint. The entrypoint
  lines count in the measure and stay uncovered: the plan reaches the
  target without them.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The test suite MUST cover at least 90.0% of the lines
  that the project coverage invocation measures. The baseline measure
  on 2026-10-20 is 83.4% (4,620 of 5,539 lines).
- **FR-002**: The pipeline coverage step MUST fail below 90%. The step
  MUST use the same invocation that produced the baseline measure,
  with the gate value raised from 80 to 90 in the same change that
  adds the tests.
- **FR-003**: The change MUST NOT alter application behavior. The diff
  on non-test code MUST stay empty, except the gate value in the
  workflow. New unit tests live inside the test modules of the file
  under test; new integration tests live under `tests/`.
- **FR-004**: Non-test code MUST NOT change to satisfy a test. When a
  line resists coverage without a non-test change, the implementation
  MUST stop and return the case as a clarification instead of changing
  the code silently.
- **FR-005**: Every pre-existing test MUST keep passing with its
  expectations unchanged.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: The coverage report of the final branch shows at least
  90.0% of lines covered under the clarified measure.
- **SC-002**: The pipeline of the pull request is green with the gate
  set to 90, and the master pipeline stays green after the merge.
- **SC-003**: The pull request diff contains no non-test code change
  outside the workflow gate value.

## Assumptions

- The measure is the line coverage of the whole crate, as the project
  coverage invocation reports it today. The clarification at the top
  of this document confirms the measure: no file is excluded.
- The uncovered lines concentrate in reachable error and edge branches
  of the request handlers (588 of 919 uncovered lines). Integration
  tests through the real router cover those branches without doubles,
  because the application takes its dependencies as plain values.
- The deployment of this change follows the standing production care
  procedure, although the change alters no runtime code.
