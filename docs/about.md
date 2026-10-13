# About

Præludium is a daily practice diary for a musician who prepares for
orchestra auditions. The application records the auditions, the pieces
each audition requires, and the rounds each audition runs. Every day
the application builds a practice plan from the free time of the day
and from an explainable score. The musician follows the plan, records
each session with minutes and a confidence rating, and watches the
preparation grow in the diary.

The name is the Latin word for a prelude: the short piece a musician
plays before the real performance.

## The daily routine

1. In the morning, declare the free and busy slots of the day.
2. Rate every new piece once. The plan starts when no piece waits for a first rating.
3. Follow the plan. Each card shows the piece, the target minutes, and the score factors.
4. Record each session with minutes and confidence. Minutes clear the skip markers.
5. Skip or postpone a piece when the day changes. The diary keeps the marker.
6. In the evening, read the diary: minutes, pieces done, and the pieces that stayed out.

## Features

- **Pieces and auditions.** A piece carries a composer, a work, a
  movement, a difficulty from 1 to 5, and an optional target tempo.
  An audition carries a date, a weight, and its rounds. A piece
  joins one or more auditions, and a round when the audition
  publishes its program.
- **The plan.** The score ranks the pieces every day. The plan
  takes the top eight and shares the usable time of the day. When
  the plan exceeds the time that remains, the application drops
  the lowest pieces and records an auto-skip. A grace of 30 minutes
  protects the plan from small overruns.
- **The diary.** One page per day lists the sessions of the day.
  The statistics show the distribution of session lengths, the
  weekly hours with a mean and percentiles, the time per audition,
  the coverage of the next 14 days, the weekly confidence, and the
  practice streak.
- **Recordings.** The browser records a take for a piece, with a
  level meter and a preview. A recording gets a public link with an
  unguessable token. Only the link is public; the application
  itself stays behind authentication.
- **Simulated auditions.** A mock runs the rounds of an audition
  in order. The musician records or scores each piece, and the
  mock keeps the mean of the round.
- **Readiness.** Each audition has a readiness page: program
  coverage, study time, the tempo signal for the pieces that
  carry a target, the mock history, and the five weakest pieces
  as the attack list.
- **Preparation map.** A grid of pieces and auditions shows the
  preparation level of every combination at a glance.
- **Six languages.** The interface speaks English, Italian,
  Spanish, French, German, and Portuguese. The language follows
  the cookie, then the browser preference, then English.

## Data and privacy

All data lives in one SQLite file. The application has no account
system and no telemetry. In production, a reverse proxy enforces
authentication in front of the application.
