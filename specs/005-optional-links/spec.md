# Feature Specification: Optional Links (A scelta)

**Feature Branch**: `feat/optional-links`

**Created**: 2026-10-29

**Amended**: 2026-10-29 (pool semantics, translations chosen,
Piatti link corrected to piece 34)

**Status**: Draft (awaiting ratification)

**Input**: User description: "Fai una PR per aggiungere il marker
'a scelta' / optional su un brano quando aggiunto a un concorso.
Guarda i bandi dei concorsi censiti: alcuni hanno dei pezzi a
scelta (esclusivo l'uno con l'altro). Dobbiamo gestire questo
caso."

Follow-up of the same day: optional pieces of one audition
and one phase are mutually exclusive. That changes nothing
in normal piece handling, except (1) the weight calculation
(weight shared with the mutually exclusive pool) and (2)
the presentation of those pieces in plan generation. The
pieces also carry a marker in the audition piece list as a
single piece (one card with several pieces, optional badge)
and in the audition heatmap. In the plan generator, pieces
required by a single audition only as optional (no other
audition requires them mandatorily) use the same single-card
approach: the plan card shows every mutually exclusive
piece, so the user picks one without declaring it in
advance. At feedback time, the user picks the piece that
was studied.

## Clarifications

### Session 2026-10-29

- "A scelta" in the bandi has two meanings. The candidate
  chooses one alternative. The commission chooses one item at
  the exam. Only the candidate choice is optional for the
  candidate.
- The marker is a property of the link (piece plus audition),
  not of the piece. The same piece is optional in one
  audition and required in another.
- The alternatives of a candidate choice are not piece rows
  in the app. Haydn in Do maggiore has no row. Schumann and
  Elgar have no rows. The Piatti capriccio numbers have no
  rows. No group of links models the choice today.
- The alternatives live in the note text of the link. The
  marker flags that the link is one side of a choice. The
  note names the other sides. The Dvořák note stays: the
  marker does not replace the note.
- The "a scelta Commissione" passi are not optional. The
  candidate prepares every passo. The commission picks at
  the exam.
- A pool is the set of optional links that share one
  audition and one phase (round). A pool with one member
  behaves as a single optional link. A pool with two or
  more members is mutually exclusive: the candidate
  presents one member, not every member.
- The heatmap of an audition is its preparation map: the
  tile map on the audition page.
- A pool member that another audition requires mandatorily
  is planned as a normal mandatory piece. The single-card
  plan treatment applies only to pool-only members: pieces
  that no audition requires mandatorily.
- The pool card requires no advance choice. The choice
  happens at log time, when the user records the session.
- The plan, the readiness, the mock runner, the search,
  and the stats keep the current behavior, except for the
  pool rules of this amendment (shared weight, single
  cards, pool tile).

## Hypothesis Challenge

The change rests on one hypothesis: some links are mutually
exclusive alternatives, so the app must mark them.

Evidence, from the official programs of the five auditions
and from the production links, read on 2026-10-29.

Candidate choices (the marker applies):

- Santa Cecilia, solo: Piatti Capriccio, a scelta fra i
  nn. 3, 6, 10, 12. Production link: Santa Cecilia with
  Piatti (piece 34, the choice placeholder). Piece 33 is
  the fixed Capriccio n. 9 of Opera Roma, a different row.
- San Carlo, eliminatoria: Haydn I mov. senza cadenza, a
  scelta fra Concerto Do maggiore e Re maggiore (ed.
  Gevaert esclusa). Production link: San Carlo with Haydn
  (piece 25), scope "Senza cadenza".
- San Carlo, finale: concerto a scelta fra Dvořák,
  Schumann, Elgar, I mov. Production link: San Carlo with
  Dvořák (piece 28). Its note already reads "finale a
  scelta".

Commission choices (the marker does not apply):

- Cagliari Secondo (Det. 330): 12 passi a scelta della
  Commissione.
- Cagliari Fila (Det. 331): 11 passi, same rule.
- Santa Cecilia: 8 passi a scelta della Commissione.

Fixed programs (no choice clause):

- Opera Roma: one list, no choice clause.
- Cagliari solo: Haydn Re maggiore and Dvořák, fixed.
- Santa Cecilia solo: Haydn Re maggiore, fixed.

