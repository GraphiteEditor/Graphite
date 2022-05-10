![Graphite logo](https://static.graphite.rs/readme/graphite-readme-logo.svg)

# Powerful 2D vector and raster editing. Procedural and nondestructive.

Graphite is a digital content creation software package for 2D graphics, merging traditional tool-based interactive editing workflows with a powerful node-based approach to procedural, non-destructive editing and compositing. The application strives to be the most user-friendly and versatile tool for vector and raster illustration, graphic design, photo editing, procedural texturing, data-driven visualization, and technical art.

While the project is still in early development, implementation progress has been moving forward at a swift pace. The Graphite Alpha release is launching soon and focuses on vector art for SVG creation. The next major milestone will introduce a nondestructive backend for vector graphics with a powerful node graph system. Later releases will provide better vector rendering capabilities and then resolution-agnostic raster editing and compositing.

Play around with Graphite right now in your browser at [editor.graphite.rs](https://editor.graphite.rs). Windows, Mac, and Linux will additionally be supported with a native (not web-based) desktop client later in the development roadmap.

## Contributing

We need Rust and web developers! See [instructions here](https://github.com/GraphiteEditor/Graphite/issues/202) for getting started.

We are also in search of artists to help create material for the website. Please get in touch for more details.

## Discord

If the Graphite project strikes your fancy, [join our Discord community](https://discord.graphite.rs) to chat with the community and development team. You're invited to stop by just to lurk, ask questions, offer suggestions, or get involved in the project. We are seeking collaborators to help design and develop the software and this is where we communicate.
## Vision

Graphite is an open source, cross-platform digital content creation desktop and web application for 2D graphics editing, photo processing, vector art, digital painting, illustration, data visualization, compositing, and more. Inspired by the open source success of Blender in the 3D domain, it aims to bring 2D content creation to new heights with efficient workflows influenced by Photoshop/Gimp and Illustrator/Inkscape and backed by a powerful node-based, nondestructive approach proven by Houdini and Substance.

The user experience of Graphite is of central importance, offering a meticulously-designed UI catering towards an intuitive and efficient artistic process. Users may draw and edit in the traditional interactive (WYSIWYG) viewport with the Layer Tree panel or jump in or out of the node graph at any time to tweak previous work and construct powerful procedural image generators that seamlessly sync with the interactive viewport. A core principle of the application is its 100% nondestructive workflow that is resolution-agnostic, meaning that raster-style image editing can be infinitely zoomed and scaled to arbitrary resolutions at a later time because editing is done by recording brush strokes, vector shapes, and other manipulations parametrically.

One might use the painting tools on a small laptop display, zoom into specific areas to add detail to finish the artwork, then perhaps try changing the simulated brush style from a blunt pencil to a soft acrylic paintbrush after-the-fact, and finally export the complete drawing at ultra high resolution for printing on a large poster.

On the surface, Graphite is an artistic medium for drawing anything imaginable— under the hood, the node graph in Graphite powers procedural graphics and parametric rendering to produce unique artwork and automated data-driven visualizations. Graphite brings together artistic workflows and empowers your creativity in a free, open source package that feels familiar but lets you delve further.

This UI mockup illustrates a future concept for the raster-based workflow in a photo editing example.

![Demo UI mockup](https://files.keavon.com/-/DodgerblueSoftCreature/capture.png)

## Roadmap

The Graphite team is focusing initial feature development on a simple vector graphics editor for the Alpha release.

Following this MVP release, the layer system will be extended into a fleshed-out node graph system, called Graphene, to offer innovative nondestructive vector editing capabilities in the next milestone release.

The following major releases will add a purpose-built render engine to support node-based raster editing, thereby providing a seamless combined raster and vector workflow.

The interim web-based frontend will be replaced by an identical native desktop client for Windows, Mac, and Linux plus the web. This new frontend will mark the release of Graphite 1.0 when complete.

Development is broken into monthly sprints culminating in a presentation at the [Rust Gamedev Meetup](https://www.youtube.com/channel/UCrbatFmtTIvX3BCgsXOy96w) and a post in the [Rust Gamedev Newsletter](https://gamedev.rs/news/). Check out the [Task Board](https://github.com/GraphiteEditor/Graphite/projects/1) to see the current features being built and prioritized.

## Technology stack

[Rust](https://www.rust-lang.org/) is the language of choice for many compelling reasons. It is low-level and highly efficient which is important because the nondestructive, resolution-agnostic editing approach will already be challenging to render quickly for interactive, real-time editing. Furthermore, Rust makes multithreading easy to implement and its safety guarantees will eliminate the inclusion of many bugs and crashes in the software. It is also simple to compile Rust code natively to Windows, Mac, Linux, and web browsers via WebAssembly, with the possibility of deploying Graphite to mobile devices down the road as well.

[Vue.js](https://vuejs.org/) is the web frontend framework initially used for building Graphite's user interface. This means, for the moment, Graphite will only run in a browser using Rust code compiled to [WebAssembly](https://webassembly.org/) (via [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)). This web-based GUI is intended to be rewritten in a native Rust GUI framework once that ecosystem matures or the project has engineering resources to write a tailor-made GUI framework for Graphite's needs. Development initially began by writing a custom GUI system throughout 2020, but slow progress led to the decision of shelving it in lieu of a temporary web-based GUI.

[WebGPU](https://gpuweb.github.io/gpuweb) (via the [WGPU Rust library](https://wgpu.rs)) will be used as the graphics API for GPU-accelerated rendering because it is modern, portable, and safe. It makes deployment on the web and native platforms easy while ensuring consistent cross-platform behavior. Shaders will be written in [Rust GPU](https://github.com/EmbarkStudios/rust-gpu) to keep the codebase in a consistent language. Early Graphite releases are relying on web browsers' built-in SVG rendering capabilities before work begins building Graphite's sophisticated render engine.
