# Task List: Repository Seed for Auditions, Pieces, and Segments

**Spec**: `specs/007-seed-auditions/spec.md` (Ratified 2026-11-24, by the owner)

**Plan**: `specs/007-seed-auditions/plan.md` (6 phases)

**Branch**: `feat/seed-auditions-spec` for this file; the
implementation runs on `feat/seed-auditions` from the
merged master (plan Phase 6).

**Evidence**: the live database dump of 2026-11-24,
`db-dump-2026-11-24.txt`, saved with the concorsi goal.
The agent has no read or write access to the live
database. The owner runs every live read and every live
write on the host.

## Conventions for this task list

- One commit per plan phase on `feat/seed-auditions`.
  One commit stays preferable when a phase is small. A
  fix amends its commit. Signed commits, repository author,
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

- Seed-data file split (plan open item, decided here):
  `src/seed_data.rs` holds the `SEED` constant only;
  `src/seed.rs` holds the step, the comparison, and the
  report. Both are declared in `src/lib.rs`. The data
  file is generated and hand-reviewed; the step file
  stays small and testable.
- Override-table name (plan open item): moot. The
  owner correction of 2026-11-24 dropped the
  `piece_settings` table entirely. Difficulty and
  target tempo stay on the piece as canonical seed
  fields. There is no override table to name, no manual
  rollout for it, and no calibration copy before the
  first seed apply.

## Phase 0: Pre-Encoding Check (blocks the encoding)

Owner tasks, on the host. The encoding does not start
until the output of task 0.1 lands.

- [ ] 0.1 Owner: re-read the two ambiguous rows in
      quoted mode. Run, on the host:

      ```console
      sudo podman run --rm -v /var/lib/praeludium:/data docker.io/nouchka/sqlite3 /data/praeludium.db ".mode quote" "SELECT piece_id, audition_id, excerpt, note, round_id, optional FROM piece_audition WHERE audition_id = 3 AND piece_id IN (29, 30) ORDER BY piece_id;"
      ```

- [ ] 0.2 Owner: run the discriminating hex check, on
      the host:

      ```console
      sudo podman run --rm -v /var/lib/praeludium:/data docker.io/nouchka/sqlite3 /data/praeludium.db "SELECT piece_id, hex(note), round_id, optional FROM piece_audition WHERE audition_id = 3 AND piece_id IN (29, 30) ORDER BY piece_id;"
      ```

- [ ] 0.3 Owner: run the count check, on the host:

      ```console
      sudo podman run --rm -v /var/lib/praeludium:/data docker.io/nouchka/sqlite3 /data/praeludium.db "SELECT COUNT(*) FROM piece_audition;"
      ```

- [ ] 0.4 Owner: paste the full output of 0.1, 0.2,
      and 0.3 back. The commands are read-only. If SQLite
      reports `database is locked`, wait a minute and
      retry; do not stop the application container for a
      read.
- [ ] 0.5 Implementer: read the quoted output. In
      `.mode quote` every value prints as an SQL literal:
      text in single quotes, NULL as NULL, integers bare.
      A `|` inside the note stays inside the quotes, so
      the field boundaries are unambiguous. Record the
      exact note and round_id of each row. `hex(note)`
      ends in `7C` exactly when the note ends with a
      literal `|`. The list-mode dump suggests row 29
      carries the note with a trailing `|` and round_id
      7, and row 30 carries the note with a trailing `|`
      and round_id NULL; encode what the quoted output
      shows, not the suggestion.
- [ ] 0.6 Implementer: confirm the count is 85. The
      count gate for the encoded data is 6 auditions, 18
      audition links, 13 rounds, 45 pieces, 85
      piece-audition links, 25 segments, and 30
      link-segment rows. A mismatch stops the phase.

## Phase 1: Migrate Writes No Data

- [ ] 1.1 Remove the `INSERT OR IGNORE INTO users`
      statement and its comment from `migrate`
      (`src/db.rs`, near line 387). Keep the `users`
      table, the `enrollments` table, the
      `SEEDED_USER_ID` constant, and `current_user_id`.
