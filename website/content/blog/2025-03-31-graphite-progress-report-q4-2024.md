+++
title = "Graphite progress report (Q4 2024)"
date = 2025-03-31
[extra]
banner = "https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024.avif"
banner_png = "https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024.png"
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q4 2024 update introduces quality of life features across drawing tools and procedural editing."
css = ["/component/demo-artwork.css"]
reddit = "https://www.reddit.com/r/graphite/comments/1jpjqcs/blog_post_graphite_progress_report_q4_2024/"
twitter = "https://x.com/GraphiteEditor/status/1907350199414206604"
bluesky = "https://bsky.app/profile/graphiteeditor.bsky.social/post/3llsxykppjs2c"
+++

[Graphite](/), a new open source 2D procedural graphics editor, has [wrapped up 2024](../year-in-review-2024-highlights-and-a-peek-at-2025) with a fourth quarter (October-December) focused on introducing **quality of life features** for both Graphite's interactive **tools-driven and procedural editing workflows**.

<!-- more -->

All Q4 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-10-01&until=2024-12-31) and all noteworthy changes are detailed below.

This is the fourth in our 2024 series of quarterly progress reports. If you missed [the previous post](../graphite-progress-report-q3-2024), be sure to check it out as well. If you'd like to help speed up future progress, please consider [getting involved](/volunteer) with code, QA/bug testing, or art/marketing projects. [Donations](/donate) are also valued, as are [stars on GitHub](https://github.com/GraphiteEditor/Graphite). Follow along and partake in our [Discord community](https://discord.graphite.rs), too.

