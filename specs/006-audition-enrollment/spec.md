# Feature Specification: Audition Enrollment (Iscrizione ai concorsi)

**Feature Branch**: `feat/audition-enrollment`

**Created**: 2026-11-12

**Status**: Complete (ratified in #58, implemented in #59, live 2026-11-12)

**Amended**: 2026-11-12 (enrollment moves from a boolean
on auditions to an enrollments join table, for the
multi-user future)

**Input**: User description: "Al momento l'utente è
sempre iscritto a tutti i concorsi definiti. Facciamo
in maniera tale che i concorsi sono opt-in per l'user
che ci si deve 'iscrivere' perché questi facciano
parte del suo piano di studio (ovviamente può
aggiungere al diario anche pezzi per concorsi a cui
non è iscritto). Dopodiché iscriviamo Agnese a Santa
Cecilia e San Carlo soltanto. Facciamo in maniera tale
che questa feature non perda dati diario attuali."

## Clarifications

### Session 2026-11-12

- Enrollment is a row in the `enrollments` table,
  one per (user, audition) pair. The same piece stays
  linked to every audition that asks it. Enrollment
  decides which auditions feed the daily plan of one
  user.
- The app has one user today and no sign-in. This
  cycle adds a minimal `users` table and seeds one
  user, the student (Agnese, ID 1). The plan reads
  the enrollments of that user. Choosing the active
  user and signing in are out of scope for this
  cycle; the tables are the multi-user groundwork.
- Enrolled means member of the daily plan. A
  non-enrolled audition stays defined, listed, and
  readable: its detail page, its preparation map, its
  mock runner, and its readiness keep working.
- The diary logs sessions per piece, with no audition
  check today: the session handler takes a piece ID
  only. Enrollment changes nothing there by
  construction. A piece of a non-enrolled audition
  keeps its log form on its piece page.
- Archive stays what it is: a concluded audition.
  Enrollment is intent, not conclusion. Archiving a
  live audition to remove it from the plan would
  misstate its state and hide it from the lists.
- Weight stays a priority from 1 to 5, not
  membership. A weight of 0 is not a membership
  switch: the form forbids it, and it would conflate
  priority with enrollment.
- A new audition starts unenrolled: it has no
  enrollments row until the user enrolls. Enrollment
  is opt-in, so the user enrolls each new audition
  explicitly. The five existing auditions keep their
  current plan membership through the migration
  backfill, which writes one enrollments row per
  audition for the seeded user; the data step then
  sets the enrollment the owner ordered.
- The readiness rows shown with the plan cover
  enrolled auditions only: they summarize the plan.
  The audition detail page keeps its own preparation
  map for every audition, enrolled or not.
- The enrollment toggle lives on the audition detail
  page. The auditions list and the detail page show
  the enrollment state with a badge.

## Hypothesis Challenge

The change rests on one hypothesis: the user studies
for every defined audition. The hypothesis is false
on the real data.

Evidence, from the production auditions and from the
live audit of 2026-11-03, read on 2026-11-12.

- The app has no enrollment concept. The plan gate,
  `upcoming_auditions`, includes every non-archived,
  not-yet-held audition linked to a piece. All five
  auditions feed the plan today, with weights 5, 4, 4,
  3, 3: Santa Cecilia, Opera Roma, San Carlo, Cagliari
  Secondo, Cagliari Fila.
- A Roma-only piece is planned today although the
  owner excludes Roma from the plan: Schubert
  Arpeggione, piece 31, is linked to Opera Roma only
  and still reaches the daily plan through its weight
  of 4. The same holds for Piatti Capriccio n. 9,
  piece 33, and Bach Suite n. 2, pieces 38, 39, 40.
- The stated priority of Agnese is Santa Cecilia >
  Opera Roma > San Carlo > Cagliari, and the London
  trip of 15-21/11 conflicts with the Cagliari tests
  of 16-17/11. The owner decides on 2026-11-12:
  enroll Santa Cecilia and San Carlo only.
- Alternatives examined and rejected: archive the
  three auditions (wrong semantics, it means
  concluded, and it hides them from the lists);
  weight 0 (forbidden by the form, and it conflates
  priority with membership).
- The first draft put an `enrolled` boolean on the
  audition. The owner pointed out the multi-user
  future, and the flag fails it: enrollment is per
  user, not per audition, so one flag cannot hold a
  different enrollment per user, and a later
  multi-user change would need a migration to move
  the flag into a table. The join table models the
  real relation from the start, at the cost of one
  extra table and one join in the reads. The flag is
  the mechanism that matches the request in shape
  (intent, reversible, per audition), and the table
  is the shape that survives more users.
- The diary is safe by construction: sessions are
  rows per piece, and no migration or data step of
  this cycle touches the sessions table, the pieces,
  or the links. Enrollment only narrows which
  auditions the plan reads.

## User Scenarios and Testing

### User Story 1: Enroll in an audition (Priority: P1)

The user opens an audition detail page and enrolls.
The audition joins the daily plan from the next plan
computation. Its pieces gain its weight in the plan
base when they have no other enrolled audition.

### User Story 2: Unenroll from an audition (Priority: P1)

The user opens an enrolled audition detail page and
unenrolls. The audition leaves the daily plan. Its
pieces stay linked and readable, and their diary
entries stay untouched.

### User Story 3: Plan excludes non-enrolled pieces (Priority: P1)

With Santa Cecilia and San Carlo enrolled only, the
daily plan holds no piece whose upcoming auditions
are all non-enrolled. Roma-only pieces (Schubert 31,
Piatti 33, Bach 38, 39, 40) are absent from the plan.

### User Story 4: Diary logging for a non-enrolled piece (Priority: P1)

The user logs a session on a piece of a non-enrolled
audition from its piece page. The session saves with
its minutes and confidence, and the diary shows it.
Enrollment never blocks logging.

### User Story 5: A new audition starts unenrolled (Priority: P2)

The user creates a new audition. It starts
unenrolled and stays out of the plan until the user
enrolls it from its detail page.

### User Story 6: No behavior change elsewhere (Priority: P1)

The readiness of an enrolled audition, the mock
runner, the search, the stats, and the audition
detail pages keep the current behavior. The plan,
the pool grouping, and the readiness rows shown with
the plan are the only computations that change.

## Requirements

- **FR-1**: Enrollment lives in an `enrollments`
  table: one row per (user, audition) pair, with a
  unique pair, foreign keys to `users` and
  `auditions`, and cascade on delete. A minimal
  `users` table holds the seeded user (ID 1,
  Agnese). The migration backfills one enrollments
  row per existing audition for the seeded user. A
  new audition starts with no enrollments row.
- **FR-2**: The audition detail page has one toggle:
  enroll when unenrolled, unenroll when enrolled.
  Enroll inserts the enrollments row for the current
  user; unenroll deletes it. The toggle posts and
  redirects back to the detail page. No other page
  writes enrollment.
- **FR-3**: The auditions list and the audition detail
  page show the enrollment state of the current user
  with a badge. The reads carry the state through
  the `Audition` record, as the archived flag does
  today.
- **FR-4**: The plan gate `upcoming_auditions` skips
  a non-enrolled audition, as it skips an archived or
  past one. The plan base, the days to the nearest
  audition, and the urgency use enrolled auditions
  only.
- **FR-5**: Pool grouping uses the same gate: a pool
  forms only from the enrolled auditions of its
  members.
- **FR-6**: The readiness rows shown with the plan
  cover enrolled auditions only.
- **FR-7**: Diary logging does not change: the session
  handler takes a piece ID only, with no audition
  check, and the piece page keeps the log form for a
  piece of a non-enrolled audition.
- **FR-8**: The new strings exist in the six locale
  files (`it`, `en`, `de`, `es`, `fr`, `pt`): the
  enroll and unenroll labels and the enrolled badge.
- **FR-9**: No change to sessions, pieces, links,
  segments, mocks, stats, or search. The migration
  and the data step write no sessions row and delete
  no row of any table.
- **FR-10**: A deploy data step sets the enrollment
  the owner ordered, through the toggle, after a
  database backup: the toggle deletes the
  enrollments rows of Opera Roma (ID 2), Cagliari
  Secondo (ID 4), and Cagliari Fila (ID 5) for the
  seeded user, and keeps Santa Cecilia (ID 1) and
  San Carlo (ID 3) enrolled.

## Success Criteria

- **SC-1**: With Santa Cecilia and San Carlo enrolled
  only, the daily plan holds no piece whose upcoming
  auditions are all non-enrolled.
- **SC-2**: The Roma-only pieces (Schubert 31, Piatti
  33, Bach 38, 39, 40) are absent from the daily plan
  after the data step, and their piece pages still
  show the log form.
- **SC-3**: The toggle round-trips: enroll then
  unenroll returns the audition to its prior state,
  and the plan follows each change.
- **SC-4**: A session logged on a piece of a
  non-enrolled audition saves and appears in the
  diary (verified on the test database, never on
  production data).
- **SC-5**: The crate coverage gate of 90 percent
  holds, and the CI pipeline is green.
- **SC-6**: The diary loses no data: the migration
  and the data step write no sessions row and delete
  no row; the diary page still lists the existing
  entries after the deploy, verified live.

## Decisions (proposed, pending ratification)

- **D-1**: Enrollment is an `enrollments` join table
  between `users` and `auditions`, not a boolean on
  the audition. Enrollment is per user, and the
  table is the shape that survives more users
  without a later migration.
- **D-2**: The migration backfills one enrollments row
  per existing audition for the seeded user. The
  deploy alone changes nothing in the plan; the data
  step then sets the enrollment the owner ordered.
- **D-3**: A new audition starts unenrolled. Opt-in
  means the user enrolls explicitly.
- **D-4**: The plan gate is the single choke point:
  `upcoming_auditions` skips non-enrolled auditions.
  The pool grouping and the plan chips follow from
  the same gate, with no second filter.
- **D-5**: The readiness rows shown with the plan
  cover enrolled auditions only, through the same
  filter inside `audition_readiness`. The audition
  detail page keeps its own preparation map for
  every audition.
- **D-6**: The toggle lives on the audition detail
  page only. The auditions list shows the state but
  does not toggle it.
- **D-7**: The data step uses the toggle UI, not a
  surgery script, after a database backup. The
  toggle is the supported write path for enrollment.
- **D-8**: The current user is the seeded user (ID 1).
  `App` carries that ID, and the reads join
  enrollments for it. User switching and sign-in are
  out of scope for this cycle.

## Resolved Questions

- **Q-1** (resolved 2026-11-12, from the request and
  the live audit): archiving is not the enrollment
  mechanism. Archive means concluded; enrollment
  means intent. A live audition the user does not
  study for stays defined and readable.
- **Q-2** (resolved 2026-11-12, from the request):
  a new audition starts unenrolled. The request says
  the concorsi are opt-in.

## Assumptions

- **A-1**: The production inventory of 2026-11-03
  stands: five auditions, IDs 1-5, with the links and
  rounds of the live audit of that day.
- **A-2**: No new dependency. The change uses the
  existing migration helper, form machinery, and
  toggle patterns.
- **A-3**: The EXTRA CARE rules apply: database backup
  before the deploy data step, live verification with
  real content, never write production data for
  tests.
- **A-4**: The `users` table holds one seeded row.
  Auth and user switching are out of scope for this
  cycle.
