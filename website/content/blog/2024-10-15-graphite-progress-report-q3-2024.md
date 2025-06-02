+++
title = "Graphite progress report (Q3 2024)"
date = 2024-10-15
[extra]
banner = "https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024.avif"
banner_png = "https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024.png"
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q3 2024 update introduces improvements to performance, node graph organization, nondestructive path editing, a new render engine, and more helpful nodes."
reddit = "https://www.reddit.com/r/graphite/comments/1g4h6ya/blog_post_graphite_progress_report_q3_2024/"
twitter = "https://x.com/GraphiteEditor/status/1846283664562573344"
css = ["/component/demo-artwork.css"]
+++

[Graphite](/), a new open source 2D procedural graphics editor, has spent July–September building major improvements to **performance, node graph organization, nondestructive path editing, a new render engine, and more helpful nodes**, amongst over 100 other features and fixes.

This has been the most productive quarter yet in the project's three-year history. Most of our [Google Summer of Code](/blog/graphite-internships-announcing-participation-in-gsoc-2024/) student intern projects have already reached their goals, adding to the goodies included in this progress report. All Q3 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-07-01&until=2024-09-30) and all noteworthy changes are detailed below.

<!-- more -->

This is the third in our series of quarterly progress reports. If you missed the [first](../graphite-progress-report-q1-2024) or [second](../graphite-progress-report-q2-2024) ones, be sure to check them out as well. If you'd like to help speed up future progress, please consider [getting involved](/volunteer) with code, QA/bug testing, or art/marketing projects. [Donations](/donate) are also valued, as are [stars on GitHub](https://github.com/GraphiteEditor/Graphite). Follow along and partake in our [Discord community](https://discord.graphite.rs), too.

The new *Changing Seasons* artwork shown here showcases some of the recently introduced nodes in this update. And it animates! Give it a try yourself by opening the artwork and dragging the percentage slider to morph from oak to maple leaves as the colors change.

<div class="demo-artwork">
	<a href="https://editor.graphite.rs/#demo/changing-seasons">
		<img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Changing Seasons" />
	</a>
	<p>
		<span>
			<em>Changing Seasons</em>
		</span>
		<br />
		<span>
			<a href="https://editor.graphite.rs/#demo/changing-seasons">Open this artwork</a> to<br />explore it yourself.
		</span>
	</p>
</div>

## Additions

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->

- Simplified *Boolean Operation* node that combines the best parts of the two previous boolean node versions, letting users now convert from a stack of shape layers (of any size!) into a single resulting vector shape that's more versatile to use in both the node graph and Layers panel <small>([#1813](https://github.com/GraphiteEditor/Graphite/pull/1813), [#1933](https://github.com/GraphiteEditor/Graphite/pull/1933))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/stack-based-booleans.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/stack-based-booleans.mp4" type="video/mp4" />
  	</video>
  </div>

