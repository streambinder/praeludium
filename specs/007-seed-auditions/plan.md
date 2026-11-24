# Implementation Plan: Repository Seed for Auditions, Pieces, and Segments

**Spec**: `specs/007-seed-auditions/spec.md` (Ratified 2026-11-24, by the owner)

**Feature Branch**: `feat/seed-auditions-spec`

**ADRs**: `.specify/memory/decisions/0002-seed-application.md`
(seed at application start),
`.specify/memory/decisions/0003-seed-users-row.md`
(users row removed from migrate)

## Technical Context

- Stack: Rust, axum, rusqlite (bundled SQLite), minijinja
  templates, single connection behind a mutex.
- Entry points: `src/main.rs` opens the database through
  `open_db` (`src/db.rs`). Tests open an in-memory database
  through `open_memory`. Both call `migrate` today.
- The seed step runs after `migrate` in both entry points
  (ADR 0002). The seed step is a separate function, not part
  of `migrate` (spec FR-11, unciae AGENTS.md §26).
- Coverage gate: `cargo llvm-cov --fail-under-lines 90`
  (`.github/workflows/push.yml`).
- The seed data file is large (216 rows). Generate it from
  the dump with a one-off script kept outside the repository
  (`/tmp`, never committed). Review the generated diff by
  hand against the dump before the implementation PR opens.
- The live database dump is the evidence file
  `db-dump-2026-11-24.txt` (saved with the concorsi goal).
  The agent has no read or write access to the live
  database. The owner runs every live read and every live
  write on the host.

## Phase 0: Pre-Encoding Check (blocks the encoding)

- The owner re-reads the two ambiguous rows on the host,
  in quoted mode: the `piece_audition` rows for pieces 29
  and 30 on audition 3. The list-mode dump shows one extra
  separator in each row. The quoted read settles whether
  each note ends with a literal `|` and what each `round_id`
  holds (spec FR-15).
- Encode exactly what the quoted read shows. Do not guess
  the note text from the list-mode dump.
- Count the encoded rows against the spec before the
  implementation PR opens: 6 auditions, 18 audition links,
  13 rounds, 45 pieces, 85 piece-audition links, 25
  segments, 30 link-segment rows. A count mismatch stops
  the phase.
- Confirm the dump columns the seed does not reproduce:
  `created_at` is absent (the database default fills it on
  insert; the seed never writes it on upsert). The piece
  `excerpt` column is empty for all 45 pieces in the dump;
  verify this during encoding.

## Phase 1: Migrate Writes No Data

- Remove the `INSERT OR IGNORE INTO users` statement from
  `migrate` (`src/db.rs`, near line 387) and its comment.
  The statement holds the last literal personal name in
  production code. Verify with grep that no personal name
  remains in `src/`.
- Keep the `users` table, the `enrollments` table, the
  `SEEDED_USER_ID` constant, and `current_user_id`. Only
  the data write goes.
- Update the `migrate_creates_the_schema_and_is_idempotent`
  test: a fresh database holds the schema and holds zero
  users rows.
- Verification: `cargo test db::` passes. A fresh database
  opens with an empty `users` table.

## Phase 2: Fresh-Database Path and Test Setup

- Problem: the enrollment toggle inserts an `enrollments`
  row, which references `users(id)` with foreign keys on.
  A fresh database has no users row, so the toggle fails
  (ADR 0003).
- Decision (this plan): ensure the row in the toggle
  handler (`handle_toggle_enrollment`, `src/handlers.rs`).
  The handler runs this statement before it inserts the
  enrollment row:

  ```sql
  INSERT OR IGNORE INTO users(id, name) VALUES(1, '')
  ```

  The name is empty because no code reads `users.name`
  (verified by grep: the column is written only by the
  removed migrate statement and never selected). The
  handler is the only production path that needs the row.
  This decision goes to the owner at PR review: ADR 0003
  left this option open as a follow-up.

- The seed never writes users. `open_memory` never writes
  users. Only the toggle handler ensures the row, exactly
  when enrollment needs it.
- Test setup: add a test-only helper in `src/db.rs`,
  `#[cfg(test)]`, that inserts the users row (ID 1, generic
  name, never a personal name) into a test database. Call
  it in every test that writes `enrollments` rows directly
  through SQL: `src/handlers.rs` (one site), `src/readiness.rs`
  (two sites), `tests/routes.rs` (five sites). Tests that
  enroll through the toggle handler need no helper: the
  handler ensures the row itself.
