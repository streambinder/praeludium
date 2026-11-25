# Feature Specification: Retire the Archived Flag (Ritiro dell'archiviazione)

**Feature Branch**: `feat/retire-audition-archived`

**Created**: 2026-11-24

**Status**: Complete (implemented in PR #69, deployed 2026-11-25)

**Amended**: 2026-11-24 (the owner extends the scope
to pieces: `archived_at` leaves the pieces too; the
three duplicates leave the database outright, and
the links of the seven archived pieces leave the
seed and the live database with them)

**Amended**: 2026-11-25 (the owner states the
product vision: plan membership is decided only
by audition enrollment, never per piece. The
links stay. The duplicates still leave the
database as data cleanup.)

**Input**: User description: "dovremmo dismettere
il campo archived_at per i concorsi, non ha senso
visto che gli utenti possono o meno iscriversi ai
concorsi. piuttosto mostra "passato" quando un
concorso è passato"

Second input, 2026-11-24: "rimuovi archived_at
anche dai pezzi, i pezzi rimangono per sempre"

## Clarifications

### Session 2026-11-24

- The `archived_at` column leaves the `auditions`
  table and the `pieces` table, the seed, and every
  read. No archived state remains anywhere.
- Past is derived, not stored: an audition is past
  when its date is before today. No stored state
  replaces the flag.
- Enrollment is the only opt-in. A past audition
  shows the "Passato" badge and no enrollment
  toggle. An upcoming audition shows the toggle,
  enrolled or not.
- Enrollment rows for past auditions stay in the
  database, inert. The plan gate already excludes
  past dates, so no enrollment migration runs.
- The live `archived_at` columns stay in the live
  database, unread and unwritten. Code-side
  migrations never alter a live table. A manual
  `DROP COLUMN` is a separate optional step, not
  part of this cycle.
- The "concluded" name goes with the flag. The
  badge, the fields, and the locale key become
  "past": one term for one concept.
- The auditions list orders upcoming auditions
  first by date, then past auditions by date. It
  keeps the shape of the archived ordering it
  replaces.
- The enrollment toggle handler gains no past
  check. The toggle is hidden in the interface
  only, as the archived check hid it before.
- The removed-routes table keeps its entries for
  the old archive and restore paths. They answer
  "gone" for old clients and bookmarks.
- Pieces remain forever: a piece is never deleted
  and never hidden by a flag. The four
  non-duplicate archived pieces keep their rows
  and their links. Plan membership is decided
  only by audition enrollment: if the user is
  enrolled in an audition, the pieces and the
  segments of that audition enter plan
  generation, otherwise not. No distinction
  happens per piece.
- The three duplicate pieces leave the seed and
  the live database as data cleanup: Beethoven 5
  III mov. (ID 5), Bach Suite n. 2 whole (ID 32),
  Hindemith Sonata op. 25 n. 3 whole (ID 35).
  All three have zero sessions, verified live on
  2026-11-24, so the deletion loses no diary
  data. The seed never deletes a row, so the
  live removal is a manual DELETE, with a
  backup first, run before the deploy. The
  delete cascades to their three link rows.
- The pieces list loses its "mostra archiviati"
  filter. With no archived piece, the filter has
  nothing to show.

## Hypothesis Challenge

The change rests on one hypothesis: the archived
flag still carries meaning that enrollment and the
date do not carry. The hypothesis is false on the
real data, for auditions and for pieces.

Evidence, from the seed data and from the live
auditions page, read on 2026-11-24.

- Three auditions are archived live: Opera Roma
  (ID 2, archived 2026-11-15), Cagliari
  Concertino (ID 4, archived 2026-11-15), and
  Cagliari Fila (ID 5, archived 2026-11-15). The
  three were archived by hand within one minute,
  as a way to hide auditions Agnese does not study
  for.
- All three are unenrolled, and all three are
  past today: Roma held 2026-11-23, Cagliari
  2026-11-16 and 2026-11-17. Enrollment already
  says "not studied for", and the date already
  says "past". The flag adds no third fact.
- The other three auditions are upcoming and not
  archived: Santa Cecilia (2027-01-08), San Carlo
  (2026-12-14), La Fenice (2027-02-15). Santa
  Cecilia and San Carlo are enrolled; La Fenice
  is not.
- The "concluded" badge already exists in code
  (`concluded: !archived && date < today`), but
  no audition shows it live: every past audition
  is archived, so the condition never holds. The
  single "concluded" on the list page is the intro
  text, not a badge. The past state exists in the
  plan formula but has no visible badge of its
  own.
- The archive routes and handlers are already
  gone: cycle 007 removed them with the
  canonical-write forms. Only the columns, the
  seed fields, and the display remain.
- Seven pieces are archived live: Beethoven
  Sinfonia n. 5 III mov. (ID 5), Mahler Sinfonia
  n. 2 (ID 21), Haydn Concerto in Do maggiore
  (ID 27), Schumann Concerto (ID 29), Elgar
  Concerto (ID 30), Bach Suite n. 2 whole (ID
  32), Hindemith Sonata op. 25 n. 3 whole (ID
  35). They carry eight links: five to enrolled
  upcoming auditions, three to past unenrolled
  ones.
- The owner corrects the first draft of this spec
  (2026-11-24): the pieces would not re-enter the
  plan anyway, because Agnese unsubscribed from
  those auditions. That correction holds for two
  pieces only. Roma and Cagliari are unenrolled,
  and they cover Mahler 2 (ID 21) and the Bach
  suite whole (ID 32). It does not hold for the
  other five: Santa Cecilia and San Carlo are
  enrolled, and they cover Beethoven 5 III (ID
  5), Haydn Do maggiore (ID 27), Schumann (ID
  29), Elgar (ID 30), and the Hindemith sonata
  whole (ID 35). The proof is live on the
  Schumann piece page: its score breakdown reads
  base 4.0 from San Carlo, 20 days away. The
  archived flag is the only thing keeping
  Schumann out of the plan.
- The owner states the product vision
  (2026-11-25): a user is enrolled in an
  audition or not. If enrolled, the pieces and
  the segments of that audition enter plan
  generation, otherwise not. No distinction
  happens per piece; the decision happens only
  at the audition level. The draft idea of
  removing links per piece is abandoned on that
  ground: the links stay, all 82 of them after
  the duplicate cleanup, and enrollment decides.
  The consequence is stated, not hidden: Haydn
  Do maggiore (ID 27), Schumann (ID 29), and
  Elgar (ID 30) enter the San Carlo plan as
  required pieces, and the Hindemith sonata
  whole (ID 35) would do the same for Santa
  Cecilia, but it leaves as a duplicate. Mahler
  2 (ID 21) stays out of the plan: Cagliari is
  unenrolled.
- The three duplicates have zero sessions,
  verified live on 2026-11-24 on their per-piece
  diary pages: Beethoven 5 III (ID 5), Bach
  suite whole (ID 32), Hindemith sonata whole
  (ID 35). Deleting them loses no diary data.
  Mahler 2 (ID 21) has 5 sessions and stays.
  The seed never deletes a row (`src/seed.rs`:
  the step applies inserts and upserts only, and
  the comparison reports extra rows instead of
  removing them), so the live removal of the
  duplicates is a manual DELETE, with a backup
  first. The delete cascades to their three link
  rows (`piece_audition.piece_id` references the
  pieces table with ON DELETE CASCADE, foreign
  keys on).
- The links stay, the pieces stay, except for the
  three duplicates, which go from the database
  outright as data cleanup. A piece linked only
  to an unenrolled audition never enters the
  plan. Every kept piece stays listed, stays
  readable, and stays loggable from its piece
  page. Its sessions, if any, are untouched:
  sessions reference pieces by ID, and the piece
  rows stay. The deleted pieces have no sessions
  at all.
- Alternatives examined and rejected: keep the
  flag as a manual hide for upcoming auditions
  (it would restate enrollment with a second
  mechanism, and the owner rejects it); keep the
  "concluded" name for the badge (the owner asks
  for "passato", and one term names one concept);
  remove links per piece (it would put a
  per-piece distinction in the plan, against the
  product vision the owner states).
- The diary is safe by construction: sessions
  are rows per piece, and no migration or data
  step of this cycle touches the sessions table,
  the pieces, the links that stay, or the
  enrollments. The deploy alone changes the
  display.
- This supersedes the clarification of cycle 006
  that "archive stays what it is: a concluded
  audition". Enrollment now exists, and the
  owner retires the flag on that ground.

## User Scenarios and Testing

### User Story 1: A past audition shows "Passato" (Priority: P1)

The user opens the auditions list. Opera Roma,
Cagliari Concertino, and Cagliari Fila show the
"Passato" badge and sit at the bottom of the list,
ordered by date. No "archiviato" badge remains
anywhere on the page.

### User Story 2: No enrollment toggle on a past audition (Priority: P1)

The user opens the detail page of a past audition.
The page shows the "Passato" badge and no enroll
or unenroll button. The preparation map and the
diary links keep working.

### User Story 3: Upcoming auditions keep the toggle (Priority: P1)

The user opens the detail page of an upcoming
audition. The enroll or unenroll button is there,
as before. Enrolling still joins the plan; the
two enrolled badges stay on Santa Cecilia and
San Carlo.

### User Story 4: A fresh database has no archived column (Priority: P1)

A fresh database opens with the seed: six
auditions and 42 pieces, no `archived_at` column
on either table, and the seed applies cleanly.
The startup comparison reports no difference on
the audition and piece rows.

### User Story 5: The four pieces stay linked, and the duplicates go (Priority: P1)

The user opens the pieces list. All 42 pieces
are listed, including the four former archived
ones (Mahler 2, Haydn Do maggiore, Schumann,
Elgar), with their audition chips and no
"mostra archiviati" filter. The three duplicates
(Beethoven 5 III mov., Bach Suite whole,
Hindemith sonata whole) are gone from the list
and from the database. The daily plan gains
Haydn Do maggiore, Schumann, and Elgar through
San Carlo, which Agnese is enrolled in, and the
manual log list gains them too. Mahler 2 stays
out of both: Cagliari is unenrolled.

### User Story 6: No behavior change elsewhere (Priority: P1)

The readiness rows, the stats, the mocks, the
diary, and the pieces keep the current behavior.
The plan formula is unchanged; only its input
changes the way the vision says: the three San
Carlo pieces the flag used to hide enter the
plan, and the manual log list gains them too.

## Requirements

- **FR-1**: The `auditions` table in the
  canonical schema has no `archived_at` column.
  A fresh database opens without it.
- **FR-2**: The seed carries no `archived_at` for
  auditions: the `SeedAudition` field goes, the
  six seed rows lose it, and the seed insert,
  upsert, select, and comparison drop the column.
- **FR-3**: The reads carry the past state through
  the `Audition` record: `list_auditions`,
  `piece_auditions`, and `audition_or_404` take
  `today` and set `past` from `date < today`.
  The `archived` field leaves the `Audition`
  struct.
- **FR-4**: The auditions list orders upcoming
  auditions first by date, then past auditions by
  date.
- **FR-5**: The auditions list and the detail page
  show the "Passato" badge for a past audition
  and no enrollment toggle. The Enrolled badge
  shows only when the audition is not past.
- **FR-6**: The diary chips and the piece-page
  chips mute for past auditions, as they muted
  for archived ones.
- **FR-7**: The plan, readiness, and stats drop
  the archived condition: `has_active` means a
  linked audition with `date >= today`, and the
  stats query filters by date only. The outcome
  is identical on the current data.
- **FR-8**: The new strings exist in the six locale
  files (`it`, `en`, `de`, `es`, `fr`, `pt`):
  the past badge, the rewritten list intro, the
  settings description, and the empty-plan text.
  No locale string mentions an archived audition.
- **FR-9**: One data step runs, before the deploy
  and after a backup: delete the three duplicate
  piece rows from the live database, and verify
  the counts (42 pieces, 82 links, sessions
  unchanged). The delete cascades to their three
  link rows. No enrollment row, session row, or
  kept link row changes. The live `archived_at`
  columns stay in place, unread and unwritten.
- **FR-10**: The `pieces` table in the canonical
  schema has no `archived_at` column. A fresh
  database opens without it.
- **FR-11**: The seed carries no `archived_at` for
  pieces: the `SeedPiece` field goes, the 42 kept
  seed rows lose it, and the seed insert, upsert,
  select, and comparison drop the column. The
  three duplicate pieces (IDs 5, 32, 35) leave
  the seed outright, with their three link rows.
  The other 82 links stay. The piece count goes
  from 45 to 42, the link count from 85 to 82.
- **FR-12**: `list_pieces` drops the
  `include_archived` parameter. The pieces page
  loses the "mostra archiviati" filter and its
  query parameter.
- **FR-13**: The mock queries and the stats query
  drop the piece archived condition.
- **FR-14**: The locale keys `pieces.archived_badge`
  and `pieces.show_archived` leave all six locale
  files. No locale string mentions an archived
  piece.
- **FR-15**: The `.card.archived` style rule goes:
  no card uses it after this cycle.

## Success Criteria

- **SC-1**: A fresh database holds the seed with
  no `archived_at` column on `auditions` or
  `pieces`, and the startup comparison reports no
  audition or piece difference.
- **SC-2**: After the deploy, the live list shows
  three "Passato" badges (Roma, Cagliari
  Concertino, Cagliari Fila), zero "archiviato"
  badges, and two Enrolled badges (Santa Cecilia,
  San Carlo).
- **SC-3**: A past audition detail page shows the
  "Passato" badge and no enrollment toggle. An
  upcoming audition detail page keeps the toggle,
  and the toggle round-trips.
- **SC-4**: The diary loses no data: the sessions
  count is unchanged after the deploy, verified
  live.
- **SC-5**: The crate coverage gate of 90 percent
  holds, and the CI pipeline is green.
- **SC-6**: After the deploy, the live pieces list
  shows all 42 pieces, the four former archived
  ones with their audition chips. The daily plan
  gains Haydn Do maggiore, Schumann, and Elgar
  through San Carlo, and the manual log list
  gains them too. Mahler 2 stays out of both.
  The sessions count is unchanged: the deleted
  pieces had no sessions.

## Decisions (proposed, pending ratification)

- **D-1**: `archived_at` leaves the auditions
  table, the seed, and every read. The change
  covers auditions and pieces (see D-9).
- **D-2**: Past is derived in the read layer.
  `list_auditions`, `piece_auditions`, and
  `audition_or_404` take `today` and set
  `Audition.past` from `date < today`. The
  templates stay dumb: they render the flag.
  The rejected alternative computes past in
  each handler from the date: it repeats the
  comparison in every handler that builds an
  `Audition` record.
- **D-3**: The list orders upcoming first by date,
  then past by date. It keeps the shape of the
  ordering the flag gave.
- **D-4**: A past audition shows the "Passato"
  badge and no enrollment toggle. The Enrolled
  badge shows only when the audition is not
  past. Enrollment rows for past auditions stay
  in the database, inert.
- **D-5**: The live columns stay in the live
  database, unread and unwritten. A manual
  `DROP COLUMN` is a separate optional step,
  outside this cycle.
- **D-6**: The "concluded" name goes: the field,
  the template variable, and the locale key
  become "past". The strings are it "passato",
  en "past", de "vergangen", es "pasado", fr
  "passé", pt "passado".
- **D-7**: The diary chips and the piece-page
  chips mute for past auditions, as they muted
  for archived ones.
- **D-8**: The enrollment toggle handler gains no
  past check. The toggle is hidden in the
  interface only, as before.
- **D-9**: `archived_at` leaves the pieces too:
  the table, the seed, the reads, the templates,
  the locales, and the mock and stats queries.
  The pieces page loses the "mostra archiviati"
  filter. No archived state remains anywhere.
- **D-10**: The three duplicates (IDs 5, 32, 35)
  leave the seed and the live database as data
  cleanup. The live removal is a manual DELETE,
  run before the deploy and after a backup,
  because the seed never deletes a row; the
  delete cascades to their three link rows. The
  other four (IDs 21, 27, 29, 30) keep their
  rows and their links. Plan membership is
  decided only by audition enrollment, never
  per piece: the enrolled auditions pull their
  pieces into the plan, the unenrolled ones do
  not.
- **D-11**: The `.card.archived` style rule goes
  with the last card that used it. The muted
  style for past auditions lives in `.card.past`.

## Resolved Questions

- **Q-1** (resolved 2026-11-24, from the live
  data): dropping the flag loses no fact. The
  three archived auditions are all unenrolled
  and all past, so enrollment and the date
  already state what the flag stated.
- **Q-2** (resolved 2026-11-24, from the
  request): the badge reads "passato", not
  "concluso". The concluded name leaves the
  code with the flag.
- **Q-3** (resolved 2026-11-24, from the code):
  the settings list and the pieces filter will
  include past auditions. They excluded archived
  ones before, through `list_auditions(false)`.
  Including them is accepted: past auditions are
  still filterable history, and their weights
  never enter the plan formula.
- **Q-4** (resolved 2026-11-25, from the owner):
  the product vision decides plan membership:
  enrollment only, at the audition level, never
  per piece. The links stay. The three
  duplicates (5, 32, 35) go from the database
  outright as data cleanup: all three have zero
  sessions, verified live. The other four keep
  their rows and their links; San Carlo pulls
  27, 29, and 30 into the plan, Cagliari keeps
  21 out of it.

## Assumptions

- **A-1**: The production inventory of 2026-11-24
  stands: six auditions, IDs 1-6, 45 pieces, 85
  piece-audition links, with the links and rounds
  of the seed. After this cycle the piece count
  is 42 and the link count is 82.
- **A-2**: No new dependency. The change uses the
  existing migration helper, read patterns, and
  toggle.
- **A-3**: The EXTRA CARE rules apply: database
  backup before the deploy, live verification
  with real content, never write production data
  for tests.
- **A-4**: The live `archived_at` values stay in
  the live database, unread. No manual column
  drop runs in this cycle.
