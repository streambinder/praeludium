# Implementation Plan: Link Excerpts

**Branch**: `feat/link-excerpts` | **Date**: 2026-10-27 | **Spec**: `specs/004-link-excerpts/spec.md`

**Input**: Feature specification from `specs/004-link-excerpts/spec.md`, draft for ratification.

## Summary

Remove the per-link free-text excerpt from the interface and
rename the segment concept to excerpt in every user-facing
string. The forms lose the text input. The audition page, the
piece page, and the mock runner lose the text line; the mock
runner shows the extract labels of the link instead. Link
writes preserve the stored text: the update handler rewrites
the link rows, so the handler carries the stored value over.
Locale values under the existing `segment.*` keys carry the
new name in six locales. A deploy data step sets the
eliminatory round on the five links whose text names that
round.

## Technical Context

**Language/Version**: Rust, current crate toolchain (edition as
in `Cargo.toml`).

**Primary Dependencies**: axum, rusqlite (bundled), minijinja,
rust-embed. No new dependency.

**Storage**: SQLite through the existing `migrate()` in
`src/db.rs`. No schema change: the column stays (spec A-2).

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
├── pieces.html          # add form: drop the text input
├── piece_form.html      # edit form: drop the text input
├── audition_detail.html # drop the text line
├── piece_detail.html    # drop the text line
└── mock_run.html        # labels in place of the text
src/
├── handlers.rs          # form reads, write preservation, mock data
├── db.rs                # link write carries the stored text
└── mocks.rs             # mock item carries extract labels
locales/
└── *.json               # segment.* values carry the new name
```

## Approach

### Phase A: Forms and rendering

Remove the `excerpt-<id>` input from `templates/pieces.html`
and `templates/piece_form.html`. Remove the text line from
`templates/audition_detail.html` and `templates/piece_detail.html`.
The handlers stop reading the form field and stop filling the
form map for the text. The `Audition.excerpt` value may stay
populated in the data layer (spec FR-8); no template reads it.

### Phase B: Write preservation

`handle_update_piece` deletes and reinserts the link rows. The
handler first reads the stored text per linked audition and
passes the value into the insert. `handle_add_piece` inserts
an empty text for new links. A test drives an unrelated edit
and asserts the stored text after the edit.

### Phase C: Mock runner labels

The mock query in `src/mocks.rs` reads the extract labels of
the item link through the same scope read the piece page uses.
`MockItem` carries the labels; `templates/mock_run.html`
renders the labels line in place of the text line.

### Phase D: Rename in the locales

Update the values of the `segment.*` keys in the six locale
files: title, new, none, parts, field, and every other value
that names the concept. Keys do not change. The Italian values
use Estratto and Estratti; the other locales use the FR-4
equivalents.

### Phase E: Deploy data step

After deploy, set the eliminatory round on the five Opera Roma
links of the solo pieces through the edit form round select.
Verify the round on the audition page and the piece page.
Under variant B of D-1, the step also clears the stored texts
after the pre-deploy backup check; under variant A the step
leaves every stored text untouched.

## Risks

- The update handler rewrites link rows on every edit: a
  missed carry-over would erase stored texts on the first edit
  after deploy. Phase B and its test gate the risk.
- The mock item struct feeds one template only; the label read
  must reuse the scope definition of cycle 003, not a copy.
