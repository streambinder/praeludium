# Implementation Plan: Retire the Archived Flag

**Spec**: `specs/008-retire-audition-archived/spec.md` (Draft, pending ratification)

**Feature Branch**: `feat/retire-audition-archived-spec`

## Technical Context

- Stack: Rust, axum, rusqlite (bundled SQLite), minijinja
  templates, single connection behind a mutex.
- The audition reads live in `src/db.rs` (`App` methods)
  and `src/handlers.rs` (`audition_or_404`). The piece
  reads live in `src/db.rs` (`list_pieces`). The seed
  lives in `src/seed.rs` (step and comparison) and
  `src/seed_data.rs` (the `SEED` constant).
- Coverage gate: `cargo llvm-cov --locked
--fail-under-lines 90` (`.github/workflows/push.yml`).
- The live columns stay: code-side migrations never
  alter a live table (spec D-5). A fresh database
  opens without the columns; the live database keeps
  them, unread and unwritten.
- The archive routes and handlers are already gone
  (cycle 007), and `set_archived` is gone too. Only
  the columns, the seed fields, and the display
  remain. The removed-routes table keeps its entries
  for the old archive and restore paths.
- The three duplicate pieces leave the seed and
  the live database as data cleanup (spec D-10).
  The other four archived pieces keep their rows
  and their links: plan membership is decided
  only by audition enrollment, never per piece.
  The link count goes from 85 to 82: the delete
  cascades to the three link rows of the deleted
  pieces.

## Phase A: Reads and Schema

- `src/db.rs`, `migrate`: drop the `archived_at TEXT`
  line from the `auditions` CREATE TABLE (near line 284) and from the `pieces` CREATE TABLE (near line
  294).
- `src/db.rs`, `Audition` struct: drop the `archived`
  field (near line 58) and add `past`, serde rename
  `Past`, with a doc line: true when the audition
  date is before today.
- `src/db.rs`, `Piece` struct: drop the `archived`
  field (near line 118).
- `src/db.rs`, `list_auditions`: take `today: &str`,
  drop the `include_archived` parameter, and set
  `past` from `date < today` in the scan. Order by
  upcoming first, then past, each by date: `ORDER BY
audition_date < ?, audition_date` with `today`
  bound. Update the five callers
  (`src/handlers.rs`, near lines 788, 1734, 1821,
  2181, 2853) and the test (`src/db.rs`, near line
  994).
- `src/db.rs`, `piece_auditions`: take `today: &str`,
  drop the `archived_at IS NOT NULL` column from
  the select (near line 482), and set `past` in the
  scan. Audit the callers with grep and update
  them.
- `src/db.rs`, `list_pieces` (near line 676): drop
  the `include_archived` parameter and the
  `archived_at IS NULL` condition. Drop the archived
  column from the select and update the row scan.
  Update the callers: the pieces page handler
  (`src/handlers.rs`, near line 1727) loses the
  `show_archived` argument; every other caller
  passes `false` today and drops the argument.
- `src/db.rs`, `has_active` (near line 724): the
  condition becomes `c.date >= today`. Update the
  doc line on the field (near line 121).
- `src/handlers.rs`, `upcoming_auditions` (near line
  415): drop the `c.archived ||` check. The date
  and enrollment checks stay.
- `src/handlers.rs`, `audition_or_404` (near lines
  236-246): take `today: &str`, drop the archived
  column from the select, and set `past` in the
  scan. Audit the callers with grep and update
  them. The toggle handler is one caller; it gains
  no past check (spec D-8).
- `src/handlers.rs`, pieces page (near lines
  1708-1749): drop the `ShowArchived` field, the
  `archiviati` query parameter, and the argument.
- `src/mocks.rs` (near lines 162 and 170) and
  `src/stats.rs` (near lines 622 and 624): drop the
  `p.archived_at IS NULL` condition from the mock
  queries and the `au.archived_at IS NULL AND`
  condition from the coverage query. The date
  conditions stay.
- Verification: `cargo test db::` passes. A fresh
  database has no `archived_at` column on
  `auditions` or `pieces`: assert it in the migrate
  test with `PRAGMA table_info`.

## Phase B: Seed, Handlers, and Stats

- `src/seed.rs`, `SeedAudition` (near lines 20-26)
  and `SeedPiece` (near lines 32-44): drop the
  `archived_at` field and its doc lines.
- `src/seed.rs`, `apply_seed`: drop `archived_at`
  from the audition select, insert, and upsert
  (near lines 289-296) and from the piece select,
  insert, and upsert (near lines 349-358).
  Update the row mappings (near lines 312-332 and
  383-410).
- `src/seed.rs`, `compare_auditions` (near line 673) and `compare_pieces` (near line 712): drop
  `archived_at` from the selects and from the
  compared tuples.
