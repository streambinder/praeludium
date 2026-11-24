# Feature Specification: Repository Seed for Auditions, Pieces, and Segments

**Feature Branch**: `feat/seed-auditions-spec`

**Created**: 2026-11-24

**Status**: Ratified (2026-11-24, by the owner)

**Input**: User description: "registriamo tutti i
concorsi con dei seed lato repository, così puoi gestirli
tu. Scope: tutti i concorsi e le audizioni, più
tutti i pezzi e i segmenti. Poi sarà la studentessa
a iscriversi a quelli che vuole."

## Clarifications

### Session 2026-11-24

- The seed data holds every audition, every piece,
  and every segment of the live database, with
  their links and rounds. The seed data lives in
  the repository. A change to the repertoire is a
  change to the seed data, reviewed in a pull
  request.
- The seed step applies the seed data to the
  database. The seed step is separate from
  migrate. Migrate creates only the canonical
  schema.
- The seed step never writes a users row or an
  enrollments row. Users are not seed data.
  Migrate writes no data at all: the users row
  is removed from migrate, and the live
  database already holds it. Enrollment stays
  an act of the user in the UI. A seeded
  audition starts unenrolled.
- The seed data is canonical. The seed data is
  the source of truth for auditions, pieces,
  and segments, with their links and rounds.
  On application the repository wins: a seeded row
  that differs in the database takes the seed
  values. User customizations never touch a
  canonical table. A customization lives in a
  user-linked table or relationship instead.
- The seed step uses explicit IDs. The IDs match
  the live database exactly.
- Sessions, availability, settings, recordings,
  mocks, mock items, users, and enrollments are
  not seed data. The seed step never writes
  them. The seed covers only auditions, pieces,
  and segments, with their links and rounds.
- A fresh database opened by the application holds
  the full seed data. The diary works from the
  seed data alone.

## Hypothesis Challenge

The change rests on one hypothesis: the repertoire
data needs a source of truth in the repository,
because the live database is the only copy today
and the manual path to change it is slow and
error-prone.

Evidence, from the live database dump of
2026-11-24 and from the work of 2026-11-24.
The evidence file is the dump saved with the
concorsi goal as db-dump-2026-11-24.txt.

- The live database holds 6 auditions, 18 links,
  13 rounds, 45 pieces, 85 piece-audition links,
  25 segments, and 30 link-segment rows. The
  repository holds none of that data. Migrate
  holds only the schema.
- The agent has no read or write access to the
  live database file. The cloud browser cannot
  reach the application. The phone browser shows
  no sign-in prompt for it. The only write path
  is manual SQL run by the owner on the host.
- The manual path failed twice before it worked:
  the La Fenice insert of 2026-11-24 needed three
  attempts: a backup without sudo, then a
  placeholder ID pasted literally. Each future
  audition repeats that risk.
- The assumed cause holds: the data has no other
  home. The owner must type SQL for every
  addition, and the agent cannot prepare, review,
  or verify the change as a pull request.
- Alternatives examined and rejected: the UI
  alone (the agent cannot reach it, and it leaves
  no reviewable record); hand-applied SQL files
  (the files are reviewable, but each deploy still
  needs a manual run, with no idempotency and no
  verification); a seed that runs only on an empty
  database (later additions would never reach the
  live database, which defeats the purpose).
- The explicit seed command stays as the recorded
  alternative: it avoids surprise writes, but a
  deploy that skips the command ships stale data.
  ADR 0002 records the choice.

## User Scenarios and Testing

### User Story 1: A new audition arrives from a repository change (Priority: P1)

The agent opens a pull request that adds the seed
entries for a new audition, with its links and
rounds. The owner merges it. After the deploy,
the audition is listed, unenrolled, with its
links and rounds. No manual SQL runs on the live
database.

### User Story 2: Fresh database, full repertoire (Priority: P1)

The application opens a new, empty database. The
database then holds 6 auditions, 18 links, 13
rounds, 45 pieces, 85 piece-audition links, 25
segments, and 30 link-segment rows. The diary
lists the auditions and the pieces with no other
step.

### User Story 3: A restart converges the repertoire (Priority: P1)

The application opens the live database again. The
seeded rows match the seed data. A second open
changes no row. The 215 sessions and the 5
enrollments rows keep their IDs, counts, and
values.

