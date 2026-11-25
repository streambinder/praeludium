# Feature Specification: Choice Clusters Independent of the Round (Cluster di scelta indipendenti dal round)

**Feature Branch**: `feat/choice-clusters-010-spec`

**Created**: 2026-11-25

**Status**: Draft

**Input**: User description: "Sì aggiorniamo il codice per permettere a prescindere dal round di fare cluster di pezzi a scelta." The owner corrects the San Carlo pool of cycle 009, from the bando screenshot sent on 2026-11-25 at 20:10: "da bando san Carlo dice a scelta tra Dvorak, Schumann, Elgar; Haydn non è a scelta"

## Clarifications

### Session 2026-11-25

- The San Carlo call, Art. 6, lists two distinct
  candidate choices, not one. The bando text
  (screenshot of 2026-11-25 and the OCR extract
  from cycle 009) reads: Haydn, first movement
  without cadenza, "a scelta tra il concerto in
  Do maggiore e il concerto in Re maggiore";
  then a concerto first movement "a scelta del
  candidato tra: Dvorak, Schumann, Elgar". The
  exam asks for one Haydn AND one concerto.
- The live state after cycle 009 shows one
  composite card with five subcards on the San
  Carlo detail page, for pieces 25, 27, 28, 29,
  and 30. That card reads as "pick one of five",
  which contradicts the bando. The plan pool
  card holds {27, 29, 30}, which mixes the two
  choices in one item.
- The seed facts, verified on `src/seed_data.rs`
  on 2026-11-25: eight optional links exist, and
  all eight carry a null round: (34, 1), (25, 3),
  (27, 3), (28, 3), (29, 3), (30, 3), (28, 6),
  (29, 6). The round cannot separate the two
  San Carlo choices, because both already share
  a null round. The round is out of the question
  as a grouping key.
- The pool rule of cycle 005 keys on audition
  and round, with a null round as one phase. The
  owner direction of 2026-11-25 (a competition
  with no stated round structure is a single
  round competition) still holds, and it is the
  reason the two choices collided: one phase
  holds both.
- The La Fenice call asks for a concerto "a
  scelta solo Dvorak o Schumann". Its two
  optional links, (28, 6) and (29, 6), form one
  choice group of two. Santa Cecilia offers
  Piatti "a scelta tra 3/6/10/12", but the seed
  links only piece 34 to that audition, so the
  Piatti choice has one member and forms no
  group.

### Hypothesis Challenge

- The live pool card shows the hypothesis is
  wrong: five optional links in one audition do
  not mean one choice of five. The bando states
  two choices, and the card must show two
  groups.
- Keying on the audition alone would keep the
  same wrong outcome: one cluster of five. The
  key must name the choice itself.
- Splitting the two choices through round rows
  would invent round structure that the bando
  does not state. Cycle 009 removed the invented
  San Carlo rounds for exactly that reason.
  The round must leave the pool key entirely.
- The chosen design is an explicit cluster field
  on the link. The cluster names the choice the
  bando states. The pool key becomes audition
  plus cluster, regardless of the round.
- The owner sentence "Haydn non è a scelta" is
  read as "Haydn is not part of the concerto
  choice", not as "Haydn has no choice". The
  bando line for Haydn reads "a scelta tra Do
  e Re maggiore", and the choice is exercised
  at the exam. Both San Carlo groups keep the
  "A scelta" badge. Ratification confirms this
  reading.

## User Scenarios & Testing

### Scenario 1: The San Carlo detail page shows two choice groups

- The owner opens the San Carlo detail page.
- The page shows two composite cards: one
  Haydn card with two subcards (Do and Re
  major) and one concerto card with three
  subcards (Dvorak, Schumann, Elgar).
- Ten single cards remain for the other pieces.
- No round badge appears anywhere.

### Scenario 2: The plan holds one pool card, for the concerto

- The owner opens the daily plan with San Carlo
  enrolled.
- The plan holds one pool card, with members
  Schumann (29) and Elgar (30). Dvorak (28)
  keeps a single item: its links to Cagliari
  and La Fenice are mandatory or optional in
  another audition, so it is not pool-only.
- The Haydn group has one eligible member,
  Haydn Do major (27), so it forms no pool
  card. Piece 27 keeps a single plan item.
  Haydn Re major (25) keeps a single item.

### Scenario 3: La Fenice shows one concerto group, Piatti stays single

- The La Fenice detail page shows one composite
  card with two subcards (Dvorak and Schumann).
- The Santa Cecilia Piatti link (34, 1) keeps
  its "A scelta" badge and stays a single item:
  a choice group of one forms no pool.

