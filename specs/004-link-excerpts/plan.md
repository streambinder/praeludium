# Implementation Plan: Link Excerpts

**Branch**: `feat/link-excerpts` | **Date**: 2026-10-27 | **Spec**: `specs/004-link-excerpts/spec.md`

**Input**: Feature specification from `specs/004-link-excerpts/spec.md`, draft for ratification, amended 2026-10-27 after the note decision.

## Summary

Remove the per-link free-text excerpt as the requirement
carrier and rename the segment concept to excerpt in every
user-facing string. The forms lose the requirement text input
and gain a note input per link. The audition page, the piece
page, and the mock runner lose the requirement text line; the
mock runner shows the extract labels of the link instead. The
pages render the note of a link when the note exists. Link
writes store the note that the form carries. Locale values
under the existing `segment.*` keys carry the new name in six
locales. A deploy data step redistributes the existing texts:
the eliminatory round onto five links, the informational
texts into the notes of their links.

## Technical Context

**Language/Version**: Rust, current crate toolchain (edition as
in `Cargo.toml`).

**Primary Dependencies**: axum, rusqlite (bundled), minijinja,
rust-embed. No new dependency.

**Storage**: SQLite through the existing `migrate()` in
`src/db.rs`. One additive change: a note column on
`piece_audition`, added through the `add_column` helper. The
excerpt column leaves every read and write path; no column
drop runs (spec A-2).

**Testing**: Unit tests beside the modules they cover;
integration tests in `tests/` for the form and preservation
flows. The crate coverage gate of 90 percent must hold (spec
SC-4).

**Target Platform**: The existing Linux container on lobster;
no platform change.

**Project Type**: Single project, server-rendered web
application.

**Performance Goals**: Negligible. The mock runner gains one
set lookup per item.

**Constraints**: Extra care rules apply: database backup before
deploy, live verification with real content after deploy, no
writes on production data outside the approved data step.

## Constitution Check

- Library selection: no new dependency. Pass.
- Testing: tests beside the code, coverage gate holds. Pass.
- One repository, one deployable unit: no boundary change.
  Pass.
- Writing standard: the specification, the plan, and the user
  strings follow ASD-STE100. Pass.
- Terminology: one term for one concept in the interface
  (spec FR-4); internal names stay stable (spec FR-8). Pass.

## Project Structure

### Documentation (this feature)

```text
specs/004-link-excerpts/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (affected paths only)

```text
templates/
├── pieces.html          # add form: note input, no text input
├── piece_form.html      # edit form: note input, no text input
├── audition_detail.html # note line, no requirement text line
├── piece_detail.html    # note line, no requirement text line
└── mock_run.html        # labels and note in place of the text
src/
├── handlers.rs          # form reads and writes, mock data
├── db.rs                # note column, link write stores the note
└── mocks.rs             # mock item carries labels and the note
locales/
└── *.json               # segment.* values carry the new name
```

## Approach

### Phase A: Forms and rendering

Remove the `excerpt-<id>` input from `templates/pieces.html`
and `templates/piece_form.html`. Add a `note-<id>` input per
link row in both forms. Remove the requirement text line from
`templates/audition_detail.html` and `templates/piece_detail.html`.
Render the note of the link in its place, labeled as a note.
The handlers stop reading and filling the requirement text.

### Phase B: Note storage and writes

Add the note column on `piece_audition` through the migration.
The link reads return the note beside the scope. The handlers
read the note from the form and pass the value into the link
insert. `handle_update_piece` deletes and reinserts the link
rows, so the form round trip carries the note. A test drives
an add and an unrelated edit and asserts the stored note after
each write.

### Phase C: Mock runner labels

The mock query in `src/mocks.rs` reads the extract labels and
the note of the item link through the same scope read the
piece page uses. `MockItem` carries both; `templates/mock_run.html`
renders the labels line and the note in place of the text line.

### Phase D: Rename in the locales

Update the values of the `segment.*` keys in the six locale
files: title, new, none, parts, field, and every other value
that names the concept. Keys do not change. The Italian values
use Estratto and Estratti; the other locales use the FR-4
equivalents.

### Phase E: Deploy data step

After the pre-deploy backup, redistribute the existing texts
through the edit forms. Set the eliminatory round on the five
Opera Roma links of the solo pieces through the round select.
Set the note of the 13 informational links to their stored
text, word for word, through the note input. The range texts
and the movement repetition texts need no action: the new
interface never reads them. Verify the rounds and the notes
on the audition pages and the piece pages (spec SC-5).

## Risks

- The update handler rewrites link rows on every edit: a field
  that the form round trip misses would reset on the first
  edit after deploy. Phase B and its test gate the risk.
- The redistribution touches 18 links by hand through the
  forms: a wrong value would show at once in the SC-5
  verification against the pre-change inventory.
- The mock item struct feeds one template only; the label read
  must reuse the scope definition of cycle 003, not a copy.
