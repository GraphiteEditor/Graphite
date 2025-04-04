+++
title = "Graphite progress report (Q2 2024)"
date = 2024-07-31
[extra]
banner = "https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024.avif"
banner_png = "https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024.png"
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q2 2024 update introduces boolean path operations, a new gradient picker, layer locking, and more improvements."
reddit = "https://www.reddit.com/r/graphite/comments/1ei9ps2/blog_post_graphite_progress_report_q2_2024/"
twitter = "https://x.com/GraphiteEditor/status/1819360794028462569"
css = ["/component/demo-artwork.css"]
+++

[Graphite](/), a new open source 2D procedural graphics editor, has spent April–June introducing **boolean path operations, a new gradient picker, layer locking**, and more improvements.

Overall, editor functionality has been shaping up and becoming an all around useful tool suite, with notable reductions in rough edges for the vector graphics workflow (our initial focus). Raster and raw photo processing workflows are also now in-development by our [Google Summer of Code](/blog/graphite-internships-announcing-participation-in-gsoc-2024/) student interns. Node graph quality-of-life improvements centered around tidy node organization are also the focus of the summer work that's underway. These projects are detailed below.

<!-- more -->

This is the second of our quarterly progress report blog posts. If you missed the [first one](../graphite-progress-report-q1-2024), be sure to check it out as well. If you'd like to help speed up future progress, please consider [getting involved](/volunteer) with code, QA/bug testing, or art/marketing projects. [Donations](/donate) are also valued, as are [stars on GitHub](https://github.com/GraphiteEditor/Graphite). Follow along and partake in our [Discord community](https://discord.graphite.rs), too.

All Q2 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-04-01&until=2024-06-30) and all noteworthy changes are detailed below. To showcase the much anticipated introduction of boolean path operations, the new *Painted Dreams* artwork shown here extensively utilizes nondestructive booleans.

<div class="demo-artwork">
	<a href="https://editor.graphite.rs/#demo/painted-dreams">
		<img src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Painted Dreams" />
	</a>
	<p>
		<span>
			<em>Painted Dreams</em>
		</span>
		<br />
		<span>
			<a href="https://editor.graphite.rs/#demo/painted-dreams">Open this artwork</a> to<br />explore it yourself.
		</span>
	</p>
</div>

## Additions

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->

