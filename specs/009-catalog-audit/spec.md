# Feature Specification: Catalog Audit Against the Official Calls (Revisione del catalogo sui bandi ufficiali)

**Feature Branch**: `feat/catalog-audit-009-spec`

**Created**: 2026-11-25

**Status**: Draft (awaiting ratification)

**Amended**: 2026-11-25 (the owner corrects
the pool model: a competition with no stated
round structure is a single-round competition,
so a null round is one phase and the optional
links cluster in it. The pool rule does not
change. No code change in this cycle.)

**Input**: User description: "fai un analisi
approfondita del nostro catalogo dai seed e
spotti OGNI INCONGRUENZA fetchando i bandi di
tutti i concorsi censiti e fixi accordingly"

## Clarifications

### Session 2026-11-25

- The evidence base is the fresh fetch of all
  six official calls, done on 2026-11-25. The
  extraction is `bandi-extract.md` in the surgery
  folder, with 13 PDFs and OCR texts. Cagliari
  comes from the local official program PDFs.
  Santa Cecilia comes from the official English
  call (the Italian version answers 404). The
  La Fenice score book (`PassiVC.pdf`, 11 pages)
  was read page by page.
- The San Carlo round split does not exist in
  the call. Article 6 lists one program. Article
  8 says the procedure "si articola in più fasi
  successive" and assigns no piece to any phase.
  The convocati list names no phase. The score
  book carries no phase header. The 2026-10-13
  verification read the split into the order of
  the list. That was an inference, not a
  quotation. The 13 San Carlo link rounds leave
  the seed. The round rows 6 and 7 stay in the
  seed, defined but unused, as rounds 1–5 and
  12–13 already are.
- A competition with no stated round structure
  is a single-round competition. A null round
  is one phase, so the optional links of that
  competition cluster in it (owner correction,
  2026-11-25). The five San Carlo optionals
  form one pool group. The plan card carries
  the pool-only members (27, 29, 30); pieces
  25 and 28 keep single items because their
  links to other auditions are mandatory, per
  the existing pool-only rule. The detail page
  carries all five in one composite card, and
  the preparation map merges them into one
  tile. No code change: the cycle 005 pool
  rule already reads null as one phase.
- The removed rows leave the seed only. The seed
  never deletes a row, so the live database
  needs manual DELETEs, with a backup first,
  run before the deploy. The removed rows are
  three links and one link-segment.
- Every other seed change arrives through the
  seed upsert at deploy: new pieces, new links,
  new link-segments, changed link fields, and
  the audition rename.
- La Fenice gains its program: 15 links and
  four new pieces. Enrollment does not change.
  La Fenice stays unenrolled. The owner decides
  enrollment separately.
- The Mozart and Wagner pieces carry no
  segments. The call text gives no bar limits.
  The Fenice score book brackets excerpts (I
  mov. from bar 114, IV mov. from about bar 49),
  but the closing bars are not legible with
  certainty. A whole movement covers the
  excerpt. No invented bar number enters the
  seed.
- The Mendelssohn Scherzo at La Fenice carries
  no segments either. The score shows fragments
  at rehearsal letters C–D and N–O, with no
  legible bar numbers.
- The new Beethoven 7 I mov. piece mirrors
  piece 3 for difficulty. The new Mozart pieces
  mirror piece 8. The new Wagner piece mirrors
  piece 22. The owner or Agnese may adjust the
  difficulties later.
- The audition "Cagliari (Concertino)" becomes
  "Cagliari (Secondo)". The call names the post
  "Secondo Violoncello con obbligo della fila".
  The rename is a label only.
- The mock round scoping needs no code change.
  The rule already says a piece with no round
  is asked in every round. San Carlo mocks draw
  from the whole audition, as Roma and Santa
  Cecilia mocks already do.

## Hypothesis Challenge

The request rests on one hypothesis: the seed
catalog matches the six official calls. The
hypothesis is false on the real data. The audit
found 13 discrepancies and one text error,
against evidence read on 2026-11-25.

Evidence, from the official calls and the seed
data.

- Santa Cecilia (2 posts, deadline 2026-12-20,
  tests 2027-01-08 to 2027-01-11). Article 4
  lists one program with no round split. The
  Mendelssohn asks "I and IV movements", in
  the official English call. The seed covers
  them with pieces 8 (IV) and 9 (I). No
  movement is missing. Only the link excerpt
  on piece 9 says "I–IV", which reads as all
  four movements.
