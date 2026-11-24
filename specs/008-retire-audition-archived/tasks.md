# Task List: Retire the Archived Flag

**Spec**: `specs/008-retire-audition-archived/spec.md` (Draft, pending ratification)

**Plan**: `specs/008-retire-audition-archived/plan.md` (5 phases)

**Branch**: `feat/retire-audition-archived-spec` for this file; the
implementation runs on `feat/retire-audition-archived` from the
merged master (plan Phase E).

**Evidence**: the seed data (`src/seed_data.rs`, six auditions,
IDs 1-6, 45 pieces, 85 piece-audition links) and the live
auditions page, read on 2026-11-24: three archived
auditions (Roma, Cagliari Concertino, Cagliari Fila),
all unenrolled and all past; seven archived pieces
(Beethoven 5 III mov. ID 5, Mahler 2 ID 21, Haydn Do
maggiore ID 27, Schumann ID 29, Elgar ID 30, Bach
Suite whole ID 32, Hindemith sonata whole ID 35),
with eight links.

## Conventions for this task list

- One commit per plan phase on
  `feat/retire-audition-archived`. One commit stays
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

- Past is derived in the read layer (plan Phase A):
  `list_auditions`, `piece_auditions`, and
  `audition_or_404` take `today` and set
  `Audition.past`. The rejected alternative computes
  past in each handler; it repeats the comparison in
  every handler that builds an `Audition` record.
- The manual `DROP COLUMN` on the live database is
  not decided here. It stays an owner decision after
  the deploy (plan R1, open item).
- The three dropped pieces are verified by listing
  them before the edit (plan R5): IDs 5, 32, and
  35, with zero sessions, verified live on their
  per-piece diary pages on 2026-11-24. The links
  stay: plan membership is decided only by
  audition enrollment, never per piece. Mahler 2
  (ID 21) keeps its 5 sessions because its row
  stays.

## Phase A: Reads and Schema

- [ ] A.1 Drop the `archived_at TEXT` line from the
      `auditions` CREATE TABLE and from the `pieces`
      CREATE TABLE in `migrate` (`src/db.rs`, near
      lines 284 and 294).
- [ ] A.2 `Audition` struct (`src/db.rs`, near line
      58): drop the `archived` field, add `past`
      (serde rename `Past`) with a doc line: true
      when the audition date is before today.
- [ ] A.3 `Piece` struct (`src/db.rs`, near line
      118): drop the `archived` field.
- [ ] A.4 `list_auditions` (`src/db.rs`, near line
      462): take `today: &str`, drop the
      `include_archived` parameter, set `past` from
      `date < today` in the scan, and order by
      upcoming first, then past, each by date.
      Update the five callers (`src/handlers.rs`,
      near lines 788, 1734, 1821, 2181, 2853) and
      the test (`src/db.rs`, near line 994).
- [ ] A.5 `piece_auditions` (`src/db.rs`, near line
      482): take `today: &str`, drop the
      `archived_at IS NOT NULL` column from the
      select, set `past` in the scan. Audit the
      callers with grep and update them.
- [ ] A.6 `list_pieces` (`src/db.rs`, near line
      676): drop the `include_archived` parameter
      and the `archived_at IS NULL` condition. Drop
      the archived column from the select and update
      the row scan. Update the callers: the pieces
      page handler (`src/handlers.rs`, near line 1727) loses the `show_archived` argument;
      every other caller passes `false` today and
      drops the argument.
- [ ] A.7 `has_active` (`src/db.rs`, near line 724):
      the condition becomes `c.date >= today`.
      Update the doc line on the field (near line
      121).
- [ ] A.8 `upcoming_auditions` (`src/handlers.rs`,
      near line 415): drop the `c.archived ||`
      check. The date and enrollment checks stay.
- [ ] A.9 `audition_or_404` (`src/handlers.rs`, near
      lines 236-246): take `today: &str`, drop the
      archived column from the select, set `past`
      in the scan. Audit the callers with grep and
      update them. The toggle handler gains no past
      check.
- [ ] A.10 Pieces page (`src/handlers.rs`, near
      lines 1708-1749): drop the `ShowArchived`
      field, the `archiviati` query parameter, and
      the argument.
- [ ] A.11 `src/mocks.rs` (near lines 162 and 170)
      and `src/stats.rs` (near lines 622 and 624):
      drop the piece archived condition from the
      mock queries and the audition archived
      condition from the coverage query. The date
      conditions stay.