Conclusion: three links carry a candidate choice. No
alternative of those choices is a link today. A boolean
marker on the link fits the data. A pool groups the
optional links of one audition and one phase, so the
grouping activates when alternatives gain rows (for
example Schumann and Elgar linked to San Carlo as
optional). The commission choices stay required: marking
them optional would drop required preparation from view.

## User Scenarios and Testing

### User Story 1: Mark a link optional (Priority: P1)

The owner links a piece to an audition, or edits the link.
The form offers one optional checkbox per link row. The
owner checks it for a candidate-choice link.

**Acceptance**: the add-piece and edit-piece forms render
one optional checkbox per link row. A checked box stores
optional on the link. An unchecked box stores not
optional. An unrelated piece edit keeps the stored value.

### User Story 2: Read the marker (Priority: P1)

The owner opens an audition page or a piece page. Each
optional link shows a badge. A required link shows no
badge.

**Acceptance**: an optional link shows the badge on the
audition page and on the piece page. A required link
shows no badge on either page.

### User Story 3: Unmark a link (Priority: P2)

The owner edits a link that carries the marker. The owner
unchecks the box. The link becomes required.

**Acceptance**: an unchecked box on an edit clears the
stored value. The badge disappears from both pages.

### User Story 4: Read a pool as one piece (Priority: P1)

The owner opens an audition page. The pieces of one pool
appear as one card. The card lists every member, each
with the optional badge.

**Acceptance**: a pool with two or more members renders
one card in the audition piece list. The card lists every
member. Each member carries the optional badge. A
single-member pool renders one card for its member.

### User Story 5: Read a pool in the heatmap (Priority: P1)

The owner opens an audition page. The preparation map
shows one tile for a pool. The tile lists the members.

**Acceptance**: a pool with two or more members renders
one tile in the audition preparation map. The tile lists
every member. A single-member pool renders its own tile.

### User Story 6: Plan a pool as one card (Priority: P1)

The owner opens the daily plan. A pool of pool-only
members appears as one card. The card lists every member.
The owner picks no member in advance. At log time, the
card form offers a member selector, and the session
records on the picked member.

**Acceptance**: a pool of pool-only members with two or
more members renders one plan card. The card lists every
member. The card occupies one plan slot. The log form
carries a member selector. A logged session records on
the picked member only.

### User Story 7: Shared weight (Priority: P1)

The owner reads the plan. A pool counts its audition
weight once, not once per member.

**Acceptance**: the base of a pool item equals the sum
of the weights of the distinct auditions that require
any member. One audition that requires three members
contributes its weight once.

### User Story 8: No behavior change elsewhere (Priority: P1)

The owner reads the readiness and runs a mock. A pool
changes nothing there.

**Acceptance**: the readiness, the mock runner, the
search, and the stats behave as before. Every
pre-existing test passes.

## Requirements

- **FR-1**: `piece_audition` MUST gain an `optional`
  column, INTEGER NOT NULL DEFAULT 0, through the
  `add_column` helper. The link reads MUST carry the
  value on the audition record.
- **FR-2**: The add-piece and edit-piece forms MUST render
  one optional checkbox per link row. The link writes
  MUST store the checkbox value. An unrelated piece edit
  MUST NOT alter a stored value.
- **FR-3**: The audition page and the piece page MUST
  render a badge on an optional link, labeled with the
  `link.optional` string. A required link MUST show no
  badge.
- **FR-4**: The readiness, the mock runner, the search,
  and the stats MUST keep the current behavior. The plan
  changes only as FR-10 to FR-13 define.
- **FR-5**: The deploy data step MUST mark exactly three
  links optional: Santa Cecilia with Piatti (34), San
  Carlo with Haydn (25), San Carlo with Dvořák (28).
  The step MUST NOT mark any commission-choice link.
- **FR-6**: The deploy data step MUST run through the
  edit-link forms, after a database backup, as the
  established form-replay surgery.
- **FR-7**: The `link.optional` string MUST exist in all
  six locales: it "A scelta", en "Optional", de "Zur
  Auswahl", es "A elegir", fr "Au choix", pt "À
  escolha".
- **FR-8**: Existing identifiers MUST NOT change. The
  column and the locale key are additions only.
- **FR-9**: A pool is the set of optional links that
  share one audition and one round (NULL round counts
  as one phase). A pool with one member behaves as a
  single optional link. A pool with two or more members
  is mutually exclusive.
- **FR-10**: The base of a pool plan item MUST equal the
  sum of the weights of the distinct auditions that
  require any member. One audition counts once, however
  many members it requires.
