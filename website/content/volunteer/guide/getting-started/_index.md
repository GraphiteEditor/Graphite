+++
title = "Getting started"
template = "book.html"
page_template = "book.html"

[extra]
order = 1 # Chapter number
+++

Graphite is built with Rust and web technologies. Install the latest LTS version of [Node.js](https://nodejs.org/) and stable release of [Rust](https://www.rust-lang.org/), as well as [Git](https://git-scm.com/).

## Installing

Clone the project:
```sh
git clone https://github.com/GraphiteEditor/Graphite.git
```

On Debian-based Linux distributions, you may need to install the following packages:
```sh
sudo apt install libgtk-3-dev libsoup2.4-dev libjavascriptcoregtk-4.0-dev libwebkit2gtk-4.0-dev
```

Then install the required Node.js packages:
```sh
cd frontend
npm install
```

You only need to explicitly install Node.js dependencies. Rust's cargo dependencies will be installed automatically on your first build. One dependency in the build chain, `wasm-pack`, will be installed automatically on your system when the Node.js packages are installing. (If you prefer to install this manually, get it from the [wasm-pack website](https://rustwasm.github.io/wasm-pack/), then install your npm dependencies with `npm install --no-optional` instead.)

One tool in the Rust ecosystem does need to be installed:

```sh
cargo install cargo-watch
```

That's it! Now, to run the project while developing, just execute:
```sh
npm start
```

This spins up the dev server at <http://localhost:8080> with a file watcher that performs hot reloading of the web page. You should be able to start the server, edit and save web and Rust code, and rarely have to kill the server (by hitting <kbd>Ctrl</kbd><kbd>C</kbd> twice). You sometimes may need to reload the browser's web page if the hot reloading didn't behave perfectly. This method compiles Graphite code in debug mode which includes debug symbols for viewing function names in stack traces. But be aware, it runs slower and takes more memory.

## Production builds

You'll rarely ever need to do this, but to compile a production build with full optimizations:
```sh
cargo install cargo-about
npm run build
```

This produces the `/frontend/dist` directory containing the static site files that must be served by your own web server.
