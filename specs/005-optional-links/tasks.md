# Tasks: Optional Links

**Input**: `specs/005-optional-links/spec.md`, `specs/005-optional-links/plan.md`

**Prerequisites**: The owner ratifies the specification.
Decisions D-1 to D-10 stand resolved in the specification.

## Phase A: Storage and writes

- [ ] T001 Add the optional column on `piece_audition`
      in the migration of `src/db.rs`; carry the value on
      the `Audition` record through the link reads.
- [ ] T002 Store the checkbox value in the link inserts
      of the add and update handlers; preserve it on an
      unrelated piece edit.

## Phase B: Forms and rendering

- [ ] T003 Render one optional checkbox per link row in
      `templates/pieces.html` and `templates/piece_form.html`.
- [ ] T004 Render the badge in
      `templates/audition_detail.html` and
      `templates/piece_detail.html` from the `link.optional`
      string.

## Phase C: Locales

- [ ] T005 Add the `link.optional` key to the six locale
      files, with the values of spec FR-7.

## Phase D: Pools

- [ ] T006 Group optional links by (audition, round);
      render one card per pool with two or more members
      in the audition piece list, listing every member.
- [ ] T007 Render one tile per pool with two or more
      members in the audition preparation map, at the
      highest member level.
- [ ] T008 Merge pool-only members into one plan item
      with pooled score inputs (spec D-6); render one
      plan card with a member selector at log time;
      record the session on the picked member only.
- [ ] T009 Plan a member with a mandatory link elsewhere
      normally, and remove it from the plan pool card.

## Phase E: Tests, gate, delivery

- [ ] T010 Add integration tests: checkbox round trip
      through add and edit, badge on both pages,
      preservation on an unrelated edit, default not
      optional, pool card, pool tile, shared weight,
      feedback member choice.
- [ ] T011 Run the local gate: format, clippy, unit
      tests, integration tests, coverage at or above 90
      percent.
- [ ] T012 Open the implementation pull request with the
      gate evidence; the owner merges.
- [ ] T013 Back up the production database, deploy, and
      run the deploy data step of plan Phase E; verify
      SC-5, SC-6, and SC-7 live.
