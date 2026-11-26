# Feature Specification: Choice Clusters Count Once at Their Best Member in Readiness (I cluster di scelta contano una volta sola, al membro più preparato, nella preparazione)

**Feature Branch**: `feat/cluster-readiness-012-spec`

**Created**: 2026-11-26

**Status**: Complete

**Errata**: the implementation matched the spec. Two helpers exist
instead of one: the grouping lives in `readiness_unit_groups`, wrapped
by `readiness_units`, so the logic sits in one place. The
implementation landed as one commit, not one per phase: the code and
test phases touch the same files (Rust unit tests are colocated), so
two commits would break the rule that each file appears in exactly one
commit. `ReadinessRow.pieces` still counts linked pieces, as the
detail page does; only the mean uses units. A pool tile now sorts by
its best member preparation instead of its worst, a visible
consequence of FR-8. One pre-existing test needed a fix: it passed
unlinked pieces, which no longer rank.

**Input**: User description: the owner sees Santa Cecilia fall from 50 percent to 43 percent on the day cycle 011 splits the Piatti choice into 4 pieces, and states the rule in his words: "i cluster come pezzo singolo di cui prendere quello al massimo della preparazione, per il resto tutti i pezzi singoli, ignorando round e simili".

## Clarifications

### Session 2026-11-26

- The readiness of an audition is the plain mean of
  the preparation of every linked piece. Three
  functions compute a mean this way, and none of
  them knows about clusters: `audition_readiness`
  in `src/handlers.rs`, `audition_prep_stats` in
  `src/prepmap.rs`, and the mean inside
  `stamp_prep_levels` in `src/prepmap.rs`.
- A piece with no session has preparation exactly 0. `prep_score` in `src/prepmap.rs` returns 0.0
  for an empty session list.
- The owner saw Santa Cecilia fall from 50 percent
  to 43 percent on 2026-11-26, the day cycle 011
  split the Piatti choice into 4 pieces. The
  denominator moved from 17 to 20 while the 3 new
  pieces add 0 to the sum. The live pin read 45
  percent later the same day: the value moves as
  Agnese logs sessions.
- The same dilution touches coverage and the
  weakest list. `readiness_coverage` in
  `src/readiness.rs` counts touched pieces over
  all pieces. `readiness_weakest` in
  `src/readiness.rs` ranks pieces, so 3 zero
  members take 3 of the 5 slots.
- The merged map tile of a pool shows the minimum
  member preparation, in the merge block of
  `src/prepmap.rs`. Its level already shows the
  maximum member level. No template reads the
  merged tile preparation today: the tile colour
  comes from the level. The minimum is latent,
  and it contradicts the rule below.
- The plan already treats a cluster as one unit:
  one pool card, shared minutes, pool-only
  members. Readiness is the only place that still
  counts members one by one.
- San Carlo carries the same dilution since cycle
  010: its Haydn cluster {25, 27} and its
  concerto cluster {28, 29, 30} count 5 members.
  Piece 27 has no session. Piece 30 has no
  session. Piece 29 has one assessment with no
  minutes and no confidence rating: the diary
  page reads "confidence never touched". All 3
  sit at 0. The San Carlo pin reads 40 percent.
- The Piatti cluster, read live on 2026-11-26:
  piece 50 has no session, piece 52 has no
  session, piece 51 has one assessment today,
  26 November 2026, confidence 3 of 5, no
  minutes, preparation 30 percent. The cluster
  maximum is piece 34.
- No clustered piece carries a target tempo: all
  9 clustered pieces have no target in the seed.
  The tempo signal is out of scope.
- The confidence trend and the study time are
  sums and histories, not means over pieces. A
  session on an unchosen member is real study
  time. Both stay unchanged.

## Hypothesis Challenge

- The hypothesis is that the best member stands
  for the cluster. Challenge: the best member is
  the chosen one in practice, because the chosen
  piece collects the sessions and the others stay
  at 0. If Agnese ever practices 2 alternatives,
  the maximum keeps the better one. No choice is
  recorded anywhere, so the maximum is the only
  reading of "the chosen piece" the data supports.
- Alternative: average the members. Rejected: the
  average punishes the audition for pieces the
  candidate will never play. That is the defect
  the owner reports.
- Alternative: drop cluster members from
  readiness entirely. Rejected: the candidate
  must prepare one of them, so the cluster must
  count once.