- **FR-11**: The audition piece list MUST group the
  members of a pool with two or more members into one
  card. The card MUST list every member, each with the
  optional badge. Single-member pools render as today.
- **FR-12**: The audition preparation map MUST render
  one tile per pool with two or more members. The tile
  MUST list every member. Its level MUST be the highest
  member level. Single-member pools render their own
  tile.
- **FR-13**: The daily plan MUST render one card per
  pool whose members are all pool-only (no member has a
  mandatory link elsewhere) and whose member count is
  two or more. The card MUST list every member. The card
  MUST occupy one plan slot. The log form MUST carry a
  member selector and MUST record the session on the
  picked member only. No advance choice is required.
- **FR-14**: A pool member with a mandatory link
  elsewhere MUST be planned as a normal mandatory item.
  That member MUST leave the plan pool card. The
  remaining members still form a plan pool card when
  their count is two or more; otherwise they render as
  single cards.

## Success Criteria

- **SC-1**: The forms render the checkbox per link row,
  and the value round trips through add and edit, by
  template inspection and tests.
- **SC-2**: The badge renders on both pages for an
  optional link and on neither page for a required link.
- **SC-3**: Every pre-existing test passes. The
  readiness, the mock runner, the search, and the stats
  behave as before.
- **SC-4**: The crate coverage gate of 90 percent holds.
- **SC-5**: After deploy, the three census links show the
  badge, and no commission-choice link shows it, by live
  page inspection.
- **SC-6**: A pool with two or more members renders one
  card in the audition piece list and one tile in the
  audition preparation map, by template inspection and
  tests.
- **SC-7**: A pool of pool-only members renders one plan
  card with a member selector, occupies one plan slot,
  counts its audition weight once, and records a logged
  session on the picked member only, by tests.

## Decisions (proposed, pending ratification)

- **D-1**: The marker is a boolean on the link. A group
  model is out: the alternatives are not rows today.
- **D-2**: The marker is informational in this cycle,
  except for the pool rules of this amendment. The
  readiness does not change.
- **D-3**: The commission-choice passi stay required.
  The deploy step never marks them.
- **D-4**: The deploy step marks the three known links.
  The owner does not mark them by hand.
- **D-5**: The pool key is (audition, round). Two
  independent choice groups in one phase would merge
  into one pool. An explicit pool identifier is
  deferred to a later cycle, if such a case ever
  appears. Current data holds at most one group per
  phase.
- **D-6**: The pool plan item reuses the single-piece
  formula with pooled inputs: weights are the distinct
  audition weights; days is the nearest pool audition
  date; confidence, recency, practiced, and logged come
  from the union of member sessions; difficulty is the
  maximum member difficulty.
- **D-7**: The plan pool card applies only to pool-only
  members. A member with a mandatory link elsewhere is
  planned normally and leaves the plan pool card.
- **D-8**: The pool preparation tile lists every member
  and carries the highest member level: the candidate
  presents the best prepared alternative.
- **D-9**: The locale values of FR-7 are chosen by the
  assistant, on delegation of the owner (2026-10-29).
- **D-10**: The Piatti census link is piece 34, the
  Santa Cecilia choice placeholder. Piece 33 is the
  fixed Capriccio n. 9 of Opera Roma, a different row.

## Resolved Questions

- **Q-1** (resolved 2026-10-29, bando San Carlo Art. 6,
  page 4, read directly): the Do alternative is still
  open. The bando asks the first movement without
  cadenza, chosen between the Do maggiore and Re
  maggiore concertos (Gevaert edition excluded), and
  sets no deadline to communicate the choice in
  advance: the choice is exercised at the exam. No
  choice by Agnese for San Carlo is on record (the
  choice closed on 2026-10-27 concerns Cagliari). The
  marker stays: it records the bando requirement. Her
  personal choice, when made, belongs in the note.

## Assumptions

- **A-1**: The production link inventory of 2026-10-29
  stands: 77 links, with the three census links as
  listed in FR-5.
- **A-2**: No new dependency. The change uses the
  existing migration helper and form machinery.
- **A-3**: The EXTRA CARE rules apply: database backup
  before the deploy data step, live verification with
  real content.
- **A-4**: Current data holds at most one candidate-choice
  group per audition and phase, so the (audition,
  round) pool key is safe for this cycle.
