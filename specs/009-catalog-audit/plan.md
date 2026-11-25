# Implementation Plan: Catalog Audit Against the Official Calls

**Spec**: `specs/009-catalog-audit/spec.md` (Draft, pending ratification)

**Feature Branch**: `feat/catalog-audit-009-spec`

## Technical Context

- Stack: Rust, axum, rusqlite (bundled SQLite), minijinja
  templates, single connection behind a mutex.
- The seed lives in `src/seed_data.rs` (the `SEED`
  constant: `AUDITIONS`, `ROUNDS`, `LINKS`,
  `PIECES`, `PIECE_AUDITIONS`, `SEGMENTS`,
  `LINK_SEGMENTS`) and `src/seed.rs` (apply step
  and comparison). The link upsert sets excerpt,
  note, round_id, and optional on conflict, so
  every link field change arrives at deploy.
  The link-segment insert is `INSERT OR IGNORE`,
  so new rows arrive at deploy. The seed never
  deletes a row.
- The pool groups live in `src/handlers.rs`:
  `plan_pools` (near line 611) builds pool cards
  for the plan, and the piece-list grouping (near
  line 1963) builds pool cards for the audition
  page. The preparation map (`src/prepmap.rs`,
  near line 247) merges pool tiles in a
  single-audition view. All three key on
  `(audition_id, round_id)` and read a null round
  as one phase. No code change in this cycle:
  the seed changes alone produce the new pool.
  The mock pool (`src/mocks.rs`, near line 156)
  needs no change: a piece with no round is asked
  in every round by design.
- Coverage gate: `cargo llvm-cov --locked
--fail-under-lines 90` (`.github/workflows/push.yml`).
- Counts before: 42 pieces, 82 links, 25 segments,
  30 link-segments, 13 rounds. Counts after: 46
  pieces, 97 links, 25 segments, 34 link-segments,
  13 rounds.

## Phase A: Seed data

- `src/seed_data.rs`, `AUDITIONS`: rename audition
  4 from "Cagliari (Concertino)" to "Cagliari
  (Secondo)". Nothing else changes on the row.
- `src/seed_data.rs`, `PIECES`: append four pieces
  after ID 45, in ID order: 46 Beethoven,
  "Sinfonia n. 7", "I mov. Vivace", kind
  "excerpt", difficulty 3; 47 Mozart, "Sinfonia
  n. 40", "I mov.", kind "excerpt", difficulty
  4; 48 Mozart, "Sinfonia n. 40", "IV mov.",
  kind "excerpt", difficulty 4; 49 Wagner,
  "Tristano e Isotta", "Preludio", kind
  "excerpt", difficulty 5. All carry an empty
  excerpt and `target_bpm: None`.
- `src/seed_data.rs`, `PIECE_AUDITIONS`: remove
  the rows (20, 5), (21, 4), and (21, 5).
- `src/seed_data.rs`, `PIECE_AUDITIONS`: add the
  rows (7, 5) with round 11 and (46, 4) and
  (46, 5) with round 9 and 11, excerpt "Vivace".
- `src/seed_data.rs`, `PIECE_AUDITIONS`: set the
  excerpt "Allegretto" on (3, 4) and (3, 5).
  Set the excerpt "chiede I e IV mov." on
  (9, 1).
- `src/seed_data.rs`, `PIECE_AUDITIONS`: set
  `optional: true` on (27, 3), (29, 3), and
  (30, 3).
- `src/seed_data.rs`, `PIECE_AUDITIONS`: set
  `round_id: None` on the 13 San Carlo links
  that carry a round: (25, 3) and (1, 2, 3, 6,
  10, 13, 16, 17, 19, 24, 28, 29) with
  audition 3.
- `src/seed_data.rs`, `PIECE_AUDITIONS`: append
  the 15 La Fenice links, all with `round_id:
None`: (25, 6) with segment 22, (26, 6),
  (28, 6) optional, (29, 6) optional, (1, 6),
  (4, 6), (6, 6), (10, 6), (47, 6), (48, 6),
  (24, 6), (22, 6), (17, 6), (16, 6), (49, 6).
  The excerpt on (25, 6) reads "senza cadenza".
  The excerpt on (28, 6) and (29, 6) reads
  "a scelta fra Dvořák e Schumann". The other
  excerpts stay empty, as the call text gives
  no limits.
- `src/seed_data.rs`, `LINK_SEGMENTS`: remove the
  row (18, 5, 18). Add the rows (1, 6, 2),
  (17, 6, 26), (16, 6, 14), (24, 6, 20), and
  (25, 6, 22).
- `src/seed_data.rs`, `ROUNDS`: no change. Rows 6
  and 7 stay defined but unused.
- Verification: a script parses the seed file and
  asserts the counts of spec SC-1 and every
  added, removed, and changed row listed above.
  The counts are the gate for this phase.

## Phase B: Tests

- Extend the seed count test (or add one near
  it) to assert 46 pieces, 97 links, 25
  segments, 34 link-segments, and 13 rounds.
- Add an integration test that seeds a test
  database, applies the seed, and asserts: no
  San Carlo link carries a round; the five San
  Carlo optionals are 25, 27, 28, 29, 30; the
  Fila page holds no Tosca link and holds
  Brahms 3; the La Fenice page holds 15 links
  and no Elgar; the diary for piece 21 still
  holds five sessions.
- Add an integration test for the pool outcome:
  with the San Carlo seed data, `plan_pools`
  returns one pool for audition 3 with members
  27, 29, and 30; the piece list for audition 3
  renders one composite card carrying all five
  optionals; the preparation map for audition 3
  merges the five into one tile.
- Keep a positive pool test: two optional links
  with the same audition and the same non-null
  round still form one pool card in the plan
  and in the piece list. Use the Cagliari
  data only if it offers such a pair; else
  seed a minimal pair in the test database.
- Verification: the full local gate passes
  (format, clippy with `-D warnings`, all
  tests, coverage with the exact CI command).

## Phase C: Verification and Deploy

- Run the full local gate: `cargo fmt --all`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets`, and the coverage
  command `cargo llvm-cov --locked
--fail-under-lines 90`.
- Open the implementation PR on the spec branch
  base. CI must be green and the state CLEAN
  before merge. The merge is the owner's.
- Data step, before the deploy, with a backup
  first (`sudo -n /etc/cron.daily/backup-praeludium`
  on lobster, exit 0): stop the container, then
  run the manual DELETEs and verify the counts.
  The exact commands go in the PR body and in
  the handoff to the owner:
  - `DELETE FROM piece_audition WHERE
(piece_id, audition_id) IN ((20, 5),
(21, 4), (21, 5));`
  - `DELETE FROM link_segments WHERE piece_id
= 18 AND audition_id = 5 AND segment_id
= 18;`
  - Verify: 46 pieces, 97 links, 34
    link-segments, 218 sessions, and the
    diary rows for piece 21 still 5.
- Deploy through the master CI run. Verify live:
  the San Carlo page shows no round badge and
  five "A scelta" badges; the Cagliari pages
  show the corrected excerpts; the La Fenice
  page lists 15 pieces; the diary for piece 21
  lists five sessions; the startup log reads
  `seed comparison missing=0 extra=0
different=0`.
- Close the cycle with a converge PR, on the
  model of cycle 008.

## Risks and Open Questions (for the tasks step)

- The Mozart excerpt end bars are not legible
  in the Fenice score book. The pieces stay
  whole. A later correction needs the score
  with readable bars or Agnese's word.
- The Cagliari dates and determination numbers
  rest on the 2026-10-01 verification. The
  live site no longer lists them.