- Alternative: count the cluster at the
  preparation of a designated member. Rejected:
  no designation exists in the seed or the
  database, and adding one is a new feature, not
  a fix.

## Functional Requirements

- FR-1: For one audition, each choice cluster
  counts as one unit. The unit preparation is the
  maximum member preparation. Rounds play no
  part.
- FR-2: Every other linked piece counts as one
  unit at its own preparation.
- FR-3: A cluster with one member counts as that
  piece. No special case.
- FR-4: On a tie for the maximum, the first member
  in list order stands for the cluster.
- FR-5: The audition mean uses units, not pieces,
  in all 3 mean sites: `audition_readiness`,
  `audition_prep_stats`, and `stamp_prep_levels`.
  One shared helper computes the units. No site
  keeps its own mean logic.
- FR-6: Coverage counts units. A cluster counts
  as touched when any member is touched in the
  window. The denominator counts units.
- FR-7: The weakest list ranks units. A cluster
  appears once, as the row of its best member
  (identifier, title, preparation, level of that
  member).
- FR-8: The merged map tile of a pool shows the
  maximum member preparation, not the minimum.
- FR-9: Nothing else changes: the per-piece
  preparation, the plan, the confidence trend,
  the study time, and the tempo signal stay as
  they are. No seed change. No migration. No
  manual data step.

## Success Criteria

- SC-1: A seeded audition with one cluster of 3
  members (preparations 0.6, 0.3, 0.0) and 2
  single pieces (0.8, 0.4) has mean
  (0.6 + 0.8 + 0.4) / 3 = 0.6. The cluster counts
  once, at 0.6.
- SC-2: The same seeded audition has coverage 3
  of 3 when the 0.3 member is the only touched
  member of the cluster and both singles are
  touched. The cluster counts as touched through
  a non-best member.
- SC-3: The weakest list of the seeded audition
  shows the cluster once, as the row of its best
  member.
- SC-4: The merged map tile of a seeded pool shows
  the maximum member preparation.
- SC-5: Live, the Santa Cecilia pin rises from 45
  percent to 51 or 52 percent, and the San Carlo
  pin rises from 40 percent to 50 or 51 percent.
  Coverage on Santa Cecilia moves from 9 of 20 to
  9 of 17. The weakest list of Santa Cecilia no
  longer shows the zero Capriccios. The ranges
  follow from the pinned values of 2026-11-26:
  the removed members sit at 0, so the sums stay
  still while the denominators fall to 17 and 12.
- SC-6: The full local gate is green: format,
  clippy with warnings denied, all tests, coverage
  at or above 90 percent with the exact CI command.

## Decisions

- D-1: Maximum, not mean, per cluster. The owner
  states the rule on 2026-11-26.
- D-2: One shared helper computes the units for
  the 3 mean sites. The duplicated mean logic
  goes away.
- D-3: Coverage counts a cluster as touched
  through any member.
- D-4: The weakest list shows a cluster as the row
  of its best member. The row links to a diary,
  so it keeps the identifier and the title of
  that member.
- D-5: The merged map tile moves from the minimum
  to the maximum member preparation.
- D-6: Rounds play no part. The owner states this
  on 2026-11-26: "ignorando round e simili".
- D-7: The confidence trend, the study time, and
  the tempo signal stay unchanged.
- D-8: A single-member cluster degenerates to the
  piece. A tie goes to the first member in list
  order.
- D-9: No seed change, no migration, no manual
  data step. The rule is computed at read time.

## Questions

- Q-1: Does the maximum rule cover coverage, the
  weakest list, and the map tile, or only the
  headline mean? The spec includes all 4 as one
  rule ("la preparazione"). Ratification confirms
  this reading.
- Q-2: Does the weakest row of a cluster name the
  cluster, or the best member? The spec keeps the
  best member (D-4). Ratification confirms.

## Assumptions

- A-1: Preparation values are the per-piece scores
  from the tile computation (0 to 1), unchanged
  by this cycle.
- A-2: The live pins on 2026-11-26 (Santa
  Cecilia 45 percent, San Carlo 40 percent) are
  the baseline. Sessions logged after the deploy
  move the values, so SC-5 states ranges.
- A-3: The cluster set is the seed state after
  cycle 011: Haydn and concerto on San Carlo,
  concerto on La Fenice, Piatti on Santa Cecilia.
  No cluster membership changes in this cycle.
