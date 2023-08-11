+++
title = "Editor and tooling"

[extra]
order = 1 # Page number after chapter intro
+++

We provide default configurations for VS Code users. When you open the project, watch for a prompt to install the project's suggested extensions. They will provide helpful web and Rust tooling. If you use a different IDE, you won't get default configurations for the project out of the box, so please remember to format your code and check CI for errors.

## Checking, linting, and formatting

While developing Rust code, `cargo check`, `cargo clippy`, and `cargo fmt` terminal commands may be run from the root directory. For web code, `npm run lint` and `npm run lint-no-fix` can be used from the `/frontend` directory to fix or view formatting issues.

If you don't use VS Code and its format-on-save feature, please remember to format before committing or consider [setting up a `pre-commit` hook](https://githooks.com/) to do that automatically. Disabling VS Code's *Auto Save* files feature is recommended to ensure you actually save (and thus format) file changes.
