# Seed Auditions, Pieces, and Segments at Application Start

## Status

Decided on 2026-11-24 by the owner.

## Context and Problem Statement

The repertoire data (auditions, pieces, segments,
and their links) lives only in the live database.
The agent cannot read or write that database, and
the owner changes it only through manual SQL on
the host. The manual path is slow and error-prone:
the La Fenice insert of 2026-11-24 needed three
attempts. The owner decided on 2026-11-24 that
the repertoire lives as seed data in the
repository, so the agent manages it through pull
requests. The question is when and how the seed
data reaches the database.

Two rules bind the choice. Praeludium#65 (merged
2026-11-24) reduced migrate to CREATE TABLE IF
NOT EXISTS only. Unciae AGENTS.md §26 (merged
2026-11-24) states that code-side migrations
create only the canonical schema: no ALTER, no
rename, no value rewrite, no seed data. The seed
mechanism must be a separate step from migrate,
not part of it.

## Considered Options

1. Seed at application start, after migrate. The
   application opens the database, migrate creates
   the schema, and the seed step then applies the
   seeded rows. The step is idempotent: a second
   start changes nothing. A deploy applies
   a merged seed change with no other step.
2. Explicit seed command. The operator runs a
   command to apply the seed data, on demand. The
   application never writes seed data on start.
3. Seed only on an empty database. The seed step
   runs once, when the database file is new. Later
   seed changes never reach an existing database.
4. Hand-applied SQL files. The repository holds
   SQL files, and the owner runs them by hand
   against the live database at each deploy.

## Decision Outcome

Decided option: option 1.

The start step makes the repository the single
source of truth: a merged seed change reaches the
live database on the next deploy, with no manual
step and no forgotten run. The step is idempotent,
so a restart is safe. The step stays separate from
migrate: migrate creates the schema, the seed step
applies the rows. Option 2 leaves a manual step per
deploy and ships stale data when the step is
skipped. Option 3 defeats the purpose: the live
database would never gain a new audition. Option 4
keeps the manual run and adds no verification.

## Consequences

- Positive: a new audition reaches production
  through a reviewed pull request and a deploy,
  with no manual SQL.
- Positive: a restart is safe by construction: the
  step changes nothing on the second run.
- Positive: migrate stays free of data writes, as
  §26 requires.
- Negative: the application writes data at start.
  A bug in the seed step can write the wrong rows
  before the server serves. The step must be
  idempotent and tested on a fresh database and on
  a copy of the live data.
- Negative: a live edit through the UI and a seed
  entry can disagree. The seed data is canonical:
  the seed step writes the seed values, so the
  repository wins and the live edit is overwritten.
  User customizations must live in user-linked
  tables or relationships, never in a canonical
  table. The read-only comparison reports a
  difference.
- Neutral: the seed step runs on every start,
  including tests through open_memory. The cost is
  one pass over a small, fixed set of rows.
