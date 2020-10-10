# What is Graphite?
Graphite is an open-source, cross-platform digital content creation desktop app for 2D graphics editing, photo processing, vector art, digital painting, illustration, and compositing. Inspired by the open-source success story of Blender in the 3D domain, it aims to bring 2D content creation to new heights with efficient workflows influenced by Photoshop and Illustrator and backed by a powerful node-based, nondestructive approach proven by Houdini and Substance. The user experience of Graphite is of central importance, offering a meticulously-designed UI catering towards an intuitive and efficient artistic process. Users may draw and edit in the traditional interactive (WYSIWYG) viewport or jump in or out of the node graph at any time to tweak previous work and construct powerful procedural image generators that seamlessly sync with the interactive viewport. A core principle of the application is its 100% nondestructive workflow that is resolution-agnostic, meaning that raster-style image editing can be infinitely zoomed and scaled to any arbitrary resolution at a later time because editing is done by recording brush strokes, vector shapes, and other manipulations parametrically. One might use the painting tools on a small laptop display, zoom into specific areas to add detail to finish the artwork, then perhaps try changing the simulated brush style from a blunt pencil to a soft acrylic paintbrush after-the-fact, and finally export the complete drawing at ultra high resolution for printing on a giant poster. On the surface, Graphite is an artistic medium for drawing anything imaginable— under the hood, the node graph in Graphite powers procedural graphics and parametric rendering to produce unique artwork and automated data-driven visualizations. Graphite brings together artistic workflows and empowers your creativity in a free, open-source package that feels familiar but lets you go further.

## Status

Graphite is in an early stage of development and its vision is highly ambitious. The project is seeking collaborators to help design and develop the software. If interested, please open an issue to get in touch or introduce yourself in the project's Discord chat server at `https://di-s-co-rd.gg/p2-a-Y-jM3` (remove the dashes).

## Design

Interactive viewport *(work-in-progress design mockup)*:
![Interactive viewport](https://files.keavon.com/-/HonoredBusyCygnet/capture.png)

Node editor *(work-in-progress design mockup)*:
![Node editor](https://files.keavon.com/-/RigidFarawayHyracotherium/capture.png)

## Technology

[Rust](https://www.rust-lang.org/) is the language of choice for a number of compelling reasons. It is low-level and highly efficient which is important because the nondestructive, resolution-agnostic editing approach will already be challenging to render fast enough for real-time, interactive editing. Furthermore, Rust makes multithreading very easy to implement and its safety guarantees will eliminate the inclusion of many bugs and crashes in the software. It is also easy to compile Rust code natively to Windows, macOS, Linux, and even web browsers via WebAssembly, with the possibility of deploying Graphite to mobile devices down the road as well.

[WebGPU](https://gpuweb.github.io/gpuweb) (via Mozilla's [WGPU Rust library](https://wgpu.rs)) is being used as the graphics API because it is modern, portable, and safe. It makes deployment on the web and native platforms easy while ensuring consistent cross-platform behavior. It also offers the ability to use compute shaders to perform many tasks that speed up graphical computations.

The [GUI framework](gui) is being custom-built for the specific needs of Graphite's interface, based on a simple XML format inspired by HTML, CSS, and Vue.js. This is the current focus of development.

JavaScript (via [Deno's V8 Rust library](https://github.com/denoland/rusty_v8)) will likely be the chosen scripting language used to extend functionality to areas of the application. This will make it faster to implement many features that are less dependent on low-level performance and it provides an integrated solution for future plugin support.

[Pathfinder](https://github.com/servo/pathfinder), backed by Mozilla, is the Rust library that will be used for vector graphics rendering.
