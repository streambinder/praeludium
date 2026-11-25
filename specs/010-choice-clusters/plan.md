# Implementation Plan: Choice Clusters Independent of the Round

**Spec**: `specs/010-choice-clusters/spec.md` (Draft, pending ratification)

**Feature Branch**: `feat/choice-clusters-010-spec`

## Technical Context

- Stack: Rust, axum, rusqlite (bundled SQLite), minijinja
  templates, single connection behind a mutex.
- The pool groups live in three sites, all keyed
  today on (audition, round): `plan_pools`
  (`src/handlers.rs`, near line 611) builds pool
  cards for the plan; the piece-list grouping
  (`src/handlers.rs`, near line 1963) builds pool
  cards for the audition detail page; the
  preparation map (`src/prepmap.rs`, near line 247) merges pool tiles in a single-audition
  view. The plan exclusion in `score_piece_scopes`
  (`src/handlers.rs`, near line 495) keys on the
  same pair. The `PoolPlan` key
  (`src/handlers.rs`, near line 1356) is
  `(i64, Option<i64>)`.
- The per-link fields travel on the `Audition`
  record (`src/db.rs`, near line 70): `round_id`,
  `round_name`, `optional`. The `piece_auditions`
  query (`src/db.rs`, near line 475) selects
  them. The seed link record
  (`src/seed.rs`, near line 60) is
  `SeedPieceAudition`, and the link upsert
  (`src/seed.rs`, near line 461) sets excerpt,
  note, round_id, and optional on conflict, so
  every link field change arrives at deploy.
  The seed never deletes a row.
- The live migration follows the rule of
  2026-11-24: code carries only the canonical
  schema. The new column enters the canonical
  `CREATE TABLE` for `piece_audition`, and the
  live database gains it through a manual
  one-shot `ALTER TABLE` before the deploy.
  The precedent is the `optional` column of
  cycle 005, added by an `ALTER TABLE` in code
  at the time and later removed from the
  migration in the cleanup; the manual step
  replaces that pattern.
- The existing pool tests seed their pairs
  through the round: `pool_card_on_audition_page_and_prepmap`,
  `pool_plan_card_shared_weight_and_member_choice`,
  `pool_member_with_mandatory_link_planned_normally`,
  and `san_carlo_null_round_forms_one_pool`
  (`tests/routes.rs`). The unit test
  `pool_plan_scores_count_member_sessions`
  (`src/handlers.rs`) exercises pool scoring.
  The map test
  `pool_tile_with_rated_member_shows_confidence`
  (`src/prepmap.rs`) exercises the tile merge.
  The mock round trip (`src/mocks.rs`) uses
  rounds for mocks, not for pools, and stays
  untouched.

## Phase A: Schema, plumbing, and the pool rule

- Add `cluster TEXT` to the canonical
  `CREATE TABLE` statement for `piece_audition`
  in `src/db.rs`.
- Add a `cluster` field to the `Audition`
  record (`Option<String>`, null when the link
  has none), select it in `piece_auditions`
  (appended after the enrolled check so the
  earlier column indices do not move), and map
  it in the row reader. Every other select that
  builds an `Audition` from a link gains the
  column too.
- Add `cluster` to `SeedPieceAudition` and to
  the link upsert in `src/seed.rs` (insert and
  on-conflict update).
- Change the pool key to (audition, cluster) in
  `plan_pools`, in the detail piece-list
  grouping, and in the preparation map tile
  merge. A null cluster never pools. Change the
  `PoolPlan` key type to `(i64, String)` and
  the plan exclusion in `score_piece_scopes` to
  the same pair.
- Update the pool tests that build pairs by
  round so they build them by cluster, and
  update their assertions to the new key.
  Rewrite `san_carlo_null_round_forms_one_pool`
  as the two-cluster assertion of the seed:
  rename it `san_carlo_two_choice_clusters`.
- Gate: format, clippy with warnings denied,
  full unit and integration test suites, and the
  CI coverage command.

## Phase B: Seed data, new tests, and deploy

- Set the cluster values in `src/seed_data.rs`:
  `haydn` on (25, 3) and (27, 3); `concerto`
  on (28, 3), (29, 3), and (30, 3); `concerto`
  on (28, 6) and (29, 6). Every other link
  stays null.
- Add the integration assertions: San Carlo
  detail shows two pool cards (two and three
  subcards) and ten single cards; the plan
  holds one pool card with members 29 and 30,
  and pieces 25, 27, and 28 keep single items;
  La Fenice detail shows one pool card with two
  subcards; Piatti 34 stays a single item with
  its badge. Extend the seed state test with
  the cluster counts (three clusters, eight
  links with a cluster value).
- Gate: the same full local gate as Phase A.
- Deploy sequence: backup of the live database
  first; then the owner runs the manual
  migration on lobster; then merge of the
  implementation PR, then the master run; then
  live verification of SC-1 to SC-4; then the
  converge PR.

## Manual migration (live, once, before the deploy)

```sh
sudo -n /etc/cron.daily/backup-praeludium
```

```sh
sqlite3 /var/lib/praeludium/praeludium.db "ALTER TABLE piece_audition ADD COLUMN cluster TEXT;"
```

Verification, with the expected counts
unchanged from cycle 009:

```sh
sqlite3 /var/lib/praeludium/praeludium.db "SELECT (SELECT COUNT(*) FROM pieces), (SELECT COUNT(*) FROM piece_audition), (SELECT COUNT(*) FROM link_segments), (SELECT COUNT(*) FROM sessions);"
```

Expected: `46|97|35|222` plus any sessions
Agnese logged since 2026-11-25. The seed
upsert sets the eight cluster values at the
first start after the deploy.

## Risks

- The deploy must not start before the manual
  migration: the seed upsert names the new
  column, so a start without it fails. The
  deploy sequence in Phase B orders the steps.
- A cluster value typo in the seed would split
  a group silently. The seed state test asserts
  the cluster counts, and the live check reads
  the two San Carlo cards.