- [ ] 1.2 Grep `src/` for personal names. Expect
      none: the removed statement held the only literal
      name in production code.
- [ ] 1.3 Update the
      `migrate_creates_the_schema_and_is_idempotent`
      test: a fresh database holds the schema and holds
      zero users rows.
- [ ] 1.4 Verify: `cargo test db::` passes. A fresh
      database opens with an empty `users` table.

## Phase 2: Fresh-Database Path and Test Setup

- [ ] 2.1 Ensure the users row in the toggle handler
      (`handle_toggle_enrollment`, `src/handlers.rs`,
      near line 2502). The handler runs this statement
      before it inserts the enrollment row:

      ```sql
      INSERT OR IGNORE INTO users(id, name) VALUES(1, '')
      ```

      The name is empty: no code reads `users.name`. The
      seed never writes users. `open_memory` never writes
      users.

- [ ] 2.2 Add a test-only helper in `src/db.rs`
      (`#[cfg(test)]`) that inserts the users row (ID 1,
      generic name, never a personal name) into a test
      database.
- [ ] 2.3 Call the helper in every test that writes
      `enrollments` rows directly through SQL:
      `src/handlers.rs` (one site, near line 4760),
      `src/readiness.rs` (two sites, near lines 382 and
      546), `tests/routes.rs` (five sites, near lines
      1251, 1586, 1649, 1706, and 1789). Tests that
      enroll through the toggle handler need no helper:
      the handler ensures the row itself.
- [ ] 2.4 New test: the toggle handler enrolls on a
      fresh database with no users row, and the database
      still holds zero users rows before the toggle runs.
- [ ] 2.5 Flag in the implementation pull request
      body: ADR 0003 left the toggle-handler option open
      as a follow-up, and this phase picks it. The owner
      reviews the choice there.

## Phase 3: Seed Data Module, Seed Step, Comparison

- [ ] 3.1 Write the one-off generator script. It
      lives in `/tmp`, is never committed, and runs once.
      Python 3, standard library only. Input: the dump
      file. Output: a draft `src/seed_data.rs`.
  - Parse only the seed sections: auditions,
    audition links, audition rounds, pieces,
    piece-audition links, segments, link-segment
    rows. Sessions and enrollments are not seed
    data; the script reads them only for the count
    cross-check (215 sessions, 5 enrollments).
  - Emit plain Rust data: one constant per table,
    typed with the structs from `src/db.rs`
    (`Audition`, `Piece`, `Segment`, `Round`,
    `AuditionLink`) and small tuple structs for the
    two link tables. Explicit IDs everywhere.
    Preserve the segment ID gaps (13, 15, 17, 19).
    Empty round_id, archived_at, and target_bpm
    become NULL. Empty excerpt becomes `''`.
    Escape strings as Rust literals; keep UTF-8
    (accented names, guillemets) byte-identical.
  - Encode the rows for pieces 29 and 30 on
    audition 3 exactly as the Phase 0 quoted output
    shows (task 0.5).
  - Verify while generating: `target_bpm` is NULL
    for all 45 pieces; `excerpt` is empty for all
    45 pieces; `created_at` is never emitted.
- [ ] 3.2 Hand-review the generated `seed_data.rs`
      against the dump, row by row (216 rows). The file
      is the reviewable artifact in the pull request.
      Then apply the count gate: the file holds 6
      auditions, 18 links, 13 rounds, 45 pieces, 85
      piece-audition links, 25 segments, and 30
      link-segment rows.
- [ ] 3.3 Create `src/seed_data.rs` (the `SEED`
      constant only) and `src/seed.rs` (the step, the
      comparison, the report). Declare both in
      `src/lib.rs`. No new dependency.
- [ ] 3.4 Implement `apply_seed(db, data) ->
Result<SeedReport>`. Data is a parameter, not a
      global: the compiled `SEED` constant is one
      instance, and tests build small custom instances.
      One transaction, foreign-key order: auditions,
      pieces, audition rounds, audition links,
      piece-audition links, segments, link-segment
      rows. The step runs this statement with the seed
      columns only:

      ```sql
      INSERT ... ON CONFLICT(id) DO UPDATE
      ```

      Never `REPLACE` and
      never delete-then-insert a piece: sessions
      reference pieces by ID with `ON DELETE CASCADE`.
      Never touch `created_at`: an insert leaves it to
      the column default, an update preserves the
      stored value. Never delete a row.

