# Implementation Plan: Audition Enrollment

**Branch**: `feat/audition-enrollment` | **Date**: 2026-11-12 | **Spec**: `specs/006-audition-enrollment/spec.md`

**Input**: Feature specification from `specs/006-audition-enrollment/spec.md`, draft for ratification.

## Summary

Add an `enrollments` join table between a minimal
`users` table and `auditions`. The enrollment of
the current user gates the daily plan: the plan gate
`upcoming_auditions` skips a non-enrolled audition,
as it skips an archived or past one, and the
readiness rows shown with the plan cover enrolled
auditions only. The audition detail page gains an
enroll/unenroll toggle, which inserts or deletes the
enrollments row; the auditions list and the detail
page show the state with a badge. A new audition
starts unenrolled, with no enrollments row. Diary
logging does not change: the session handler takes a
piece ID only. A deploy data step sets the
enrollment the owner ordered, through the toggle,
after a database backup: Santa Cecilia and San Carlo
enrolled; Opera Roma, Cagliari Secondo, and Cagliari
Fila not enrolled.

## Technical Context

**Language/Version**: Rust, current crate toolchain
(edition as in `Cargo.toml`).

**Primary Dependencies**: axum, rusqlite (bundled),
minijinja, rust-embed. No new dependency.

**Storage**: SQLite through the existing `migrate()` in
`src/db.rs`. Two additive tables: `users` (ID, name)
and `enrollments` (user ID, audition ID, unique
pair, foreign keys with cascade on delete). The
migration seeds one user (ID 1, Agnese) and
backfills one enrollments row per existing audition
for that user. The `Audition` record carries the
enrolled value through the reads, as the archived
flag does today: the reads join enrollments for the
current user. `App` carries the current user ID
(the seeded user). New auditions insert no
enrollments row.

**Testing**: Unit tests beside the modules they cover;
integration tests in `tests/` for the toggle round
trip, the plan exclusion, and diary logging on a
piece of a non-enrolled audition. The crate coverage
gate of 90 percent must hold (spec SC-5).

**Target Platform**: The existing Linux container on
lobster; no platform change.

**Project Type**: Single project, server-rendered web
application.

**Performance Goals**: Negligible. One join per
audition read. The plan gate adds one boolean check per
linked audition.

**Constraints**: Extra care rules apply: database backup
before the deploy data step; live verification with
real content; never write production data for tests.

## Phases

### Phase A: Storage and reads

Create the `users` and `enrollments` tables in the
migration of `src/db.rs`, seed the one user, and
backfill one enrollments row per existing audition
for that user. Carry the enrolled value on the
`Audition` record through `scan_audition` and every
audition read (`list_auditions`, `piece_auditions`,
`audition_or_404`), joining enrollments for the
current user ID carried by `App`. A new audition
inserts no enrollments row.

### Phase B: Plan gate

Skip a non-enrolled audition in `upcoming_auditions`
in `src/handlers.rs`, next to the archived and past
checks. Skip it in `audition_readiness` the same way.
The pool grouping, the plan chips, and the days and
urgency follow from the same gate, with no second
filter.

### Phase C: Toggle and rendering

Add the enroll/unenroll toggle: one POST route on the
audition detail, one handler that inserts the
enrollments row on enroll and deletes it on
unenroll, then redirects back to the detail page.
Show the state with a badge in
`templates/auditions.html` and
`templates/audition_detail.html`. Render the toggle
form on the detail page only.

### Phase D: Locales

Add the enroll label, the unenroll label, and the
enrolled badge to the six locale files, with the
values of spec FR-8.

### Phase E: Tests, gate, delivery

Add the unit test that extends
`upcoming_filters_and_nearest` with a non-enrolled
audition, and integration tests for the toggle round
trip, the plan exclusion of a piece whose auditions
are all non-enrolled, and diary logging on such a
piece. Run the local gate (`cargo fmt`, `cargo
clippy`, `cargo test`, coverage). Open the
implementation PR only after the owner ratifies the
spec. Deploy with the standard sequence, then run
the data step (spec FR-10), which deletes the three
enrollments rows through the toggle, and verify live
with real content: the plan holds enrolled pieces only, the
Roma-only pieces are absent from the plan with their
log form intact, and the diary still lists the
existing entries.