### User Story 4: Enrollment stays a user act (Priority: P1)

The user enrolls in La Fenice from its detail
page. The next deploy keeps the enrollment. The
seed step writes no enrollments row at any start.

### User Story 5: A future piece arrives as seed entries (Priority: P2)

The agent adds the seed entries for a new excerpt
of La Fenice, with its round and segments. After
the deploy, the piece is linked to La Fenice.
The existing pieces keep their IDs, and their
sessions stay attached to them.

### User Story 6: Seed reviewable in a pull request (Priority: P2)

A reviewer reads the pull request that adds seed
entries. The diff shows exactly what the live
database will gain. The reviewer needs no access
to the live database to review the change.

## Requirements

- **FR-1**: The seed data holds all 6 auditions
  of the dump of 2026-11-24, with their IDs 1-6,
  names, cities, dates, weights, and archived
  states: Santa Cecilia (1, active), Opera di
  Roma (2, archived), San Carlo (3, active),
  Cagliari Concertino (4, archived), Cagliari
  Fila (5, archived), La Fenice (6, active).
- **FR-2**: The seed data holds all 18 audition
  links and all 13 audition rounds of the dump,
  with their IDs.
- **FR-3**: The seed data holds all 45 pieces of
  the dump, with their IDs 1-45, composers,
  works, movements, excerpts, kinds,
  difficulties, target tempos, and archived
  states. The 7 archived pieces (5, 21, 27, 29,
  30, 32, 35) stay archived in the seed data.
- **FR-4**: The seed data holds all 85
  piece-audition links of the dump, with their
  excerpt texts, notes, round IDs, and optional
  flags. La Fenice (6) has no link yet.
- **FR-5**: The seed data holds all 25 segments
  of the dump, with their IDs, and all 30
  link-segment rows. The ID gaps (13, 15, 17, 19) stay gaps.
- **FR-6**: The seed data uses explicit IDs
  everywhere. No row is matched by a natural
  key: pieces 2 and 5 share composer, work, and
  movement, and the same work appears split or
  replaced across IDs (Mahler 2: 21 against 36
  and 37; Bach Suite 2: 32 against 38, 39, and
  40; Hindemith Sonata: 35 against 41-45).
- **FR-7**: The seed data is the source of truth
  for the canonical tables. The seed step writes
  every seeded row with the seed values (upsert).
  A seeded row that differs in the database takes
  the seed values. The repository wins on conflict.
- **FR-8**: The seed step never writes users,
  enrollments, sessions, availability, settings,
  recordings, mocks, or mock items.
- **FR-9**: The seed step never deletes a row.
  A row that exists in the database but not in
  the seed data stays in place. Deleting such a
  row is out of scope for the seed step. The
  read-only comparison (FR-14) reports it.
  Removing a seeded row from the live database
  needs a recorded decision and a manual
  rollout.
- **FR-10**: When a seeded ID already holds a
  row with different values, the seed step
  writes the seed values and reports the
  overwrite. The repository wins on conflict. The
  step never skips a seeded row silently.
- **FR-11**: The seed step is separate from
  migrate. Migrate creates only the canonical
  schema, as decided in praeludium#65 and in
  unciae AGENTS.md §26. The seed data lives
  outside migrate.
- **FR-12**: A fresh database opened by the
  application holds the full seed data: 6
  auditions, 18 links, 13 rounds, 45 pieces, 85
  piece-audition links, 25 segments, and 30
  link-segment rows. The users row is absent:
  it is removed from migrate and never comes
  from the seed (ADR 0003). The enrollment
  toggle needs a users row, so the plan step
  handles the fresh-database path and the test
  setup.
- **FR-13**: A new audition, piece, or segment
  enters the seed data with a new ID above the
  current maximum. The seed step adds it on the
  next start. No manual SQL runs on the live
  database for the addition.
- **FR-14**: The cycle delivers a read-only
  comparison of the live database against the
  seed data. The comparison reports a missing
  row, an extra row, and a different value. The
  comparison writes nothing.
- **FR-15**: The implementation re-reads the rows
  for pieces 29 and 30 on audition 3 in quoted
  mode before it encodes them, and it encodes the
  exact text it reads. The list-mode dump shows
  one extra separator in those two rows.

## Success Criteria