- [ ] 3.5 The report lists every overwritten row ID,
      per table, with inserted, updated, and unchanged
      counts. Log the report at start, in the existing
      `eprintln!("event ...")` style.
- [ ] 3.6 Implement `compare_seed(db, data) ->
SeedDiff`: pure reads, no writes. It reports
      missing rows, extra rows, and differing values,
      per table. The seed step calls it after applying
      and logs the counts (missing, extra, different)
      at startup. No new HTTP endpoint and no new CLI
      subcommand in this cycle.
- [ ] 3.7 Wire the call site: `open_db` and
      `open_memory` call `apply_seed` after `migrate`,
      through one shared path. The seed runs on every
      start, including tests.
- [ ] 3.8 `Audition.city` is written through SQL:
      the `Audition` struct does not expose `city`
      today, and exposing it is out of scope for this
      cycle.

## Phase 4: UI Redesign

Audit first, then remove. Verify every caller with
grep before each removal; a helper with a remaining
caller stays.

- [ ] 4.1 Caller audit. Grep every call site of
      `find_or_create_segment` (`src/handlers.rs`, near
      lines 1878 and 2150), `gc_orphan_segments` (near
      lines 1888 and 2163), `clear_piece_link_segments`
      (near line 2144), `set_archived` (`src/handlers.rs`,
      near lines 2217, 2236, 2470, and 2489),
      `clamp_difficulty` (near line 1731),
      `form_target_bpm` (near line 1777), and the
      `ARCHIVE_*_SQL` / `RESTORE_*_SQL` constants.
      Record which die with the forms below.
- [ ] 4.2 Remove, with routes, handlers, form
      buttons, and templates:
  - `POST /pieces` (`handle_add_piece`,
    `src/handlers.rs`, near line 1828).
  - `GET /pieces/{id}/edit` (`handle_edit_piece`,
    near line 1970) and `POST /pieces/{id}`
    (`handle_update_piece`, near line 2082).
  - `POST /auditions` (`handle_add_audition`, near
    line 2392) and `POST /auditions/{id}`
    (`handle_update_audition`, near line 2427).
  - `GET /auditions/{id}/edit`
    (`handle_audition_edit`, near line 2685).
  - `POST /auditions/{id}/links` and
    `POST /auditions/{id}/links/{linkID}/delete`
    (`handle_add_audition_link`, near line 2716;
    `handle_delete_audition_link`, near line 2748).
  - `POST /auditions/{id}/rounds` and
    `POST /auditions/{id}/rounds/{roundID}/delete`
    (`handle_add_round`, near line 2768;
    `handle_delete_round`, near line 2805).
  - `POST /pieces/{id}/archive`,
    `POST /pieces/{id}/restore`,
    `POST /auditions/{id}/archive`,
    `POST /auditions/{id}/restore`
    (`handle_archive_piece`, near line 2208;
    `handle_restore_piece`, near line 2227;
    `handle_archive_audition`, near line 2461;
    `handle_restore_audition`, near line 2480).
    Archive state is seed data: archiving becomes a
    seed change in a pull request.
  - The route table entries for all of the above
    (`src/handlers.rs`, near lines 4531-4560).
- [ ] 4.3 Remove the helpers when no caller remains:
      `set_archived` (near line 2173), the
      `ARCHIVE_*_SQL` / `RESTORE_*_SQL` constants,
      `find_or_create_segment` (`src/db.rs`, near line
      661), `gc_orphan_segments` (near line 693),
      `clear_piece_link_segments` (near line 650),
      `clamp_difficulty` (near line 1731), and
      `form_target_bpm` (near line 1777). Keep
      `add_segment`, `delete_segment`, and
      `set_link_segments` as the database-level
      operations the tests use.
