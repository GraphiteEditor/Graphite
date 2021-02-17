<div align="center">
<img src="graphite_logo.png" alt="Graphite Logo" width="288" height="288" />
<br />
<h1>Graphite: An open source node-based 2D graphics editor</h1>
</div>

## Overview

**[Watch the Announcement Talk at the Rust Gamdev Meetup (20 minutes)](https://www.youtube.com/watch?v=Ea4Wt_FgEEw&t=563s)**

Graphite is an open-source, cross-platform digital content creation desktop app for 2D graphics editing, photo processing, vector art, digital painting, illustration, and compositing. Inspired by the open-source success story of Blender in the 3D domain, it aims to bring 2D content creation to new heights with efficient workflows influenced by Photoshop and Illustrator and backed by a powerful node-based, nondestructive approach proven by Houdini and Substance. The user experience of Graphite is of central importance, offering a meticulously-designed UI catering towards an intuitive and efficient artistic process. Users may draw and edit in the traditional interactive (WYSIWYG) viewport or jump in or out of the node graph at any time to tweak previous work and construct powerful procedural image generators that seamlessly sync with the interactive viewport. A core principle of the application is its 100% nondestructive workflow that is resolution-agnostic, meaning that raster-style image editing can be infinitely zoomed and scaled to any arbitrary resolution at a later time because editing is done by recording brush strokes, vector shapes, and other manipulations parametrically. One might use the painting tools on a small laptop display, zoom into specific areas to add detail to finish the artwork, then perhaps try changing the simulated brush style from a blunt pencil to a soft acrylic paintbrush after-the-fact, and finally export the complete drawing at ultra high resolution for printing on a giant poster. On the surface, Graphite is an artistic medium for drawing anything imaginable— under the hood, the node graph in Graphite powers procedural graphics and parametric rendering to produce unique artwork and automated data-driven visualizations. Graphite brings together artistic workflows and empowers your creativity in a free, open-source package that feels familiar but lets you go further.

## Status

Graphite is in an early stage of development and its vision is highly ambitious. The project is seeking collaborators to help design and develop the software. If interested, please open an issue to get in touch or introduce yourself in the project's Discord chat server at `https://di-s-co-rd.gg/p2-a-Y-jM3` (remove the dashes).

## Design mockups

Interactive viewport **mockup** *(work-in-progress design)*:
![Interactive viewport](https://files.keavon.com/-/EmotionalShoddyTurnstone/capture.png)

Node editor **mockup** *(work-in-progress design)*:
![Node editor](https://files.keavon.com/-/PartialTalkativePooch/capture.png)

## Technology

[Rust](https://www.rust-lang.org/) is the language of choice for a number of compelling reasons. It is low-level and highly efficient which is important because the nondestructive, resolution-agnostic editing approach will already be challenging to render fast enough for real-time, interactive editing. Furthermore, Rust makes multithreading very easy to implement and its safety guarantees will eliminate the inclusion of many bugs and crashes in the software. It is also easy to compile Rust code natively to Windows, macOS, Linux, and web browsers via WebAssembly, with the possibility of deploying Graphite to mobile devices down the road as well.

[WebGPU](https://gpuweb.github.io/gpuweb) (via the [WGPU Rust library](https://wgpu.rs)) is being used as the graphics API because it is modern, portable, and safe. It makes deployment on the web and native platforms easy while ensuring consistent cross-platform behavior. It also offers the ability to use compute shaders to perform many tasks that speed up graphical computations.

[Vue.js](https://vuejs.org/) is the web frontend framework initally used for building Graphite's user interface. This means, for the moment, Graphite will only run in a browser using Rust code compiled to [WebAssembly](https://webassembly.org/) (via [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)). This web-based GUI is intended to be rewritten in a native Rust GUI framework once that ecosystem matures or a developer can write a custom GUI framework suitable to the subset of featured needed by Graphite's user interface. The project was initially trying to write a custom GUI framework throughout 2020, but this was halting progress on higher-priority features.

[Pathfinder](https://github.com/servo/pathfinder) is a Rust library that will be used for vector graphics rendering.

Extension scripting language: the current plan is to use WASM modules that call APIs exposed for adding to parts of the GUI and controlling functionality around the application.

Node code scripting language: to be decided. Some nodes expose a code editor for writing short scripts to perform actions more efficiently than stringing together a bunch of nodes. (For comparison, Houdini uses its Vex language for this.) It may be possible to build a language runtime into a "language server node" (some would be provided by extensions, some out-of-the-box); other nodes could then consume a chosen language server node and execute logic written in the language of the user's choice.

## Running the code

The project is split between a Rust crates in `/packages` and web-based frontend in `/web-frontend` (this will be replaced by a native GUI system in the future in order to compile Graphite for Windows, Mac, and Linux). Currently the Vue.js frontend runs with the Vue CLI but the WASM bindings HTML/JS is built with WebPack (see [issue #29](https://github.com/Keavon/Graphite/issues/29)).

### Running the web frontend

```
cd web-frontend
npm install
npm run serve
```

### Running the WASM binding generator

PREREQUISITE: [Download and install](https://rustwasm.github.io/wasm-pack/) wasm-pack first.
```
cd web-frontend
npm install
npm run webpack-start
```

### Running the Rust code

```
cargo run
```
