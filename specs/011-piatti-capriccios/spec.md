# Feature Specification: Piatti Capriccios as Four Pieces in One Choice Cluster (Quattro Capricci Piatti in un cluster di scelta)

**Feature Branch**: `feat/piatti-capriccios-011-spec`

**Created**: 2026-11-26

**Status**: Draft

**Input**: User description: the owner asks for one distinct piece per Capriccio, clustered as a choice on Santa Cecilia, and for the whole diary of Agnese to move onto Capriccio n. 6. The owner answers the open count question of 2026-11-26 10:07 with "fai come da bando".

## Clarifications

### Session 2026-11-26

- The Santa Cecilia call asks for one Capriccio,
  the candidate choice among nn. 3, 6, 10, and 12
  of the 12 Capricci op. 25 by C. A. Piatti. Two
  sources agree on four numbers: the bando
  extract of cycle 009 ("1 di 4") and the
  compilation of 2026-10-01 ("a scelta fra nn.
  3, 6, 10, 12"). The owner words name the
  three pieces to add besides the one
  Agnese chose. Ratification confirms this
  reading.
- The seed today holds one generic piece for the
  choice: piece 34, work "Capriccio (op. 25)",
  movement "a scelta fra nn. 3, 6, 10, 12", kind
  solo, difficulty 4. Its link (34, 1) is
  optional with a null cluster. The other three
  numbers have no piece.
- Each Capriccio is a complete standalone piece,
  not a movement of one work and not an excerpt.
  The catalog already models one Capriccio this
  way: piece 33, work "Capriccio n. 9 (op. 25)",
  for Opera Roma.
- The owner states Agnese chose Capriccio n. 6.
  The diary on piece 34 holds five sessions,
  read live from the diary page on 2026-11-26:
  3 October 2026, assessment, no minutes;
  24 October 2026, 19 minutes, confidence 3/5;
  9 November 2026, 20 minutes, confidence 4/5;
  16 November 2026, 20 minutes, confidence 4/5;
  20 November 2026, 20 minutes, confidence 4/5.
  A message of 2026-11-26 00:26 said no session
  was recorded on piece 34. That claim was
  wrong: the page checks missed the rows because
  dates render "Friday 20 November 2026" and
  minutes render "20m".
- The seed upsert updates a piece on conflict:
  `seed_pieces` in `src/seed.rs` sets work,
  movement, excerpt, kind, difficulty, and
  target_bpm. The link upsert sets excerpt, note,
  round_id, optional, and cluster. The deploy
  applies a rename and a cluster value with no
  manual SQL.
- The cluster mechanism of cycle 010 already
  supports this change. No code change is needed.
  No manual data step is needed.

### Hypothesis Challenge

- The singleton optional link was incoherent:
  a choice needs alternatives, and the badge
  marked a choice with no member in the data.
  The owner said so on 2026-11-26 00:26.
- The generic piece conflated four distinct
  works into one row. Four pieces in one cluster
  state the choice the bando states.
- A session move through `UPDATE` would be the
  risky path: it rewrites diary rows and, if the
  piece were deleted after, the foreign key
  cascade would destroy them on a missed step.
  The rename keeps the diary intact by identity:
  the same row, the same ID, a corrected label.
  The diary lands on n. 6 with no session edit
  and no delete.

## User Scenarios & Testing

### Scenario 1: The Santa Cecilia detail page shows one Piatti group

- The owner opens the Santa Cecilia detail page.
- The page shows one composite card with four
  subcards (nn. 3, 6, 10, 12), each with the
  "A scelta" badge.
- Sixteen single cards remain for the other
  pieces. Twenty pieces are linked in total.

### Scenario 2: The plan holds one Piatti pool card

- The owner opens the daily plan with Santa
  Cecilia enrolled.
- The plan holds one pool card for the Piatti
  cluster. All four members are pool-only: each
  links only to Santa Cecilia.
- The pool card carries the shared weight of
  one audition, as the other clusters do.

### Scenario 3: The diary of piece 34 reads Capriccio n. 6

- The owner opens the diary page of piece 34.
- The page title reads "Capriccio n. 6
  (op. 25)".
- The same five sessions appear, with the same
  dates, minutes, and confidence values.

## Requirements

### Functional Requirements

- **FR-1**: The seed renames piece 34: work
  "Capriccio n. 6 (op. 25)", movement empty.
  Composer, kind, difficulty, and target_bpm do
  not change.
- **FR-2**: The seed adds three pieces: ID 50,
  "Capriccio n. 3 (op. 25)"; ID 51, "Capriccio
  n. 10 (op. 25)"; ID 52, "Capriccio n. 12
  (op. 25)". Each is kind solo, difficulty 4,
  target_bpm none, by analogy with piece 34.
- **FR-3**: The seed adds three links: (50, 1),
  (51, 1), and (52, 1). Each is optional, cluster
  `piatti`, round null, excerpt and note empty.
- **FR-4**: The seed sets cluster `piatti` on
  the link (34, 1). The link stays optional.
- **FR-5**: No code change. No manual data step.
  The deploy applies the rename, the three new
  pieces, the three new links, and the cluster
  through the seed upsert.
- **FR-6**: No session row changes. The five
  sessions on piece 34 stay on piece 34.

### Key Entities

- **Piatti cluster**: the choice cluster `piatti`
  on audition 1 (Santa Cecilia), with members
  34 (n. 6), 50 (n. 3), 51 (n. 10), and 52
  (n. 12). It follows the choice cluster entity
  of cycle 010: one composite card, one map tile,
  and one plan card when two or more members are
  pool-only.

## Success Criteria

- **SC-1**: The Santa Cecilia detail page shows
  one composite card with four subcards and
  sixteen single cards.
- **SC-2**: The plan with Santa Cecilia enrolled
  holds one Piatti pool card with four members.
  Membership is asserted in test, as in cycle
  010, because the daily top 8 may not surface
  the card on a given day.
- **SC-3**: The diary page of piece 34 reads
  "Capriccio n. 6 (op. 25)" and lists the same
  five sessions with the same dates, minutes,
  and confidence values.
- **SC-4**: The catalog counts become 49 pieces,
  100 links, 25 segments, 35 link-segments, and
  13 rounds. The seed comparison after the
  deploy reads 0 missing, 0 extra, 0 different.
- **SC-5**: The full local gate stays green
  (format, clippy, unit and integration tests,
  coverage at the CI threshold) and the CI run
  of the implementation PR is green and clean.

## Decisions

- **D-1**: Piece 34 becomes Capriccio n. 6 by
  rename, not by a new row plus a session move.
  The diary is preserved by identity, with no
  session surgery and no delete.
- **D-2**: New piece IDs 50, 51, and 52, in
  Capriccio number order 3, 10, 12. The next
  free ID after the current maximum 49 is 50.
- **D-3**: Cluster value `piatti`, scoped to
  audition 1, like `haydn` and `concerto` in
  cycle 010. It never reaches the UI.
- **D-4**: Difficulty 4 for the three new pieces,
  by analogy with piece 34 and with piece 33
  (difficulty 3) as the only other Capriccio in
  the catalog. The number stays in the work
  field, per the convention of piece 33.
- **D-5**: The owner words mean the three pieces
  to add besides n. 6. The bando lists four
  numbers in total. Ratification confirms this
  reading.

## Assumptions

- **A-1**: The seed counts of cycle 010 hold:
  46 pieces, 97 links, 25 segments, 35
  link-segments, 13 rounds.
- **A-2**: Agnese chose Capriccio n. 6, per the
  owner statement of 2026-11-26 10:13.
- **A-3**: The five sessions on piece 34, read
  live on 2026-11-26, are the complete diary
  for that piece. No session is recorded on any
  other Piatti piece for Santa Cecilia.
- **A-4**: No other Capriccio number is needed:
  n. 9 stays linked only to Opera Roma.

## Questions

- **Q-1**: Four Capriccios or three? Four, per
  the bando. The owner words name the three to
  add. Ratification confirms the reading.

## Out of Scope

- Any change to other auditions or clusters.
- Any UI copy change or new locale key.
- Any change to the round model, the enrollment
  model, or the mock flow.
- Any edit to a session row.
