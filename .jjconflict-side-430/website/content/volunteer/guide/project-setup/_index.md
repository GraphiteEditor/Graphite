+++
title = "Project setup"
template = "book.html"
page_template = "book.html"

[extra]
order = 1 # Chapter number
+++

To begin working with the Graphite codebase, you will need to set up the project to build and run on your local machine. Development usually involves running the dev server which watches for changes to frontend (web) and backend (Rust) code and automatically recompiles and reloads the Graphite editor in your browser.

## Dependencies

Graphite is built with Rust and web technologies, which means you will need to install:
- [Rust](https://www.rust-lang.org/) (the latest stable release)
- [Node.js](https://nodejs.org/) (the latest LTS version)
- [Git](https://git-scm.com/) (any recent version)

## Repository

Clone the project to a convenient location:

```sh
git clone https://github.com/GraphiteEditor/Graphite.git
```

## Development builds

In the project directory, run the build system by executing:

```sh
cargo run
```

This will check for the required system dependency versions, help you install any that are missing, and spin up the dev server at <http://localhost:8080> serving the web app with debug optimizations. A file watcher hot-reloads the web app when you save a code file. Shut down the dev server by double pressing <kbd>Ctrl</kbd><kbd>C</kbd>.

For additional build commands, see:

```sh
cargo run help
```

For example, if you must proxy the dev server connection over a slow network where the >100 MB unoptimized binary size would pose an issue, you may need to run with release optimizations using `cargo run release`.

## Development tooling

We provide default configurations for VS Code users. When you open the project, watch for a prompt to install the project's [suggested extensions](https://github.com/GraphiteEditor/Graphite/blob/master/.vscode/extensions.json). They will provide helpful web and Rust tooling. If you use a different IDE, you won't get default configurations for the project out of the box, so please remember to format your code and check CI for errors.

### Checking, linting, and formatting

While developing Rust code: `cargo check`, `cargo clippy`, and `cargo fmt` terminal commands may be run from the root directory. For web code: errors, code quality lints, and formatting issues can be checked using `npm run check` (to view them) and `npm run fix` (to fix them) if run from the `/frontend` directory.

If you don't use VS Code and its format-on-save feature, please remember to format before committing or [set up a `pre-commit` hook](https://githooks.com/) to do that automatically. Disabling VS Code's *Auto Save* files feature is recommended to ensure you actually save (and thus format) file changes. CI will enforce that everything passes these checks before your PR can be merged.
