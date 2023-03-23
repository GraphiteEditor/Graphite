// This file is the browser's entry point for the JS bundle

// reflect-metadata allows for runtime reflection of types in JavaScript.
// It is needed for class-transformer to work and is imported as a side effect.
// The library replaces the Reflect API on the window to support more features.
import "reflect-metadata";

import App from "@graphite/App.svelte";

document.body.setAttribute("data-app-container", "");

export default new App({ target: document.body });
