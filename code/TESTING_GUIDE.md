```markdown
# Malachite code testing guide
```
This short guide explains where to start when you want to run or extend tests
in the `code/` part of the repository.

## 1. Recommended commands

From the repository root:

- `cargo test` – run the default Rust test suite.
- `cargo test -p <crate-name>` – run tests for a single crate when focusing on a specific module.
- `cargo test -- --nocapture` – see test output when debugging failing cases.

Check the main README and any crate-level `README.md` files for additional, more targeted commands.

## 2. Working with examples

Some features are easier to exercise via example applications and tutorials
linked from `ARCHITECTURE.md` and `docs/tutorials/`. When you touch consensus-critical
code, try to:

- add or update an example that demonstrates the new behaviour, and
- add at least one unit or integration test that guards against regressions.

## 3. Before opening a PR

- Make sure the relevant tests pass locally.
- Prefer small, well-named tests that document behaviour.
- When fixing a bug, include a test that fails before the fix and passes after it.
