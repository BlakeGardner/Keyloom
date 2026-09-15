# Working on Keyloom

Keyloom is a Rust desktop application built with libcosmic. Follow the
existing code patterns and keep changes focused on the requested work.

Before writing, reviewing, or refactoring Rust code, invoke the `rust-skills`
skill (when available) and follow its guidance.

## Keep product documentation current

Treat the product docs as a durable description of Keyloom, not a changelog
for every implementation detail. Update them when a change adds, removes, or
materially changes a real user-facing capability, workflow, supported setup,
data or persistence behavior, backend integration, important limitation, or
roadmap commitment.

- `docs/Functionality_TODO.md`: track substantive feature work, limitations,
  and meaningful follow-up tasks. Do not add entries for routine polish.
- `docs/Current_Features.md`: describe implemented capabilities and relevant
  limitations at the product level. Keep it accurate, but do not catalog every
  control behavior or presentation detail.

Documentation updates are generally unnecessary for animations, spacing,
styling, copy edits, small usability refinements, internal refactors,
test-only changes, or bug fixes that merely restore already documented
behavior. Update the docs only when one of those changes materially alters a
documented workflow or limitation.

When a documentation update is warranted, update the affected document(s)
alongside the implementation and keep them consistent. Do not edit both files
mechanically when only one is relevant. If no product documentation is needed,
no documentation-only change or routine justification in the final response
is required.

## Format and validate

Add or update meaningful tests when behavior changes. Before completing a
Rust code change, and before any commit, run these commands from the
repository root in order:

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --verbose --locked
```

Fix failures before committing. Do not bypass checks, weaken tests, or
suppress warnings merely to obtain a passing result. If validation is
blocked by the environment, report the exact blocker and do not claim the
checks passed or proceed with a commit.

Review formatter changes before staging. After further code edits, repeat
the affected checks so the final code is covered by validation. Keep these
commands aligned with CI when the validation workflow changes.

For documentation-only work that is not being committed, review the
documentation diff and run `git diff --check`; Rust checks are not needed.

## Finish and commit

- Review the final diff for unrelated changes and accidental files.
- Run `git diff --check` before committing.
- Preserve existing user changes; stage only files relevant to the task.
- Commit only when the user requests it.
- In the final response, summarize the change, documentation updates (or
  why none were needed), and validation results, including any checks that
  could not run.