- Verification: a new test enrolls through the toggle
  handler on a fresh database and succeeds. A fresh
  database still holds zero users rows before the toggle
  runs.

## Phase 3: Seed Data Module, Seed Step, Comparison

- New module `src/seed.rs`, declared in `src/lib.rs`. It
  holds the seed data, the seed step, and the read-only
  comparison. No new dependency.
- Seed data layout: plain Rust data, one constant per
  table, typed with the structs from `src/db.rs`
  (`Audition`, `Piece`, `Segment`, `Round`,
  `AuditionLink`) and small tuple structs for the two link
  tables (`piece_audition`, `link_segments`). Constants
  keep the data compiler-checked and the diff reviewable
  in a pull request. No file parsing at runtime. No JSON,
  no embedded SQL.
- Encoding rules:
  - Explicit IDs everywhere (spec D-4). New rows later
    take a new ID above the current maximum (spec FR-13).
  - Auditions carry `city` (spec FR-1). The `Audition`
    struct does not expose `city` today; the seed writes
    the column through SQL. Exposing it in the struct is
    out of scope for this cycle.
  - Archived rows carry their `archived_at` timestamp from
    the dump; active rows carry NULL (spec FR-1, FR-3).
  - Pieces carry `difficulty` and `target_bpm` from the
    dump. `target_bpm` is NULL for all 45 pieces in the
    dump; verify during encoding.
  - Segment ID gaps (13, 15, 17, 19) stay gaps (spec FR-5).
  - Sessions, availability, settings, recordings, mocks,
    mock items, users, and enrollments are not seed data
    (spec FR-8).
- Seed step: `pub fn apply_seed(db: &Connection, data:
&SeedData) -> Result<SeedReport>`. The data is a
  parameter, not a global: the compiled `SEED` constant is
  one instance, and tests build small custom instances.
  The step runs inside one transaction, in foreign-key
  order: auditions, pieces, audition rounds, audition
  links, piece-audition links, segments, link-segment
  rows.
- Upsert semantics (spec FR-7, FR-10, D-3, D-6): the step
  runs this statement with the seed columns only:

  ```sql
  INSERT ... ON CONFLICT(id) DO UPDATE
  ```

  The repository wins on conflict. The step never skips a
  seeded row silently. The report lists every overwritten
  row ID, per table, with inserted, updated, and unchanged
  counts. The step logs the report at start, in the
  existing `eprintln!("event ...")` style.

- Hard constraint: never use `REPLACE` and never
  delete-then-insert a piece. Sessions reference pieces
  by ID with `ON DELETE CASCADE`. A replace would delete
  the sessions of the piece. `ON CONFLICT DO UPDATE`
  keeps the row and its sessions.
- `created_at` is never in an upsert column list. An
  insert leaves it to the column default. An update
  preserves the stored value (spec Q-2).
- The seed step never deletes a row (spec FR-9, D-5). A
  row that exists in the database but not in the seed data
  stays in place.
- Call site: `open_db` and `open_memory` call
  `apply_seed` after `migrate`, through one shared path.
  The seed runs on every start, including tests
  (ADR 0002).
- Read-only comparison (spec FR-14): `pub fn
compare_seed(db: &Connection, data: &SeedData) ->
SeedDiff`, pure reads, no writes. It reports missing
  rows, extra rows, and differing values, per table.
  Delivery form (this plan): the seed step calls the
  comparison after applying and logs the counts (missing,
  extra, different) at startup. No new HTTP endpoint and
  no new CLI subcommand in this cycle: the startup log is
  the verification the owner reads after a deploy.
- Verification: the Phase 5 tests cover the step, the
  report, and the comparison.

## Phase 4: UI Redesign (spec D-3)

- Rule: a user customization never touches a canonical
  table. Every current UI path that writes a canonical
  table is removed or redirected. Read paths stay.
