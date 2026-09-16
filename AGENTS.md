# Working on Keyloom

Keyloom is a Rust desktop application built with libcosmic. Follow the
existing code patterns and keep changes focused on the requested work.

Before writing, reviewing, or refactoring Rust code, invoke the `rust-skills`
skill (when available) and follow its guidance.

## Guiding principles

- **Feel at home on COSMIC and work across desktops.** Use libcosmic's
  conventions and integration capabilities to provide a first-party-quality
  experience on COSMIC. Changes and features must also work across all major
  desktop environments. Prefer shared desktop standards, detect optional
  integration capabilities, and provide usable fallbacks when a desktop does
  not support them. Do not make core workflows depend on COSMIC-only services.
  Consider compatibility beyond the development desktop when implementing and
  validating changes, and state any compatibility that could not be verified.
- **Make the interface beautiful and delightful.** Treat visual polish,
  consistency, responsiveness, accessibility, and thoughtful feedback as part
  of feature quality. Follow established libcosmic patterns and use purposeful
  details that help users understand and enjoy the application.
- **Provide power through simplicity.** Make common tasks obvious and easy,
  with sensible defaults and clear language. Use progressive disclosure:
  reveal advanced options when users seek them out or the task requires them,
  keeping everyday workflows approachable while preserving depth and control
  for users who want it. Avoid adding controls, settings, or steps without a
  clear user need.

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

When a change touches what `src/xremap.rs` generates for layers, or
`scripts/xremap-harness/tests_keyloom.rs`, also run
`./scripts/verify-layers-with-xremap.sh`, which CI runs too. It builds the
pinned xremap *source* under `target/xremap-harness/` and runs the generated
documents through xremap's in-process tests. It is safe on a machine that
uses xremap: it never runs an xremap binary, opens an input device, or reads
the installed xremap or its configuration. It needs network access for the
first clone and a few minutes to build. Never verify remapping by creating a
virtual keyboard or starting another xremap: a running remapper can grab
such devices and type into the developer's session.

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
