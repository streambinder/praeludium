# Tasks: Audition Enrollment

**Input**: `specs/006-audition-enrollment/spec.md`, `specs/006-audition-enrollment/plan.md`

**Prerequisites**: The owner ratifies the specification.
Decisions D-1 to D-7 stand resolved in the specification.

## Phase A: Storage and reads

- [x] T001 Create the `users` and `enrollments`
      tables in the migration of `src/db.rs`, with a
      unique (user, audition) pair, foreign keys, and
      cascade on delete.
- [x] T002 Seed the one user (ID 1, Agnese) and
      backfill one enrollments row per existing
      audition for that user.
- [x] T003 Carry the enrolled value on the `Audition`
      record through `scan_audition` and every
      audition read (`list_auditions`,
      `piece_auditions`, `audition_or_404`), joining
      enrollments for the current user ID carried by
      `App`. A new audition inserts no enrollments
      row.

## Phase B: Plan gate

- [x] T004 Skip a non-enrolled audition in
      `upcoming_auditions` in `src/handlers.rs`, next
      to the archived and past checks.
- [x] T005 Skip a non-enrolled audition in
      `audition_readiness` in `src/handlers.rs`.

## Phase C: Toggle and rendering

- [x] T006 Add one POST route and handler that
      inserts the enrollments row on enroll and
      deletes it on unenroll, then redirects back to
      the detail page.
- [x] T007 Render the enroll/unenroll toggle form on
      `templates/audition_detail.html` only.
- [x] T008 Show the enrollment state with a badge in
      `templates/auditions.html` and
      `templates/audition_detail.html`.

## Phase D: Locales

- [x] T009 Add the enroll label, the unenroll label,
      and the enrolled badge to the six locale files,
      with the values of spec FR-8.

## Phase E: Tests, gate, delivery

- [x] T010 Extend the `upcoming_filters_and_nearest`
      unit test with a non-enrolled audition.
- [x] T011 Add integration tests: toggle round trip,
      plan exclusion of a piece whose auditions are
      all non-enrolled, diary logging on such a piece.
- [x] T012 Run the local gate and open the
      implementation PR only after the owner ratifies
      the spec.
- [x] T013 Deploy with the standard sequence, back up
      the database, run the data step of spec FR-10
      through the toggle (delete the three enrollments
      rows), and verify live: the plan holds enrolled
      pieces only, the Roma-only pieces are absent
      from the plan with their log form intact, and
      the diary still lists the existing entries.