At the time of publication, there is **one week left to apply for a summer 2025 internship with us through Google Summer of Code.** [Learn more here](../internships-for-a-rust-graphics-engine-gsoc-2025) and apply before [April 8 at 18:00 UTC](https://www.wolframalpha.com/input?i=April+8%2C+18%3A00+UTC).

To showcase the newly introduced feature of converting selected nodes into conveniently reusable subgraphs, the new *Parametric Dunescape* artwork is presented below. Every dune layer is generated just by its custom parameters for color, height, and random seed.

<div class="demo-artwork">
	<a href="https://editor.graphite.rs/#demo/parametric-dunescape">
		<img src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Parametric Dunescape" />
	</a>
	<p>
		<span>
			<em>Parametric Dunescape</em>
		</span>
		<br />
		<span>
			<a href="https://editor.graphite.rs/#demo/parametric-dunescape">Open this artwork</a> to<br />explore it yourself.
		</span>
	</p>
</div>

## Additions

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->
- Node graph support for making a custom node by merging the selected nodes into a subgraph with the node context menu's "Merge Selected Nodes" option, or the shortcut <kbd>Ctrl</kbd><kbd>M</kbd> (macOS: <kbd>⌘</kbd><kbd>M</kbd>) <small>([#2097](https://github.com/GraphiteEditor/Graphite/pull/2097))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/merge-selected-nodes.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/merge-selected-nodes.mp4" type="video/mp4" />
  	</video>
  </div>

- Text controls for line height, character spacing, and wrappable box areas that can be dragged with the Text tool <small>([#2016](https://github.com/GraphiteEditor/Graphite/pull/2016), [#2118](https://github.com/GraphiteEditor/Graphite/pull/2118))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/text-area-controls.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/text-area-controls.mp4" type="video/mp4" />
  	</video>
  </div>

- Pinnable node sections in the Properties panel <small>([commit e6d8c47](https://github.com/GraphiteEditor/Graphite/commit/e6d8c4743d2aff15985c929df2cc7381a61908a0))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/pinnable-node-sections.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/pinnable-node-sections.mp4" type="video/mp4" />
  	</video>
  </div>
- New demo artwork, [*Changing Seasons*](https://editor.graphite.rs/#demo/changing-seasons), featured in the [previous progress report](../graphite-progress-report-q3-2024) <small>([commit fa6b5f2](https://github.com/GraphiteEditor/Graphite/commit/fa6b5f298adf395362e1aaa2c07be89fa89eaee2))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/changing-seasons-demo.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/changing-seasons-demo.mp4" type="video/mp4" />
  	</video>
  </div>

- *Offset Path* node that expands or contracts a vector shape <small>([#2030](https://github.com/GraphiteEditor/Graphite/pull/2030))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/offset-path-node.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/offset-path-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *Flatten Vector Elements* node that turns multiple layers of vector paths into a single combined path; and changes to the *Copy to Points*, *Repeat*, and *Circular Repeat* nodes so they output group data instead of a single vector path, allowing each separate layer to be modified by nodes which operate on groups (like *Assign Colors*), or flattened with *Flatten Vector Elements* to have the prior behavior <small>([#2011](https://github.com/GraphiteEditor/Graphite/pull/2011), [#2045](https://github.com/GraphiteEditor/Graphite/pull/2045))</small>

  <img src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/flatten-vector-elements-node.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Visual description of the change introduced by the Flatten Vector Elements node and its usage with instancing nodes" />

- Support for *Fill* and *Stroke* nodes with groups, applying to each vector layer within <small>([#2046](https://github.com/GraphiteEditor/Graphite/pull/2046))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/fill-and-stroke-nodes-on-groups.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/fill-and-stroke-nodes-on-groups.mp4" type="video/mp4" />
  	</video>
  </div>

- *Switch* node that routes one of two data connections based on a true or false value <small>([#2064](https://github.com/GraphiteEditor/Graphite/pull/2064))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/switch-node.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/switch-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *Bevel* node that flattens the corners of vector shapes <small>([#2067](https://github.com/GraphiteEditor/Graphite/pull/2067), [#2096](https://github.com/GraphiteEditor/Graphite/pull/2096))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/bevel-node.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/bevel-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *Jitter Points* node that randomly offsets each point in a vector path <small>([commit 7d86bf4](https://github.com/GraphiteEditor/Graphite/commit/7d86bf4abf7edfe6a5d021075e050614bee07c13))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/jitter-points-node.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/jitter-points-node.mp4" type="video/mp4" />
  	</video>
  </div>

- Node insertion button, and layer renaming, directly from the Properties panel <small>([#2072](https://github.com/GraphiteEditor/Graphite/pull/2072), [#2081](https://github.com/GraphiteEditor/Graphite/pull/2081))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/node-insertion-button-in-properties-panel.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/node-insertion-button-in-properties-panel.mp4" type="video/mp4" />
  	</video>
  </div>

- Path tool feature where pressing <kbd>Space</kbd> while dragging a handle makes the anchor be dragged as well <small>([#2065](https://github.com/GraphiteEditor/Graphite/pull/2065))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/path-tool-space-to-drag-anchor.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/path-tool-space-to-drag-anchor.mp4" type="video/mp4" />
  	</video>
  </div>

- Path tool feature where pressing <kbd>Tab</kbd> while dragging a handle makes it swap to the opposite handle <small>([#2058](https://github.com/GraphiteEditor/Graphite/pull/2058))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/path-tool-tab-to-swap-handle.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/path-tool-tab-to-swap-handle.mp4" type="video/mp4" />
  	</video>
  </div>

- Pen tool feature allowing the connection of layers by their endpoints so they both get merged into a single layer <small>([#2076](https://github.com/GraphiteEditor/Graphite/pull/2076))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/pen-tool-combining-layers.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/pen-tool-combining-layers.mp4" type="video/mp4" />
  	</video>
  </div>

- *Clamp* node that limits an input number between a minimum and maximum range <small>([#2087](https://github.com/GraphiteEditor/Graphite/pull/2087))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/clamp-node.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/clamp-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *To U32* and *To U64* nodes that convert numbers to a positive integer type required by a few nodes, as a workaround for automatic type conversion not being fully supported yet <small>([#2087](https://github.com/GraphiteEditor/Graphite/pull/2087))</small>
- *Dot Product* node that calculates the mathematical dot product between two numerical vectors <small>([#2126](https://github.com/GraphiteEditor/Graphite/pull/2126))</small>
- *Math* node that calculates a custom math expression with variables "A" and "B" <small>([#2121](https://github.com/GraphiteEditor/Graphite/pull/2121))</small>

  <img src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/math-node.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Visualization of the Math node properties showing the example expression: 0.33 * A + (1 - 0.33) * B" />

- Degrees/radians option in the trig-related math nodes and "Always Positive" option in the *Modulo* node for more convenient usage of the math nodes <small>([commit d649052](https://github.com/GraphiteEditor/Graphite/commit/d649052255c10c15754c3a3707f2edf996d2468d))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/modulo-and-trig-node-additions.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/modulo-and-trig-node-additions.mp4" type="video/mp4" />
  	</video>
  </div>

- Node graph control bar revamp <small>([#2093](https://github.com/GraphiteEditor/Graphite/pull/2093))</small>

  <img src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/node-graph-control-bar-revamp.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="View of the control bar area of the UI in the node graph featuring new buttons" />

- Freehand tool feature for drawing new subpaths on an existing vector layer by holding <kbd>Shift</kbd> <small>([commit ed119ad](https://github.com/GraphiteEditor/Graphite/commit/ed119ad3d799030dbc488ccfc8ca9ad057eeff2c))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/freehand-tool-draw-appended-to-layer.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/freehand-tool-draw-appended-to-layer.mp4" type="video/mp4" />
  	</video>
  </div>

- Proper automatic placement of layers into the artboard they're drawn inside of <small>([#2110](https://github.com/GraphiteEditor/Graphite/pull/2110))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-placement-into-artboard-drawn-on.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-placement-into-artboard-drawn-on.mp4" type="video/mp4" />
  	</video>
  </div>

- Menu bar additions of *Layer* > *New*, *Layer* > *Group Selected*, and *Layer* > *Delete Selected* <small>([commit feba874](https://github.com/GraphiteEditor/Graphite/commit/feba87449bb490e47df6f267576bec5ab4238dc3))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-menu-bar-additions.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-menu-bar-additions.mp4" type="video/mp4" />
  	</video>
  </div>

- Select tool box selection feature for subtracting the targetted layers from the active selection with a modifier key as shown in the contextual input hints at the bottom of the editor <small>([#2162](https://github.com/GraphiteEditor/Graphite/pull/2162))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/box-selection-subtraction.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/box-selection-subtraction.mp4" type="video/mp4" />
  	</video>
  </div>

- Path tool feature for snapping to 15° increments and locking the angles of dragged handles when <kbd>Shift</kbd> and <kbd>Ctrl</kbd> modifier keys are pressed <small>([#2160](https://github.com/GraphiteEditor/Graphite/pull/2160))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/path-tool-angle-locking-and-15deg-increments.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/path-tool-angle-locking-and-15deg-increments.mp4" type="video/mp4" />
  	</video>
  </div>

- Support for multiple top output wires extending from the same layer stack <small>([#2049](https://github.com/GraphiteEditor/Graphite/pull/2049))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-stack-multi-top-outputs.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-stack-multi-top-outputs.mp4" type="video/mp4" />
  	</video>
  </div>

- Style improvements to the Layers panel UI to clarify which layers contain selected children, even if hidden within a collapsed layer which previously obscured where selected layers were within the hierarchy <small>([commit 1264ea8](https://github.com/GraphiteEditor/Graphite/commit/1264ea8246cbb06e0602a93be983762ab17adf30))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-panel-show-selected-within-collapsed.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/layer-panel-show-selected-within-collapsed.mp4" type="video/mp4" />
  	</video>
  </div>

- Revamped quick measurement overlays now supporting every layer arrangement scenario <small>([#2147](https://github.com/GraphiteEditor/Graphite/pull/2147), [#2155](https://github.com/GraphiteEditor/Graphite/pull/2155))</small>

  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/quick-measurement-improvements.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024/quick-measurement-improvements.mp4" type="video/mp4" />
  	</video>
  </div>

## Fixes
- Fix for viewport tools no longer remaining active in the background when the node graph is open <small>([#2093](https://github.com/GraphiteEditor/Graphite/pull/2093))</small>
- Fix to boolean operations so open subpaths are automatically closed <small>([#2014](https://github.com/GraphiteEditor/Graphite/pull/2014))</small>
- Fix for a problem with double clicking an anchor for converting it between smooth and sharp <small>([#2023](https://github.com/GraphiteEditor/Graphite/pull/2023))</small>
- Fix for a *Scatter Points* node breakage <small>([commit 7a56af0](https://github.com/GraphiteEditor/Graphite/commit/7a56af01efc82460e780c78b008a52487972a7eb))</small>
- Fix for properly considering artboard clipping when calculating click targets <small>([#2028](https://github.com/GraphiteEditor/Graphite/pull/2028), [#2036](https://github.com/GraphiteEditor/Graphite/pull/2036))</small>
- Fix for <kbd>Ctrl</kbd><kbd>H</kbd> layer hiding and <kbd>Ctrl</kbd><kbd>L</kbd> layer locking only working with the graph open <small>([#2029](https://github.com/GraphiteEditor/Graphite/pull/2029))</small>
- Fix to artboard label positioning and styling of text overlays <small>([#2032](https://github.com/GraphiteEditor/Graphite/pull/2032))</small>
- Fix for an assortment of crashes and bugs <small>([#2075](https://github.com/GraphiteEditor/Graphite/pull/2075))</small>
- Fix for broken gradient transforms with the Vello renderer <small>([#2059](https://github.com/GraphiteEditor/Graphite/pull/2059))</small>
- Fix for alignment snapping not preserving aspect ratio when <kbd>Shift</kbd> is held <small>([#2062](https://github.com/GraphiteEditor/Graphite/pull/2062))</small>
- Fix for the Text tool clearing the text when hitting <kbd>Esc</kbd> <small>([#2052](https://github.com/GraphiteEditor/Graphite/pull/2052))</small>
- Fix for allowing the Path tool to edit an upstream vector path even if there's a type conversion midway <small>([#2055](https://github.com/GraphiteEditor/Graphite/pull/2055))</small>
- Fix for the number input widget not updating its unit symbol when changing to show another input field <small>([#2080](https://github.com/GraphiteEditor/Graphite/pull/2080))</small>
- Fix to make the *Sample Points*, *Scatter Points*, and *Splines from Points* nodes generate segments (not just invisible points) and work with subpaths <small>([#2085](https://github.com/GraphiteEditor/Graphite/pull/2085))</small>
- Fix for sometimes breaking the selected layer upon switching away from the Select tool <small>([commit 8d3da83](https://github.com/GraphiteEditor/Graphite/commit/8d3da83606c23366d2688602afbc0917e7224e68))</small>
- Fix to remove a visual cutout from the left border of a layer in the node graph when a wire doesn't entering through the cutout <small>([commit 12ca060](https://github.com/GraphiteEditor/Graphite/commit/12ca06035cd7463ed895671ff7eebe53fde655c6))</small>
- Fix to make point nudging with the Path tool work in document space <small>([#2095](https://github.com/GraphiteEditor/Graphite/pull/2095))</small>
- Fix to make the *Spline* node algorithm be continuous across start/end points <small>([#2092](https://github.com/GraphiteEditor/Graphite/pull/2092))</small>
- Fix to properly support layer nudging when the view is tilted and make nudge resizing work in the Artboard tool <small>([#2098](https://github.com/GraphiteEditor/Graphite/pull/2098))</small>
- Fix to disable menu bar entries when no layer is selected <small>([#2098](https://github.com/GraphiteEditor/Graphite/pull/2098))</small>
- Fix for clarifying the present state of the Brush tool with a warning message <small>([commit de366f9](https://github.com/GraphiteEditor/Graphite/commit/de366f951424fcdf4463a419db3fa659910fabfd))</small>
- Fix to load the editor faster by moving font catalog loading to document creation time <small>([commit de366f9](https://github.com/GraphiteEditor/Graphite/commit/de366f951424fcdf4463a419db3fa659910fabfd))</small>
- Fix to make the Pen tool only append new paths when <kbd>Shift</kbd> is held <small>([#2102](https://github.com/GraphiteEditor/Graphite/pull/2102))</small>
- Fix to make the Pen tool always snap to endpoint anchors, even when snapping is off <small>([#2107](https://github.com/GraphiteEditor/Graphite/pull/2107))</small>
- Fix crash when upgrading a document with a *Modulo* node from 3 commits ago <small>([commit 4c4d559](https://github.com/GraphiteEditor/Graphite/commit/4c4d559d97b4d131d2777c0aab19590531ae47a9))</small>
- Fix to clean up the consistency of the editor preferences dialog <small>([commit 99cf8f0](https://github.com/GraphiteEditor/Graphite/commit/99cf8f0c4f91a051b59fc2c9e5cc6c7417bdd74b))</small>
- Fix to remove the inconsistently functioning double-click behavior of switching to the Path tool on vector layers, which previously worked only on layers with a Path node <small>([#2116](https://github.com/GraphiteEditor/Graphite/pull/2116))</small>
- Fix for dragging a pair of colinear handles to break the colinearity so they can move without their anchor <small>([#2120](https://github.com/GraphiteEditor/Graphite/pull/2120))</small>
- Fix for the broken bounding box of image layers, which also impacted their layer thumbnails <small>([#2122](https://github.com/GraphiteEditor/Graphite/pull/2122))</small>
- Fix to restore the keyboard shortcut label in the menu bar's *File* > *Close* menu item <small>([#2135](https://github.com/GraphiteEditor/Graphite/pull/2135))</small>
- Fix to the syntax of exported SVG files that minorly deviated from spec and may have impact some strict SVG viewers <small>([#2131](https://github.com/GraphiteEditor/Graphite/pull/2131))</small>
- Fix for the UI by removing most "coming soon" elements left over from earlier times when placeholders were necessary <small>([commit 1264ea8](https://github.com/GraphiteEditor/Graphite/commit/1264ea8246cbb06e0602a93be983762ab17adf30))</small>
- Fix for issues with selection history <small>([#2138](https://github.com/GraphiteEditor/Graphite/pull/2138))</small>
- Fix for a cancellation of a transform cage rotation causing broken state upon the next transformation <small>([#2149](https://github.com/GraphiteEditor/Graphite/pull/2149))</small>
- Fix to make the Path tool deselect all of a filled shape's points when single-clicked, and select all when double-clicked <small>([#2148](https://github.com/GraphiteEditor/Graphite/pull/2148))</small>
- Fix for the Select tool's box selection not being able to extend a selection with <kbd>Shift</kbd> <small>([#2157](https://github.com/GraphiteEditor/Graphite/pull/2157))</small>
- Fix for several bugs in vector-related nodes <small>([commit b81f483](https://github.com/GraphiteEditor/Graphite/commit/b81f48385afc8c9c27820ffe8d5953529f89b7bd))</small>
- Fix for the Text tool making it easier to select existing text layers with more forgiving click targets <small>([#2145](https://github.com/GraphiteEditor/Graphite/pull/2145))</small>
- Fix for text layers getting deselected after clicking out of Text tool's interactive editing mode <small>([#2144](https://github.com/GraphiteEditor/Graphite/pull/2144))</small>
- Fix to make the Artboard tool shift its contents if resizing from the top/left so artwork remains stationary <small>([#2166](https://github.com/GraphiteEditor/Graphite/pull/2166))</small>


## Internal
- Change to make the *Upload Texture* node resolution-aware <small>([#2018](https://github.com/GraphiteEditor/Graphite/pull/2018))</small>
- Fix for faulty contravariance checking in the type system <small>([#2025](https://github.com/GraphiteEditor/Graphite/pull/2025))</small>
- Update of Wasm dependencies to fix a crash in Firefox with Vello due to a WebGPU spec change <small>([#2027](https://github.com/GraphiteEditor/Graphite/pull/2027))</small>
- Simplification of the Bezier-rs interactive web demo code <small>([#2020](https://github.com/GraphiteEditor/Graphite/pull/2020), [commit 4df7803](https://github.com/GraphiteEditor/Graphite/commit/4df780391c0cbb87b12812d0249ed9b62e2740e4))</small>
- Fix for Clippy code warnings <small>([commit a395fbf](https://github.com/GraphiteEditor/Graphite/commit/a395fbf0637c23b8b05a7c451ff7b8421587b655), [#2119](https://github.com/GraphiteEditor/Graphite/pull/2119))</small>
- Refactor of assorted parts of the RawKit crate <small>([#1972](https://github.com/GraphiteEditor/Graphite/pull/1972), [#2071](https://github.com/GraphiteEditor/Graphite/pull/2071), [#2066](https://github.com/GraphiteEditor/Graphite/pull/2066), [#2082](https://github.com/GraphiteEditor/Graphite/pull/2082), [#2088](https://github.com/GraphiteEditor/Graphite/pull/2088))</small>
- Code tidyness cleanup for node ID generation <small>([#2009](https://github.com/GraphiteEditor/Graphite/pull/2009))</small>
- Refactor collection of snap targets <small>([#2114](https://github.com/GraphiteEditor/Graphite/pull/2114))</small>
- System for parsing node and parameter descriptions from doc comments, enabling better node tooltips going forward <small>([#2089](https://github.com/GraphiteEditor/Graphite/pull/2089), [#2163](https://github.com/GraphiteEditor/Graphite/pull/2163), [commit d649052](https://github.com/GraphiteEditor/Graphite/commit/d649052255c10c15754c3a3707f2edf996d2468d))</small>
- Fix for the rectangle constructor in the Bezier-rs library to produce linear segments <small>([#2109](https://github.com/GraphiteEditor/Graphite/pull/2109))</small>
- Math-Parser library for reading and evaluating math expressions <small>([#2033](https://github.com/GraphiteEditor/Graphite/pull/2033))</small>
- Simplifications to the implementation of buffered message passing within the editor architecture <small>([#2123](https://github.com/GraphiteEditor/Graphite/pull/2123))</small>
- Updates to dependencies <small>([#2134](https://github.com/GraphiteEditor/Graphite/pull/2134))</small>

## Website
- Publication of the previous blog post, [*Graphite progress report (Q3 2024)*](../graphite-progress-report-q3-2024) <small>([#2013](https://github.com/GraphiteEditor/Graphite/pull/2013))</small>
- Updates to the screenshots on the home page carousel <small>([commit f892687](https://github.com/GraphiteEditor/Graphite/commit/f89268757614bda5f949144f84ccc79bd33ddec2))</small>
- Updates to the website roadmap and other details <small>([commit d7a271f](https://github.com/GraphiteEditor/Graphite/commit/d7a271f67595863835e80e58636fead9b1fe0a1d), [commit ce0cd39](https://github.com/GraphiteEditor/Graphite/commit/ce0cd39c9b4291e76d59d5b7c427afd39702aed6), [commit 740fcb7](https://github.com/GraphiteEditor/Graphite/commit/740fcb73cc1202d89107a08f3dfccd88dd17a6a3), [commit 54926d5](https://github.com/GraphiteEditor/Graphite/commit/54926d5474f3bcaffd54d0fd0d2509e989cfa425))</small>
