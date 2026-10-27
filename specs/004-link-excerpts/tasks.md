# Tasks: Link Excerpts

**Input**: `specs/004-link-excerpts/spec.md`, `specs/004-link-excerpts/plan.md`

**Prerequisites**: The owner ratifies the specification and
decides D-1 and D-2.

## Phase A: Forms and rendering

- [ ] T001 Remove the `excerpt-<id>` input from
  `templates/pieces.html` and `templates/piece_form.html`;
  remove the handler reads of the field and the form map entry.
- [ ] T002 Remove the text line from
  `templates/audition_detail.html` and
  `templates/piece_detail.html`.
- [ ] T003 Update the tests that assert the text input or the
  text line; add a test that the edit form renders no such
  input.

## Phase B: Write preservation

- [ ] T004 Carry the stored text over the link rewrite in
  `handle_update_piece` and in the link insert of
  `src/db.rs`; new links insert an empty text.
- [ ] T005 Add an integration test: an unrelated piece edit
  leaves every stored link text unchanged.

## Phase C: Mock runner labels

- [ ] T006 Read the extract labels of the mock item link in
  `src/mocks.rs`; carry the labels on the mock item.
- [ ] T007 Render the labels line in `templates/mock_run.html`
  in place of the text line; test both the scoped and the
  whole-piece item.

## Phase D: Rename in the locales

- [ ] T008 Update the `segment.*` values in the six locale
  files to the excerpt name of spec FR-4.
- [ ] T009 Grep the templates and the locale files for the old
  names; assert no user-facing string names the concept
  segment or parts.

## Phase E: Gate and delivery

- [ ] T010 Run the local gate: format, clippy, unit tests,
  integration tests, coverage at or above 90 percent.
- [ ] T011 Open the implementation pull request with the gate
  evidence; the owner merges.
- [ ] T012 Back up the production database, deploy, and run
  the deploy data step of plan Phase E; verify the scoped
  links, the migrated rounds, and the stored texts against
  the pre-change values (SC-5).