- **SC-1**: A fresh database opened by the
  application holds exactly the seeded rows: 6
  auditions, 18 links, 13 rounds, 45 pieces, 85
  piece-audition links, 25 segments, and 30
  link-segment rows. Counts and IDs match the
  dump of 2026-11-24.
- **SC-2**: Opening the same database a second
  time changes no row. A dump before and after
  the second open is identical.
- **SC-3**: The read-only comparison of the live
  database against the seed data reports no
  missing row and no different value, except
  created_at, which the dump did not capture,
  and the two note rows re-read in quoted mode.
- **SC-4**: The 215 sessions and the 5
  enrollments rows survive the seed step
  unchanged: same IDs, same counts, same values.
- **SC-5**: The crate coverage gate of 90 percent
  holds, and the CI pipeline is green.
- **SC-6**: No manual SQL on the live database is
  needed to add a new audition. A test addition
  through a seed entry appears after a restart
  with no other step, verified on a test
  database, never on production data.

## Decisions

- **D-1** (decided 2026-11-24 by the owner): The
  seed step runs at application start, after
  migrate, and is idempotent. The explicit seed
  command is the recorded alternative. ADR 0002
  records the choice.
- **D-2** (decided 2026-11-24 by the owner): The
  users row (ID 1) is removed from migrate
  completely. Migrate writes no data at all.
  The live database already holds the row, so
  the live deployment is unaffected. Users are
  not seed data. The seed never writes a users
  row. ADR 0003 records the removal.
- **D-3** (decided 2026-11-24 by the owner): The
  seed data is canonical. The seed step writes
  every seeded row with the seed values (upsert).
  The repository wins on conflict. User
  customizations live in user-linked tables or
  relationships, never in a canonical table.
  The plan step redesigns any current UI path
  that edits a canonical table: the path is
  removed or redirected to user-linked storage.
- **D-4** (decided 2026-11-24 by the owner, ratified): The
  seed data uses explicit IDs. The alternative
  is a natural key, which the dump refutes:
  pieces 2 and 5 share one key.
- **D-5** (decided 2026-11-24 by the owner, ratified): The
  seed step never deletes a row. Removal needs a
  recorded decision and a manual rollout. A row
  that exists in the database but not in the
  seed data is reported by the read-only
  comparison (FR-14), not deleted.
- **D-6** (decided 2026-11-24 by the owner, with
  D-3): On an ID conflict, the seed step writes
  the seed values and reports the overwrite.
  The step never skips a seeded row silently.

## Resolved Questions

- **Q-1** (resolved 2026-11-24, from the dump):
  explicit IDs, not natural keys. Pieces 2 and 5
  share composer, work, and movement.
- **Q-2** (resolved 2026-11-24, from the code):
  the seed does not reproduce created_at. The
  dump did not capture it, and no application
  behavior reads created_at for auditions or
  pieces. Only recordings reads it.
- **Q-3** (resolved 2026-11-24, from the dump):
  La Fenice has no piece link yet. The seed adds
  none for it. Its excerpts come from the Passi
  document in a later change.
- **Q-4** (resolved 2026-11-24, from the dump):
  archived rows stay in the seed data as
  archived. The archive state is part of the
  repertoire, and the sessions of archived pieces
  stay readable.
- **Q-5** (decided 2026-11-24 by the owner):
  users are not seeded. The seed covers only
  auditions, pieces, and segments, with their
  links and rounds. The users row is removed
  from migrate completely; the live database
  already holds it.
- **Q-6** (decided 2026-11-24 by the owner):
  the seed is canonical, not additive-only. The
  repository wins on conflict. User customizations
  live in user-linked tables or relationships,
  never in a canonical table.

## Assumptions

- **A-1**: The dump of 2026-11-24 is the complete
  inventory of the live database.
- **A-2**: The seed data covers the repertoire as
  of 2026-11-24 and is its source of truth.
  Later additions enter through seed entries,
  not through edits to live rows. A live edit to
  a canonical row is overwritten by the seed
  step on the next start.
- **A-3**: Sessions, mocks, recordings,
  availability, settings, users, and
  enrollments stay out of scope. They are user
  data, not repertoire data.
- **A-4**: The EXTRA CARE rules apply: database
  backup before every deploy, live verification
  with real content, never write production data
  for tests.
