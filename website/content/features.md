+++
title = "Graphite features"

[extra]
css = ["/page/features.css", "/component/feature-box.css", "/component/feature-icons.css", "/component/youtube-embed.css"]
js = ["/js/youtube-embed.js"]
+++

<section>
<div class="block">

# Graphite features

The current alpha version of Graphite is a tool for vector art and graphic design. It also supports a limited, experimental raster editing toolset. This tooling is built around a procedural graphics engine, letting artists build complex graphics and animations in its visual scripting language.

In 2025, stay tuned for performance improvements, a native multiplatform desktop app, and the beginnings of a full raster editing tool suite.

</div>
</section>

<section>
<div class="block">

<div class="block video-container">
<div>
<div class="youtube-embed aspect-16x9">
	<img data-youtube-embed="ZUbcwUC5lxA" loading="lazy" src="https://static.graphite.rs/content/features/podcast-interview-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Rust-Powered Graphics Editor: How Graphite's Syntax Trees Revolutionize Image Editing" />
</div>
</div>
</div>

</div>
</section>

<section>

<div class="diptych">

<div class="block">

## Layers & nodes: hybrid editing

Graphite combines the best ideas from multiple categories of digital content creation software to reimagine the workflows of 2D graphics editing. It is influenced by the core editing experience of traditional layer-based raster and vector tools, the nondestructive approaches of VFX compositing programs used by film studios, and the boundless creative possibilities of procedural production tools daily-driven by the 3D industry.

Classic layer-based image editing is easy to understand, employing collapsable folders that help artists stay organized. A variety of interactive viewport tools make it easy to manipulate the layers by drawing directly onto the canvas. On the other hand, node-based editing is essentially artist-friendly programming. It works by describing manipulations as steps in a flowchart, which is vastly more powerful but comes with added complexity.

The hybrid workflow pioneered by Graphite is able to deliver a classic tool-centric, layer-based editing experience built around a procedural, node-based compositor. Users can ignore the node graph, use it exclusively, or switch back and forth with the press of a button while creating content. Interacting with the canvas using tools will manipulate the nodes behind the scenes. And the layer panel and node graph provide two equivalent, interchangeable views of the same document structure.

</div>
<div class="block">

## Raster & vector: sharp at all sizes

Digital 2D art commonly takes two forms. Raster artwork is made out of pixels which means it can look like anything imaginable, but it becomes blurry or pixelated when upscaling to a higher resolution. Vector artwork is made out of curved shapes which is perfect for some art styles but limiting to others. The magic of vector is that its mathematically-described curves can be enlarged to any size and remain crisp.

Other apps commonly focus on just raster or vector, forcing artists to buy and learn separate products for both. Mixing art styles requires shuttling content back and forth between programs. And since picking a raster document resolution is a one-time commitment, artists often choose to start out big, resulting in sluggish editing performance and multi-gigabyte documents.

Graphite reinvents raster rendering so it stays sharp at any scale. Artwork is treated as data, not pixels, and is always redrawn at the current viewing resolution. Zoom the viewport and export images at any size— the document's paint brushes, masks, filters, and effects will always be rendered in full detail.

Marrying vector and raster under one roof enables both art forms to complement each other in one cohesive creative workflow. *(Scalable raster compositing is still experimental.)*

</div>

</div>

</section>

<section>
<div class="block">

## Roadmap

