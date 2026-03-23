# Overview of `/frontend/src/`

## Svelte components: `components/`

Svelte components that build the Graphite editor GUI from layouts, panels, widgets, and menus. These each contain a TypeScript section, a Svelte-templated HTML template section, and an SCSS stylesheet section. The aim is to avoid implementing much editor business logic here, just enough to make things interactive and communicate to the backend where the real business logic should occur.

## Managers: `managers/`

TypeScript files, constructed by the editor frontend, which manage the input/output of browser APIs and link this functionality with the editor backend. These files subscribe to frontend messages to execute JS APIs, and in response to these APIs or user interactions, they may call functions in the backend (defined in `/frontend/wrapper/editor_wrapper.rs`).

Each manager module stores its dependencies (like `subscriptionsRouter` and `editorWrapper`) in module-level variables and exports a `create*()` and `destroy*()` function pair. `Editor.svelte` calls each `create*()` constructor in its `onMount` and calls each `destroy*()` in its `onDestroy`. Managers replace themselves during HMR updates if they are modified live during development.

## Stores: `stores/`

TypeScript files, constructed by the editor frontend, which provide reactive state to Svelte components. Each module persists a Svelte writable store at module level (surviving HMR via `import.meta.hot.data`) and exports a `create*()` function that sets up frontend message subscriptions and returns `{ subscribe }` (the shape required by Svelte's custom store contract). A corresponding `destroy*()` function is also exported. Some stores also export standalone action functions (like `createCrashDialog()` or `toggleFullscreen()`) as module-level exports.

In `Editor.svelte`, each store is created synchronously during component initialization (not in `onMount`, since child components need `getContext` access during their own initialization) and passed to Svelte's `setContext()`. Components access stores via calls like `getContext<DialogStore>("dialog")`. Unlike managers, stores do not replace themselves during HMR; instead, `Editor.svelte` is remounted to replace them entirely.

## *Managers vs. stores*

*Both managers and stores subscribe to frontend messages and may interact with browser APIs. The difference is that stores expose reactive state to components via `setContext()`/`getContext()`, while managers are self-contained systems that operate for the lifetime of the application and aren't accessed by Svelte components.*

## Utility functions: `utility-functions/`

TypeScript files which define and `export` individual helper functions for use elsewhere in the codebase. These files should not persist state outside each function.

## Subscriptions router: `subscriptions-router.ts`

Associates messages from the backend with subscribers in the frontend, and routes messages to subscriber callbacks. This module provides a `subscribeFrontendMessage(messageType, callback)` function which JS code throughout the frontend can call to be registered as the exclusive handler for a chosen message type. The router's other function, `handleFrontendMessage(messageType, messageData)`, is called via the callback passed to `EditorWrapper.create()` in `App.svelte` when the backend sends a `FrontendMessage`. When this occurs, the subscriptions router delivers the message to the subscriber by executing its registered `callback` function.

## Svelte app entry point: `App.svelte`

The entry point for the Svelte application. Initializes and creates the `EditorWrapper` backend instance and the subscriptions router and renders `Editor.svelte` once both are ready. The `EditorWrapper` is the wasm-bindgen interface to the Rust editor backend (defined in `/frontend/wrapper/editor_wrapper.rs`), providing access to callable backend functions. Both the editor and subscriptions router are passed as props to `Editor.svelte` and set as Svelte contexts for use throughout the component tree.

## Editor base instance: `Editor.svelte`

This is where we define global CSS style rules, construct all stores and managers, set store contexts for component access, and call each module's `destroy*()` function on unmount (on HMR during development).

## Global type augmentations: `global.d.ts`

Extends built-in browser type definitions using TypeScript's interface merging. This includes Graphite's custom properties on the `window` object, custom events like `pointerlockmove`, and experimental browser APIs not yet in TypeScript's standard library. New custom events or non-standard browser APIs used by the frontend should be declared here.

## JS bundle entry point: `main.ts`

The entry point for the entire project's code bundle. Mounts the Svelte application with `export default mount(App, { target: document.body })`.