- *Path* node which enables nondestructive vector editing at stages along the geometry calculation pipeline, even after a procedural shape (like a star) defines the parameters that can be tweaked anytime <small>([#1676](https://github.com/GraphiteEditor/Graphite/pull/1676))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/path-node.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/path-node.mp4" type="video/mp4" />
  	</video>
  </div>

- Additional snapping criteria for alignment and distribution between layers <small>([#1793](https://github.com/GraphiteEditor/Graphite/pull/1793))</small>
  
  <img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/new-snapping-modes.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Snapping choices popover menu" />

- Snapping now included with the Artboard tool for precise alignment with other artboards <small>([#1734](https://github.com/GraphiteEditor/Graphite/pull/1734))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/artboard-snapping.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/artboard-snapping.mp4" type="video/mp4" />
  	</video>
  </div>

- Persistence of the last-chosen settings each time the Export dialog is opened (so it's not always reset to defaults), plus artboard names are now correctly written in the Bounds dropdown <small>([commit 4d3e459](https://github.com/GraphiteEditor/Graphite/commit/4d3e459f1f52f04be81727fe5946010bcabeb1eb))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/export-settings-persistence.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/export-settings-persistence.mp4" type="video/mp4" />
  	</video>
  </div>

- Demo artwork, *Painted Dreams*, featured in the [last blog post](../graphite-progress-report-q2-2024) <small>([commit 80cf486](https://github.com/GraphiteEditor/Graphite/commit/80cf486790d9300510904622924162a9b4017b1e))</small>

  <div class="demo-artwork" style="justify-content: left">
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

- Quick measurement feature that is shown by holding <kbd>Alt</kbd> to momentarily see the distance between the selected and hovered shapes <small>([#1894](https://github.com/GraphiteEditor/Graphite/pull/1894))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/quick-measurement.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/quick-measurement.mp4" type="video/mp4" />
  	</video>
  </div>

- Support for drag-and-drop and copy-paste of files more widely throughout the editor <small>([#2012](https://github.com/GraphiteEditor/Graphite/pull/2012))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/drag-and-drop-import.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/drag-and-drop-import.mp4" type="video/mp4" />
  	</video>
  </div>

- Crash mitigation preventing node faults from taking down the whole editor, working to isolate crashes within the graph so the user can undo the change and save the file (however, reloading the editor is required to stabilize it after this occurs) <small>([#1846](https://github.com/GraphiteEditor/Graphite/pull/1846))</small>
  
  <img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/graph-crash-error.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graph execution crash recovery message" />

- Seed parameters now included in all nodes with random generation <small>([commit c39032a](https://github.com/GraphiteEditor/Graphite/commit/c39032ab548d4f33d18a399c64b77d3d6f4ecd45))</small>
  
  <img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-seed-parameters.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Three nodes and their new seed parameters" />

- *Percentage Value* node for easy sliding between the numbers 0 and 100 <small>([commit d7546fb](https://github.com/GraphiteEditor/Graphite/commit/d7546fb18310490d5ce10d406e7a3faaf1ae98fe))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/percentage-value-node.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/percentage-value-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *Gradient Map* node for recoloring grayscale color values to corresponding colors along a chosen gradient <small>([commit 501b562](https://github.com/GraphiteEditor/Graphite/commit/501b562d0f17b91a54a14c83a4a0f90a1e03e369))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/gradient-map-node.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/gradient-map-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *Assign Colors* node for replacing the fill or stroke colors of individual paths in a group of vector elements using choices uniquely sampled along a gradient <small>([#1938](https://github.com/GraphiteEditor/Graphite/pull/1938))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/assign-colors-node.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/assign-colors-node.mp4" type="video/mp4" />
  	</video>
  </div>

- *Dehaze* node for reducing the appearance of the atmospheric haze or fog in photographs <small>([#1882](https://github.com/GraphiteEditor/Graphite/pull/1882))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/dehaze-node.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/dehaze-node.mp4" type="video/mp4" />
  	</video>
  </div>

- Node catalog reorganized with the naming and categorization of nodes given improved consistency <small>([commit e647ca9](https://github.com/GraphiteEditor/Graphite/commit/e647ca9f91a5e823137122126fe9e980f65d62ea))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/reorganized-node-catalog.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/reorganized-node-catalog.mp4" type="video/mp4" />
  	</video>
  </div>

- *Noise Pattern* node updated to generate resolution-aware coherent noise, meaning it extends forever in all directions and zoom depths <small>([#1909](https://github.com/GraphiteEditor/Graphite/pull/1909))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/resolution-aware-noise.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/resolution-aware-noise.mp4" type="video/mp4" />
  	</video>
  </div>

- [Dennis Kobert](https://github.com/truedoctor)'s Google Summer of Code project has concluded, adding many improvements to performance and internal improvements listed in the following sections, but also:

  - Alternate render engine using [Vello](https://github.com/linebender/vello) that brings increased code simplicity and performance (especially with raster layers), but currently only works in browsers with [WebGPU support](https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API#browser_compatibility) and must be enabled via *File* > *Preferences* <small>([#1802](https://github.com/GraphiteEditor/Graphite/pull/1802), [#1865](https://github.com/GraphiteEditor/Graphite/pull/1865), [#1844](https://github.com/GraphiteEditor/Graphite/pull/1844), [#1871](https://github.com/GraphiteEditor/Graphite/pull/1871), [#1874](https://github.com/GraphiteEditor/Graphite/pull/1874), [#1875](https://github.com/GraphiteEditor/Graphite/pull/1875), [#1899](https://github.com/GraphiteEditor/Graphite/pull/1899), [#1905](https://github.com/GraphiteEditor/Graphite/pull/1905), [#1900](https://github.com/GraphiteEditor/Graphite/pull/1900), [#1907](https://github.com/GraphiteEditor/Graphite/pull/1907), [#1902](https://github.com/GraphiteEditor/Graphite/pull/1902), [#1897](https://github.com/GraphiteEditor/Graphite/pull/1897), [#1915](https://github.com/GraphiteEditor/Graphite/pull/1915), [#1996](https://github.com/GraphiteEditor/Graphite/pull/1996))</small>
    
    <img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/vello-preference.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Editor preferences menu with Vello setting" />

  - The [final report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1773) are available for more details

- [Adam Gerhant](https://github.com/adamgerhant)'s Google Summer of Code project has concluded, adding several high-impact features and improvements to the node graph editing experience, including:

  - Layer node chains that help keep a linear sequence of nodes organized by encapsulating them within their destination layer <small>([#1794](https://github.com/GraphiteEditor/Graphite/pull/1794), [#1812](https://github.com/GraphiteEditor/Graphite/pull/1812), [commit e28e841](https://github.com/GraphiteEditor/Graphite/commit/e28e841e3bef401c19f5ddcf01c68fedd6b5c9c3))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-chains.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-chains.mp4" type="video/mp4" />
    	</video>
    </div>

  - Subgraph data import/export connectors drawn along the top left/right sides of the graph, instead of the previous representation as node-looking boxes <small>([#1794](https://github.com/GraphiteEditor/Graphite/pull/1794), [commit 8e774ef](https://github.com/GraphiteEditor/Graphite/commit/8e774efe9dae51c0c1db2928e22c6de5b93d0584), [#1911](https://github.com/GraphiteEditor/Graphite/pull/1911))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-imports-exports.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-imports-exports.mp4" type="video/mp4" />
    	</video>
    </div>

  - Layer selection history feature, letting users go back and forth between prior states of which layers were selected using the back/forward navigation buttons on some mice (Chromium-based browsers only) or the <kbd>Alt</kbd><kbd>[</kbd> and <kbd>Alt</kbd><kbd>]</kbd> hotkeys <small>([#1945](https://github.com/GraphiteEditor/Graphite/pull/1945))</small>
      <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/selection-history.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/selection-history.mp4" type="video/mp4" />
    	</video>
    </div>

  - Improved layer positioning for upstream nodes when being reordered so they don't chaotically overlap or leave behind large gaps anymore <small>([#1928](https://github.com/GraphiteEditor/Graphite/pull/1928), [#1945](https://github.com/GraphiteEditor/Graphite/pull/1945))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-reordering.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-reordering.mp4" type="video/mp4" />
    	</video>
    </div>

  - Improved shifting of layers in stacks so they act as blocks that collide and bump other layers/nodes instead of getting stuck inside each other <small>([#1940](https://github.com/GraphiteEditor/Graphite/pull/1940))</small>
    <div class="video-background">
    	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-block-collision.webm" type="video/webm" />
    		<source src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/node-organization-block-collision.mp4" type="video/mp4" />
    	</video>
    </div>

- [Elbert Ronnie](https://github.com/elbertronnie)'s Google Summer of Code project, building a library for decoding raw image formats, has continued and will wrap up in early November:

  - Implementation of the pre-processing and demosaicing steps in the decoding pipeline <small>([#1796](https://github.com/GraphiteEditor/Graphite/pull/1796))</small>

  - Implementation of the post-processing steps in the decoding pipeline <small>([#1923](https://github.com/GraphiteEditor/Graphite/pull/1923))</small>

  - Factoring in of camera white balance metadata when available <small>([#1941](https://github.com/GraphiteEditor/Graphite/pull/1941))</small>

  - Flipping and rotating of images based on camera orientation metadata <small>([#1954](https://github.com/GraphiteEditor/Graphite/pull/1954))</small>

  - Automated tests now made to run in parallel <small>([#1968](https://github.com/GraphiteEditor/Graphite/pull/1968))</small>

  - [Weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1771) are being posted while the project is ongoing, following a mid-summer hiatus

## Performance

- Caching of boolean operations enabled by fixing the Select tool's layer click targets robustly despite the boolean node altering the shape data <small>([commit 6ecb173](https://github.com/GraphiteEditor/Graphite/commit/6ecb173c1c06807f13a859ef90b7d7f43af042be), [#1946](https://github.com/GraphiteEditor/Graphite/pull/1946), [#1986](https://github.com/GraphiteEditor/Graphite/pull/1986))</small>

- Rewritten boolean operations algorithm (our new `path-bool` library) that runs purely in Rust instead of making high-overhead calls into a JavaScript library <small>([#1952](https://github.com/GraphiteEditor/Graphite/pull/1952), [#2000](https://github.com/GraphiteEditor/Graphite/pull/2000))</small>

- Internal hash-based data tracking now benefits from caching of hash calculations <small>([#1876](https://github.com/GraphiteEditor/Graphite/pull/1876))</small>

- Refactored document node type lookup process that fixes performance degradation over time <small>([#1878](https://github.com/GraphiteEditor/Graphite/pull/1878))</small>

- Speed-ups to the node graph compilation that must occur after every change before it's rendered <small>([#1924](https://github.com/GraphiteEditor/Graphite/pull/1924))</small>

- Usage of a coarse bounding box when clicking on layers to avoid a detailed check for intersection <small>([#1887](https://github.com/GraphiteEditor/Graphite/pull/1887))</small>

## Fixes

- Fix for rulers showing in the wrong spot when initially opening a document <small>([#1801](https://github.com/GraphiteEditor/Graphite/pull/1801))</small>

- Fix for backwards compatibility broken in a prior change <small>([commit 621f469](https://github.com/GraphiteEditor/Graphite/commit/621f469a150d4a14c86ba9be87df82aae5dded74))</small>

- Fix for boolean operations containing multiple subpaths <small>([#1804](https://github.com/GraphiteEditor/Graphite/pull/1804))</small>

- Fix for a bug causing some node outputs to fail connecting wires to other inputs <small>([commit 84d7262](https://github.com/GraphiteEditor/Graphite/commit/84d72621e545ba7743e38d46ce1607133bb13461))</small>

- Fix for the 'Zoom with Scroll' preference not working after a page refresh <small>([#1807](https://github.com/GraphiteEditor/Graphite/pull/1807))</small>

- Fix for broken extending of paths in the Pen and Freehand tools <small>([#1809](https://github.com/GraphiteEditor/Graphite/pull/1809))</small>

- Fix for a snapping bug where snapped-together shapes couldn't be moved <small>([#1818](https://github.com/GraphiteEditor/Graphite/pull/1818))</small>

- Fix for transforms in groups not being pre-applied in the boolean node <small>([#1825](https://github.com/GraphiteEditor/Graphite/pull/1825))</small>

- Fix for dragging groups so the transform node is inserted as intended <small>([#1829](https://github.com/GraphiteEditor/Graphite/pull/1829))</small>

- Fix for restoring older prototype GPU infrastructure functionality to work again <small>([#1797](https://github.com/GraphiteEditor/Graphite/pull/1797))</small>

- Fix for a breakage to shallow select mode in the Select tool <small>([#1843](https://github.com/GraphiteEditor/Graphite/pull/1843))</small>

- Fix for the graph not being updated in the UI after using <kbd>Ctrl</kbd><kbd>D</kbd> to duplicate a node <small>([#1852](https://github.com/GraphiteEditor/Graphite/pull/1852))</small>

- Fix for the Layers panel UI not updating when the selection changes or a layer is expanded/collapsed <small>([#1847](https://github.com/GraphiteEditor/Graphite/pull/1847))</small>

- Fix for the rendering of non-closed paths and additional improvements to layer bounding box calculation <small>([#1859](https://github.com/GraphiteEditor/Graphite/pull/1859))</small>

- Fix for how documents are saved with images now correctly serialized in linear space instead of sRGB <small>([#1856](https://github.com/GraphiteEditor/Graphite/pull/1856))</small>

- Fix for the grid overlay color choice that was incorrectly appearing as no color <small>([commit a4a5139](https://github.com/GraphiteEditor/Graphite/commit/a4a513911dac29ee5c6546842be194b1513f2cec))</small>

- Fix for viewport bounds getting out of sync at times, like when toggling rulers <small>([commit 0617759](https://github.com/GraphiteEditor/Graphite/commit/06177597ae8ef52f7a273875d6afb63fb09ec3c8))</small>

- Fix for compatibility issues with rendered SVG output syntax <small>([#1880](https://github.com/GraphiteEditor/Graphite/pull/1880))</small>

- Fix for node graph type errors that were not being shown <small>([#1917](https://github.com/GraphiteEditor/Graphite/pull/1917))</small>

- Fix to add colors to all nodes in a graph, even if disconnected, and properly display hidden network imports <small>([#1921](https://github.com/GraphiteEditor/Graphite/pull/1921))</small>

- Fix to respect the "Clip" parameter on Artboards when exporting "All Artworks" <small>([#1916](https://github.com/GraphiteEditor/Graphite/pull/1916))</small>

- Fix to improve the undo/redo history system's robustness <small>([#1945](https://github.com/GraphiteEditor/Graphite/pull/1945))</small>

- Fix for a crash caused by gradients with bounds of zero <small>([#1950](https://github.com/GraphiteEditor/Graphite/pull/1950))</small>

- Fix for SVG importing and exporting which had both broken <small>([#1953](https://github.com/GraphiteEditor/Graphite/pull/1953))</small>

- Fix for a crash in our Bezier-rs library when outlining a small path <small>([#1958](https://github.com/GraphiteEditor/Graphite/pull/1958))</small>

- Fix for a crash due to an invalid segment when dissolving a vector path point loop <small>([#1959](https://github.com/GraphiteEditor/Graphite/pull/1959))</small>

- Fix to improve how integer ruler intervals are set when zoomed in <small>([#1966](https://github.com/GraphiteEditor/Graphite/pull/1966))</small>

- Fix for layer stacks inadvertently producing a 0x0 image because of an incorrect default type for the disconnected bottom layer node input <small>([#1965](https://github.com/GraphiteEditor/Graphite/pull/1965))</small>

- Fix to make the primitive shape tools avoid setting a negative transform scale <small>([#1973](https://github.com/GraphiteEditor/Graphite/pull/1973))</small>

- Fix to nudge only the shallowest selected layers to avoid amplified translation <small>([#1975](https://github.com/GraphiteEditor/Graphite/pull/1975))</small>

- Fix to the Select tool's click targets which had extended too far on narrow layers <small>([#1976](https://github.com/GraphiteEditor/Graphite/pull/1976))</small>

- Fix to correctly apply transforms to vector data and strokes <small>([#1977](https://github.com/GraphiteEditor/Graphite/pull/1977))</small>

- Fix for the types imported into subgraphs <small>([#1978](https://github.com/GraphiteEditor/Graphite/pull/1978))</small>

- Fix to now properly use overlays for drawing artboard names in the viewport <small>([#1981](https://github.com/GraphiteEditor/Graphite/pull/1981))</small>

- Fix for broken Opacity slider in Layers panel <small>([#2004](https://github.com/GraphiteEditor/Graphite/pull/2004))</small>

## Internal

- Refactor for graph editing to go through a new abstraction layer, the node network interface <small>([#1794](https://github.com/GraphiteEditor/Graphite/pull/1794), [#1906](https://github.com/GraphiteEditor/Graphite/pull/1906))</small>

- Addition of a new Scope API for exposing data within graphs and subgraphs <small>([#1814](https://github.com/GraphiteEditor/Graphite/pull/1814))</small>

- Reorganization and upgrading of most Cargo dependencies <small>([#1815](https://github.com/GraphiteEditor/Graphite/pull/1815))</small>

- Removal of artboards as a type of graphical element, which is no longer in use as such <small>([#1824](https://github.com/GraphiteEditor/Graphite/pull/1824), [#1830](https://github.com/GraphiteEditor/Graphite/pull/1830))</small>

- Cleanup of many Clippy warnings <small>([#1808](https://github.com/GraphiteEditor/Graphite/pull/1808), [#1936](https://github.com/GraphiteEditor/Graphite/pull/1936), [#1995](https://github.com/GraphiteEditor/Graphite/pull/1995))</small>

- Build process simplifications by auto-installing npm packages and reducing the contributor guide's project setup complexity <small>([#1799](https://github.com/GraphiteEditor/Graphite/pull/1799))</small>

- Fixes for Rust-Analyzer conflicts with build targets and other compile speed issues <small>([commit 857bc77](https://github.com/GraphiteEditor/Graphite/commit/857bc772de8c5731a2eaf7f442beb573ccf8ca4c))</small>

- Fixes, documentation, and updates for npm dependencies and tooling, as well as fixes for Bezier-rs demos not building <small>([#1857](https://github.com/GraphiteEditor/Graphite/pull/1857))</small>

- Partial restoration of the Imaginate node within the modern node graph architecture, but more work remains <small>([#1908](https://github.com/GraphiteEditor/Graphite/pull/1908))</small>

- Profiling metrics development infrastructure for Graphene graph compilation <small>([#1924](https://github.com/GraphiteEditor/Graphite/pull/1924), [#1974](https://github.com/GraphiteEditor/Graphite/pull/1974))</small>

- CI infrastructure to post Clippy warnings as PR comments <small>([#1926](https://github.com/GraphiteEditor/Graphite/pull/1926), [commit a93dcb2](https://github.com/GraphiteEditor/Graphite/commit/a93dcb2776027bfcb385ae9dfddff249bdfd896b))</small>

- CI infrastructure which profiles the demo artwork test documents and comments on PRs that induce sizable changes in performance <small>([#1925](https://github.com/GraphiteEditor/Graphite/pull/1925), [#1939](https://github.com/GraphiteEditor/Graphite/pull/1939))</small>

- CI infrastructure to post a ZIP of self-hosted build assets for the [latest editor release tag on GitHub](https://github.com/GraphiteEditor/Graphite/releases/tag/latest-stable) <small>([commit f2493d5](https://github.com/GraphiteEditor/Graphite/commit/f2493d5308ace728c3525d51609c3a2efcbdc139))</small>

- Refactor to support node ID path insertion at compile time <small>([#1947](https://github.com/GraphiteEditor/Graphite/pull/1947))</small>

- Switch of attribute-based vector data from referencing point IDs to indexes in the points table <small>([#1949](https://github.com/GraphiteEditor/Graphite/pull/1949))</small>

- Refactor for the navigation metadata that stores the node graph UI view's current pan and zoom <small>([#1956](https://github.com/GraphiteEditor/Graphite/pull/1956))</small>

- Refactor for the node definition syntax to make it easier, shorter, and more consistent to define nodes across the board <small>([#1942](https://github.com/GraphiteEditor/Graphite/pull/1942), [#1999](https://github.com/GraphiteEditor/Graphite/pull/1999), [#2002](https://github.com/GraphiteEditor/Graphite/pull/2002))</small>

- Fix for a crash when using the experimental Graphene CLI tool <small>([#1993](https://github.com/GraphiteEditor/Graphite/pull/1993))</small>

- Upgrades to quality of third-party library license notice generation <small>([commit 14de67c](https://github.com/GraphiteEditor/Graphite/commit/14de67c5a78a1e8369a8b023da441237a44e48cb))</small>

- Cleanup for various TypeScript code errors and additions to CI infrastructure to enforce TS error checking <small>([commit 1ee5ffb](https://github.com/GraphiteEditor/Graphite/commit/1ee5ffbbe8a64da5a83da889dd16d41c3463c332))</small>

- Simplifications to the [Bezier-rs web demos](https://graphite.rs/libraries/bezier-rs/) to reduce the bundle size and streamline its code <small>([commit 0b0169a](https://github.com/GraphiteEditor/Graphite/commit/0b0169a415453b9d9910d765f0538ecd67b618c0))</small>

- Manually-runnable benchmarks for document runtime speed profiling <small>([#2005](https://github.com/GraphiteEditor/Graphite/pull/2005))</small>

## Website

- Revamps to the website <small>([commit 5d74178](https://github.com/GraphiteEditor/Graphite/commit/5d74178f5f417afdd70abec596d38f22d490240a), [commit 40fd447](https://github.com/GraphiteEditor/Graphite/commit/40fd4473a784ba24fc3105f8da56baacccf2dcf5), [commit 98ab069](https://github.com/GraphiteEditor/Graphite/commit/98ab069a1703e89cea031fedffc55c3bf4191b5d))</small>

- Updates to various content on the website <small>([commit fb7d597](https://github.com/GraphiteEditor/Graphite/commit/fb7d5970b38c61fc1a35aeefd7028858c96f5ff2))</small>

## Announcements

### Graphite @ Maker Faire

<img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/maker-faire-banner.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Poster for the Bay Area Maker Faire" />

Graphite will have a booth at the [Bay Area Maker Faire](https://makerfaire.com/bay-area/) **this weekend, October 18–20**. If you're in northern California, come visit and meet our team. We'll be located in Coal Shed 2 and plan to showcase the latest features, answer questions, teach how to use the app, get to know the maker community, and give away stickers and art postcards. We hope to see you there!

### Nodevember

<img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/nodevember-logo.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Poster for the Bay Area Maker Faire" />

We're excited to be participating in [Nodevember](https://nodevember.io/) next month! The event celebrates procedural art and design with themed prompts throughout the month. We're looking forward to seeing what the community creates with Graphite's nodes. We'll be sharing some of our favorite submissions on our social media channels. Be sure to tag `@GraphiteEditor` and use the `#MadeWithGraphite` hashtag together with `#Nodevember2024`.

### Community art contest

Congratulations to [VDawg](https://www.instagram.com/vdawg.jpg/), the winner of the community art contest, whose work is featured below.

<img src="https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024/art-contest-winner.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Winning artwork depicts a dark landscape with an ethereal diamond forming a constellation with the stars above" />
