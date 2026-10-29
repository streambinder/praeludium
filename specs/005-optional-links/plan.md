# Implementation Plan: Optional Links

**Branch**: `feat/optional-links` | **Date**: 2026-10-29 | **Spec**: `specs/005-optional-links/spec.md`

**Input**: Feature specification from `specs/005-optional-links/spec.md`, draft for ratification.

## Summary

Add a boolean optional marker to the piece-audition link.
The add-piece and edit-piece forms gain one optional
checkbox per link row. The audition page and the piece
page gain a badge on optional links. Optional links that
share one audition and one phase form a pool. A pool
renders as one card in the audition piece list and one
tile in the audition preparation map. A pool of pool-only
members renders as one card in the daily plan, with a
member selector at log time, and counts its audition
weight once. The readiness, the mock runner, the search,
and the stats do not change. A deploy data step marks
the three candidate-choice links of the census, through
the edit-link forms, after a database backup.

## Technical Context

**Language/Version**: Rust, current crate toolchain
(edition as in `Cargo.toml`).

**Primary Dependencies**: axum, rusqlite (bundled),
minijinja, rust-embed. No new dependency.

**Storage**: SQLite through the existing `migrate()` in
`src/db.rs`. One additive change: an optional column on
`piece_audition`, added through the `add_column` helper.
The `Audition` record carries the value through the link
reads, as the note and the round do today.

**Testing**: Unit tests beside the modules they cover;
integration tests in `tests/` for the form, badge, pool
card, pool tile, and shared-weight flows. The crate
coverage gate of 90 percent must hold (spec SC-4).

**Target Platform**: The existing Linux container on
lobster; no platform change.

**Project Type**: Single project, server-rendered web
application.

**Performance Goals**: Negligible. One column read per
link. Pool grouping runs over the links of one audition
or one plan.

**Constraints**: Extra care rules apply: database backup
before the deploy data step; live verification with real
content; never write production data for tests.

## Phases

### Phase A: Storage and writes

Add the optional column in the migration of `src/db.rs`.
Carry the value on the `Audition` record through the
link reads. Store the checkbox value in the link inserts
of the add and update handlers. Preserve the value on an
unrelated piece edit.

### Phase B: Forms and rendering

Render one optional checkbox per link row in
`templates/pieces.html` and `templates/piece_form.html`.
Render the badge in `templates/audition_detail.html`
and `templates/piece_detail.html` from the
`link.optional` string.

### Phase C: Locales

Add the `link.optional` key to the six locale files,
with the values of spec FR-7.

### Phase D: Pools

Group optional links by (audition, round) in the
audition piece list and render one card per pool with
two or more members. Render one tile per such pool in
the audition preparation map, at the highest member
level. In the daily plan, merge pool-only members into
one plan item with pooled score inputs (spec D-6) and
one card with a member selector at log time. Plan a
member with a mandatory link elsewhere normally, and
remove it from the plan pool card.

### Phase E: Gate and delivery

Run the local gate: format, clippy, unit tests,
integration tests, coverage at or above 90 percent.
Open the implementation pull request with the gate
evidence; the owner merges. Back up the production
database, deploy, and run the deploy data step: mark
the three census links optional through the edit-link
forms. Verify SC-5, SC-6, and SC-7 live: the three
links show the badge, no commission-choice link shows
it, and a pool renders as one card, one tile, and one
plan card where its member count allows.
