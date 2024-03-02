+++
title = "Graphite features"

[extra]
css = ["features.css"]
+++

<section>
<div class="section">

# Graphite features

The current alpha version of Graphite is a tool for vector art and graphic design. It also supports a limited, experimental raster editing toolset. All this is built around a central node graph that stores layer data and provides a basic—but continually improving—procedural design and nondestructive editing workflow which is a unique feature among vector editing software.

</div>
</section>

<section>

<div class="diptych">

<div class="section">

## Layers & nodes: hybrid compositing

Graphite combines the best ideas from multiple categories of digital content creation software to redefine the workflows of 2D graphics editing. It is influenced by the core editing experience of traditional layer-based raster and vector apps, the nondestructive approaches of VFX compositing programs used by film studios, and the boundless creative possibilities of procedural production tools daily-driven by the 3D industry.

Classic layer-based image editing is easy to understand, with collapsable folders that help artists stay organized. A variety of interactive viewport tools make it easy to manipulate the layers by drawing directly onto the canvas. On the other hand, node-based editing is like artist-friendly programming. It works by describing manipulations as steps in a flowchart, which is vastly more powerful but comes with added complexity.

The hybrid workflow of Graphite offers a classic tool-centric, layer-based editing experience built around a procedural, node-based compositor. Users can ignore the node graph, use it exclusively, or switch back and forth with the press of a button while creating content. Interacting with the canvas using tools will manipulate the nodes behind the scenes. And the layer panel and node graph provide two equivalent, interchangeable views of the same document structure.

</div>
<div class="section">

## Raster & vector: sharp at all sizes <span class="status-flag">not fully implemented yet</span>

Digital 2D art commonly takes two forms. Raster artwork is made out of pixels which means it can look like anything imaginable, but it becomes blurry or pixelated when upscaling to a higher resolution. Vector artwork is made out of curved shapes which is perfect for some art styles but limiting to others. The magic of vector is that its mathematically-described curves can be enlarged to any size and remain crisp.

Other apps usually focus on just raster or vector, forcing artists to buy and learn both products. Mixing art styles requires shuttling content back and forth between programs. And since picking a raster document resolution is a one-time deal, artists may choose to start really big, resulting in sluggish editing performance and multi-gigabyte documents.

Graphite reinvents raster rendering so it stays sharp at any scale. Artwork is treated as data, not pixels, and is always redrawn at the current viewing resolution. Zoom the viewport and export images at any size— the document's paint brushes, masks, filters, and effects will all be rendered at the native resolution.

Marrying vector and raster under one roof enables both art forms to complement each other in a cohesive content creation workflow.

</div>

</div>

</section>

<section>

<div class="diptych">

<div class="section">

## Powered by Graphene

**Graphene** is the node graph engine that powers Graphite's compositor and procedural graphics pipeline. It's a visual scripting environment built upon the high-performance Rust programming language. Its runtime is [designed](/blog/distributed-computing-in-the-graphene-runtime/) to distribute rendering across CPU cores, GPUs, and network/cloud machines while optimizing for interactive frame rates.