- [ ] A.12 Verify: `cargo test db::` passes. Assert
      in the migrate test that a fresh database has
      no `archived_at` column on `auditions` or
      `pieces` (`PRAGMA table_info`).

## Phase B: Seed, Handlers, and Stats

- [ ] B.1 `SeedAudition` (`src/seed.rs`, near lines
      20-26) and `SeedPiece` (near lines 32-44):
      drop the `archived_at` field and its doc
      lines.
- [ ] B.2 `apply_seed` (`src/seed.rs`): drop
      `archived_at` from the audition select,
      insert, and upsert (near lines 289-296) and
      from the piece select, insert, and upsert
      (near lines 349-358). Update the row mappings
      (near lines 312-332 and 383-410).
- [ ] B.3 `compare_auditions` (`src/seed.rs`, near
      line 673) and `compare_pieces` (near line
      712): drop `archived_at` from the selects and
      from the compared tuples.
- [ ] B.4 List the three duplicate pieces before
      the edit, and verify them against the list
      in the decisions above. Then drop them from
      `src/seed_data.rs`: the three duplicate
      `SeedPiece` rows (IDs 5, 32, 35) and their
      three `SeedPieceAudition` rows, and the
      `archived_at` line from all six
      `SeedAudition` rows (near lines 26-71) and
      all 42 kept `SeedPiece` rows. Verify the
      counts are 42 pieces and 82 links with grep
      before the implementation pull request opens.
- [ ] B.5 `handle_auditions` (`src/handlers.rs`,
      near line 1849): the row carries `past`, true
      when the audition date is before today. Drop
      the `archived` and `concluded` row fields, and
      the fields from the row struct.
- [ ] B.6 `handle_audition_detail`
      (`src/handlers.rs`, near line 2036): replace
      the `concluded` line with a `past` binding.
      The `AuditionDetailData` field renames to
      `past`.
- [ ] B.7 `AuditionChip` (`src/handlers.rs`, near
      lines 2069-2075): the `archived` field renames
      to `past`. The diary chip construction (near
      line 2202) sets it from `c.past`.
- [ ] B.8 Verify: `cargo test` passes in full. Grep
      `src/` for `archived` and confirm zero hits.

## Phase C: Templates, Style, and Locales

- [ ] C.1 `templates/auditions.html`: the card class
      becomes `past` from `r.Past`; the badges
      become the past badge from `r.Past` and the
      enrolled badge from `r.Enrolled` when the
      audition is not past.
- [ ] C.2 `templates/audition_detail.html`: the
      title badges become the past badge from
      `Past` and the enrolled badge from
      `Audition.Enrolled` when the audition is not
      past; the enrollment toggle shows only when
      the audition is not past.
- [ ] C.3 `templates/diary.html`: the chip class
      reads `chip.Past`.
- [ ] C.4 `templates/piece_detail.html`: the
      archived badge goes. The audition chips read
      `c.Past`.
- [ ] C.5 `templates/pieces.html`: the card class
      loses the archived branch; the archived badge
      goes; the audition chips read `c.Past`; the
      "mostra archiviati" checkbox goes with its
      label.
- [ ] C.6 `static/style.css` (near line 420):
      delete the `.card.archived` rule. The muted
      style for past auditions lives in `.card.past`.
- [ ] C.7 Locales, all six files: rename
      `auditions.concluded_badge` to
      `auditions.past_badge` with the strings of
      plan Phase C. Rewrite `auditions.intro_b`,
      `settings.base_desc`, and
      `today.empty_plan_pre` per plan Phase C.
      Delete `pieces.archived_badge` and
      `pieces.show_archived` from all six files.
      Verify with grep that no locale string
      mentions an archived audition or piece.
- [ ] C.8 Verify: `cargo test` passes; prettier
      checks the locale files and the CSS.

## Phase D: Tests

- [ ] D.1 Update the `upcoming_filters_and_nearest`
      fixture (`src/handlers.rs`, near line 4089):
      the archived audition becomes a past audition
      by date. The enrolled-false case stays. Audit
      every test fixture with grep for a `Piece` or
      `Audition` literal with `archived: true` and
      drop the field.
- [ ] D.2 Update the `stats.rs` test seed insert
      (near line 1003): drop the `archived_at`
      column from the `INSERT INTO auditions`
      statement. Audit every test SQL insert into
      `auditions` or `pieces` with grep for
      `archived_at` and drop the column.
