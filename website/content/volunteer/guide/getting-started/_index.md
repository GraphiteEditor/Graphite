+++
title = "Getting started"
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
```

You'll likely get faster build times if you manually install this specific version of `wasm-bindgen-cli`. It is supposed to be installed automatically but a version mismatch causes it to reinstall every single recompilation. It may need to be manually updated periodically to match the version of the `wasm-bindgen` dependency in [`Cargo.toml`](https://github.com/GraphiteEditor/Graphite/blob/master/Cargo.toml):

```sh
cargo install -f wasm-bindgen-cli@0.2.92
```

On Linux, you may need to install this set of additional packages, for the Tauri parts of our tech stack to work, if you run into issues:

<br />
<details>
<summary>Click to view</summary>

```sh
# On Debian-based (Ubuntu, Mint, etc.) distributions:
sudo apt install libgtk-3-dev libsoup2.4-dev libjavascriptcoregtk-4.0-dev libwebkit2gtk-4.0-dev

# On Fedora-based (RHEL, CentOS, etc.) distributions:
sudo dnf install gtk3-devel libsoup-devel javascriptcoregtk4.0-devel webkit2gtk4.0-devel

# On OpenSUSE-based distributions:
sudo zypper install gtk3-devel libsoup-devel webkit2gtk3-soup2-devel

# On NixOS or when using the Nix package manager:
nix-shell
```

</details>

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

This spins up the dev server at <http://localhost:8080> with a file watcher that performs hot reloading of the web page. You should be able to start the server, edit and save web and Rust code, and shut it down by double pressing <kbd>Ctrl</kbd><kbd>C</kbd>. You sometimes may need to reload the browser's page if hot reloading didn't behave right.

This method compiles Graphite code in debug mode which includes debug symbols for viewing function names in stack traces. But be aware, it runs slower and the Wasm binary is much larger. Having your browser's developer tools open will also significantly impact performance in both debug and release builds, so it's best to close that when not in use.

## Production builds

You'll rarely need to compile your own production builds because our CI/CD system takes care of deployments. However, you can compile a production build with full optimizations by first installing the additional `cargo-about` dev dependency:

```sh
cargo install cargo-about
```

And then running:

```sh
npm run build
```

This produces the `/frontend/dist` directory containing the static site files that must be served by your own web server.