<!-- Rust programmers may find the following technical details to be of interest. Graphene node graphs are programs built out of reusable Rust functions using Graphite as a visual "code" editor. New nodes and data types can be implemented by writing custom Rust code with a built-in text editor. `no_std` code also gets compiled to GPU compute shaders using [`rust-gpu`](https://github.com/EmbarkStudios/rust-gpu). Each node is independently pre-compiled by `rustc` into portable WASM binaries and linked at runtime. Groups of nodes may be compiled into one unit of execution, utilizing Rust's zero-cost abstractions and optimizations to run with less overhead. And whole node graphs can be compiled into standalone executables for use outside Graphite. -->

</div>
<div class="section">

<!-- ## Proudly written in Rust -->
## Written in Rust

Always on the bleeding edge and built to last— Graphite is written on a robust foundation with Rust, a modern programming language optimized for creating fast, reliable, future-proof software. Even the GPU compute shaders are written in Rust, enabling reuse of CPU code implementations for nodes.

<!-- The underlying node graph engine that computes and renders Graphite documents is called Graphene. The Graphene engine is an extension of the Rust language, acting as a system for chaining together modular functions into useful pipelines with GPU and parallel computation. Artists can harness these powerful capabilities directly in the Graphite editor without touching code. Technical artists and programmers can write reusable Rust functions to extend the capabilities of Graphite and create new nodes to share with the community. -->

</div>

</div>

</section>

<section>
<div class="section">

## Roadmap

<div class="roadmap">
	<div class="informational-group features">
		<!-- Pre-Alpha -->
		<div class="informational complete heading" title="Began February 2021" data-year="2021">
			<h3>— Pre-Alpha —</h3>
		</div>
		<div class="informational complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Editor systems; basic vector art tools</span>
		</div>
		<!-- Alpha 1 -->
		<div class="informational complete heading" title="Began February 2022" data-year="2022">
			<h3>— Alpha 1 —</h3>
		</div>
		<div class="informational complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 1" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Better tools; node graph prototyping</span>
		</div>
		<!-- Alpha 2 -->
		<div class="informational complete heading" title="Began February 2023" data-year="2023">
			<h3>— Alpha 2 —</h3>
		</div>
		<div class="informational complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Fully node graph-driven documents</span>
		</div>
		<!-- Alpha 3 -->
		<div class="informational ongoing heading" title="Began February 2024" data-year="2024">
			<h3>— Alpha 3 —</h3>
		</div>
		<div class="informational ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Procedurally-defined vector data</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Graph data attribute spreadsheet</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>WebGPU accelerated rendering</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>New vector graphics renderer (Vello)</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 14" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Adaptive resolution raster rendering</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Desktop app with built-in AI models</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 42" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Timeline and animation curves</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 22" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Select Mode: marquee masking</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Custom subgraph nodes</span>
		</div>
		<!-- Alpha 4 -->
		<div class="informational heading" title="Expected to begin February 2025" data-year="2025">
			<h3>— Alpha 4 —</h3>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Outliner panel: node graph tree view</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 20" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Imported RAW photo processing</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 29" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Guide Mode: construction geometry</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 30" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>CAD-like constraint relationships</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Remote compile/render server</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Code editor for custom nodes</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 27" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Interactive graph auto-layout</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 18" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Stable document format</span>
		</div>
		<!-- Beta -->
		<div class="informational heading">
			<h3>— Beta —</h3>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 19" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Document history system</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 24" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Internationalization and accessibility</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 23" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Frozen-in-time graph references</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 26" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Liquify and non-affine rendering</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 25" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Reconfigurable workspace panels</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 28" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Automation and batch processing</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 34" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>HDR and WCG color handling</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 35" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Node manager and marketplace</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 36" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Predictive graph rendering/caching</span>
		</div>
		<!-- 1.0 Release -->
		<div class="informational heading">
			<h3>— 1.0 Release —</h3>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 21" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Procedural styling of paint brushes</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 31" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Constraint models for UI layouts</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 32" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Advanced typography and typesetting</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 33" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>PDF export</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 37" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Distributed graph rendering</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 38" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Cloud document storage</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 39" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Multiplayer collaborative editing</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 40" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Offline edit resolution with CRDTs</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 41" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Native UI rewrite from HTML frontend</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 48" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>SVG animation authorship</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 43" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Live video compositing</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 44" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Pen and touch-only interaction</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 45" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>iPad app</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 46" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span>Portable, embeddable render engine</span>
		</div>
		<div class="informational">
			<img class="atlas" style="--atlas-index: 49" src="https://static.graphite.rs/icons/icon-atlas-roadmap.png" alt="" />
			<span><em>…and that's all just the beginning…</em></span>
		</div>
	</div>
</div>

</div>
</section>