- [ ] D.3 Update the `db.rs` link tests (near lines
      1110 and 1118): the `list_pieces` calls drop
      the `include_archived` argument.
- [ ] D.4 Extend the integration test at
      `tests/routes.rs`, near line 749: assert the
      list page shows the "Past" badge for the past
      audition and no "archived" string anywhere on
      the page, and the past detail page shows the
      badge and no enrollment toggle.
- [ ] D.5 New integration test: a fresh database
      holds the seed with no `archived_at` column on
      `auditions` or `pieces`, 42 pieces, and 82
      piece-audition links (spec SC-1, on a test
      database, never on production data).
- [ ] D.6 New integration test: the toggle
      round-trips on an upcoming audition and the
      list order puts past auditions last (spec
      SC-2, SC-3).
- [ ] D.7 New integration test: the pieces list
      shows the four former archived pieces with
      their audition chips and without the
      archived filter, and the plan holds Haydn Do
      maggiore, Schumann, and Elgar through San
      Carlo, but not Mahler 2 (spec SC-6).
- [ ] D.8 Verify: the full test suite passes, and
      the coverage gate of the conventions holds.

## Phase E: Verification and Deploy

- [ ] E.1 Push the spec branch
      `feat/retire-audition-archived-spec`, open its
      pull request, and merge it. The spec pull
      request holds the single amended commit.
- [ ] E.2 Implementation branch
      `feat/retire-audition-archived` from the merged
      master. One commit per phase; one commit stays
      preferable when a phase is small. A fix amends
      its commit.
- [ ] E.3 Preflight per the `preflight_code_changes`
      skill before the implementation pull request
      opens. The pull request is called ready only on
      green CI verified on the exact head.
- [ ] E.4 EXTRA CARE deploy and data step (spec
      FR-9, plan Phase E): backup the live
      database before anything else (the
      established container backup pattern on the
      host). Stop the container. The owner runs
      the DELETE on the live database on lobster,
      then the verification queries, expecting 42,
      82, and the sessions count unchanged. Merge
      the implementation pull request; the CI
      deploy pulls the new image and restarts the
      container.
- [ ] E.5 Live verification from the host: the
      auditions list shows three "Past" badges and
      zero "archived" strings; the detail pages of
      Roma and Cagliari show the badge and no toggle;
      the pieces list shows all 42 pieces; the
      sessions count is unchanged; the two
      Enrolled badges stay on Santa Cecilia and San
      Carlo; the daily plan holds Haydn Do
      maggiore, Schumann, and Elgar through San
      Carlo, but not Mahler 2.
- [ ] E.6 Rollback is a redeploy of the previous
      image, plus a restore from the pre-deploy
      backup only if a deploy write went wrong.
      The manual DELETE is covered by the same
      backup: the three deleted pieces had no
      sessions, and the four kept pieces keep
      their rows and their sessions.

## Acceptance checklist (spec success criteria)

- [ ] SC-1: a fresh database holds the seed with no
      `archived_at` column on `auditions` or
      `pieces`, and the startup comparison reports no
      audition or piece difference (tasks A.12, D.5).
- [ ] SC-2: after the deploy, the live list shows
      three "Past" badges (Roma, Cagliari
      Concertino, Cagliari Fila), zero "archiviato"
      badges, and two Enrolled badges (Santa
      Cecilia, San Carlo) (tasks D.4, D.6, E.5).
- [ ] SC-3: a past audition detail page shows the
      "Passato" badge and no enrollment toggle. An
      upcoming audition detail page keeps the
      toggle, and the toggle round-trips (tasks D.4,
      D.6, E.5).
- [ ] SC-4: the diary loses no data: the sessions
      count is unchanged after the deploy, verified
      live (task E.5).
- [ ] SC-5: the coverage gate of 90 percent holds,
      and CI is green (tasks D.8, E.3).
- [ ] SC-6: after the deploy, the live pieces list
      shows all 42 pieces, the four former archived
      ones with their audition chips. The daily plan
      gains Haydn Do maggiore, Schumann, and Elgar
      through San Carlo, and the manual log list
      gains them too. Mahler 2 stays out of both.
      The sessions count is unchanged: the deleted
      pieces had no sessions (tasks D.7, E.4, E.5).
