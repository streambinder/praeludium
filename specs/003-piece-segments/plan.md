# Implementation Plan: Piece Segments

**Branch**: `feat/segments-implementation` | **Date**: 2026-10-26 | **Spec**: `specs/003-piece-segments/spec.md`

**Input**: Feature specification from `specs/003-piece-segments/spec.md`, ratified by the merge of the specification pull request on 2026-10-26.

## Summary

Add segments to pieces and scope study by segment set. A segment
is one contiguous range of a piece, stored as two endpoints
(anchor kind, anchor value, offset in bars) with an optional
alias. A piece-audition link references zero or more segments;
zero means the whole piece. Sessions gain an optional segment
reference. Audition-scoped computations (readiness, prep map,
study time) and the daily plan read sessions through the scope
rule of the specification: whole-piece sessions count everywhere,
a segment session counts toward its own scopes and the
whole-piece scope, never sideways. Unsegmented data keeps the
current behavior exactly.

## Technical Context

**Language/Version**: Rust, current crate toolchain (edition as in
`Cargo.toml`).

**Primary Dependencies**: axum, rusqlite (bundled), minijinja,
rust-embed. No new dependency: segments need no library beyond
the existing stack (constitution, library selection).

**Storage**: SQLite through the existing `migrate()` in
`src/db.rs`. New tables ride the `CREATE TABLE IF NOT EXISTS`
batch; the new session column rides the `add_column` helper. The
migration is idempotent and touches no existing row.

**Testing**: Unit tests beside the modules they cover;
integration tests in `tests/` for migration and link flows. The
crate coverage gate of 90 percent must hold (spec SC-4).

**Target Platform**: The existing Linux container on lobster;
no platform change.

**Project Type**: Single project, server-rendered web
application.

**Performance Goals**: Negligible. The data set holds tens of
pieces and hundreds of sessions; scope filtering is a set
membership test on rows already loaded.

**Constraints**: The daily flows keep their current cost when no
segment is involved (spec FR-10). Templates follow the existing
i18n pattern. Non-test code never changes to satisfy a test
(constitution, testing).

**Scale/Scope**: Four storage changes, five surfaces: piece
detail, audition detail, plan (today page), diary logging,
readiness and prep map computations.

## Constitution Check

- One repository, one deployable unit: unchanged.
- Tests beside the code, integration separate, 90 percent gate:
  planned in every phase.
- Library selection: no new dependency.
- Data precondition FR-9: executed on production on 2026-10-24
  and recorded in the specification.
- Deploy care for this application: database backup and live
  verification with real content after the deploy, as for every
  production change.

## Design

### Storage (src/db.rs)

```sql
CREATE TABLE IF NOT EXISTS segments(
    id INTEGER PRIMARY KEY,
    piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
    alias TEXT NOT NULL DEFAULT '',
    start_kind TEXT NOT NULL DEFAULT 'start',
    start_value TEXT NOT NULL DEFAULT '',
    start_offset INTEGER NOT NULL DEFAULT 0,
    end_kind TEXT NOT NULL DEFAULT 'end',
    end_value TEXT NOT NULL DEFAULT '',
    end_offset INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_segments_alias
    ON segments(piece_id, alias) WHERE alias <> '';
CREATE TABLE IF NOT EXISTS link_segments(
    piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
    audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
    segment_id INTEGER NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
    PRIMARY KEY(piece_id, audition_id, segment_id)
);
ALTER TABLE sessions ADD COLUMN segment_id INTEGER
    REFERENCES segments(id) ON DELETE SET NULL;  -- via add_column
```

Anchor kinds: `start`, `end`, `bar`, `mark`. A segment label is
the alias when present, else the rendered range (for example
"bb. 1-123", "A-E", "start-end"). Endpoint rendering lives in
one function with unit tests; it renders the stored form and
never converts between bar and mark scales.

Deleting a segment that a link references is refused at the
application layer (spec story 4, scenario 2); the foreign keys
are the backstop, not the mechanism.

### Scope rule (one definition, several readers)

A pure helper decides, for a session and a link scope, whether
the session counts:

- Scope is the whole piece (link has no segments): every session
  of the piece counts.
- Scope is a segment set: a session counts when its segment is
  null (whole piece) or belongs to the set.

Readers that apply the helper, each only for audition-scoped
figures: `readiness_coverage`, `audition_conf_rows`,
`readiness_study_time`, `readiness_weakest` in
`src/readiness.rs`, and the prep-map tile levels for an audition
in `src/prepmap.rs`. Piece-scoped figures (diary totals, piece
page history) do not change.

### Plan (src/handlers.rs, src/score.rs)

`fill_today_plan` groups candidate links by piece and segment
set. One `PlanItem` per group; the item names every destination
audition of the group. The item's scoring inputs (sessions for
need, recency, rest) are the scope-filtered sessions of the
group. `compute_score` and `allocate_minutes` keep their
signatures; the filtering happens when the inputs are gathered.
An unsegmented group is exactly today's item for the piece.

The today card shows the segment label and the destination
auditions only when the item carries segments (spec FR-6,
FR-10).

### Surfaces

- `piece_detail.html`: a segments section lists the segments of
  the piece with their labels; a form adds a segment (alias
  optional, two endpoint fieldsets); delete is offered and
  refused with a message while a link references the segment.
- `audition_detail.html`: the link editor gains, per linked
  piece that has segments, one checkbox per segment. Saving
  rewrites the `link_segments` rows of that link. The excerpt
  note stays as it is (spec FR-8).
- Program rendering on the audition page lists the segment
  labels of each link next to the piece, as the excerpt note
  renders today.
- Diary logging (`day.html` and the piece page form): when the
  piece has segments, an optional segment select joins the
  minutes and confidence fields. Absent selection posts no
  segment, which is the whole-piece default (spec FR-5).
- The session lists show the segment label when a session
  carries one.

## Project Structure

### Documentation (this feature)

```text
specs/003-piece-segments/
├── spec.md    # ratified 2026-10-26
├── plan.md    # this file
└── tasks.md   # generated from this plan
```

### Source Code (repository root)

```text
src/
├── db.rs         # migration, Segment struct, segment and link queries
├── segments.rs   # endpoint rendering and the scope helper (new)
├── readiness.rs  # scope-filtered audition figures
├── prepmap.rs    # scope-filtered tile levels per audition
├── handlers.rs   # segment forms, link editor, plan grouping, card labels
└── score.rs      # unchanged signatures; inputs gathered per scope
templates/
├── piece_detail.html
├── audition_detail.html
├── today.html
└── day.html
tests/            # migration and link-flow integration tests
```

**Structure Decision**: One new module, `src/segments.rs`, owns
endpoint rendering and the scope helper so every reader applies
one definition. All other changes land in the modules that own
the behavior today.

## Complexity Tracking

No constitution deviation to justify. The one design risk is
dispersion of the scope rule; the plan answers it with the
single helper in `src/segments.rs`.
