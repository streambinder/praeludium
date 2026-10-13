# The daily plan formula

For each piece, on each day:

```text
score = base × urgency × need × recovery (× 1.5 if postponed from yesterday)
```

- **base** is the sum of the weights of the future auditions that include
  the piece. Past auditions and archived auditions do not count.
  A piece with no future audition leaves the plan.
- **urgency** is 1 + k × max(0, 1 − days_to_the_audition / horizon),
  with k = 2 and horizon = 60 days by default.
- **need** is 1 + (5 − confidence) / 5. Confidence runs from 1 to 5 and
  comes from the last check-in. The value is 2.5 when the piece has
  never been rated.
- **recovery** is 1 + min(days_since_the_last_session / 7, cap),
  with cap = 1 by default. This term applies spaced repetition.

The top 8 pieces by score share the budget of the day in proportion
to the score. The budget comes from the free time slots declared in
the morning. Each piece gets at least 5 minutes. Minutes round to 5.

Each piece page shows the full computation.
The musician always sees why a piece is in the plan today.

Change k, horizon and cap in Settings. Change the audition weights
in Auditions.