- [ ] 4.4 Remove the orphaned i18n keys of the
      removed forms. Grep-verify that no key is
      referenced only by a removed template.
- [ ] 4.5 Keep unchanged: the enrollment toggle,
      sessions and the baseline handler
      (`POST /pieces/{id}/baseline` stays), mock setup
      and run, recordings, availability, settings,
      readiness, diary, and all read paths for
      auditions, pieces, segments, links, and rounds.
      Difficulty and target tempo read paths stay:
      they read the seed values from `pieces`
      (`src/handlers.rs` near lines 477, 588, 679-744,
      905, and 944; `src/prepmap.rs`; `src/readiness.rs`,
      near line 301). Difficulty and target tempo
      change only through a seed change in a pull
      request.

## Phase 5: Tests

New tests, all in the existing inline `#[cfg(test)]`
modules or `tests/routes.rs`:

- [ ] 5.1 Fresh database holds the seed:
      `open_memory` yields 6 auditions, 18 links, 13
      rounds, 45 pieces, 85 piece-audition links, 25
      segments, and 30 link-segment rows, with the IDs
      of the dump (spec SC-1).
- [ ] 5.2 Second open changes nothing: a file
      database opened twice yields an identical dump
      before and after the second open (spec SC-2).
      Reuse the `reopening_never_enrolls` pattern
      (temp file, cleanup of the WAL files).
- [ ] 5.3 Overwrite is reported and applied: change
      one seeded row, reopen, assert the seed values
      are restored and the report lists the row
      (spec FR-10).
- [ ] 5.4 Extra row stays and is reported: insert a
      row that the seed does not hold, reopen, assert
      it stays and the comparison reports it
      (spec FR-9, FR-14).
- [ ] 5.5 Sessions and enrollments survive: seed
      sessions and an enrollment on a test database,
      reopen, assert the same IDs, counts, and values
      (spec SC-4).
- [ ] 5.6 Reopening never enrolls: extend the
      existing test to the seeded database (a seeded
      audition starts unenrolled; the enrollments count
      stays 0).
- [ ] 5.7 Seed addition through a custom instance:
      build a small `SeedData` with one extra audition,
      apply it twice to a test database, assert the
      audition appears after the first apply and the
      second apply changes nothing (spec SC-6, on a
      test database, never on production data).
- [ ] 5.8 Toggle on a fresh database: the toggle
      handler enrolls on a database with no users row
      (Phase 2, task 2.4).
- [ ] 5.9 `created_at` survives an overwrite: change
      a seeded row, reopen, assert the stored
      `created_at` value is unchanged (plan R6).

Rework of the existing tests. The seed runs in
`open_memory`, so no test starts from an empty
database any more. Rules for the rework:

- Rows a test creates take IDs above the seed maxima
  (auditions above 6, pieces above 45, segments above
  29, rounds above 13, links above 18) or
  auto-assigned IDs captured from `last_insert_rowid`.
  Never hardcode a seeded ID.
- Count assertions scope to the rows the test creates,
  or assert the seeded counts where the test covers
  the repertoire.
- Tests that write `enrollments` directly use the
  Phase 2 helper (task 2.3).

Per-file rework list (104 inline tests, 13
integration tests):

- [ ] 5.10 `src/db.rs` (7 tests): the link tests
      hardcode piece 1 and audition 1, which the seed
      now holds; use auto-assigned IDs or IDs above the
      seed maxima. Update the migrate test per task 1.3.
- [ ] 5.11 `src/handlers.rs` (28 tests): the
      enrollment fixture uses the Phase 2 helper (near
      line 4760). Tests that post to removed routes are
      deleted or rewritten against seed data. The
      `clamp_difficulty` / `form_target_bpm` unit tests
      (near lines 4852-4867) go away with the helpers.
- [ ] 5.12 `src/readiness.rs` (9 tests): explicit
      IDs 1-3 become IDs above the seed maxima or
      captured IDs. The enrollment inserts (near lines
      382 and 546) use the Phase 2 helper.
