![Graphite Logo](graphite_splash.png)

# Powerful 2D vector and raster editing: procedural and nondestructive.

Graphite is an in-development desktop graphics editor that strives to be the most powerful, intuitive, and versatile tool for nondestructive vector and raster art creation. While still early in development, implementation progress is moving forward at a swift pace and the product design, UI, and UX are relatively complete. The first release, Graphite 0.1, will focus on SVG editing. Then 0.2 will make that nondestructive with a node graph. Later revisions will provide full resolution-agnostic raster editing capabilities.

We need Rust and web developers! Feel free to peruse the [documentation](docs/index.md) and hop onto Discord to ask where to start:

## Discord

If the Graphite project strikes your fancy, join our Discord community to chat with its developers and contributors. You're invited to stop by just to lurk, ask questions, offer suggestions, or get involved in the project. We are seeking collaborators to help design and develop the software and this is where we communicate. Paste `https://di-sco-rd.gg/uM-jBz-5N68W` into your browser and delete the dashes. (Please don't share the link on any website without the dashes.)

## Contributing

[Instructions here.](https://github.com/GraphiteEditor/Graphite/issues/202)

## Design mockup

This is a **work-in-progress mockup** of the Document, Properties, and Layer Tree panels in a state of editing a raster-based workflow (raster editing will be part of version 0.3 and later). The mockup is a nonfunctional pixel-perfect design prototype and is not yet fully implemented in code.

![Demo UI mockup](https://files.keavon.com/-/SturdyElasticSnowdog/capture.png)

## Vision

**[Watch the Announcement Talk at the Rust Gamedev Meetup (20 minutes)](https://www.youtube.com/watch?v=Ea4Wt_FgEEw&t=563s)**

Graphite is an open source, cross-platform digital content creation desktop and web application for 2D graphics editing, photo processing, vector art, digital painting, illustration, data visualization, and compositing. Inspired by the open source success story of Blender in the 3D domain, it aims to bring 2D content creation to new heights with efficient workflows influenced by Photoshop and Illustrator and backed by a powerful node-based, nondestructive approach proven by Houdini and Substance.

The user experience of Graphite is of central importance, offering a meticulously-designed UI catering towards an intuitive and efficient artistic process. Users may draw and edit in the traditional interactive (WYSIWYG) viewport with the Layer Tree panel or jump in or out of the node graph at any time to tweak previous work and construct powerful procedural image generators that seamlessly sync with the interactive viewport. A core principle of the application is its 100% nondestructive workflow that is resolution-agnostic, meaning that raster-style image editing can be infinitely zoomed and scaled to arbitrary resolutions at a later time because editing is done by recording brush strokes, vector shapes, and other manipulations parametrically.

One might use the painting tools on a small laptop display, zoom into specific areas to add detail to finish the artwork, then perhaps try changing the simulated brush style from a blunt pencil to a soft acrylic paintbrush after-the-fact, and finally export the complete drawing at ultra high resolution for printing on a large poster.

On the surface, Graphite is an artistic medium for drawing anything imaginable— under the hood, the node graph in Graphite powers procedural graphics and parametric rendering to produce unique artwork and automated data-driven visualizations. Graphite brings together artistic workflows and empowers your creativity in a free, open source package that feels familiar but lets you delve further.

## Roadmap

We have recently scrapped the custom GUI library effort from 2020 and now use a web-based UI through HTML/CSS/Vue.js to rapidly build a temporary user interface for the minimum viable product (MVP). While the UI remains web-based, Graphite will only compile to the web and Rust code will be run through WebAssembly (WASM) in the browser. The web UI will be replaced by a native interface when Rust GUI frameworks mature, although Graphtie will still compile to the web using the WebGPU API for rendering. We are also focusing initial feature development on a destructive SVG vector editor for the 0.1 release. This will have all the basic vector editing features to read and write SVG files and edit with an improved user experience compared to Inkscape and Illustrator.

Following this MVP release, these destructive features will be slotted into a fleshed-out node graph system to offer innovative nondestructive vector editing capabilities in a 0.2 release.

The 0.3 release will add support for the Graphite concept node-based raster editing. It will extend the tools from only vector editing to a fully combined raster and vector workflow. The Charcoal render engine will be the major feature in this release to power raster-based graphics processing.

Development is broken into monthly sprints culminating in a presentation at the [Rust Gamedev Meetup](https://www.youtube.com/channel/UCrbatFmtTIvX3BCgsXOy96w) and a post in the [Rust Gamedev Newsletter](https://gamedev.rs/news/). [Check out the sprint list](https://github.com/GraphiteEditor/Graphite/milestones) to see the current tasks and features being built.

## Technology stack

[Rust](https://www.rust-lang.org/) is the language of choice for a number of compelling reasons. It is low-level and highly efficient which is important because the nondestructive, resolution-agnostic editing approach will already be challenging to render fast enough for real-time, interactive editing. Furthermore, Rust makes multithreading very easy to implement and its safety guarantees will eliminate the inclusion of many bugs and crashes in the software. It is also easy to compile Rust code natively to Windows, macOS, Linux, and web browsers via WebAssembly, with the possibility of deploying Graphite to mobile devices down the road as well.

[Vue.js](https://vuejs.org/) is the web frontend framework initally used for building Graphite's user interface. This means, for the moment, Graphite will only run in a browser using Rust code compiled to [WebAssembly](https://webassembly.org/) (via [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)). This web-based GUI is intended to be rewritten in a native Rust GUI framework once that ecosystem matures or a developer can write a custom GUI framework suitable to the subset of features needed by Graphite's user interface. The project was initially trying to write a custom GUI framework throughout 2020, but this was halting progress on higher-priority features.

[WebGPU](https://gpuweb.github.io/gpuweb) (via the [WGPU Rust library](https://wgpu.rs)) will be used as the graphics API because it is modern, portable, and safe. It makes deployment on the web and native platforms easy while ensuring consistent cross-platform behavior. Shaders will be written in [Rust GPU](https://github.com/EmbarkStudios/rust-gpu) to keep the codebase in a consistent language. Graphite 0.1 and 0.2 will rely on the browser's built-in vector rendering capabilities for SVG editing before diving into building Graphite's complex render engine.

## Running the code

The project is split between clients and core libraries (which use the clients). Currently the only important client is the web frontend (`/client/web`). There's also a CLI client (`/client/cli`) that may be useful for testing. The only core library so far is the Editor Core Library (`/core/editor`). The web client's Vue code lives in `/client/web/src` and a Rust wrapper for the Editor Core Library (which exposes functions to JavaScript through bindings generated by wasm-bindgen) lives in `/client/web/wasm`.

A good starting point for learning about the code structure and architecture is reading the [documentation](docs/index.md).

### Web client

This is the primary means of running and developing the project. You may need to download and install a recent version of [Rust](https://www.rust-lang.org/) and [Node.js](https://nodejs.org/).

```
npm run serve
```

After editing some Rust and TypeScript code, you may need to manually restart the developments server because this is not properly watching all files right now. (Please submit a PR to help fix this!)

You can also use `npm run lint` as well as `npm run build` (the static files will end up in `/client/web/dist`).

### CLI client

This may be useful for testing at some point in the future.

```
cargo run
```

### Editor core library

This will be run and tested mainly through the web frontend, which builds the Rust code into WebAssembly automatically. But if you need to build the code on its own:

```
cd core/editor
cargo build
```
