// This file is the browser's entry point for the JS bundle

import { mount, unmount } from "svelte";
import App from "/src/App.svelte";

document.body.setAttribute("data-app-container", "");

const app = mount(App, { target: document.body });

// Ensure the old component tree is properly torn down during HMR so all onDestroy hooks fire (which clean up IO managers, state providers, etc.)
import.meta.hot?.dispose(() => unmount(app));

export default app;
