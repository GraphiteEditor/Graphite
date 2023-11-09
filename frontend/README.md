# Overview of `/frontend/`

The Graphite frontend is a web app that provides the presentation for the editor. It displays the GUI based on state from the backend and provides users with interactive widgets that send updates to the backend, which is the source of truth for state information. The frontend is built out of reactive components using the [Svelte](https://svelte.dev/) framework. The backend is written in Rust and compiled to WebAssembly (WASM) to be run in the browser alongside the JS code.

For lack of other options, the frontend is currently written as a web app. Maintaining web compatibility will always be a requirement, but the long-term plan is to port this code to a Rust-based native GUI framework, either written by the Rust community or created by our project if necessary. As a medium-term compromise, we may wrap the web-based frontend in a desktop webview windowing solution like Electron (probably not) or [Tauri](https://tauri.studio/) (probably).

## Bundled assets: `assets/`

Icons and images that are used in components and embedded into the application bundle by the build system.

## Public assets: `public/`

Static content like favicons that are copied directly into the root of the build output by the build system.

## Svelte/TypeScript source: `src/`

Source code for the web app in the form of Svelte components and [TypeScript](https://www.typescriptlang.org/) files.

## WebAssembly wrapper: `wasm/`

Wraps the editor backend codebase (`/editor`) and provides a JS-centric API for the web app to use unburdened by Rust's complex data types that are incompatible with JS data types. Bindings (JS functions that call into the WASM module) are provided by [wasm-bindgen](https://rustwasm.github.io/docs/wasm-bindgen/) in concert with [wasm-pack](https://github.com/rustwasm/wasm-pack).

## ESLint configurations: `.eslintrc.js`

[ESLint](https://eslint.org/) is the tool which enforces style rules on the JS, TS, and Svelte files in our frontend codebase. As it is set up in this config file, ESLint will complain about bad practices and often help reformat code automatically when (in VS Code) the file is saved or `npm run lint` is executed. (If you don't use VS Code, remember to run this command before committing!) This config file for ESLint sets our style preferences and configures our usage of extensions/plugins for Svelte support, [Airbnb](https://github.com/airbnb/javascript)'s popular catalog of sane defaults, and [Prettier](https://prettier.io/)'s role as a code formatter.

## npm ecosystem packages: `package.json`

While we don't use Node.js as a JS-based server, we do have to rely on its wide ecosystem of packages for our build system toolchain. If you're just getting started, make sure to install the latest LTS copy of Node.js and then run `cd frontend && npm install` to install these packages on your system. Our project's philosophy on third-party packages is to keep our dependency tree as light as possible, so adding anything new to our `package.json` should have overwhelming justification. Most of the packages are just development tooling (TypeScript, Vite, ESLint, Prettier, wasm-pack, and [Sass](https://sass-lang.com/)) that run in your console during the build process.

## npm package installed versions: `package-lock.json`

Specifies the exact versions of packages installed in the npm dependency tree. While `package.json` specifies which packages to install and their minimum/maximum acceptable version numbers, `package-lock.json` represents the exact versions of each dependency and sub-dependency. Running `npm install` will grab these exact versions to ensure you are using the same packages as everyone else working on Graphite. `npm update` will modify `package-lock.json` to specify newer versions of any updated (sub-)dependencies and download those, as long as they don't exceed the maximum version allowed in `package.json`. To check for newer versions that exceed the max version, run `npm outdated` to see a list. Unless you know why you are doing it, try to avoid committing updates to `package-lock.json` by mistake if your code changes don't pertain to package updates. And never manually modify the file.

## TypeScript configurations: `tsconfig.json`

Basic configuration options for the TypeScript build tool to do its job in our repository.

## Vite configurations: `vite.config.js`

We use the [Vite](https://vitejs.dev/) bundler/build system. This file is where we configure Vite to set up plugins (like the third-party license checker/generator). Part of the license checker plugin setup includes some functions to format web package licenses, as well as Rust package licenses provided by [cargo-about](https://github.com/EmbarkStudios/cargo-about), into a text file that's distributed with the application to provide license notices for third-party code.