<div class="roadmap">
	<div class="feature-icons">
		<!-- Pre-Alpha -->
		<div class="feature-icon complete heading" title="Began February 2021" data-year="2021">
			<h3>— Pre-Alpha —</h3>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 1" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Editor systems; basic vector art tools</span>
		</div>
		<!-- Alpha 1 -->
		<div class="feature-icon complete heading" title="Began February 2022" data-year="2022">
			<h3>— Alpha 1 —</h3>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Better tools; node graph prototyping</span>
		</div>
		<!-- Alpha 2 -->
		<div class="feature-icon complete heading" title="Began February 2023" data-year="2023">
			<h3>— Alpha 2 —</h3>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Node graph integration in documents</span>
		</div>
		<!-- Alpha 3 -->
		<div class="feature-icon complete heading" title="Began February 2024" data-year="2024">
			<h3>— Alpha 3 —</h3>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Procedural vector editing and usability</span>
		</div>
		<!-- Alpha 4 -->
		<div class="feature-icon ongoing heading" title="Began February 2025" data-year="2025">
			<h3>— Alpha 4 —</h3>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 46" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Parametric animation</span>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 1" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
			<span>Instancer repeat nodes</span>
		</div>
		<div class="feature-icon complete" title="Development Complete">
			<img class="atlas" style="--atlas-index: 9; transform: scaleX(-1)" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Table-based graphical data</span>
		</div>
		<div class="feature-icon ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Desktop app (Windows, Mac, Linux)</span>
		</div>
		<div class="feature-icon ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>GPU-accelerated raster rendering</span>
		</div>
		<div class="feature-icon ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 10" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Evolution of the graphical data format</span>
		</div>
		<div class="feature-icon ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 48" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Robust vector mesh editing/rendering</span>
		</div>
		<div class="feature-icon ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 43; transform: rotate(90deg)" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Automatic image trace vectorization</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 41" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Timeline panel for animation curves</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 40" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Simplified main properties panel</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Custom attributes for table data</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 57" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Signed distance field rendering</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 53" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Local fonts access</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 54" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Local file browser for saving/loading</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 18" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Node version management</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Stable document format</span>
		</div>
		<!-- <div class="feature-icon ongoing" title="Development Ongoing">
			<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Imaginate tool</span>
		</div> -->
		<!-- Beta -->
		<div class="feature-icon heading">
			<h3>— Beta —</h3>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 56" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Variable color swatches</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 52" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Command palette and context menus</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 28" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Physical units of measure</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Brush tool rewrite</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 43" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Stylus and touch interaction</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Broader SVG support including filters</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 50" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Shape builder tool</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 24" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Dockable and multi-window panels</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Code editor for custom nodes</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 18" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Document history management</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 39" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Offline edit resolution with CRDTs</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 22" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>History brush and clone stamp tools</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 34" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Asset libraries and node marketplace</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 27" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Automation and batch processing</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 45" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Standalone parametric documents</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 19" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Raw photo processing</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 21" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Select mode (marquee masking)</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 25" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Liquify and warp transforms</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 31" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Advanced typography and typesetting</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 32" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>PDF, EPS, AI, DXF, PSD, and TIFF</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 55" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>CMYK, spot color, and ICC profiles</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 33" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>HDR and WCG color handling</span>
		</div>
		<!-- 1.0 Release -->
		<div class="feature-icon heading">
			<h3>— 1.0 Release —</h3>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 23" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Internationalization and accessibility</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 4" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Outliner panel (node graph tree view)</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 49" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>AI nodes and tools (e.g. magic wand)</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 20" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Procedural styling of paint brushes</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
			<span>Infinite generative vector patterns</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 29" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>CAD style constraint relationships</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 30" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Responsive design constraint solvers</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
			<span>Authoring animated SVGs, Lottie, etc.</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 42" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Live video stream compositing</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 44" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>iPad app and keyboard-free controls</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 37" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Cloud document storage</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 38" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Multiplayer collaborative editing</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 35" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Predictive graph rendering/caching</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 36" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Distributed graph rendering</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 15" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span>Cloud rendering accelerator service</span>
		</div>
		<div class="feature-icon">
			<img class="atlas" style="--atlas-index: 47" src="https://static.graphite.rs/icons/icon-atlas-roadmap__3.png" alt="" />
			<span><em>…and that's all just the beginning…</em></span>
		</div>
	</div>
</div>

</div>
</section>