- Feature for locking layers from being selected in the viewport <small>([#1702](https://github.com/GraphiteEditor/Graphite/pull/1702))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/layer-locking.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/layer-locking.mp4" type="video/mp4" />
  	</video>
  </div>

- Fine-grained choices in the Snapping options popover <small>([#1730](https://github.com/GraphiteEditor/Graphite/pull/1730))</small>
  
  <img src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/snapping-options.png" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Snapping choices popover menu" />

- Corner rounding added to the *Rectangle* node <small>([#1648](https://github.com/GraphiteEditor/Graphite/pull/1648))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/rectangle-corner-rounding.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/rectangle-corner-rounding.mp4" type="video/mp4" />
  	</video>
  </div>

- Rotation added to the *Repeat* node <small>([commit 72ba4dd](https://github.com/GraphiteEditor/Graphite/commit/72ba4ddfe421c0e17930ad1c2be85be2c69e04ea))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/repeat-node-rotation.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/repeat-node-rotation.mp4" type="video/mp4" />
  	</video>
  </div>

- Visibility and deletion buttons added to node sections in the Properties panel <small>([commit 07fd2c2](https://github.com/GraphiteEditor/Graphite/commit/07fd2c27827e4a91ad238d790d41396a33ef2389))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/hide-delete-properties-sections.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/hide-delete-properties-sections.mp4" type="video/mp4" />
  	</video>
  </div>

- Grid color customization and dotted appearance choices <small>([#1743](https://github.com/GraphiteEditor/Graphite/pull/1743))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/grid-coloration.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/grid-coloration.mp4" type="video/mp4" />
  	</video>
  </div>

- Demo artwork, *Isometric Fountain*, featured in the [last blog post](../graphite-progress-report-q1-2024) <small>([commit 6b0822d](https://github.com/GraphiteEditor/Graphite/commit/6b0822d31890b6699c4533c3e828da0e9e8c9490))</small>
  
  <div class="demo-artwork" style="justify-content: left">
  	<a href="https://editor.graphite.rs/#demo/isometric-fountain">
  		<img src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024__2.png" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Isometric Fountain" />
  	</a>
  	<p>
  		<span>
  			<em>Isometric Fountain</em>
  		</span>
  		<br />
  		<span>
  			<a href="https://editor.graphite.rs/#demo/isometric-fountain">Open this artwork</a> to<br />explore it yourself.
  		</span>
  	</p>
  </div>

- *Area* and *Centroid* nodes which calculate a shape's interior size and center of mass, respectively <small>([#1729](https://github.com/GraphiteEditor/Graphite/pull/1729), [#1749](https://github.com/GraphiteEditor/Graphite/pull/1749))</small> 
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/area-centroid-nodes.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/area-centroid-nodes.mp4" type="video/mp4" />
  	</video>
  </div>

- *Boolean Operation* node for combining two shape paths with a *Union*, *Subtract Front*, *Subtract Back*, *Intersect*, or *Difference* mode of cutting and combining <small>([#1759](https://github.com/GraphiteEditor/Graphite/pull/1759))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/boolean-ops-1.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/boolean-ops-1.mp4" type="video/mp4" />
  	</video>
  </div>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/boolean-ops-2.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/boolean-ops-2.mp4" type="video/mp4" />
  	</video>
  </div>

- Gradient picker <small>([#1778](https://github.com/GraphiteEditor/Graphite/pull/1778))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/gradient-picker.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/gradient-picker.mp4" type="video/mp4" />
  	</video>
  </div>

- Labels for artboard names displayed in the viewport <small>([#1795](https://github.com/GraphiteEditor/Graphite/pull/1795))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/artboard-names.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/artboard-names.mp4" type="video/mp4" />
  	</video>
  </div>

- [Dennis Kobert](https://github.com/truedoctor)'s Google Summer of Code project has begun adding raster editing infrastructure centered around GPU acceleration — to date this has included:

  - *Rasterize* node for converting graphical content (like vector art) into an image within a chosen area and resolution scale <small>([#1755](https://github.com/GraphiteEditor/Graphite/pull/1755))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/rasterize-node.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/rasterize-node.mp4" type="video/mp4" />
    	</video>
    </div>

  - [Weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1773) are being posted while the project is ongoing

- [Elbert Ronnie](https://github.com/elbertronnie)'s Google Summer of Code project has begun building a library for decoding raw image formats, with the goal of supporting photo processing in Graphite once the project is completed — to date this has included:

  - Initial code foundations including a basic custom TIFF decoder <small>([#1757](https://github.com/GraphiteEditor/Graphite/pull/1757))</small>

  - Initial decoder for Sony ARW data encoding formats <small>([#1775](https://github.com/GraphiteEditor/Graphite/pull/1775), [#1776](https://github.com/GraphiteEditor/Graphite/pull/1776))</small>

  - [Weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1771) are being posted while the project is ongoing

- [Adam Gerhant](https://github.com/adamgerhant)'s Google Summer of Code project has begun upgrading the node graph UI capabilities, quality-of-life features, and automatic layout management — to date this has included:

  - Generalization of layers to support the concept of vertically stackable adjustment layers <small>([#1712](https://github.com/GraphiteEditor/Graphite/pull/1712), [#1738](https://github.com/GraphiteEditor/Graphite/pull/1738), [commit 244c8ad](https://github.com/GraphiteEditor/Graphite/commit/244c8ad10ad79c9ca4fcdb21260c5d5dc60b3a21), [#1763](https://github.com/GraphiteEditor/Graphite/pull/1763), [#1739](https://github.com/GraphiteEditor/Graphite/pull/1739), [#1748](https://github.com/GraphiteEditor/Graphite/pull/1748))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/stackable-layers.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/stackable-layers.mp4" type="video/mp4" />
    	</video>
    </div>

  - Support for viewing and editing subgraphs by double-clicking nodes with internal node networks <small>([#1750](https://github.com/GraphiteEditor/Graphite/pull/1750))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/subgraph-editing.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/subgraph-editing.mp4" type="video/mp4" />
    	</video>
    </div>

  - Extended support for navigating the graph just like the viewport (with autopanning on dragging near edges, working scrollbars, and zooming shortcuts) by migrating interaction code to the backend <small>([#1768](https://github.com/GraphiteEditor/Graphite/pull/1768), [#1790](https://github.com/GraphiteEditor/Graphite/pull/1790))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/graph-navigation.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024/graph-navigation.mp4" type="video/mp4" />
    	</video>
    </div>

- [Weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1769) are being posted while the project is ongoing

## Fixes

- Polished up design for the Layers panel <small>([commit 938a688](https://github.com/GraphiteEditor/Graphite/commit/938a688fa08511ed4a5f3c3d5bcfa26f4bd1eb47))</small>

- Inclusion of alpha in the *Extract Channel* node and removal of the now-redundant *Extract Alpha* node <small>([#1731](https://github.com/GraphiteEditor/Graphite/pull/1731))</small>

- Fixed interactive outlining of layers within the Select tool's box selection <small>([#1727](https://github.com/GraphiteEditor/Graphite/pull/1727))</small>

- Insertion of duplicated layers directly above their selected source layers <small>([#1726](https://github.com/GraphiteEditor/Graphite/pull/1726))</small>

- Crash fixes <small>([#1701](https://github.com/GraphiteEditor/Graphite/pull/1701), [#1754](https://github.com/GraphiteEditor/Graphite/pull/1754), [#1770](https://github.com/GraphiteEditor/Graphite/pull/1770))</small>

- Small bug fixes and polish for viewport tilt and zoom navigation <small>([commit 597c96a](https://github.com/GraphiteEditor/Graphite/commit/597c96a7db06fe44dbd4dc170511c011c7239073))</small>

- Regression fix for the Select tool's shallow selection mode <small>([#1725](https://github.com/GraphiteEditor/Graphite/pull/1725))</small>

- Previewable dropdown menu states and color button widget undo/redo history improvements <small>([#1598](https://github.com/GraphiteEditor/Graphite/pull/1598))</small>

- Persisting of overlays/snapping/grid state in saved documents and toggling them with hotkeys <small>([commit 7845302](https://github.com/GraphiteEditor/Graphite/commit/7845302c50705332711b6b18b4bdfe28a2f3c306))</small>

- Improved robustness to the backwards compatability of the document format (still offered on a best-effort basis) <small>([commit de84e39](https://github.com/GraphiteEditor/Graphite/commit/de84e39c4ec6c191f73702a51be0a5ec9c662642))</small>

- Visual bug fixes for shape drawing and nondeterministic one-pixel viewport shifting when recentering with <kbd>Ctrl</kbd><kbd>0</kbd> <small>([#1751](https://github.com/GraphiteEditor/Graphite/pull/1751))</small>

- Fixed bug where Path tool point insertion often wasn't working <small>([commit cf01f52](https://github.com/GraphiteEditor/Graphite/commit/cf01f522a8b57bfd625f15a10ae6b2812ed4c0a4))</small>

- Fixed bugs where the *Poisson-Disk Points*, *Repeat*, *Circular Repeat*, and *Bounding Box* node input shapes weren't using their transform <small>([#1784](https://github.com/GraphiteEditor/Graphite/pull/1784), [#1792](https://github.com/GraphiteEditor/Graphite/pull/1792))</small>

- Improvements to the Path tool's point selection behavior so clicking a selected anchor deselects all other selected points <small>([#1782](https://github.com/GraphiteEditor/Graphite/pull/1782))</small>

- Regression fix for being unable to arrange layers with no artboard present <small>([#1786](https://github.com/GraphiteEditor/Graphite/pull/1786))</small>

## Internal

- Removal of the editor "instances" concept, allowing for cleaned up JS interop code <small>([commit 19eb6ce](https://github.com/GraphiteEditor/Graphite/commit/19eb6ce0ab10065ec6acd6e49edd2f072729fc77), [commit 1ce3d59](https://github.com/GraphiteEditor/Graphite/commit/1ce3d59e0f39e6733c7fa170af00ef59fd10ffd9))</small>

- Loosened type system in Graphene to allow contravariant function arguments <small>([#1740](https://github.com/GraphiteEditor/Graphite/pull/1740), [#1746](https://github.com/GraphiteEditor/Graphite/pull/1746))</small>

## Website and documentation

- [Roadmap](/features#roadmap) updates featuring new features and icons <small>([commit 6a1a145](https://github.com/GraphiteEditor/Graphite/commit/6a1a145d190887f65f1f851282bd86c8836f85a7), [commit e4d3faa](https://github.com/GraphiteEditor/Graphite/commit/e4d3faa52af42173eaf67b2dc5c2df5d6e6f23a8))</small>

- Initial [Graphene docs](/volunteer/guide/graphene) included in the contributor guide <small>([#1686](https://github.com/GraphiteEditor/Graphite/pull/1686))</small>

- Website meta tags fixed to provide better SEO and page previews for links posted on social media <small>([commit d0c493c](https://github.com/GraphiteEditor/Graphite/commit/d0c493cdb6867763d507a24f12d92f2617385114))</small>

- Redesigned website layout featuring a narrower max width and better typography <small>([#1753](https://github.com/GraphiteEditor/Graphite/pull/1753))</small>
