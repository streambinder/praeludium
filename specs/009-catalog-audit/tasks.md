# Task List: Catalog Audit Against the Official Calls

**Spec**: `specs/009-catalog-audit/spec.md` (Draft, pending ratification)

**Plan**: `specs/009-catalog-audit/plan.md` (3 phases)

**Branch**: `feat/catalog-audit-009-spec` for this file; the
implementation runs on `feat/catalog-audit-009` from the
merged master (plan Phase C).

**Evidence**: the six official calls fetched fresh on
2026-11-25 (`bandi-extract.md` in the surgery folder,
13 PDFs with OCR) and the seed data
(`src/seed_data.rs`, six auditions, IDs 1-6, 42 pieces,
82 piece-audition links, 25 segments, 30 link-segments,
13 rounds). The La Fenice score book (`PassiVC.pdf`,
11 pages) was read page by page.

## Conventions for this task list

- One commit per plan phase on
  `feat/catalog-audit-009`. One commit stays
  preferable when a phase is small. A fix amends its
  commit. Signed commits, repository author,
  Conventional Commits, rebase merge.
- Preflight per the `preflight_code_changes` skill
  before the implementation pull request opens. The
  pull request is called ready only on green CI verified
  on the exact head.
- Coverage gate: `cargo llvm-cov --locked
--fail-under-lines 90`.
- A task marked owner runs on the host, by the owner,
  never by the agent.

## Decisions recorded in this step

- The pool rule does not change (spec D-2, owner
  correction 2026-11-25): a null round is one
  phase, so the five San Carlo optionals form one
  pool group. The plan card carries the pool-only
  members (27, 29, 30); the detail page carries
  all five in one composite card; the map merges
  all five into one tile. No code change. The
  rejected alternative amends the rule to require
  a non-null round; the owner corrected it.
- The removed links and the removed link-segment
  leave the live database by manual DELETE, with a
  backup first (plan Phase C). The seed never
  deletes a row. The rejected alternative adds a
  delete step to the seed; the seed is insert and
  upsert only, by cycle 007 design.
- The Mozart and Wagner pieces carry no segments
  (spec D-6). The call text gives no bar limits,
  and the score brackets give no legible closing
  bars. A whole movement covers the excerpt.

## Phase A: Seed data

- [ ] A.1 `AUDITIONS` (`src/seed_data.rs`, near line
      24): rename audition 4 from "Cagliari
      (Concertino)" to "Cagliari (Secondo)".
- [ ] A.2 `PIECES` (`src/seed_data.rs`, near line
      261): append pieces 46, 47, 48, and 49 in ID
      order, with the composers, works, movements,
      kinds, and difficulties of plan Phase A.
- [ ] A.3 `PIECE_AUDITIONS` (`src/seed_data.rs`,
      near line 684): remove the rows (20, 5),
      (21, 4), and (21, 5).
- [ ] A.4 `PIECE_AUDITIONS`: add the rows (7, 5)
      with round 11, and (46, 4) with round 9 and
      (46, 5) with round 11, excerpt "Vivace".
- [ ] A.5 `PIECE_AUDITIONS`: set the excerpt
      "Allegretto" on (3, 4) and (3, 5), and the
      excerpt "chiede I e IV mov." on (9, 1).
- [ ] A.6 `PIECE_AUDITIONS`: set `optional: true`
      on (27, 3), (29, 3), and (30, 3).
- [ ] A.7 `PIECE_AUDITIONS`: set `round_id: None`
      on the 13 San Carlo links that carry a round:
      (25, 3) and the 12 links (1, 2, 3, 6, 10, 13,
      16, 17, 19, 24, 28, 29) with audition 3.
- [ ] A.8 `PIECE_AUDITIONS`: append the 15 La
      Fenice links of plan Phase A, all with
      `round_id: None`, with the excerpts of plan
      Phase A on (25, 6), (28, 6), and (29, 6).
