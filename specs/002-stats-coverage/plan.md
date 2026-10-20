# Implementation Plan: Stats Module Coverage

**Branch**: `feat/stats-coverage` | **Date**: 2026-10-20 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-stats-coverage/spec.md`

## Summary

Add unit tests inside the test module of `src/stats.rs` until the
line coverage of the file reaches at least 97.0%. The change adds
tests only. The change alters no application behavior, no non-test
code, and no gate value.

## Technical Context

**Language/Version**: Rust, stable toolchain of the repository

**Primary Dependencies**: axum, rusqlite (bundled), minijinja,
rust-embed; cargo-llvm-cov as the coverage tool

**Testing**: unit tests inside the test module of `src/stats.rs`;
the database aggregations run against an in-memory SQLite fixture,
the pattern the test modules of `src/db.rs` and `src/readiness.rs`
already use

**Coverage invocation**: `cargo llvm-cov --locked
--fail-under-lines 90`, the invocation the pipeline already runs;
the per-file measure comes from the JSON report of the same tool

**Baseline (2026-10-20)**: `src/stats.rs` at 94.7% of lines (504 of
532), 28 lines uncovered. The whole crate sits at 91.8%.

**Constraints**: non-test code never changes to satisfy a test
(constitution, principle IV). The diff on non-test code stays empty.

**Scale/Scope**: one file. The uncovered lines sit in the pure
functions (percentile, week aggregation, streak, chart builders)
and in the query closures of the `App` aggregations.

## Constitution Check

- Principle IV (tests and the coverage gate): the plan is this
  principle applied to one file. Test layout follows the toolchain:
  unit tests next to the code.
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

### Pure functions — edge inputs in the module

- `percentile`: an exact rank (the lo == hi path), p at 0 and 100.
- `aggregate_weeks` and `aggregate_conf_weeks`: a day with zero or
  negative minutes, a day with an unparsable date, rows in more
  than one week.
- `week_counts`: days out of chronological order, an unparsable
  date, only invalid days (the empty result), zero-minute days.
- `current_streak`: an unparsable today, an empty day list.

### Chart builders — empty, single, and zero inputs

- `build_dist_chart`: totals with zero and negative entries, and
  the empty list (the zero-height arm).
- `build_week_chart`: the empty list, one week, weeks whose p95 is
  zero (the axis floor).
- `build_conf_chart`: the empty list, one week, two weeks.
- `build_const_chart`: the empty list, counts of zero days only,
  and more than eight weeks (the label stepping).

### App aggregations — an in-memory fixture

Seed auditions, pieces, links, sessions, and mock items in an
in-memory database, then call the aggregations directly:
`piece_totals`, `day_totals`, `audition_time_rows`,
`coverage_rows`, and `diary_stats` on both the seeded fixture and
the empty database. The coverage rows read the real calendar: the
fixture computes its dates relative to `today_str()`, with one
session inside the 14-day window, one outside it, one mock inside
it, one archived audition, and one past audition.

## Risks and Stop Conditions

- An error propagation out of a query (the `?` on a prepare or a
  collect) fires only when the database fails. Without doubles the
  suite cannot reach those lines. Each such line stays uncovered,
  gets recorded here at convergence, and never forces a non-test
  change (spec FR-003).
- The percentage counts the test module lines of the file too.
  Verbose tests that cover nothing still lift the measure; the
  implementation counts that effect as margin, never as the method:
  every new test names a branch of the production code.

## Recorded Unreachable Lines

Eleven lines stay uncovered at the final measure of 98.6%. Every
one resists coverage without a non-test change, so the plan records
the line instead of forcing it (spec FR-003):

- The error propagations of the five query preparations in the
  `App` aggregations (`db.prepare(...)?` in `piece_totals`,
  `day_totals`, `audition_time_rows`, `coverage_rows`, and the
  confidence query of `diary_stats`). The lines fire only when
  SQLite rejects a static query, and the suite holds no double that
  makes a query fail.
- The zero fallback of the percentage in `audition_time_rows`
  (line 596). A row exists only when a positive minute sum exists,
  so the maximum is positive whenever the map runs.
- The `total == 0` guard in `coverage_rows` (line 639). A grouped
  row exists only when at least one piece joins the audition, so
  the total is never zero.

## Verification

1. `cargo test --locked` passes, pre-existing expectations unchanged.
2. `cargo llvm-cov --locked --fail-under-lines 90` passes locally,
   and the JSON report shows `src/stats.rs` at or above 97.0%.
3. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
   pass.
4. The pull request pipeline is green; the master pipeline stays
   green after the merge, and the deploy verification follows the
   production care procedure.

## Project Structure

### Documentation (this feature)

```text
specs/002-stats-coverage/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (repository root)

```text
src/
└── stats.rs           # unit tests added in the module only
```

## Complexity Tracking

No constitution violation to justify.
