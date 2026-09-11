# Working on Keyloom

Keyloom is a Rust desktop application built with libcosmic. Follow the
existing code patterns and keep changes focused on the requested work.

## Keep documentation current

For every change, review both:

- `docs/Functionality_TODO.md`: mark completed work, record remaining work,
  and update any affected limitations or follow-up tasks.
- `docs/Current_Features.md`: describe the behavior that is actually
  implemented, including relevant limitations. Do not present planned or
  partially implemented behavior as complete.

Update affected documentation alongside the implementation and include it
in the same commit. Keep the two documents consistent. If neither document
needs an update, explain why in the final response rather than making
unnecessary edits.

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
