# Tasks: Link Excerpts

**Input**: `specs/004-link-excerpts/spec.md`, `specs/004-link-excerpts/plan.md`

**Prerequisites**: The owner ratifies the amended
specification. Decisions D-1 and D-2 stand resolved in the
specification.

## Phase A: Forms and rendering

- [ ] T001 Remove the `excerpt-<id>` input from
      `templates/pieces.html` and `templates/piece_form.html`;
      remove the handler reads of the field and the form map entry.
      Add a `note-<id>` input per link row in both forms.
- [ ] T002 Remove the requirement text line from
      `templates/audition_detail.html` and
      `templates/piece_detail.html`. Render the note of the link
      in its place, labeled as a note.
- [ ] T003 Update the tests that assert the text input or the
      text line; add a test that the edit form renders a note
      input and no requirement text input.

## Phase B: Note storage and writes

- [ ] T004 Add the note column on `piece_audition` in the
      migration of `src/db.rs`; return the note in the link reads;
      store the note from the form in the link inserts of the add
      and update handlers.
- [ ] T005 Add an integration test: a note set at add time
      round trips through an unrelated piece edit unchanged.

## Phase C: Mock runner labels

- [ ] T006 Read the extract labels and the note of the mock
      item link in `src/mocks.rs`; carry both on the mock item.
- [ ] T007 Render the labels line and the note in
      `templates/mock_run.html` in place of the text line; test
      the scoped item, the whole-piece item, and the noted item.

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
      links, the migrated rounds, and the notes against the
      pre-change inventory (SC-5).