- [ ] 5.13 `src/stats.rs` (17 tests): explicit IDs
      become IDs above the seed maxima or captured IDs.
- [ ] 5.14 `src/mocks.rs` (1 test): the fixture
      inserts an audition and pieces with auto IDs
      (near lines 201-221); the `piece_audition`
      inserts for (1, 1) and (2, 1) now collide with
      seeded rows, so use the seeded rows or fresh IDs
      above the maxima.
- [ ] 5.15 `src/segments.rs` (6 tests), `src/score.rs`
      (5 tests), `src/prepmap.rs` (6 tests): audit each
      test. They build fixtures through the app; fix
      any count assertion or ID assumption the seed
      breaks. `prepmap` reads difficulty from seeded
      pieces; its expectations stay valid.
- [ ] 5.16 `src/i18n.rs` (12 tests),
      `src/recordings.rs` (4 tests), `src/render.rs`
      (9 tests): audit each test. They write no seeded
      table; fix any empty-list assumption the seed
      breaks.
- [ ] 5.17 `tests/routes.rs` (13 tests): page tests
      that assume empty lists (`empty_app_pages_render`,
      `auditions_list_and_diary_markers`) now render
      seeded rows; assert the seeded content.
      `audition_links_and_rounds_crud` and
      `piece_edit_archive_and_filters` post to removed
      routes; delete them or rewrite them against seed
      data. The five enrollment sites (task 2.3) use
      the Phase 2 helper. `mock_flow` and
      `mock_with_round_and_rerate_guard` use seeded
      audition IDs or fresh IDs above the maxima.
- [ ] 5.18 `reopening_never_enrolls`: the inserted
      audition takes an auto-assigned ID; assert that
      audition is unenrolled and the enrollments count
      is 0, not that the auditions count is 1.
- [ ] 5.19 Verify: `cargo test` passes in full.
      `cargo llvm-cov --locked --fail-under-lines 90`
      passes. The seed step, the comparison, and the
      toggle ensure have direct test coverage.

## Phase 6: Verification and Deploy

- [ ] 6.1 Push the spec branch
      `feat/seed-auditions-spec`, open its pull request,
      and merge it. The spec pull request holds the
      single amended commit.
- [ ] 6.2 Implementation branch `feat/seed-auditions`
      from the merged master. One commit per phase; one
      commit stays preferable when a phase is small. A
      fix amends its commit.
- [ ] 6.3 Preflight per the `preflight_code_changes`
      skill before the implementation pull request
      opens. The pull request is called ready only on
      green CI verified on the exact head.
- [ ] 6.4 EXTRA CARE deploy: backup the live
      database before the deploy (the established
      container backup pattern on the host). Deploy.
- [ ] 6.5 Live verification from the host: the
      audition list shows 6 auditions, with the seeded
      one unenrolled; the sessions count is 215; the
      enrollments count is 5. Read the startup log: the
      seed report and the comparison counts show the
      convergence.
- [ ] 6.6 Post-deploy: read the comparison output.
      If a live difficulty or target tempo differed from
      the dump, open a seed correction pull request if
      the live value was the right one. No copy ran
      before the deploy, by design (owner correction
      2026-11-24).
- [ ] 6.7 Rollback is a redeploy of the previous
      image, plus a restore from the pre-deploy backup
      only if a seed write went wrong.

## Acceptance checklist (spec success criteria)

- [ ] SC-1: a fresh database holds exactly the seeded
      rows (task 5.1).
- [ ] SC-2: a second open changes no row (task 5.2).
- [ ] SC-3: the read-only comparison reports no
      missing row and no different value, except
      `created_at` and the two note rows re-read in
      quoted mode (tasks 0.5, 3.6, 6.5).
- [ ] SC-4: the 215 sessions and the 5 enrollments
      rows survive unchanged (tasks 5.5, 6.5).
- [ ] SC-5: the coverage gate of 90 percent holds, and
      CI is green (tasks 5.19, 6.3).
- [ ] SC-6: a test addition through a seed entry
      appears after a restart with no other step,
      verified on a test database, never on production
      data (task 5.7).
