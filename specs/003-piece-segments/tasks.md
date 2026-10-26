# Tasks: Piece Segments

**Input**: Design documents from `specs/003-piece-segments/`

**Prerequisites**: plan.md (required), spec.md (required,
ratified 2026-10-26)

**Tests**: The specification requires the 90 percent coverage
gate to hold (SC-4). Test tasks ride with each phase, in the
shape of features 001 and 002.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: The task can run in parallel with other [P] tasks.
- **[USn]**: The task serves user story n of the specification.

## Phase 1: Foundation — storage and the one scope definition

- [x] T001 Add the migration to `src/db.rs`: tables `segments`
      and `link_segments`, the partial unique index on
      `(piece_id, alias)`, and `sessions.segment_id` through
      `add_column`; done when the migration test proves a fresh
      database and an upgraded database reach the same schema
      and the migration runs twice without error
- [x] T002 Add the `Segment` struct and the segment queries to
      `src/db.rs`: list by piece, insert, delete, and the
      referenced-by-link count that guards deletion
- [x] T003 [P] Create `src/segments.rs` with endpoint rendering
      (`start`, `end`, `bar`, `mark`, offsets) and the segment
      label function; unit tests cover every anchor kind, a
      positive and a negative offset, and the alias precedence
- [x] T004 [P] Add the scope helper to `src/segments.rs`:
      `counts(session_segment, scope)` for the whole-piece scope
      and for a segment set; unit tests cover the three flows of
      the rule (down, up, never sideways)

**Checkpoint**: Storage migrates, segments render, the scope
rule exists in one place. No surface changes yet.

## Phase 2: User Story 4 and User Story 1 — define and link

- [ ] T005 [US4] Piece detail: segments section and add form in
      `templates/piece_detail.html`, handlers in
      `src/handlers.rs`; done when an owner can define an aliased
      and an unnamed segment on a piece and see both labels
- [ ] T006 [US4] Segment deletion in `src/handlers.rs`: refused
      with a message while any link references the segment;
      allowed otherwise; integration test covers both branches
- [ ] T007 [US1] Link editor: per linked piece with segments,
      one checkbox per segment in
      `templates/audition_detail.html`; saving rewrites the
      `link_segments` rows of the link in `src/handlers.rs`
- [ ] T008 [US1] Program rendering: the audition page lists the
      segment labels of each link beside the piece; a link
      without segments renders exactly as today (SC-3 anchor)

**Checkpoint**: The Haydn case of SC-1 is expressible end to
end: one Cadenza segment, three links, three programs that
differ.

## Phase 3: User Story 3 — sessions and scope readers

- [ ] T009 [US3] `sessions.segment_id` in the log flows: the
      optional segment select in `templates/day.html` and the
      piece page form, shown only when the piece has segments;
      the session lists show the segment label when present
- [ ] T010 [US3] Apply the scope helper in `src/readiness.rs`:
      `readiness_coverage`, `audition_conf_rows`,
      `readiness_study_time`, `readiness_weakest` read
      scope-filtered sessions for segmented links and unchanged
      sessions otherwise
- [ ] T011 [US3] Apply the scope helper to the prep-map tile
      levels of an audition in `src/prepmap.rs`
- [ ] T012 [P] [US3] Unit and integration tests for the scope
      readers: a segment session raises only its scopes, a
      whole-piece session raises every scope of the piece, an
      unsegmented link reads as before

**Checkpoint**: The independent test of story 3 passes on a
seeded database.

## Phase 4: User Story 2 — the plan speaks in parts

- [ ] T013 [US2] Group plan candidates by piece and segment set
      in `fill_today_plan` (`src/handlers.rs`); `PlanItem`
      carries the segment set and every destination audition of
      its group; scoring inputs of an item are its
      scope-filtered sessions
- [ ] T014 [US2] Today card in `templates/today.html`: segment
      label and destination auditions render only for segmented
      items; unsegmented cards are byte-identical to today
- [ ] T015 [P] [US2] Tests for grouping and merging: two links
      with equal piece and set merge into one item; two sets of
      one piece stay two items; an unsegmented day plan equals
      the pre-feature plan on the same fixture

**Checkpoint**: The independent test of story 2 passes; SC-2
holds.

## Phase 5: Verification and converge

- [ ] T016 Run the full local gate: format, lint, unit and
      integration tests, coverage at or above 90 percent of the
      crate with the new modules measured
- [ ] T017 Regression pass on unsegmented fixtures for SC-3:
      pieces, auditions, diary, readiness and plan pages render
      as before the feature
- [ ] T018 Update the specification status and this task list,
      and record the cycle outcome in the pull request body

## Dependencies

- Phase 1 blocks every later phase.
- Phase 2 depends on T001 and T002. Phase 3 depends on T004.
  Phase 4 depends on T004 and T007.
- Phases 2, 3 and 4 can overlap once their dependencies hold;
  the order above is the delivery order of the stories.