## Requirements

### Functional Requirements

- **FR-1**: The `piece_audition` table gains a
  nullable text column `cluster`. The canonical
  `CREATE TABLE` statement carries it, so a
  fresh database has it. The live database gains
  it through a manual one-shot `ALTER TABLE`
  before the deploy, per the migration rule of
  2026-11-24.
- **FR-2**: Reads carry the cluster. The
  `Audition` record gains a `cluster` field
  (null when the link has none), the
  `piece_auditions` query selects it, the
  `SeedPieceAudition` record gains it, and the
  seed upsert sets it on conflict.
- **FR-3**: The pool key becomes (audition, cluster)
  in all three grouping sites: `plan_pools`, the
  audition detail piece-list grouping, and the
  preparation map single-audition tile merge. A
  null cluster never pools, whatever the round.
  The round leaves the pool key entirely.
- **FR-4**: The plan exclusion that keeps pooled
  pieces out of single items keys on the new
  pool identity (audition, cluster).
- **FR-5**: The seed sets the cluster values:
  `haydn` on (25, 3) and (27, 3); `concerto`
  on (28, 3), (29, 3), and (30, 3); `concerto`
  on (28, 6) and (29, 6). Every other link
  carries a null cluster.
- **FR-6**: The pool-only rule, the shared
  weight, the pool scoring, and the mock flow
  do not change. They apply per cluster.
- **FR-7**: No UI copy changes. The "A scelta"
  badge stays on all eight optional links.
  No new locale key is needed.

### Key Entities

- **Choice cluster**: an explicit group of
  alternative links for one audition, named by
  the `cluster` value on each link. A cluster
  with two or more members forms one composite
  card on the detail page, one tile on the
  preparation map, and one plan card when two
  or more members are pool-only. A cluster with
  one member, or a null cluster, forms no group.

## Success Criteria

- **SC-1**: The San Carlo detail page shows two
  pool cards (two and three subcards) and ten
  single cards, with no round badge.
- **SC-2**: The plan with San Carlo enrolled
  holds one pool card, with members 29 and 30.
  Pieces 25, 27, and 28 keep single plan items.
- **SC-3**: The La Fenice detail page shows one
  pool card with two subcards. The Santa Cecilia
  Piatti link stays a single item with its badge.
- **SC-4**: The catalog counts do not change:
  46 pieces, 97 links, 25 segments, 35
  link-segments, 13 rounds. The seed
  comparison after the deploy reads 0 missing,
  0 extra, 0 different.
- **SC-5**: The full local gate stays green
  (format, clippy, unit and integration tests,
  coverage at the CI threshold) and the CI run
  of the implementation PR is green and clean.

## Decisions

- **D-1**: Explicit cluster field, not a
  round split and not an audition-only key.
  The cluster names the choice the bando
  states. The pool key is (audition, cluster).
- **D-2**: Cluster values are short stable IDs
  scoped to the audition: `haydn` and
  `concerto` for San Carlo, `concerto` for La
  Fenice. They never reach the UI.
- **D-3**: Both San Carlo groups keep the
  "A scelta" badge. The owner correction
  separates the groups; it does not remove the
  Haydn choice.
- **D-4**: The live migration is manual and
  once: backup, then `ALTER TABLE
piece_audition ADD COLUMN cluster TEXT`,
  executed by the owner before the deploy.
  The code carries only the canonical schema.
- **D-5**: This cycle supersedes the pool rule
  of cycle 005 and the D-2 amendment of cycle 009. A null round is still one phase, but the
  phase no longer groups the choices. The
  constitution carries no pool rule, so no
  amendment is needed.

## Assumptions

- **A-1**: The seed counts of cycle 009 hold:
  46 pieces, 97 links, 25 segments, 35
  link-segments, 13 rounds.
- **A-2**: The eight optional links of the
  seed are the complete set of candidate
  choices in the catalog.
- **A-3**: The La Fenice concerto choice is
  Dvorak or Schumann only, per the bando
  extract of cycle 009. Elgar is not an
  option there.

## Questions

- **Q-1**: Does Haydn keep the "A scelta"
  badge? Answered yes in D-3. Ratification
  confirms the reading.
- **Q-2**: Should the Piatti choice gain a
  cluster when the seed links only one piece?
  No: a group of one forms no pool, so the
  value would change nothing. It stays null.

## Out of Scope

- New cluster values for other auditions.
- A UI label for the cluster name.
- Any change to the round model, the
  enrollment model, or the mock flow.
- Any change to the catalog counts or the
  link set.