- [ ] A.9 `LINK_SEGMENTS` (`src/seed_data.rs`, near
      line 1621): remove the row (18, 5, 18) and add
      the rows (1, 6, 2), (17, 6, 26), (16, 6, 14),
      (24, 6, 20), and (25, 6, 22).
- [ ] A.10 Verify: parse the seed file and assert
      the counts of spec SC-1 (46 pieces, 97 links,
      25 segments, 34 link-segments, 13 rounds) and
      every added, removed, and changed row of tasks
      A.1–A.9.

## Phase B: Tests

- [ ] B.1 Extend the seed count test (or add one
      near it) to assert 46 pieces, 97 links, 25
      segments, 34 link-segments, and 13 rounds.
- [ ] B.2 Add an integration test that seeds a test
      database, applies the seed, and asserts: no
      San Carlo link carries a round; the five San
      Carlo optionals are 25, 27, 28, 29, 30; the
      Fila page holds no Tosca link and holds
      Brahms 3; the La Fenice page holds 15 links
      and no Elgar; the diary for piece 21 still
      holds five sessions.
- [ ] B.3 Add an integration test for the pool
      outcome: with the San Carlo seed data,
      `plan_pools` returns one pool for audition 3
      with members 27, 29, and 30; the piece list
      for audition 3 renders one composite card
      carrying all five optionals; the preparation
      map for audition 3 merges the five into one
      tile.
- [ ] B.4 Keep a positive pool test: two optional
      links with the same audition and the same
      non-null round still form one pool card in the
      plan and in the piece list. Seed a minimal
      pair in the test database when the Cagliari
      data offers none.
- [ ] B.5 Verify: the full local gate passes
      (`cargo fmt --all`, `cargo clippy
--all-targets -- -D warnings`, `cargo test
--all-targets`, `cargo llvm-cov --locked
--fail-under-lines 90`).

## Phase C: Verification and Deploy

- [ ] C.1 Run the full local gate (see B.5).
- [ ] C.2 Open the implementation PR. CI must be
      green and the state CLEAN before merge. The
      merge is the owner's.
- [ ] C.3 Data step, owner, before the deploy, with
      a backup first (`sudo -n
/etc/cron.daily/backup-praeludium` on lobster,
      exit 0): stop the container, run the manual
      DELETEs of plan Phase C, and verify the counts
      (46 pieces, 97 links, 34 link-segments, 218
      sessions, 5 diary rows for piece 21).
- [ ] C.4 Deploy through the master CI run. Verify
      live: the San Carlo page shows no round badge
      and five "A scelta" badges; the Cagliari pages
      show the corrected excerpts; the La Fenice page
      lists 15 pieces; the diary for piece 21 lists
      five sessions; the startup log reads `seed
comparison missing=0 extra=0 different=0`.
- [ ] C.5 Close the cycle with a converge PR, on the
      model of cycle 008.

## Acceptance checklist (spec success criteria)

- [ ] SC-1: The seed counts read 46 pieces, 97
      links, 25 segments, 34 link-segments, and 13
      rounds.
- [ ] SC-2: The live San Carlo page shows no round
      badge on any link and five "A scelta" badges
      (pieces 25, 27, 28, 29, 30).
- [ ] SC-3: The live Cagliari Secondo page shows the
      Mahler as pieces 36 and 37 only, and the
      Beethoven 7 as pieces 46 and 3. The Fila page
      shows Brahms 3 and no Tosca.
- [ ] SC-4: The live La Fenice page lists 15 pieces
      and no Elgar. The audition stays unenrolled.
- [ ] SC-5: The live diary page for piece 21 still
      lists its five sessions.
- [ ] SC-6: The seed comparison at startup reads
      missing 0, extra 0, different 0 after the
      manual DELETEs and the deploy.
- [ ] SC-7: The plan shows one pool card for the
      San Carlo choice, carrying pieces 27, 29, and 30. The detail page shows one composite card
      carrying all five optionals. A pool card still
      forms for optional links that share one
      audition and one non-null round.