- Opera Roma (1 post, tests 2026-11-23 and
  2026-12-06). The program PDF lists one
  program with no round split and no candidate
  choice. All 18 seed links match it, with
  bar limits and the Schott/Gendron cadenza.
  Nothing is missing and nothing is extra.
- San Carlo (1 post, test 2026-12-14,
  Agnese n. 71 of 111). Article 6 lists one
  program: Haydn first movement without
  cadenza, at the candidate's choice between
  C major and D major (Gevaert excluded); a
  first movement at the candidate's choice
  between Dvořák, Schumann, and Elgar; and
  the orchestral excerpts. No piece is
  assigned to any phase. The seed assigns 13
  links to rounds 6 and 7. The Haydn C major
  piece (ID 27), the Schumann (ID 29), and
  the Elgar (ID 30) carry no optional flag,
  although Article 6 names them as candidate
  choices. The Elgar link (30, 3) also
  carries no round, while Dvořák (28) and
  Schumann (29) carry round 7.
- Cagliari Secondo, Det. 330/2026 (test
  2026-11-16). First test: Haydn D major I
  and II movements, Dvořák I movement.
  Second test: 12 excerpts at the
  commission's choice. The Mahler item is one
  item with bar limits across two movements.
  The seed represents it with three pieces:
  21 (combined I and II), 36 (I), and 37
  (II). The Beethoven 7 item asks "Vivace e
  Allegretto", the I and II movements. The
  seed holds only piece 3 (II movement) with
  the combined excerpt.
- Cagliari Fila, Det. 331/2026 (test
  2026-11-17). First test: Haydn D major I
  movement only, Dvořák I movement. Second
  test: the same excerpts except Tosca, and
  Otello as "solo parte del secondo" with no
  act and no bar limits. The seed links the
  Tosca piece (ID 20) to this audition. The
  seed does not link the Brahms 3 piece
  (ID 7), although the call asks it. The seed
  links the Mahler combined piece (ID 21).
  The seed attaches segment 18 (RR+17 to SS)
  to the Fila Otello link, although the call
  gives no bar limits and the approved
  2026-10-27 mapping says the Fila link stays
  whole.
- La Fenice (1 post, deadline 2027-01-31
  23:00, tests 2027-02-15 to 2027-02-16,
  final 2027-02-22). Article 4 lists one
  program with no round split: Haydn
  Concerto n. 2 in D major, I movement
  without cadenza and II movement, fixed;
  a concerto at the candidate's choice
  between Dvořák and Schumann only; and the
  excerpts. The seed holds the audition with
  zero links. Three works are new to the
  catalog: Mozart Symphony n. 40 (I and IV
  movements) and the Wagner Tristan Prelude.
- The 2026-10-01 compilation wrote
  "Mendelssohn 4 (I–IV)" for Santa Cecilia.
  The official English call says "I and IV
  movements". The compilation paraphrased
  loosely. The seed coverage was right.
- The 2026-10-01 compilation wrote that the
  San Carlo score book page 8 ("Allegro",
  bars 132–209, letter B, 3 flats) was
  "verosimilmente Beethoven 5". The page
  sits inside the Beethoven 5 part, after
  the Andante, and the III movement is in
  C minor. The seed assignment to Beethoven
  5 III movement stands, now verified.
- The San Carlo Čajkovskij 6 link stays
  whole by Agnese's decision D3 of
  2026-10-27, not by an error. The call
  gives bar limits, but Agnese said the
  test asks for everything.
- The Cagliari dates and determination
  numbers (2026-11-16, 2026-11-17, Det.
  330/331) are not verifiable online. The
  site was restructured and its calls page
  holds no "violoncello" match. The programs
  are the official local PDFs. The recorded
  dates stay.

## User Scenarios and Testing

### User Story 1: The catalog matches the calls (Priority: P1)

Agnese opens the San Carlo page. No link
shows a round badge. The Haydn C major, the
Schumann, and the Elgar show the "A scelta"
badge. The Dvořák shows it too. The page
shows one composite card carrying all five
optional pieces. The plan shows one pool card
for the choice, carrying the Haydn C major,
the Schumann, and the Elgar; the Haydn D
major and the Dvořák keep single items,
because other auditions require them.

### User Story 2: Cagliari shows the right excerpts (Priority: P1)

