+++
title = "Features and roadmap"
template = "page.html"

[extra]
css = "/features.css"
+++

<section class="section-row">
<div class="section">

# Features and roadmap.

The current version of Graphite provides tools for designing vector art with Bézier curves, similar to desktop applications like Inkscape, Illustrator, and Affinity Designer.
</div>
</section>

<section class="section-row">
<div class="diptych">

<div id="graphite-today" class="section">

## Graphite today.

Graphite is a lightweight vector graphics editor that runs in your browser. Its nascent node-based compositor lets you apply image effects and co-create amazing art with AI.

<div class="informational-group features">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Vector editing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 1" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Node graph image effects</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>AI-assisted art creation</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Open source and free forever</span>
	</div>
</div>

<a href="https://editor.graphite.rs" class="link arrow">Launch Graphite</a>

</div>
<div id="graphite-tomorrow" class="section">

## Graphite tomorrow.

All the digital content creation tools a professional needs— in one streamlined package:

<div class="informational-group features">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 4" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Looks and feels like traditional <span style="text-decoration: underline dotted; text-decoration-color: #16323f77;" title="&quot;what you see is what you get&quot;">WYSIWYG</span> design apps</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Sleek, intuitive interface built by designers, for designers</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Real-time collaborative editing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Windows/Mac/Linux/Web/iPad</span>
	</div>
</div>

The full Graphite vision wholly embraces procedural workflows:

<div class="informational-group features">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Fully non-destructive editing with node-driven layers</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Infinitely scalable raster content with no pixelation</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 10" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Integrates leading-edge AI models and graphics algorithms</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/content/index/icon-atlas-features-preview.png" alt="" />
		<span>Procedural pipelines for studio production environments</span>
	</div>
</div>

</div>

</div>
</section>

<section id="upcoming-tech" class="feature-box">
<div class="box">

<h1 class="box-header">Upcoming Tech: How it works</h1>

---

<!-- Graphite's concept is unique among graphics editors and requires some explanation. Here's a glimpse at that secret sauce. -->

<div class="diptych">

<div class="section">

## Layers & nodes: hybrid compositing.

Graphite combines the best ideas from multiple categories of digital content creation software to form a design for the ultimate general-purpose 2D graphics editor. It is influenced by the central editing experience of traditional layer-based raster and vector apps. It takes inspiration from the non-destructive workflows of VFX compositing programs used in Hollywood. And it borrows the creative superpowers of procedural asset creation applications in the 3D industry.

Classic layer-based image editing is easy to understand and its collapsable folders help artists stay organized. A variety of interactive viewport tools make it easy to manipulate the layers by drawing directly onto the canvas. On the other hand, node-based editing is like artist-friendly programming. It works by describing manipulations as steps in a flowchart, which is vastly more powerful but comes with added complexity.

The hybrid workflow of Graphite offers a classic tool-centric, layer-based editing experience built around a procedural, node-based compositor. Users can ignore the node graph, use it exclusively, or switch back and forth with the press of a button while creating content. Interacting with the canvas using tools will manipulate the nodes behind the scenes. And the layer panel and node graph provide two equivalent, interchangeable views of the same document structure.

</div>
<div class="section">

## Raster & vector: sharp at all sizes.

Digital 2D art commonly takes two forms. Raster artwork is made out of pixels which means it can look like anything imaginable, but it becomes blurry or pixelated from upscaling to a higher resolution. Vector artwork is made out of curved shapes which is perfect for some art styles but limiting to others. The magic of vector is that its mathematically-described curves can be enlarged to any size and remain crisp.

Other apps usually focus on just raster or vector, forcing artists to buy and learn both products. Mixing art styles requires shuttling content back and forth between programs. And since picking a raster document resolution is a one-time deal, artists may choose to start really big, resulting in sluggish editing performance and multi-gigabyte documents.

Graphite reinvents raster rendering so it stays sharp at any scale. Artwork is treated as data, not pixels, and is always drawn at the current view resolution. Zoom the viewport and export images at any size— the document's paint brushes, masks, filters, and effects will all be rendered at the native resolution.

Marrying vector and raster under one roof enables both art forms to complement each other in a holistic content creation workflow.

</div>

</div>

</div>
</section>

<section class="section-row">
<div class="section">

## Development roadmap.

Short-to-medium-term feature development is tracked at a granular level in the [Task Board](https://github.com/GraphiteEditor/Graphite/projects/1) on GitHub. Graphite uses a continuous release cycle without version numbers where changes can be tracked by [commit hash](https://github.com/GraphiteEditor/Graphite/commits/master). The stable release at [editor.graphite.rs](https://editor.graphite.rs) deploys a [recent commit](https://github.com/GraphiteEditor/Graphite/releases/tag/latest-stable) from the past several weeks, while [dev.graphite.rs](https://dev.graphite.rs) hosts the latest commit.

<h3>— Pre-Alpha (complete) —</h3>
<ul>
<li>First year of development (complete, details omitted)</li>
</ul>
<h3>— Alpha Milestone 1 (ongoing) —</h3>
<ul>
	<li>Second year of development (complete, details omitted)</li>
	<li>Brush tool (in-progress)</li>
	<li>WebGPU in supported browsers (in-progress)</li>
	<li>Vertical compositing of nodes</li>
	<li>Node-based layer tree</li>
	<li>Graph-based documents</li>
	<li>Self-updating desktop app</li>
	<li>Custom subgraph document nodes</li>
</ul>
<h3>— Alpha Milestone 2 —</h3>
<ul>
	<li>Graph data attributes</li>
	<li>Spreadsheet-based vector data</li>
	<li>Editable SVG import</li>
	<li>Rust-based vector renderer</li>
	<li>Select mode and masking</li>
	<li>New viewport overlays system</li>
	<li>Resolution-agnostic raster rendering</li>
	<li>Powerful snapping and grid system</li>
	<li>Remote compile/render server</li>
	<li>Code editor for custom nodes</li>
	<li>Document history system</li>
	<li>Stable document format</li>
</ul>
<h3>— Alpha Milestone 3 —</h3>
<ul>
	<li>RAW photo import and processing</li>
	<li>Procedural paint brush styling</li>
	<li>Frozen history references</li>
	<li>Internationalization and accessability</li>
	<li>Reconfigurable workspace panels</li>
	<li>Liquify and non-affine rendering</li>
	<li>Interactive graph auto-layout</li>
	<li>Automation and batch processing</li>
</ul>
<h3>— Beta —</h3>
<ul>
	<li>Guide mode</li>
	<li>CAD-like constraint solver</li>
	<li>Constraint models for UI layouts</li>
	<li>Advanced typography and typesetting</li>
	<li>PDF export</li>
	<li>HDR and WCG color handling</li>
	<li>Node manager and marketplace</li>
	<li>Predictive graph rendering/caching</li>
	<li>Distributed graph rendering</li>
	<li>Cloud document storage</li>
	<li>Multiplayer collaborative editing</li>
	<li>Offline edit resolution with CRDTs</li>
	<li>Native UI rewrite from HTML frontend</li>
</ul>
<h3>— 1.0 Release —</h3>
<ul>
	<li>Timeline and renderer for animation</li>
	<li>Live video compositing</li>
	<li>Pen and touch-only interaction</li>
	<li>iPad app</li>
	<li>Portable render engine</li>
	<li>SVG animation</li>
</ul>
<h3>— Future Releases —</h3>
<ul>
	<li><em>…and that's just the beginning…</em></li>
</ul>

</div>
</section>