- Remove, with routes, handlers, form buttons, and
  templates:
  - `POST /pieces` (`handle_add_piece`): creates pieces,
    piece-audition links, and inline segments.
  - `POST /pieces/{id}` (`handle_update_piece`): edits
    composer, work, movement, kind, difficulty, and
    target tempo; rewrites piece-audition links and
    link-segment rows.
  - `GET /pieces/{id}/edit` (`handle_edit_piece`): the
    edit form.
  - `POST /auditions` (`handle_add_audition`) and
    `POST /auditions/{id}` (`handle_update_audition`):
    create and edit auditions.
  - `GET /auditions/{id}/edit` (`handle_audition_edit`):
    the edit form.
  - `POST /auditions/{id}/links` and
    `POST /auditions/{id}/links/{linkID}/delete`
    (`handle_add_audition_link`,
    `handle_delete_audition_link`).
  - `POST /auditions/{id}/rounds` and
    `POST /auditions/{id}/rounds/{roundID}/delete`
    (`handle_add_round`, `handle_delete_round`).
  - `POST /pieces/{id}/archive`, `POST /pieces/{id}/restore`,
    `POST /auditions/{id}/archive`,
    `POST /auditions/{id}/restore`: archive state is seed
    data (spec FR-1, FR-3). Archiving becomes a seed
    change in a pull request.
  - Remove the `set_archived` helper and the
    `ARCHIVE_*_SQL` / `RESTORE_*_SQL` constants when no
    caller remains.
  - Remove the orphaned i18n keys of the removed forms.
    Verify with grep that no key is referenced only by a
    removed template.
- Segments have no dedicated routes today: segments enter
  only through the piece add and update forms, through
  `find_or_create_segment`, `set_link_segments`, and
  `gc_orphan_segments` (`src/db.rs`). Those forms go away
  with this phase. Remove `find_or_create_segment`,
  `gc_orphan_segments`, and `clear_piece_link_segments`
  when no caller remains. Keep `add_segment`,
  `delete_segment`, and `set_link_segments` as the
  database-level operations the tests use. The tasks step
  verifies the callers with grep before each removal.
- Correction 2026-11-24 (owner): difficulty and target
  tempo stay on the piece. They are canonical seed
  fields on `pieces` (spec FR-3), not user
  customizations. There is no override table, no merge,
  and no manual rollout for them. An earlier draft of
  this plan invented a user-linked `piece_settings`
  table for them; that table is dropped entirely.
- Consequence: the piece add and update forms go away
  whole, with every field including difficulty and
  target tempo. Difficulty and target tempo change only
  through a seed change in a pull request. Reads use
  `pieces.difficulty` and `pieces.target_bpm` directly
  (the seed values); nothing merges an override.
  `clamp_difficulty` and `form_target_bpm` lose their
  callers with the forms and go away in the caller
  audit. A live difficulty or target tempo that differs
  from the dump is overwritten on the first seed apply
  by design (repository wins, spec D-3/D-6). The comparison
  from Phase 3 lists the differing rows; the owner
  reads them after the deploy and opens a seed
  correction pull request if a live value was the right
  one. No copy runs before the deploy.
- Keep unchanged: the enrollment toggle (user-linked),
  sessions and the baseline handler (user data), mock
  setup and run, recordings, availability, settings,
  readiness, diary, and all read paths for auditions,
  pieces, segments, links, and rounds.

## Phase 5: Tests

- New tests, all in the existing inline `#[cfg(test)]`
  modules or `tests/routes.rs`:
  - Fresh database holds the seed: `open_memory` yields
    6 auditions, 18 links, 13 rounds, 45 pieces, 85
    piece-audition links, 25 segments, and 30
    link-segment rows, with the IDs of the dump
    (spec SC-1).
  - Second open changes nothing: a file database opened
    twice yields an identical dump before and after the
    second open (spec SC-2). Reuse the
    `reopening_never_enrolls` pattern (temp file,
    cleanup of the WAL files).
  - Overwrite is reported and applied: change one seeded
    row, reopen, assert the seed values are restored and
    the report lists the row (spec FR-10).
  - Extra row stays and is reported: insert a row that
    the seed does not hold, reopen, assert it stays and
    the comparison reports it (spec FR-9, FR-14).
  - Sessions and enrollments survive: seed sessions and
    an enrollment on a test database, reopen, assert the
    same IDs, counts, and values (spec SC-4).
  - Reopening never enrolls: extend the existing test to
    the seeded database (a seeded audition starts
    unenrolled; the enrollments count stays 0).
  - Seed addition through a custom instance: build a
    small `SeedData` with one extra audition, apply it
    twice to a test database, assert the audition appears
    after the first apply and the second apply changes
    nothing (spec SC-6, on a test database, never on
    production data).
  - Toggle on a fresh database: the toggle handler
    enrolls on a database with no users row (Phase 2).
