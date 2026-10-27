# Feature Specification: Link Excerpts

**Feature Branch**: `feat/link-excerpts`

**Created**: 2026-10-27

**Status**: Draft

**Input**: User description: "Remove the free-text excerpt field
shown when a piece is added to an audition, now that the segment
field exists. Rename the segment field to excerpt. A link can
clearly hold more than one excerpt, as already seen."

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
residual texts need an explicit decision (Open Decision D-1).

## User Scenarios and Testing

### User Story 1: Link a piece without a text box (Priority: P1)

The owner links a piece to an audition. The form offers the
extracts of the piece and the new-extract block. The form shows
no free-text box for the requirement.

**Acceptance**: the add-piece and edit-piece forms contain no
free-text input for a link requirement. The extract checkboxes
and the new-extract block render as before.

### User Story 2: Read a requirement as extracts (Priority: P1)

The owner opens an audition page or a piece page. Each linked
piece shows its extracts. No page shows the free text.

**Acceptance**: no rendered page contains the stored free text
of a link. A scoped link shows its extract labels.

### User Story 3: Run a mock audition (Priority: P2)

The player runs a mock audition. The current item shows the
extracts that the audition asks for.

**Acceptance**: a mock item on a scoped link shows the extract
labels of the link. A mock item on a whole-piece link shows no
extract line.

### User Story 4: One name in the interface (Priority: P1)

The owner reads "Estratti" everywhere the interface named the
concept "Segmenti", "Parti", or "Segmento".

**Acceptance**: the parts line, the new-extract block, the
session form field, and the related strings carry the new name
in every locale.

## Requirements

- **FR-1**: The add-piece and edit-piece forms MUST NOT render
  a free-text input for a link requirement.
- **FR-2**: The audition page, the piece page, and the mock
  runner MUST NOT render the link free text.
- **FR-3**: The mock runner MUST show the extract labels of the
  current item link in place of the free text.
- **FR-4**: Every user-facing string that names the segment
  concept MUST name the concept excerpt: it Estratto/Estratti,
  en Excerpt/Excerpts, de Auszug/Auszüge, es Extracto/Extractos,
  fr Extrait/Extraits, pt Excerto/Excertos.
- **FR-5**: Link writes MUST preserve the stored free-text
  values unchanged. An unrelated edit of a piece MUST NOT alter
  a stored text (variant A of D-1).
- **FR-6**: The five links whose text names the eliminatory
  round MUST gain that round as their structured round in the
  deploy data step (D-2).
- **FR-7**: Scopes, sessions, the daily plan, and readiness MUST
  keep the current behavior. The rename is terminology only.
  The removal covers presentation and forms only.
- **FR-8**: Internal identifiers MUST NOT change: tables,
  columns, types, routes, and locale keys keep the segment and
  excerpt names that the code uses today.

## Success Criteria

- **SC-1**: No free-text requirement input exists in the forms
  and no free text renders on the audition, piece, and mock
  pages, by template inspection and tests.
- **SC-2**: The mock runner shows extract labels on a scoped
  link.
- **SC-3**: Every pre-existing test passes. Behavior without
  extracts is unchanged.
- **SC-4**: The crate coverage gate of 90 percent holds.
- **SC-5**: After deploy, the 24 scoped links show the
  "Estratti:" line, the 5 migrated links show the eliminatory
  round, and the stored texts match the pre-change values
  (variant A of D-1).

## Open Decisions

- **D-1**: Fate of the 13 residual texts. Variant A (recommended):
  the values stay stored, unrendered and uneditable, until a
  future decision. Variant B: the deploy data step clears the
  values; the pre-deploy backup holds the only copy.
- **D-2**: Migrate the five eliminatory texts to the structured
  round at deploy. Recommended: yes, under either D-1 variant.

## Assumptions

- **A-1**: The equivalents in FR-4 are the established terms of
  each locale for an orchestral excerpt.
- **A-2**: The physical column `piece_audition.excerpt` stays in
  the schema under both D-1 variants. No column drop runs on
  the production database.
