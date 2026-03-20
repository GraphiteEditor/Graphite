# Overview of `/frontend/src/`

## Svelte components: `components/`

Svelte components that build the Graphite editor GUI. These each contain a TypeScript section, a Svelte-templated HTML template section, and an SCSS stylesheet section. The aim is to avoid implementing much editor business logic here, just enough to make things interactive and communicate to the backend where the real business logic should occur.

## Managers: `managers/`

TypeScript files which manage the input/output of browser APIs and link this functionality with the editor backend. These files subscribe to backend messages to execute JS APIs, and in response to these APIs or user interactions, they may call functions into the backend (defined in `/frontend/wasm/editor_api.rs`).

Each manager module exports a factory function (e.g. `createClipboardManager(editor)`) that sets up message subscriptions and returns a `{ destroy }` object. In `Editor.svelte`, each manager is created at startup and its `destroy()` method is called on unmount to clean up subscriptions and side-effects (e.g. event listeners). Managers use self-accepting HMR to tear down and re-create with updated code during development.

## Stores: `stores/`

TypeScript files which provide reactive state to Svelte components. Each module persists a Svelte writable store at module level (surviving HMR via `import.meta.hot.data`) and exports a factory function (e.g. `createDialogStore(editor)`) that sets up backend message subscriptions and returns an object containing the store's `subscribe` method, any action methods for components to call, and a `destroy` method.

In `Editor.svelte`, each store is created and passed to Svelte's `setContext()`. Components access stores via `getContext<DialogStore>("dialog")` and use the `subscribe` method for reactive state and action methods (like `createCrashDialog()`) to trigger state changes.

## *Managers vs. stores*

*Both managers and stores subscribe to backend messages and may interact with browser APIs. The difference is that stores expose reactive state to components via `setContext()`/`getContext()`, while managers are self-contained systems that operate for the lifetime of the application and aren't accessed by Svelte components.*

## Utility functions: `utility-functions/`

TypeScript files which define and `export` individual helper functions for use elsewhere in the codebase. These files should not persist state outside each function.

## Wasm editor: `editor.ts`

Instantiates the Wasm and editor backend instances. The function `initWasm()` asynchronously constructs and initializes an instance of the Wasm bindings JS module provided by wasm-bindgen/wasm-pack. The function `createEditor()` constructs an instance of the editor backend. In theory there could be multiple editor instances sharing the same Wasm module instance. The function returns an object where `raw` is the Wasm memory, `handle` provides access to callable backend functions, and `subscriptions` is the subscriptions router (described below).

`initWasm()` occurs in `main.ts` right before the Svelte application is mounted, then `createEditor()` is run in `Editor.svelte` during the Svelte app's creation. Similarly to the stores described above, the editor is given via `setContext()` so other components can get it via `getContext` and call functions on `editor.handle` or `editor.subscriptions`.

## Subscriptions router: `subscriptions-router.ts`

Associates messages from the backend with subscribers in the frontend, and routes messages to subscriber callbacks. This module provides a `subscribeFrontendMessage(messageType, callback)` function which JS code throughout the frontend can call to be registered as the exclusive handler for a chosen message type. The router's other function, `handleFrontendMessage(messageType, messageData)`, is called via the callback passed to `EditorHandle.create()` in `editor.ts` when the backend sends a `FrontendMessage`. When this occurs, the subscriptions router delivers the message to the subscriber by executing its registered `callback` function.

## Svelte app entry point: `App.svelte`

The entry point for the Svelte application.

## Editor base instance: `Editor.svelte`

This is where we define global CSS style rules, construct all stores and managers with the editor instance, set store contexts for component access, and clean up all `destroy()` methods on unmount.

## Global type augmentations: `global.d.ts`

Extends built-in browser type definitions using TypeScript's interface merging. This includes Graphite's custom properties on the `window` object, custom events like `pointerlockmove`, and experimental browser APIs not yet in TypeScript's standard library. New custom events or non-standard browser APIs used by the frontend should be declared here.

## JS bundle entry point: `main.ts`

The entry point for the entire project's code bundle. Here we simply mount the Svelte application with `export default mount(App, { target: document.body });`.
