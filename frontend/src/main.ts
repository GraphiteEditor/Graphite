// This file is the browser's entry point for the JS bundle

import { mount } from "svelte";

import App from "@graphite/App.svelte";

document.body.setAttribute("data-app-container", "");

export default mount(App, { target: document.body });
