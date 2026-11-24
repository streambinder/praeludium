# The Users Row Is Removed from Migrate

## Status

Decided on 2026-11-24 by the owner. The users
row is removed from migrate completely.

## Context and Problem Statement

The enrollments table references users(ID) with
foreign keys on. The enrollment toggle inserts an
enrollments row for the current user, so a fresh
database needs a users row or the toggle fails.
Migrate still writes that one row (an INSERT OR
IGNORE for ID 1), because praeludium#65 left it
in place as a follow-up. Unciae AGENTS.md §26
states that code-side migrations create only the
canonical schema and never insert seed data. The
live database already holds the users row. The
question was where the users row lives once the
seed mechanism exists.

## Considered Options

1. Move the users row into the seed mechanism.
   The seed data holds the row (ID 1), and the
   seed step inserts it like any other seeded
   row. Migrate then writes no data at all.
2. Ensure the row in the enrollment toggle
   handler. The handler inserts the users row
   (INSERT OR IGNORE) before it inserts the
   enrollments row. Migrate drops the seed, and
   the row exists exactly when enrollment needs
   it.
3. Drop the users table and the foreign key.
   Enrollments keep a plain user ID, or drop the
   user column while the application has one user.
   The schema change needs a manual migration on
   the live database, and it removes the
   multi-user groundwork of spec 006.
4. Remove the users row from migrate completely.
   Migrate writes no data at all. The live
   database already holds the row, so the live
   deployment is unaffected. A fresh database
   has no users row.

## Decision Outcome

Decided: option 4. The owner decided on
2026-11-24 to remove the users row from migrate
completely. The live database already holds it,
so the removal is not a problem for the live
deployment.

Rejected: option 1. The owner decided earlier
on 2026-11-24 that users are not seeded by hand:
the seed covers only auditions, pieces, and
segments, with their links and rounds. Users are
not repertoire, and the users row is not seed
data. It is never seeded by hand.

Options 2 and 3 stay open as future follow-ups
for the fresh-database path. Neither is decided
here.

## Consequences

- Positive: migrate writes no data at all. The
  §26 rule holds with no exception.
- Positive: the removal takes the last literal
  personal name out of production code, as the
  private-information rule of 2026-11-24
  requires. The name appears once in the code,
  in the migrate INSERT.
- Positive: the live deployment is unaffected.
  The live database already holds the row.
- Negative: a fresh database has no users row,
  so the enrollment toggle fails on a fresh
  database until a users row exists. The plan
  step must handle the fresh-database path and
  the test setup: existing tests insert
  enrollment rows directly.
- Neutral: the plan step removes the INSERT from
  migrate. The plan step must not move the row
  into the seed.
- Neutral: the users row stays a single row for
  the single user. User switching and sign-in
  stay out of scope, as in spec 006.
