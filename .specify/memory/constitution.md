# Præludium Constitution

**Status**: Ratified by the repository owner on 2026-10-20.

The canonical standard for this repository is the unciae golden standard
(`streambinder/unciae`, file `AGENTS.md`). This constitution restates the
rules of that standard that bind this repository, in the form that the
Spec Kit workflow checks. When a rule here and the canonical standard
disagree, the canonical standard wins and this document gets amended.

## Core Principles

### I. One Repository, One Deployable Unit

The repository builds one application and one container image. The root
stays minimal: a new root entry needs a justification. Configuration
files live in their conventional locations, never ad hoc.
(unciae §1, §2)

### II. CI Is the Source of Truth

A change is done when the continuous integration pipeline is green.
Local checks mirror the pipeline; local checks never replace the
pipeline. Do not push on red. (unciae §1, §17, §18)

### III. One Linting System

super-linter is the linter. Its configuration files live only under
`.github/linters/`. No linter configuration sits at the repository
root, not even as a symlink. Format code with the canonical formatter
of the language before every push: for Rust, `cargo fmt` and
`cargo clippy`. (unciae §3)

### IV. Tests and the Coverage Gate

Unit tests live next to the code that the tests cover, in the layout
that the toolchain dictates. Integration tests live separately. CI
runs the tests on every push. A failing test blocks the merge. CI
measures coverage on every push. The minimum line coverage is 90%.
Coverage below 90% blocks the merge. Non-test code never changes to
satisfy a test; the test changes instead. (unciae §15)

### V. Minimal Owned Surface

The repository owns its product logic and nothing else. For every other
functionality, first simplify the problem away, then delegate to a
specialized library with an established reputation. Pin exact
dependency versions where possible. (unciae §16, §16.1)

### VI. Commits and Pull Requests

Commits follow Conventional Commits: one logical change per commit,
signed, with no artificial-intelligence attribution trailers. Commit
messages and pull request descriptions never name a verification
subject; describe the class of the subject and the outcome instead.
The owner merges. (unciae §7, §19, §24)

### VII. Containers and Releases

Build for linux/arm64 only. Pin every action and base image. Publish
never runs on a pull request. (unciae §5, §10, §11)

### VIII. Writing and Consistency

Technical writing follows ASD-STE100: short sentences, one term for
one concept, active voice. A change follows the conventions of the
file that the change touches. Do not reformat parts that the change
does not touch. (unciae §22, §23)

## Project Constraints

These constraints are decisions of the repository owner for this
project. Each constraint stands until the owner replaces it with a
recorded decision.

### Production Care

The application serves a real user every day. Before every deploy,
take a database backup and verify its snapshot. After every deploy,
verify the recreated container and the real live content. Never write
on production data to test. Never change production without a local
quality check first.

### Data Compatibility

The production SQLite file is a contract. The application must open
the existing file without data loss. A schema change needs a recorded
decision and a migration path before implementation.

### Specification Cycle

Every bounded change follows the Spec Kit cycle: specification,
clarification, plan, tasks, analysis, implementation, convergence.
The specification describes the next change, never the whole system
retroactively. Completed feature directories stay as immutable
historical records. A significant decision goes to the decision
register under `.specify/memory/decisions/`, in MADR format, one file
per decision. An accepted decision is reused; a replaced decision
stays in the register and names its successor.

## Working Agreements

This section restates the operative form of the principles, so the
repository stays self-contained for the points that the golden
standard states in full. A new session applies these agreements
without further sources.

### Branches and commits

- A feature branch carries the name `feat/<short-name>`.
- A commit follows Conventional Commits. The subject holds at most
  72 characters, uses the imperative, carries no terminal period,
  and contains none of the characters `()[]"'`/,:;`.
- A commit body holds one sentence of at most 100 characters and
  ends with a period. The body states why, not what.
- One logical change makes one commit.
- A fix for a red feature branch amends the failing commit. The
  push uses force with an explicit lease that names the remote
  commit. Nobody amends or force-pushes the default branch.

### Text and formatting

- Prettier formats every authored Markdown file. The author runs
  prettier, in the major version the lint image bundles, on each
  authored file before the push.
- Codespell checks every text file in CI. Authored text passes
  codespell before the push.

### Test fixtures

- A test that needs an application instance builds the instance in
  memory: an `App` value with a mutex over `open_memory()` and a
  temporary data directory. The test modules of `src/db.rs`,
  `src/readiness.rs`, and `src/mocks.rs` hold the pattern.

## Governance

The owner ratifies this constitution and every amendment. The agent
drafts amendments and records the ratified text. Every pull request
review checks compliance with this constitution. A rule that proves
wrong is amended through a recorded decision; the rule is never
silently ignored.

**Version**: 1.1.0 | **Ratified**: 2026-10-20 | **Last Amended**: 2026-10-20