Agnese opens the Cagliari Secondo page. The
Mahler appears as two movements (36 and 37),
not three entries. The Beethoven 7 appears
as two movements (46 and 3). The Fila page
shows Brahms 3 and no Tosca. The Fila Otello
link has no segment.

### User Story 3: La Fenice has a program (Priority: P1)

Agnese opens the La Fenice page. It lists 15
pieces: Haydn I and II, the Dvořák and
Schumann choices, and 11 excerpts. No Elgar
appears. The audition stays unenrolled, so
the pieces stay out of the daily plan.

### User Story 4: The diary loses nothing (Priority: P1)

The Mahler combined piece (ID 21) keeps its
row and its five sessions. The diary page
for piece 21 still lists them. The removed
links were never diary data.

### User Story 5: No behavior change elsewhere (Priority: P1)

The Roma and Santa Cecilia pages are
unchanged. The plan formula, the enrollment
gate, the manual log list, and the mock
flows behave as before.

### User Story 6: A null round is one phase (Priority: P1)

Five optional links with no round form one
pool group, in the plan, in the piece list,
and in the preparation map. The plan card
carries the pool-only members. Two optional
links with the same audition and the same
non-null round still form one pool card, as
before.

## Requirements

### Functional Requirements

- FR-1: The seed holds 46 pieces. The four
  new pieces are Beethoven Symphony n. 7 I
  mov. Vivace (ID 46, difficulty 3), Mozart
  Symphony n. 40 I mov. (ID 47, difficulty
  4), Mozart Symphony n. 40 IV mov. (ID 48,
  difficulty 4), and Wagner Tristan Prelude
  (ID 49, difficulty 5).
- FR-2: The seed holds 97 links. The removed
  links are (20, 5), (21, 4), and (21, 5).
  The added links are (7, 5), (46, 4),
  (46, 5), and the 15 La Fenice links.
- FR-3: The La Fenice links are (25, 6) with
  segment 22 and the excerpt "senza cadenza",
  (26, 6), (28, 6) optional with the excerpt
  "a scelta fra Dvořák e Schumann", (29, 6)
  optional with the same excerpt, (1, 6) with
  segment 2, (4, 6), (6, 6), (10, 6), (47, 6),
  (48, 6), (24, 6) with segment 20, (22, 6),
  (17, 6) with segment 26, (16, 6) with
  segment 14, and (49, 6). All carry no round.
- FR-4: The San Carlo links (25, 1, 2, 3, 6,
  10, 13, 16, 17, 19, 24, 28, 29) carry no
  round. The links (27, 3), (29, 3), and
  (30, 3) carry optional true.
- FR-5: The link (9, 1) carries the excerpt
  "chiede I e IV mov.". The links (3, 4) and
  (3, 5) carry the excerpt "Allegretto".
  The links (46, 4) and (46, 5) carry the
  excerpt "Vivace".
- FR-6: The link-segment (18, 5, 18) leaves
  the seed. The segment 18 stays on the
  link (18, 4).
- FR-7: The seed holds 34 link-segments. The
  five added rows are (1, 6, 2), (17, 6, 26),
  (16, 6, 14), (24, 6, 20), and (25, 6, 22).
  No new segment is defined.
- FR-8: The audition 4 name is "Cagliari
  (Secondo)". The date, city, and weight do
  not change.
- FR-9: The pool rule does not change. A null
  round counts as one phase (cycle 005). The
  five San Carlo optionals form one pool group:
  the plan card carries pieces 27, 29, and 30;
  the detail page carries all five in one
  composite card; the preparation map merges
  all five into one tile.
- FR-10: The live database removal runs as
  manual DELETEs, with a backup first,
  before the deploy: the three links of
  FR-2 and the link-segment of FR-6. The
  seed upsert delivers every other change.
- FR-11: The round rows 6 and 7 stay in the
  seed, defined but unused. No round row is
  added or removed.

### Non-Functional Requirements

- NFR-1: The seed stays the single source
  for the catalog. No live edit happens
  outside the seed, except the manual
  DELETEs of FR-10.
- NFR-2: The diary loses no row. No session
  is deleted. Piece 21 keeps its five
  sessions.
- NFR-3: The change ships through the normal
  CI deploy. The deploy backup runs first.

## Success Criteria

- SC-1: The seed counts read 46 pieces, 97
  links, 25 segments, 34 link-segments, and
  13 rounds.