- `src/seed_data.rs`: drop the `archived_at` line
  from all six `SeedAudition` rows (near lines
  26-71) and all 42 kept `SeedPiece` rows. Drop
  the three duplicate `SeedPiece` rows outright
  (IDs 5, 32, 35) and their three
  `SeedPieceAudition` rows (piece 5 to audition
  3, piece 32 to audition 2, piece 35 to
  audition 1). The other 82 links stay. The
  piece count goes from 45 to 42, the link count
  from 85 to 82. Verify both counts with grep
  before the implementation pull request opens.
- `src/handlers.rs`, `handle_auditions` (near line
  1849): the row carries `past: c.date < today`.
  Drop the `archived` and `concluded` row fields.
  The row struct loses both fields.
- `src/handlers.rs`, `handle_audition_detail`
  (near line 2036): `let past = c.date < today;`
  replaces the `concluded` line. The
  `AuditionDetailData` field renames to `past`.
- `src/handlers.rs`, `AuditionChip` (near lines
  2069-2075): the `archived` field renames to
  `past`. The diary chip construction (near line 2202) sets it from `c.past`.
- Verification: `cargo test` passes in full. Grep
  `src/` for `archived` and confirm zero hits.

## Phase C: Templates, Style, and Locales

- `templates/auditions.html`: the card class
  becomes `past` from `r.Past`; the badges become
  the past badge from `r.Past` and the enrolled
  badge from `r.Enrolled and not r.Past`.
- `templates/audition_detail.html`: the title
  badges become the past badge from `Past` and
  the enrolled badge from `Audition.Enrolled and
not Past`; the enrollment toggle shows only
  when `not Past`.
- `templates/diary.html`: the chip class reads
  `chip.Past`.
- `templates/piece_detail.html`: the archived badge
  goes. The audition chips read `c.Past`.
- `templates/pieces.html`: the card class loses
  the archived branch; the archived badge goes;
  the audition chips read `c.Past`; the
  "mostra archiviati" checkbox goes with its
  label.
- `static/style.css` (near line 420): delete the
  `.card.archived` rule. The muted style for past
  auditions lives in `.card.past`.
- Locales, all six files: rename
  `auditions.concluded_badge` to
  `auditions.past_badge` with it "passato", en
  "past", de "vergangen", es "pasado", fr "passé",
  pt "passado". Rewrite `auditions.intro_b` to
  drop the archived sentence: it "restano solo
  per lo storico. Tocca un concorso per aprirne
  il dettaglio.", en "they remain for history
  only. Tap an audition to open its detail.", de
  "sie bleiben nur für die Historie. Tippe auf ein
  Probespiel, um sein Detail zu öffnen.", es
  "quedan solo para el historial. Toca una
  audición para abrir su detalle.", fr "elles
  restent seulement pour l'historique. Touche une
  audition pour ouvrir son détail.", pt "ficam só
  para o histórico. Toca numa audição para abrir
  o seu detalhe.". Rewrite `settings.base_desc`
  to drop "or archived" in all six files.
  Rewrite `today.empty_plan_pre` to drop both
  the concluded auditions and the archived pieces:
  it "Nessun pezzo nel piano di oggi: concorsi
  passati, oppure il tempo di studio di oggi è
  già esaurito. Vedi ", en "No piece in today's
  plan: auditions past, or today's study time is
  already used up. See ", de "Kein Stück im
  heutigen Plan: Probespiele vergangen, oder die
  heutige Übungszeit ist bereits aufgebraucht.
  Siehe ", es "Ninguna pieza en el plan de hoy:
  audiciones pasadas, o el tiempo de estudio de
  hoy ya está agotado. Ver ", fr "Aucun morceau
  dans le plan d'aujourd'hui : auditions passées,
  ou temps d'étude du jour déjà épuisé. Voir ",
  pt "Nenhuma peça no plano de hoje: audições
  passadas, ou o tempo de estudo de hoje já está
  esgotado. Ver ". Delete `pieces.archived_badge`
  and `pieces.show_archived` from all six files.
  Verify with grep that no locale string mentions
  an archived audition or piece.
- Verification: `cargo test` passes; prettier
  checks the locale files and the CSS.

## Phase D: Tests

- Update the `upcoming_filters_and_nearest`
  fixture (`src/handlers.rs`, near line 4089):
  the archived audition becomes a past audition
  by date. The enrolled-false case stays. Audit
  every test fixture with grep for a `Piece` or
  `Audition` literal with `archived: true` and
  drop the field.
- Update the `stats.rs` test seed insert (near
  line 1003): drop the `archived_at` column from
  the `INSERT INTO auditions` statement. Audit
  every test SQL insert into `auditions` or
  `pieces` with grep for `archived_at` and drop
  the column.
- Update the `db.rs` link tests (near lines 1110
  and 1118): the `list_pieces` calls drop the
  `include_archived` argument.
