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
- [Node.js](https://nodejs.org/) (the latest LTS version)
- [Rust](https://www.rust-lang.org/) (the latest stable release)
- [Git](https://git-scm.com/) (any recent version)

Next, install the dependencies required for development builds:

```sh
cargo install cargo-watch
cargo install wasm-pack
cargo install -f wasm-bindgen-cli@0.2.100
```

Regarding the last one: you'll likely get faster build times if you manually install that specific version of `wasm-bindgen-cli`. It is supposed to be installed automatically but a version mismatch causes it to reinstall every single recompilation. It may need to be manually updated periodically to match the version of the `wasm-bindgen` dependency in [`Cargo.toml`](https://github.com/GraphiteEditor/Graphite/blob/master/Cargo.toml).

## Repository

Clone the project to a convenient location:

```sh
git clone https://github.com/GraphiteEditor/Graphite.git
cd Graphite
```

## Development builds

From either the `/` (root) or `/frontend` directories, you can run the project by executing:

```sh
npm start
```

This spins up the dev server at <http://localhost:8080> with a file watcher that performs hot reloading of the web page. You should be able to start the server, edit and save web and Rust code, and shut it down by double pressing <kbd>Ctrl</kbd><kbd>C</kbd>. You sometimes may need to reload the browser's page if hot reloading didn't behave right— always refresh when Rust recompiles.

This method compiles Graphite code in debug mode which includes debug symbols for viewing function names in stack traces. But be aware, it runs slower and the Wasm binary is much larger. (Having your browser's developer tools open will also significantly impact performance in both debug and release builds, so it's best to close that when not in use.)

To run the dev server in optimized mode, which is faster and produces a smaller Wasm binary:

```sh
# Includes debug symbols
npm run profiling

# Excludes (most) debug symbols, used in release builds
npm run production
```

<details>
<summary>Production build instructions: click here</summary>

You'll rarely need to compile your own production builds because our CI/CD system takes care of deployments. However, you can compile a production build with full optimizations by first installing the additional `cargo-about` dev dependency:

```sh
cargo install cargo-about
```

And then running:

```sh
npm run build
```

This produces the `/frontend/dist` directory containing the static site files that must be served by your own web server.

</details>

## Development tooling

We provide default configurations for VS Code users. When you open the project, watch for a prompt to install the project's [suggested extensions](https://github.com/GraphiteEditor/Graphite/blob/master/.vscode/extensions.json). They will provide helpful web and Rust tooling. If you use a different IDE, you won't get default configurations for the project out of the box, so please remember to format your code and check CI for errors.

### Checking, linting, and formatting

While developing Rust code, `cargo check`, `cargo clippy`, and `cargo fmt` terminal commands may be run from the root directory. For web code, formatting issues can be linted using `npm run lint` (to view) and `npm run lint-fix` (to fix) if run from the `/frontend` directory.

If you don't use VS Code and its format-on-save feature, please remember to format before committing or [set up a `pre-commit` hook](https://githooks.com/) to do that automatically. Disabling VS Code's *Auto Save* files feature is recommended to ensure you actually save (and thus format) file changes.