- SC-2: The live San Carlo page shows no
  round badge on any link and five "A scelta"
  badges (pieces 25, 27, 28, 29, 30).
- SC-3: The live Cagliari Secondo page shows
  the Mahler as pieces 36 and 37 only, and
  the Beethoven 7 as pieces 46 and 3. The
  Fila page shows Brahms 3 and no Tosca.
- SC-4: The live La Fenice page lists 15
  pieces and no Elgar. The audition stays
  unenrolled.
- SC-5: The live diary page for piece 21
  still lists its five sessions.
- SC-6: The seed comparison at startup reads
  missing 0, extra 0, different 0 after the
  manual DELETEs and the deploy.
- SC-7: The plan shows one pool card for the
  San Carlo choice, carrying pieces 27, 29, and 30. The detail page shows one composite card
  carrying all five optionals. A pool card still
  forms for optional links that share one
  audition and one non-null round.

## Decisions (proposed, pending ratification)

- D-1: The San Carlo round split leaves the
  seed. The call does not state it. The
  owner rule of 2026-11-03 governs: rounds
  come only from the calls.
- D-2: The pool rule stays. A null round is one
  phase (cycle 005). The owner states that a
  competition with no stated round structure is
  a single-round competition, so the optional
  links cluster in it. No code change.
- D-3: The Mahler item stays as two pieces
  (36 and 37), following the one-piece-per-
  movement convention and the approved
  2026-10-27 mapping. The combined piece 21
  keeps its row and sessions but loses its
  Cagliari links.
- D-4: The Beethoven 7 item becomes two
  pieces (46 and 3), following the same
  convention. The combined excerpt on piece
  3 splits into "Vivace" (46) and
  "Allegretto" (3).
- D-5: The Fila Otello link loses its
  segment. The call gives no bar limits.
  The approved 2026-10-27 mapping says the
  Fila link stays whole.
- D-6: The Mozart and Wagner pieces carry
  no segments. The call text gives no bar
  limits, and the score brackets do not
  give legible closing bars.
- D-7: The removed links and the removed
  link-segment leave the live database by
  manual DELETE, with a backup first. The
  seed never deletes a row.
- D-8: The round rows 6 and 7 stay defined
  but unused, as rounds 1–5 and 12–13
  already are. No round row is deleted.
- D-9: The new piece difficulties are set by
  analogy: 46 mirrors 3, 47 and 48 mirror
  8, 49 mirrors 22. The owner or Agnese may
  adjust them later.
- D-10: La Fenice enrollment does not change
  in this cycle. The program arrives; the
  decision to enroll stays with the owner.

## Resolved Questions

- Q-1: Does Santa Cecilia ask for Mendelssohn
  4 movements II and III? No. The official
  English call says "I and IV movements".
  The 2026-10-01 "I–IV" was a loose
  paraphrase. No piece is missing.
- Q-2: Is the San Carlo score book page 8
  really Beethoven 5 III movement? Yes. The
  page sits inside the Beethoven 5 part after
  the Andante, and the III movement is in
  C minor (3 flats). The seed assignment
  stands, now verified.
- Q-3: Does the San Carlo Čajkovskij 6 link
  need a segment? No. Agnese decided on
  2026-10-27 (D3) that the test asks for
  everything, so the link stays whole.
- Q-4: Does the Fila Otello need its segment?
  No. The call gives no bar limits. The
  approved mapping says the link stays whole.
- Q-5: Do the removed links lose diary data?
  No. Links are not diary data. Piece 21
  keeps its row and its five sessions.
- Q-6: Does the mock flow need a code change?
  No. The rule "a piece with no round is
  asked in every round" already covers San
  Carlo after D-1.

## Assumptions

- A-1: The Cagliari program PDFs in the
  local goal folder are the official
  programs. The calls page online holds no
  violoncello match, so the dates and
  determination numbers rest on the
  2026-10-01 verification.
- A-2: The Santa Cecilia English call is the
  official program. The Italian version
  answers 404.
- A-3: The La Fenice score book excerpts
  for Beethoven 5 (bars 1–123), Vespri
  (D to E), Requiem (bars 1–62), and
  Prokofiev (from bar 450) match the reused
  segments 2, 26, 14, and 20. The pages
  show exactly those windows.
- A-4: The owner decided on 2026-11-25: the
  San Carlo rounds leave the seed, and the
  pool rule stays (a null round is one phase).
  The spec is a draft until the owner ratifies
  it by merging the spec pull request.