- Rework of the existing tests. The seed runs in
  `open_memory`, so no test starts from an empty
  database any more. Rule for the rework:
  - Rows a test creates take IDs above the seed maxima
    (auditions above 6, pieces above 45, segments above
    29, rounds above 13, links above 18) or auto-assigned
    IDs captured from `last_insert_rowid`. Never hardcode
    a seeded ID.
  - Count assertions scope to the rows the test creates,
    or assert the seeded counts where the test covers the
    repertoire.
  - Tests that write `enrollments` directly use the
    Phase 2 helper.
  - Affected files: `src/db.rs` (the link tests hardcode
    piece 1 and audition 1, which the seed now holds),
    `src/handlers.rs` (28 tests, including the enrollment
    fixture), `src/readiness.rs` (explicit IDs 1-3 and
    enrollments), `src/stats.rs` (explicit IDs),
    `src/mocks.rs`, and `tests/routes.rs` (page tests
    that assume empty lists, and five enrollment sites).
  - `reopening_never_enrolls` changes: the inserted
    audition takes an auto-assigned ID, and the test
    asserts that audition is unenrolled and the
    enrollments count is 0, not that the auditions count
    is 1.
- Verification: `cargo test` passes in full.
  `cargo llvm-cov --locked --fail-under-lines 90` passes.
  The seed data constants are not executable lines, so
  the data file does not move the gate. The seed step,
  the comparison, and the toggle ensure need direct test
  coverage.

## Phase 6: Verification and Deploy

- Branch strategy: push the spec branch
  `feat/seed-auditions-spec` first, open its pull
  request, and merge it. The implementation runs on a
  new branch `feat/seed-auditions` from the merged
  master. One commit per phase; one commit stays
  preferable when a phase is small. A fix amends its
  commit. The implementation pull request follows the
  repository rules: signed commits with the repository author,
  Conventional Commits,
  rebase merge, no real verification subject named in
  the commit or the pull request body.
- Preflight before the pull request opens, per the
  `preflight_code_changes` skill: remote branch state,
  local `cargo test`, `cargo llvm-cov`, `cargo fmt`,
  `cargo clippy`, and the CI jobs from `push.yml`
  reproduced locally where the skill prescribes them.
  The pull request is called ready only on green CI
  verified on the exact head.
- EXTRA CARE deploy (standing rule): backup the live
  database before the deploy. Deploy. Verify live from
  the host: the audition list shows 6 auditions, with
  the seeded one unenrolled; the sessions count is 215;
  the enrollments count is 5. Read the startup log: the
  seed report and the comparison counts show the
  convergence. Rollback is a redeploy of the previous
  image, plus a restore from the pre-deploy backup if a
  seed write went wrong.

## Risks and Open Questions (for the tasks step)

- R1: a live difficulty or target tempo that differs
  from the dump is overwritten on the first seed apply.
  That is the decided behavior (repository wins, spec D-3):
  difficulty and target tempo are canonical seed fields
  (owner correction 2026-11-24), so there is no copy
  before the deploy. The comparison output lists the
  differing rows; the owner reads them after the deploy
  and opens a seed correction pull request if a live
  value was the right one.
- R2: the seed data file is hand-reviewed generated data
  (~216 rows). A wrong row ships to production on the
  next deploy. The Phase 0 count check and the
  comparison after the first deploy are the guards. The
  two ambiguous rows block the encoding until the
  quoted-mode re-read lands.
- R3: the test rework is the largest part by file count
  (about 100 inline tests plus the integration tests).
  Some tests are easier to rewrite against the seeded
  rows than to isolate from them. The tasks step audits
  each file and picks the cheaper rewrite per test.
- R4: `city` is stored but never read. The seed writes
  it (spec FR-1). Exposing it in the `Audition` struct
  and the templates is out of scope for this cycle.
- R5: the comparison delivery form is the startup log
  (Phase 3). The tasks step confirms no smaller form
  serves the owner better. A dedicated endpoint or CLI
  subcommand is a later cycle if the owner asks for it.
- R6: `created_at` differs between a fresh database and
  the live one by construction. The upsert must never
  touch it. A test asserts the stored value survives an
  overwrite.
- R7: the seed step runs on every start, including
  tests. The cost is one pass over 216 rows. No
  performance work is planned.
- R8: removing the piece and audition forms removes a
  capability the owner used for quick fixes. After this
  cycle, every repertoire change is a seed pull request.
  That is the decided trade-off (spec User Story 6).
- Decided in tasks.md: the seed data lives in its own
  file, `src/seed_data.rs` (the `SEED` constant only);
  `src/seed.rs` holds the step, the comparison, and the
  report. The override-table name is moot: the table
  was dropped by the owner correction of 2026-11-24.
- Open: whether `add_segment` stays for tests or the
  segment tests move to seeded segments. The tasks step
  decides after the caller audit.
