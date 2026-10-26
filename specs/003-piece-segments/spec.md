# Feature Specification: Piece Segments

**Feature Branch**: `feat/piece-segments`

**Created**: 2026-10-24

**Status**: Complete (merged 2026-10-26 in PR #40, deployed and verified live the same day; coverage 94.00 percent against the 90 percent gate)

**Input**: User description: "Audition programs ask for parts of a
piece, not always the whole piece. The Haydn first movement is
required with the cadenza in one audition and without it in others.
Pieces must gain segments: named or ranged parts of a piece. A link
between a piece and an audition specifies which segments that
audition requires. No segment specified means the whole piece."

## Clarifications

### Session 2026-10-24

- Bar numbers are unique inside one movement. Rehearsal marks are
  unique and ordered inside one movement. The two scales are two
  coordinate systems on the same axis. Rehearsal marks depend on the
  edition; bar numbers do not.
- An endpoint is an anchor plus an optional offset in bars. Anchor
  kinds: piece start, piece end, bar number, rehearsal mark. The
  form covers relative requirements such as "17 bars after RR" and
  "second bar of H".
- The application stores an endpoint as the program states it. The
  application never converts between the two scales.
- A segment belongs to one piece. A piece covers exactly one
  movement, so segment coordinates are never ambiguous. The
  movement column exists today; this feature makes the column carry
  the identity of the piece.
- A segment has an optional alias. Auditions reuse a segment that
  the piece already defines. A segment without an alias is defined
  by its endpoints alone.
- A link references zero or more segments of the linked piece. Zero
  segments means the whole piece. The reading matches every link
  that exists today.
- A session references one piece, always. A session may reference
  one segment of that piece. No segment reference means the session
  covers the whole piece.
- Credit flows down and up, never sideways. A session on the whole
  piece counts toward every segment scope of the piece. A session
  on a segment counts toward the whole-piece scope and toward the
  scopes whose set includes that segment. A session on a segment
  never counts toward a different segment.

## Hypothesis Challenge

The feature rests on one hypothesis: audition requirements are
partial often enough that the piece is the wrong unit of study.

Evidence, from the five registered audition programs, read in full
on 2026-10-24:

- Partial ranges are the norm, not the exception. Eleven of twelve
  orchestral excerpts at Opera Roma carry an explicit range. Eight
  carry one at San Carlo. At least seven pieces are shared between
  auditions with a different range in each.
- The Haydn first movement differs per audition in the cadenza
  alone: with cadenza at Opera Roma, without cadenza at San Carlo
  and at Santa Cecilia, unstated at Cagliari.
- One requirement can hold two disjoint ranges of one piece, for
  example the Beethoven Fifth second movement at Opera Roma.

Alternatives considered:

- Keep the free-text excerpt note as the only record. Rejected:
  the plan, the score, and readiness reason per piece, so a note
  the engine cannot read cannot steer study.
- Split pieces per range, as done for movements. Rejected as the
  general mechanism: ranges overlap partially, so the registry
  would multiply pieces that share most of their study. The split
  stays the right tool for movements only.
- Segments, as specified here. The piece stays the unit of study;
  the link gains the precision of the program.

One data question stays open and does not block the model: the
Cagliari programs do not state the cadenza. Until the foundation
clarifies, the Cagliari Haydn links carry no segment, which reads
as the whole piece. The ambiguity stays visible; the model does
not resolve it silently.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - A link states the exact parts (Priority: P1)

The owner registers the Haydn first movement once and defines a
segment with alias "Cadenza". The Opera Roma link references the
main range and the Cadenza segment. The San Carlo and Santa
Cecilia links reference the main range alone. Each audition
program shows exactly the parts that audition requires.

**Why this priority**: This is the defect that opened the feature.
Without it, one piece silently stands for two different
requirements.

**Independent Test**: Open the three audition pages. Each program
shows the Haydn entry with its own parts, and the stored links
differ exactly in the Cadenza segment.

**Acceptance Scenarios**:

1. **Given** a piece with a defined segment, **When** the owner
   links the piece to an audition and selects the segment,
   **Then** the audition program names that segment for the piece.
2. **Given** a link with no segment selected, **When** the program
   renders, **Then** the piece reads as required whole, exactly as
   links render today.
3. **Given** two auditions that reference the same segment,
   **When** the owner edits the segment definition once, **Then**
   both programs show the updated definition.

### User Story 2 - The plan names the part and the destination (Priority: P1)

A plan card that comes from a segmented link names the segment
and the audition it serves, for example "Haydn I, Cadenza, for
Cagliari". When two auditions require the same part of the same
piece, the plan shows one card that names both destinations.

**Why this priority**: The plan is where study time is allocated.
A card that hides which part is due allocates time to the wrong
scope.

**Independent Test**: Build a day whose plan includes a segmented
requirement shared by two auditions. The day shows one card with
the segment name and both audition names.

**Acceptance Scenarios**:

1. **Given** a segmented link, **When** the plan allocates the
   piece, **Then** the card names the segment set of the link.
2. **Given** two links with the same piece and the same segment
   set, **When** the plan allocates, **Then** one card serves both
   and names both auditions.
3. **Given** a link without segments, **When** the plan allocates
   the piece, **Then** the card renders as today, with no segment
   text.

### User Story 3 - Study is recorded at the grain of the work (Priority: P2)

Logging study keeps today's cost: minutes on a piece, no extra
step. A session may name one segment of the piece when the work
covered only that part. Readiness and the score read a session on
the whole piece as covering every segment scope of the piece, and
a session on a segment as covering the scopes that include it.

**Why this priority**: Per-part requirements change what "ready"
means, but a daily flow that costs extra taps will not survive
daily use.

**Independent Test**: Log a session on a segment, then read the
readiness of a link that includes the segment and of a link that
does not. Only the first link gains from the session.

**Acceptance Scenarios**:

1. **Given** a piece with segments, **When** the player logs
   minutes without naming a segment, **Then** the session counts
   toward every scope of the piece, as today.
2. **Given** a session that names a segment, **When** readiness
   runs for a link without that segment, **Then** the session
   does not raise that link.

### User Story 4 - Segments are born at link time and reused (Priority: P2)

Segments have no management surface of their own (amendment of
the owner, 2026-10-26). A segment comes into being inside the
flow that links a piece to an audition: the link form offers
the whole piece as the default, the segments the piece already
carries as a choice, and an inline definition of a new segment.
Every later link to that piece can reference the segment. A
segment that only one audition needs can stay unnamed; its
endpoints define it. A segment that no link references ceases
to exist: the application removes it when the link that last
referenced it changes.

**Why this priority**: Reuse keeps one definition of a part that
several programs share, so the definitions cannot drift apart.
Link-time definition keeps one surface for the whole act of
stating what an audition asks.

**Independent Test**: Link a piece to two auditions, defining
one unnamed ranged segment inline on the first link and
selecting it on the second. Both programs show the same range,
from the single definition. Unlink the segment from both: the
segment is gone.

**Acceptance Scenarios**:

1. **Given** a piece, **When** the owner defines a segment with
   an alias at link time, **Then** the alias is unique within
   the piece, and a repeated identical definition reuses the
   existing segment.
2. **Given** a segment referenced by one link, **When** that
   link stops referencing it and no other link does, **Then**
   the segment no longer exists and no link dangles.

## Functional Requirements

- **FR-1**: A segment belongs to exactly one piece. A segment is
  one contiguous range: a start endpoint and an end endpoint.
- **FR-2**: An endpoint is an anchor kind (piece start, piece end,
  bar number, rehearsal mark), an optional anchor value, and a
  signed offset in bars that defaults to zero.
- **FR-3**: A segment may carry an alias. An alias is unique among
  the segments of its piece.
- **FR-11**: Segments are created and selected only inside the
  link flow between a piece and an audition. A segment that no
  link references is removed automatically; sessions that named
  it fall back to the whole piece.
- **FR-4**: A link between a piece and an audition references zero
  or more segments of that piece. Zero references mean the whole
  piece.
- **FR-5**: A session may reference one segment of its piece. No
  reference means the session covers the whole piece.
- **FR-6**: The daily plan derives its items from links. An item
  carries the segment set of its link. Items with equal piece and
  equal segment set merge into one item that names every
  destination audition.
- **FR-7**: Readiness and scoring are computed per link scope. The
  heat map of a link counts the sessions on the whole piece and
  the sessions on the segments in the set of the link. Sessions on
  other segments of the piece do not count. A link without
  segments counts every session of the piece.
- **FR-8**: The free-text excerpt note on a link stays as display
  text. Segments add structure; the note keeps wording that no
  range expresses.
- **FR-9**: Registering a piece requires its movement. A
  requirement that spans movements registers one piece per
  movement. This census rule is a precondition of FR-1 and FR-2.
- **FR-10**: When no segment is involved, every flow keeps its
  current cost and rendering: plan, diary, program, readiness.

## Data Precondition (executed 2026-10-24)

The audit of 2026-10-24 found three active pieces that broke
FR-9. On the owner's decision, the remediation ran before this
cycle, on a copy of the production database, with a verified
backup first:

- Mahler, Symphony No. 2, "I e II mov." became two pieces, one per
  movement. The Cagliari excerpt notes were split per movement.
- Bach, Suite No. 2 became three pieces: Preludio, Sarabanda,
  Giga.
- Hindemith, Sonata op. 25 No. 3 became five pieces, one per
  movement of the sonata.
- The originals are archived. Their history rows are baseline and
  auto-skip markers only; real minutes on the three pieces were
  zero. The markers were replicated to every sub-piece, under the
  owner's rule that every sub-variant counts as practiced. Total
  study minutes are unchanged: 1457 before and after. Live pages
  verified after the swap: 38 active pieces, audition programs
  correct.

## Success Criteria

- **SC-1**: The Haydn first movement exists once, with a Cadenza
  segment. The Opera Roma link includes the segment; the San
  Carlo and Santa Cecilia links exclude it; the three programs
  render the difference.
- **SC-2**: A plan day that serves the Opera Roma Haydn shows a
  card naming the cadenza part and the audition. No second card
  duplicates the same part.
- **SC-3**: With no segment defined anywhere, every page renders
  as before the feature, on the production data set of 38 active
  pieces.
- **SC-4**: The crate coverage gate of 90 percent holds, with the
  segment logic covered by tests in the shape of features 001
  and 002.

## Assumptions and Open Points

- **A-1 (decided 2026-10-24)**: The scope credit rule of FR-7 and
  the whole-piece default of FR-5 are owner decisions, recorded
  in the clarifications of this specification.
- **A-3**: The Cagliari cadenza stays unresolved in the data
  until the foundation clarifies the program. The model does not
  depend on the answer.
- Out of scope: "one of" alternative requirements (for example
  the Piatti choice at Santa Cecilia), instrumental part
  selection (for example a second-cello part), pieces that span
  movements as a unit, and any change to scoring weights.
