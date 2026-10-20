# Implementation Plan: Coverage Gate Restoration

**Branch**: `feat/coverage-90` | **Date**: 2026-10-20 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-coverage-90/spec.md`

## Summary

Add tests until the line coverage of the whole crate reaches at
least 90.0%, then raise the CI coverage gate from 80 to 90. The
change adds tests and one gate value. The change alters no
application behavior and no non-test code.

## Technical Context

**Language/Version**: Rust, stable toolchain of the repository

**Primary Dependencies**: axum, rusqlite (bundled), minijinja,
rust-embed; cargo-llvm-cov as the coverage tool

**Testing**: unit tests inside the test module of the file under
test; integration tests under `tests/` through the real router, on a
temporary SQLite fixture per test

**Coverage invocation**: `cargo llvm-cov --locked
--fail-under-lines <gate>`, the invocation the pipeline already runs

**Baseline (2026-10-20)**: 83.4% of lines (4,620 of 5,539). The
target needs at least 366 additional covered lines.

**Constraints**: non-test code never changes to satisfy a test
(constitution, principle IV). The diff on non-test code stays empty
except the gate value in `.github/workflows/push.yml`.

**Scale/Scope**: five files sit below the target. `handlers.rs`
holds 588 uncovered lines; `i18n.rs` 39; `readiness.rs` 36;
`render.rs` 27; `main.rs` 141, counted and intentionally untouched.

## Constitution Check

- Principle IV (tests and the coverage gate): the plan is this
  principle applied. Test layout follows the toolchain: unit tests
  next to the code, integration tests separate.
- Principle II (CI is the source of truth): the local coverage run
  mirrors the pipeline invocation; the merge waits for a green
  pipeline.
- Principle III (one linting system): new test code passes
  `cargo fmt` and `cargo clippy` before every push.
- Project constraint (production care): the merge triggers a deploy
  of unchanged runtime code; the standing backup and live
  verification procedure applies.

No violation. The complexity tracking section stays empty.

## Approach

### handlers.rs — integration tests through the router

The uncovered lines are error and edge branches spread across the
whole file: invalid form values, unknown identifiers, conflict
states, missing fields, wrong content types, boundary values. One
integration test file addition per route area, in `tests/`, reusing
the fixture and request helpers of the existing suite. Each test
names the branch that the test covers in its own name or comment.

### i18n.rs — unit tests in the module

The uncovered lines are locale resolution fallbacks: missing cookie,
unknown language tag, unsupported header values. Unit tests call the
resolution functions with those inputs directly.

### readiness.rs — unit tests in the module

The uncovered lines are score edge inputs: empty histories, extreme
intervals, boundary weights. Unit tests call the scoring functions
with those inputs directly.

### render.rs — unit tests in the module

The uncovered lines are context builders on empty or extreme data.
Unit tests build the contexts with those inputs directly.

### main.rs — no action

The entrypoint lines count in the measure and stay uncovered. The
pool outside the entrypoint holds about 690 uncovered lines against
a need of 366, so the target does not depend on the entrypoint.

### The gate value

`.github/workflows/push.yml`: `--fail-under-lines 80` becomes
`--fail-under-lines 90`, in the same pull request that adds the
tests, after the local measure reaches the target.

## Risks and Stop Conditions

- A defensive branch that no request reaches resists integration
  coverage. The implementation records the branch here and counts the
  reachable pool again. When the reachable pool cannot reach the
  target, the implementation stops and returns the case to the owner
  as a new clarification (spec FR-004). The implementation never
  refactors non-test code for testability on its own authority.
- Multipart error branches need crafted request bodies. The existing
  suite holds a working precedent for the recording upload flow.

## Verification

1. `cargo test --locked` passes, pre-existing expectations unchanged.
2. `cargo llvm-cov --locked --fail-under-lines 90` passes locally.
3. `cargo fmt --check` and `cargo clippy -- -D warnings` pass.
4. The pull request pipeline is green; the master pipeline stays
   green after the merge, and the deploy verification follows the
   production care procedure.

## Project Structure

### Documentation (this feature)

```text
specs/001-coverage-90/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (repository root)

```text
src/
├── handlers.rs        # unit tests only inside its test module
├── i18n.rs            # unit tests added in the module
├── readiness.rs       # unit tests added in the module
└── render.rs          # unit tests added in the module
tests/
└── routes.rs          # integration tests added, grouped by route area
.github/workflows/
└── push.yml           # gate value 80 → 90
```

## Complexity Tracking

No constitution violation to justify.
