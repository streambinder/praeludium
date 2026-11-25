# Task List: Choice Clusters Independent of the Round

**Spec**: `specs/010-choice-clusters/spec.md` (Draft, pending ratification)

**Plan**: `specs/010-choice-clusters/plan.md` (3 phases)

**Branch**: `feat/choice-clusters-010-spec` for this file; the
implementation runs on `feat/choice-clusters-010` from the
merged master (plan Phase C).

**Evidence**: the San Carlo bando, Art. 6 (screenshot
of 2026-11-25 and the OCR extract of cycle 009),
and the seed data (`src/seed_data.rs`, six
auditions, IDs 1-6, 46 pieces, 97 piece-audition
links, 25 segments, 35 link-segments, 13 rounds).
The eight optional links are (34, 1), (25, 3),
(27, 3), (28, 3), (29, 3), (30, 3), (28, 6),
(29, 6), all with a null round.

## Conventions for this task list

- One commit per plan phase; a fix amends the
  commit it touches. The spec PR holds one
  commit, the implementation PR one commit per
  phase.
- Every task states its checkable outcome.
  Mark a task done only when its outcome is
  verified, not when the edit is typed.
- The local gate is `cargo fmt --all`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets`, and the exact CI
  coverage command
  `cargo llvm-cov --locked --fail-under-lines 90`.

## Phase A: Schema and plumbing

- [x] T001: The canonical `CREATE TABLE` for
      `piece_audition` in `src/db.rs` gains
      `cluster TEXT`. A fresh database has the
      column.
- [x] T002: The `Audition` record gains a
      `cluster` field (`Option<String>`, null when
      the link has none). The `piece_auditions`
      query selects `pc.cluster`, appended after
      the enrolled check so the earlier column
      indices do not move, and the row reader maps
      it. Every other select that builds an
      `Audition` from a link gains the column too.
- [x] T003: A database unit test asserts a
      fresh database has the `cluster` column and
      a link read returns a null cluster.
- [x] T004: The full local gate is green on
      the Phase A commit.

## Phase B: Pool rule, seed data, and tests

- [x] T005: `SeedPieceAudition` in `src/seed.rs`
      gains a `cluster` field, and the link upsert
      inserts it and sets it on conflict.
- [x] T006: `src/seed_data.rs` sets the eight
      cluster values: `haydn` on (25, 3) and
      (27, 3); `concerto` on (28, 3), (29, 3), and
      (30, 3); `concerto` on (28, 6) and (29, 6).
      Every other link literal carries a null
      cluster.
- [x] T007: `plan_pools` keys on (audition,
      cluster). A null cluster never pools. The
      `PoolPlan` key type becomes `(i64, String)`.
      The plan exclusion in `score_piece_scopes`
      uses the same pair.
- [x] T008: The audition detail piece-list
      grouping keys on (audition, cluster).
- [x] T009: The preparation map
      single-audition tile merge keys on (audition,
      cluster).
- [x] T010: The three synthetic pool tests
      (`pool_card_on_audition_page_and_prepmap`,
      `pool_plan_card_shared_weight_and_member_choice`,
      `pool_member_with_mandatory_link_planned_normally`)
      build their pairs by cluster instead of by
      round, and pass. The pool unit test and the
      pool tile test are updated if the key type
      touches them, and pass.
- [x] T011: `san_carlo_null_round_forms_one_pool`
      is rewritten as `san_carlo_two_choice_clusters`:
      the plan holds one pool card with members 29
      and 30; pieces 25, 27, and 28 keep single
      plan items; the detail page shows two pool
      cards (two and three subcards) and ten single
      cards, with no round badge.
- [x] T012: New assertions pass (seven links with a cluster value, not eight: Piatti stays null per Q-2): the La Fenice
      detail page shows one pool card with two
      subcards; the Santa Cecilia Piatti link stays
      a single item with its badge; the seed state
      test asserts three clusters and eight links
      with a cluster value.
- [x] T013: The full local gate is green on
      the Phase B commit.

## Phase C: Deploy and converge

- [x] T014: The live database is backed up on
      lobster before any manual step.
- [x] T015: The owner runs the manual migration (2026-11-25 22:28, counts 46|97|35|228)
      on lobster and the verification query returns
      the cycle 009 counts (46 pieces, 97 links, 35
      link-segments, sessions 222 or more).
- [x] T016: The implementation PR is merged on
      a green and clean CI run, and the master run
      is green through the deploy job.
- [x] T017: Live verification confirms SC-1 to
      SC-4: two San Carlo pool cards, one plan pool
      card with members 29 and 30, one La Fenice
      pool card, Piatti single, and a clean seed
      comparison (0 missing, 0 extra, 0 different).
      The plan pool card is covered by the
      integration test; the live plan of 2026-11-25
      ranks other pieces above the pool, so no pool
      card shows there today.
- [ ] T018: The converge PR marks this spec
      Complete, checks the tasks, and is merged on
      a green CI run.
