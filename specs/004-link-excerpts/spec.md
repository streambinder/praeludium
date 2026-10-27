# Feature Specification: Link Excerpts

**Feature Branch**: `feat/link-excerpts`

**Created**: 2026-10-27

**Status**: Complete (merged 2026-10-27 in PR #44, deployed and verified live the same day; coverage 92.67 percent against the 90 percent gate)

**Input**: User description: "Remove the free-text excerpt field
shown when a piece is added to an audition, now that the segment
field exists. Rename the segment field to excerpt. A link can
clearly hold more than one excerpt, as already seen." Follow-up
of the same day: "Introduce a note field that holds the texts
with real information that cannot be mapped. Move the rest
where it belongs, for example the round."

## Clarifications

### Session 2026-10-27

- "Estratto" in the Italian interface is the user-facing name of
  the concept that cycle 003 implemented as the segment. The
  rename covers user-facing strings only.
- The free-text field is the per-link excerpt text
  (`piece_audition.excerpt`). The interface renders the text on
  the audition page, on the piece page, and in the mock runner.
  The add-piece and edit-piece forms edit the text per link.
- A link already holds more than one extract: the link scope is
  a set of segments. The set model does not change.
- The requirement of a link is expressed only by its extracts.
  A link gains a separate note field. The note holds free
  information about the link. The note never expresses the
  requirement.
- Every existing text moves to its proper place. Ranges and
  movement repetitions leave the link text. The eliminatory
  texts become the structured round of the link. The
  informational texts become the note of the link.

## Hypothesis Challenge

The change rests on one hypothesis: the free text now duplicates
the extracts, so the field can go without loss.

Evidence, from the production data of the five auditions, read on
2026-10-27. The links are 77. The texts classify as follows:

- 26 texts are empty.
- 17 texts are ranges. The link scope expresses every range
  since the cycle 003 encoding of 2026-10-27.
- 12 texts repeat the movement name of the piece. The movement
  field of the piece carries the name.
- 5 texts name the eliminatory round of the link. The round is
  not set on those links. The text is the only phase record.
  The audition defines the round, so a structured migration
  preserves the fact.
- 13 texts carry facts that the extracts do not express: two
  editions, one performance mode (with piano), seven
  instrumental-part specifications, one scoring specification,
  one choice note, one vagueness marker of the program itself.

Conclusion: removal is sound for the range and repetition
classes. The eliminatory texts migrate to the round. The 13
informational texts migrate to the new note field (Decision
D-1). No class loses its fact.

## User Scenarios and Testing

### User Story 1: Link a piece without a requirement text box (Priority: P1)

The owner links a piece to an audition. The form offers the
extracts of the piece, the new-extract block, and a note field.
The form shows no free-text box for the requirement.

**Acceptance**: the add-piece and edit-piece forms contain no
free-text input for a link requirement. The forms contain one
note input per link. The extract checkboxes and the new-extract
block render as before.

### User Story 2: Read a requirement as extracts (Priority: P1)

The owner opens an audition page or a piece page. Each linked
piece shows its extracts. A link with a note shows the note as
a note. No page shows a requirement free text.

**Acceptance**: no rendered page contains a link requirement
text. A scoped link shows its extract labels. A link with a
non-empty note shows the note, labeled as a note.

### User Story 3: Run a mock audition (Priority: P2)

The player runs a mock audition. The current item shows the
extracts that the audition asks for. The item shows the note
of the link when the note exists.

**Acceptance**: a mock item on a scoped link shows the extract
labels of the link. A mock item on a whole-piece link shows no
extract line. A mock item shows the note of its link when the
note is non-empty.

### User Story 4: One name in the interface (Priority: P1)

The owner reads "Estratti" everywhere the interface named the
concept "Segmenti", "Parti", or "Segmento".

**Acceptance**: the parts line, the new-extract block, the
session form field, and the related strings carry the new name
in every locale.

### User Story 5: No information is lost (Priority: P1)

The existing texts move to their proper place at deploy. The
owner finds each informational text as the note of its link.
The owner finds the eliminatory phase as the round of its link.

**Acceptance**: after the deploy data step, the 13 informational
texts are the notes of their links, word for word. The 5
eliminatory links carry the eliminatory round. No link keeps a
requirement text.

## Requirements

- **FR-1**: The add-piece and edit-piece forms MUST NOT render
  a free-text input for a link requirement. The forms MUST
  render one note input per link.
- **FR-2**: The audition page, the piece page, and the mock
  runner MUST NOT render a link requirement text. The pages
  MUST render the note of a link, labeled as a note, when the
  note is non-empty.
- **FR-3**: The mock runner MUST show the extract labels of the
  current item link in place of the requirement text.
- **FR-4**: Every user-facing string that names the segment
  concept MUST name the concept excerpt: it Estratto/Estratti,
  en Excerpt/Excerpts, de Auszug/Auszüge, es Extracto/Extractos,
  fr Extrait/Extraits, pt Excerto/Excertos.
- **FR-5**: Link writes MUST store the note that the form
  carries. An unrelated edit of a piece MUST NOT alter a note
  that the form carried unchanged.
- **FR-6**: The five links whose text names the eliminatory
  round MUST gain that round as their structured round in the
  deploy data step (Decision D-2).
- **FR-7**: The deploy data step MUST move the 13 informational
  texts into the note of their link, word for word. The step
  MUST drop the range texts and the movement repetition texts.
  After the step, no link holds a requirement text.
- **FR-8**: Scopes, sessions, the daily plan, and readiness MUST
  keep the current behavior. The rename is terminology only.
  The removal covers the requirement text only. The note is a
  new, separate value of the link.
- **FR-9**: Existing internal identifiers MUST NOT change:
  tables, columns, types, routes, and locale keys keep the
  segment and excerpt names that the code uses today. The note
  is an addition, not a rename.

## Success Criteria

- **SC-1**: No requirement text input exists in the forms and
  no requirement text renders on the audition, piece, and mock
  pages, by template inspection and tests. A note input exists
  per link in the forms.
- **SC-2**: The mock runner shows extract labels on a scoped
  link and the note on a link that carries one.
- **SC-3**: Every pre-existing test passes. Behavior without
  extracts is unchanged.
- **SC-4**: The crate coverage gate of 90 percent holds.
- **SC-5**: After deploy, the 24 scoped links show the
  "Estratti:" line, the 5 migrated links show the eliminatory
  round, and the 13 informational texts appear as the note of
  their link, word for word.

## Decisions

- **D-1** (resolved 2026-10-27): The 13 informational texts move
  into a dedicated per-link note field. The field is editable
  in the piece forms and renders as a note. The texts do not
  stay hidden in storage, and the deploy step does not clear
  them.
- **D-2** (resolved 2026-10-27): The five eliminatory texts
  migrate to the structured round of the link at deploy.

## Assumptions

- **A-1**: The equivalents in FR-4 are the established terms of
  each locale for an orchestral excerpt.
- **A-2**: The physical column `piece_audition.excerpt` leaves
  every read and write path after the redistribution. No column
  drop runs on the production database in this cycle. The note
  storage is an additive schema change.
