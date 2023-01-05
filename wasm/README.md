# Overview of `/frontend/wasm/`

## How to build

First, you need wasm-pack installed to build this.
```
cargo install wasm-pack
```
Then, run the build script `./build.mjs`.

## Watch command

Need: `cargo install cargo-watch`

```
# Unix
NODE_ENV=development cargo watch -s ./build.mjs

# Windows
# set envvar (TK)
cargo watch -s "node build.mjs"
```

## WASM wrapper API: `src/editor_api.rs`
Provides bindings for JS to call functions defined in this file, and for FrontendMessages to be sent from Rust back to JS in the form of a callback to the subscription router. This WASM wrapper crate, since it's written in Rust, is able to call into the Editor crate's codebase and send FrontendMessages back to JS.


## WASM wrapper helper code: `src/helpers.rs`
Assorted function and struct definitions used in the WASM wrapper.

## WASM wrapper initialization: `src/lib.rs`
Entry point for the Rust entire codebase in the WASM environment. Initializes the WASM module and persistent storage for editor and WASM wrapper instances.

## WASM wrapper tests: `tests/`
We currently have no WASM wrapper tests, but this is where they would go.