- Extend the integration test at
  `tests/routes.rs`, near line 749 ("One concluded
  audition (past, not archived) and one
  upcoming"): assert the list page shows the
  "Past" badge for the past audition and no
  "archived" string anywhere on the page, and
  the past detail page shows the badge and no
  enrollment toggle.
- New integration test: a fresh database holds
  the seed with no `archived_at` column on
  `auditions` or `pieces`, 42 pieces, and 82
  piece-audition links (spec SC-1, on a test
  database, never on production data).
- New integration test: the toggle round-trips
  on an upcoming audition and the list order
  puts past auditions last (spec SC-2, SC-3).
- New integration test: the pieces list shows the
  four former archived pieces with their
  audition chips and without the archived filter,
  and the plan holds Haydn Do maggiore,
  Schumann, and Elgar through San Carlo, but not
  Mahler 2 (spec SC-6).
- Verification: `cargo test --all-targets`
  passes. `cargo llvm-cov --locked
--fail-under-lines 90` passes.

## Phase E: Verification and Deploy

- Branch strategy: push the spec branch
  `feat/retire-audition-archived-spec` first,
  open its pull request, and merge it. The
  implementation runs on a new branch
  `feat/retire-audition-archived` from the merged
  master. One commit per phase; one commit stays
  preferable when a phase is small. A fix amends
  its commit. The implementation pull request
  follows the repository rules: signed commits
  with the repository author, Conventional
  Commits, rebase merge, no real verification
  subject named in the commit or the pull request
  body.
- Preflight before the pull request opens, per the
  `preflight_code_changes` skill: remote branch
  state, local `cargo test`, `cargo llvm-cov`,
  `cargo fmt`, `cargo clippy`, and the CI jobs
  from `push.yml` reproduced locally where the
  skill prescribes them. The pull request is
  called ready only on green CI verified on the
  exact head.
- EXTRA CARE deploy (standing rule) and data step
  (spec FR-9): the seed never deletes a row, so
  the live removal is manual. The order is:
  backup the live database (the established
  container backup pattern on the host, which the
  agent can run); stop the container; run the
  DELETE on the live database on the host;
  verify the counts; merge; the CI deploy pulls
  the new image and restarts. The exact commands
  for the owner, on lobster, database
  `/var/lib/praeludium/praeludium.db`:

  ```sh
  sqlite3 /var/lib/praeludium/praeludium.db "DELETE FROM pieces WHERE id IN (5, 32, 35);"
  sqlite3 /var/lib/praeludium/praeludium.db "SELECT COUNT(*) FROM pieces; SELECT COUNT(*) FROM piece_audition; SELECT COUNT(*) FROM sessions;"
  ```

  The verification expects 42, 82, and the
  sessions count unchanged. The delete cascades
  to the three link rows of the deleted pieces.
  The container stop runs before the DELETE so
  no app write can race it; the deploy restarts
  the container.
  Verify live from the host after the deploy:
  the auditions list shows three "Past" badges
  and zero "archived" strings; the detail pages
  of Roma and Cagliari show the badge and no
  toggle; the pieces list shows all 42 pieces;
  the sessions count is unchanged; the two
  Enrolled badges stay on Santa Cecilia and San
  Carlo; the daily plan holds Haydn Do maggiore,
  Schumann, and Elgar through San Carlo, but not
  Mahler 2. No other data step runs.

- Rollback is a redeploy of the previous image,
  plus a restore from the pre-deploy backup only
  if a deploy write went wrong. No seed write
  and no data step run in this cycle, so the
  backup is precaution only.

## Risks and Open Questions (for the tasks step)

- R1: the live columns keep stale timestamps,
  unread. A later reader of the raw database
  could mistake them for state. The tasks step
  records the optional manual `DROP COLUMN` as
  a follow-up, outside this cycle.
- R2: the settings list and the pieces filter
  gain past auditions as options (spec Q-3).
  The tasks step asserts the new options render
  and the plan formula still ignores past dates.
- R3: the read-layer signature change touches
  five `list_auditions` callers, the
  `piece_auditions` callers, the `audition_or_404`
  callers, and a dozen `list_pieces` callers. The
  tasks step audits each caller with grep before
  editing; a missed caller is a compile error,
  not a silent change.
- R4: the locale strings for `intro_b` and
  `empty_plan_pre` in de, es, fr, and pt are
  drafted in this plan, not by a native speaker.
  The tasks step verifies them against the
  existing locale style.
- R5: the three dropped pieces and their three
  dropped links are the only record that the
  duplicates were ever there. After this cycle
  that history lives in Git history and in the
  pre-deploy backup only. The tasks step verifies
  the three zero-session pieces by listing them
  before the edit, and the data step verifies the
  counts after the DELETE.
- Open: whether the manual `DROP COLUMN` on the
  live database is worth doing at all. The tasks
  step does not decide it; the owner decides
  after the deploy.
