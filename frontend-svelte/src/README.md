# Overview of `/frontend/src/`

## Svelte components: `components/`

Svelte components that build the Graphite editor GUI, which are mounted in `App.svelte`. These each contain a Svelte-templated HTML section, an SCSS (Stylus CSS) section, and a script section. The aim is to avoid implementing much editor business logic here, just enough to make things interactive and communicate to the backend where the real business logic should occur.

## I/O managers: `io-managers/`

TypeScript files which manage the input/output of browser APIs and link this functionality with the editor backend. These files subscribe to backend events to execute JS APIs, and in response to these APIs or user interactions, they may call functions into the backend (defined in `/frontend/wasm/editor_api.rs`).

Each I/O manager is a self-contained module where one instance is created in `Editor.svelte` when it's mounted to the DOM at app startup.

During development when HMR (hot-module replacement) occurs, these are also unmounted to clean up after themselves, so they can be mounted again with the updated code. Therefore, any side-effects that these managers cause (e.g. adding event listeners to the page) need a destructor function that cleans them up. The destructor function, when applicable, is returned by the module and automatically called in `Editor.svelte` on unmount.

## State providers: `state-providers/`

TypeScript files which provide reactive state and importable functions to Svelte components. Each module defines a Svelte writable store `const { subscribe, update } = writable({ .. });` and exports the `subscribe` method from the module in the returned object. Other functions may also be defined in the module and exported after `subscribe`, which provide a way for Svelte components to call functions to manipulate the state.

In `Editor.svelte`, an instance of each of these are given to Svelte's [`setContext()`](https://svelte.dev/docs#run-time-svelte-setcontext) function. This allows any component to access the state provider instance using `const exampleStateProvider = getContext<ExampleStateProvider>("exampleStateProvider");`.

## _I/O managers vs. state providers_

_Some state providers, similarly to I/O managers, may subscribe to backend events, call functions from `editor_api.rs` into the backend, and interact with browser APIs and user input. The difference is that state providers are meant to be made available to components via `getContext()` to use them for reactive state, while I/O managers are meant to be self-contained systems that operate for the lifetime of the application and aren't touched by Svelte components._

## Utility functions: `utility-functions/`

TypeScript files which define and `export` individual helper functions for use elsewhere in the codebase. These files should not persist state outside each function.

## WASM communication: `wasm-communication/`

TypeScript files which serve as the JS interface to the WASM bindings for the editor backend.

### WASM editor: `editor.ts`

Instantiates the WASM and editor backend instances. The function `initWasm()` asynchronously constructs and initializes an instance of the WASM bindings JS module provided by wasm-bindgen/wasm-pack. The function `createEditor()` constructs an instance of the editor backend. In theory there could be multiple editor instances sharing the same WASM module instance. The function returns an object where `raw` is the WASM module, `instance` is the editor, and `subscriptions` is the subscription router (described below).

`initWasm()` occurs in `main.ts` right before the Svelte application exists, then `createEditor()` is run in `Editor.svelte` during the Svelte app's creation. Similarly to the state providers described above, the editor is given via `setContext()` so other components can get it via `getContext` and call functions on `editor.raw`, `editor.instance`, or `editor.subscriptions`.

### Message definitions: `messages.ts`

Defines the message formats and data types received from the backend. Since Rust and JS support different styles of data representation, this bridges the gap from Rust into JS land. Messages (and the data contained within) are serialized in Rust by `serde` into JSON, and these definitions are manually kept up-to-date to parallel the message structs and their data types. (However, directives like `#[serde(skip)]` or `#[serde(rename = "someOtherName")]` may cause the TypeScript format to look slightly different from the Rust structs.) These definitions are basically just for the sake of TypeScript to understand the format, although in some cases we may perform data conversion here using translation functions that we can provide.

### Subscription router: `subscription-router.ts`

Associates messages from the backend with subscribers in the frontend, and routes messages to subscriber callbacks. This module provides a `subscribeJsMessage(messageType, callback)` function which JS code throughout the frontend can call to be registered as the exclusive handler for a chosen message type. This file's other exported function, `handleJsMessage(messageType, messageData, wasm, instance)`, is called in `editor.ts` by the associated editor instance when the backend sends a `FrontendMessage`. When this occurs, the subscription router delivers the message to the subscriber for given `messageType` by executing its registered `callback` function. As an argument to the function, it provides the `messageData` payload transformed into its TypeScript-friendly format defined in `messages.ts`.

## Svelte app entry point: `App.svelte`

The entry point for the Svelte application.

## Editor base instance: `Editor.svelte`

This is where we define global CSS style rules, create/destroy the editor instance, construct/destruct the I/O managers, and construct and `setContext()` the state providers.

## JS bundle entry point: `main.ts`

The entry point for the entire project's code bundle. Here we simply initialize the Svelte application with `export default new App({ target: document.body });`.
