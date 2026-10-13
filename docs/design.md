# Design

Præludium is a single Go binary. The binary embeds the HTML
templates, the static assets, and the six language catalogs. The
only runtime dependency is one SQLite file, reached through a
pure Go driver on a single connection. The pages are server
rendered. The only client script is the recorder, in vanilla
JavaScript.

## The daily plan

The home page builds the plan for the current day, in the
Europe/Rome timezone. The plan needs two inputs: the availability
of the day and a first confidence rating for every active piece.
Until every active piece carries a rating, the home page shows
the rating list instead of the plan. This is the baseline gate.

The budget of the plan is the time that remains usable today: a
window in the future counts whole, a window in progress counts
to its end, and a finished window counts zero. Minutes already
recorded do not reduce the budget. When no usable time remains,
the plan stays visible with the day budget and drops nothing.

## The score

Each piece with at least one future audition gets a score:

```text
score = base × urgency × need × recency × rest × difficulty
```

- **base** is the sum of the weights of the future auditions
  that include the piece. Past auditions do not count. A piece
  with no future audition leaves the plan.
- **urgency** is `1 + k × max(0, 1 − days / horizon)`. The
  default slope k is 2. The default horizon is 60 days, and the
  horizon grows by 25 percent per difficulty point above 1:
  hard pieces enter the ramp earlier.
- **need** is `1 + (5 − confidence) / 5`. Confidence runs from
  1 to 5 and comes from the most recent rating. A piece never
  rated counts 2.5.
- **recency** is `1 + min(days_since_last_session / 7, cap)`.
  The default cap is 1. The term applies spaced repetition.
- **rest** is 0.5 when the piece was practiced yesterday, and
  1 otherwise. The term spreads the plan across the repertoire.
- **difficulty** is `1 + 0.075 × (difficulty − 1)`.

The coefficients live in the settings table. The user edits the
coefficients in Settings and the weights in Auditions. Each piece
page shows the full breakdown of its score. The musician always
sees why a piece is in the plan today.

## Allocation and overflow

The top eight pieces by score share the budget in proportion to
the score. Each piece gets at least 5 minutes. Minutes round to
the nearest 5.

When the allocated plan exceeds the usable time by more than a
grace of 30 minutes, the application drops the lowest pieces
until the plan fits. A dropped piece gets an auto-skip marker
for the day. A manual skip always wins over the marker of the
plan.

## Preparation

Each piece also carries a preparation score between 0 and 1.
The score combines three parts:

- **confidence**, the mean of the five most recent ratings,
  weight 0.6;
- **volume**, the total minutes over the piece, scaled so that
  a piece is covered at one hour per difficulty point, weight
  0.25;
- **recency**, the sessions of the last 21 days, weight 0.15,
  with three sessions for the maximum.

A piece with no rating uses volume and recency only, with
weights 0.65 and 0.35. After seven idle days the score decays:
the score halves for every two weeks of neglect. The score maps
to five levels, from new to mastered. The preparation map, the
colored dots on the pieces, and the attack list of the readiness
page all read this score.

## Readiness

The readiness page of an audition combines the program coverage,
the study time of the last seven days, the mocks of the audition,
and the attack list: the five pieces with the lowest preparation.
The tempo signal counts a piece only when the piece carries a
target tempo or a recorded take with a tempo. The signal never
speaks about pieces the musician does not measure.
