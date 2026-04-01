# Overview of `/frontend/wrapper/`

## Wasm wrapper API: `src/editor_wrapper.rs`

Provides bindings for JS to call functions defined in this file, and for `FrontendMessage`s to be sent from Rust back to JS in the form of a callback to the subscription router. This Wasm wrapper crate, since it's written in Rust, is able to call into the Editor crate's codebase and send `FrontendMessage`s back to JS.

## Wasm wrapper helper code: `src/helpers.rs`

Assorted function and struct definitions used in the Wasm wrapper.

## Native communication: `src/native_communication.rs`

Handles receiving serialized `FrontendMessage`s from the native desktop app via an `ArrayBuffer` and forwarding them to JS through the editor wrapper.

## Wasm wrapper initialization: `src/lib.rs`

Entry point for the Rust codebase in the Wasm environment. Sets up panic hooks and logging, and defines thread-local storage for the editor instance, editor wrapper, message buffer, and panic dialog callback.
