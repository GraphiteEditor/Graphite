+++
title = "Graphite progress report (Q1 2024)"
date = 2024-05-09
[extra]
banner = "https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024__2.avif"
banner_png = "https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024__2.png"
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q1 2024 update introduces a precise snapping system and a customizable grid for enhanced design control. The update also includes improved procedural scattering with the 'Copy to Points' node, demonstrated in new demo artwork."
reddit = "https://www.reddit.com/r/graphite/comments/1coa0if/blog_post_graphite_progress_report_q1_2024/"
twitter = "https://twitter.com/GraphiteEditor/status/1788698448348266946"
css = ["/component/demo-artwork.css"]
+++

[Graphite](/) is a new 2D vector graphics editor bringing a modern, nondestructive approach to creative workflows with node-based procedural generation. The project is currently three years into development, with a focus on streamlining the creative process for procedural vector artwork. See the [roadmap](/features#roadmap) for a more in-depth summary of the goals for 2024 and beyond.

<!-- more -->

Starting in 2024, we are now publishing quarterly reports to summarize the new features and improvements made to Graphite. If you missed the [2023 year in review](../looking-back-on-2023-and-what-s-next), be sure to check it out after this. We anticipate sending our first email newsletter (with more to follow roughly quarterly) in the near future as well, so be sure to [subscribe](/#newsletter) if you haven't already.

Over the first three months of the year, we are delighted to have seen many contributions both from new and recurrent contributors, including substantial interest by students through [Google Summer of Code](/blog/graphite-internships-announcing-participation-in-gsoc-2024/). We would like to send a big thanks to all of the contributors who made this progress happen. If you are interested in getting involved or just following development, see the [contributor guide](/volunteer/guide) and join [our Discord](https://discord.graphite.rs).

All Q1 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-01-01&until=2024-03-31) and all noteworthy changes are detailed below. As two of the major new features are the grid and snapping systems, the *Isometric Fountain* artwork shown on this blog post demonstrates what those features can achieve.

<div class="demo-artwork">
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

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->

## Additions

- *Copy to Points* node improvements, including randomization of scale and rotation with biasing <small>([#1540](https://github.com/GraphiteEditor/Graphite/pull/1540), [#1541](https://github.com/GraphiteEditor/Graphite/pull/1541), [commit ed82c5](https://github.com/GraphiteEditor/Graphite/commit/ed82c5f20fccd66a959334dee33351657968cdb6), [#1592](https://github.com/GraphiteEditor/Graphite/pull/1592), [commit 7e5069](https://github.com/GraphiteEditor/Graphite/commit/7e5069f638cfcc3e7af21f32eded67a005490402))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/copy-to-points-node-improvements.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/copy-to-points-node-improvements.mp4" type="video/mp4" />
  	</video>
  </div>

- Improvements to the node graph UI <small>([#1564](https://github.com/GraphiteEditor/Graphite/pull/1564), [#1568](https://github.com/GraphiteEditor/Graphite/pull/1568))</small>

- Grid overlay for the canvas with customizable rectangular and isometric grid lines <small>([#1521](https://github.com/GraphiteEditor/Graphite/pull/1521), [commit 5c9d3c](https://github.com/GraphiteEditor/Graphite/commit/5c9d3c5d755e67411c110c0d5fd38d991cb6696c))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/grid-popover.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/grid-popover.mp4" type="video/mp4" />
  	</video>
  </div>
  
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/grid-demo-art.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/grid-demo-art.mp4" type="video/mp4" />
  	</video>
  </div>

- New and improved snapping system for aligning shapes with one another and the grid <small>([#1521](https://github.com/GraphiteEditor/Graphite/pull/1521), [#1567](https://github.com/GraphiteEditor/Graphite/pull/1567), [#1547](https://github.com/GraphiteEditor/Graphite/pull/1547), [#1570](https://github.com/GraphiteEditor/Graphite/pull/1570), [#1574](https://github.com/GraphiteEditor/Graphite/pull/1574))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/snapping.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/snapping.mp4" type="video/mp4" />
  	</video>
  </div>

- *Morph* node for tweening vector shapes <small>([#1576](https://github.com/GraphiteEditor/Graphite/pull/1576))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/morph-node.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/morph-node.mp4" type="video/mp4" />
  	</video>
  </div>

- Visualize which nodes are the source of a graph type error <small>([#1577](https://github.com/GraphiteEditor/Graphite/pull/1577))</small>
  
  <img src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/graph-error-visualization.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graph error visualization screenshot" />

- Improvements to document tilting and resetting tilt <small>([commit 8eef96](https://github.com/GraphiteEditor/Graphite/commit/8eef96511e575d58a36289f3d0e30eb68098b4e7))</small>

- SVG import <small>([#1579](https://github.com/GraphiteEditor/Graphite/pull/1579), [#1656](https://github.com/GraphiteEditor/Graphite/pull/1656))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/svg-import.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/svg-import.mp4" type="video/mp4" />
  	</video>
  </div>

- Re-add Select tool functionality that was lost from a code change last year <small>([#1583](https://github.com/GraphiteEditor/Graphite/pull/1583))</small>

- *Poisson-Disk Points* node for sampling randomly distributed points in a shape, with Red Dress demo artwork <small>([#1586](https://github.com/GraphiteEditor/Graphite/pull/1586), [commit 12e16b](https://github.com/GraphiteEditor/Graphite/commit/12e16b9a4efe40fcf779c34f83d3df8b3e3542f3), [#1590](https://github.com/GraphiteEditor/Graphite/pull/1590), [#1596](https://github.com/GraphiteEditor/Graphite/pull/1596))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/poisson-disk-points-node-demo-art.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/poisson-disk-points-node-demo-art.mp4" type="video/mp4" />
  	</video>
  </div>

- Pen tool point-by-point undo while drawing without wiping out the full in-progress shape <small>([#1587](https://github.com/GraphiteEditor/Graphite/pull/1587), [#1597](https://github.com/GraphiteEditor/Graphite/pull/1597))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/pen-tool-point-by-point-undo.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/pen-tool-point-by-point-undo.mp4" type="video/mp4" />
  	</video>
  </div>

- Freehand tool support for extending the endpoints of open paths <small>([#1594](https://github.com/GraphiteEditor/Graphite/pull/1594), [#1623](https://github.com/GraphiteEditor/Graphite/pull/1623))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/freehand-tool-endpoint-extension.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/freehand-tool-endpoint-extension.mp4" type="video/mp4" />
  	</video>
  </div>

- Path tool support for breaking a closed shape into an open path by deleting (not dissolving) a point <small>([#1593](https://github.com/GraphiteEditor/Graphite/pull/1593))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/point-deletion-breaking-closed-shapes.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/point-deletion-breaking-closed-shapes.mp4" type="video/mp4" />
  	</video>
  </div>

- Path tool insertion of a point on a path segment by sliding to the desired spot <small>([#1581](https://github.com/GraphiteEditor/Graphite/pull/1581))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/sliding-point-insertion.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/sliding-point-insertion.mp4" type="video/mp4" />
  	</video>
  </div>

- Box-based drag selection in the node graph <small>([#1616](https://github.com/GraphiteEditor/Graphite/pull/1616))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/box-selection-in-node-graph.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/box-selection-in-node-graph.mp4" type="video/mp4" />
  	</video>
  </div>

- Auto-save the document every 30 seconds <small>([#1580](https://github.com/GraphiteEditor/Graphite/pull/1580))</small>

- Auto-panning when drawing with each interactive tool when the user's pointer extends past the edge of the viewport <small>([#1625](https://github.com/GraphiteEditor/Graphite/pull/1625), [#1652](https://github.com/GraphiteEditor/Graphite/pull/1652), [#1682](https://github.com/GraphiteEditor/Graphite/pull/1682), [#1690](https://github.com/GraphiteEditor/Graphite/pull/1690))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/auto-panning-in-viewport.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/auto-panning-in-viewport.mp4" type="video/mp4" />
  	</video>
  </div>

- Launch of the Alpha 3 release series in February <small>([commit f02dd5](https://github.com/GraphiteEditor/Graphite/commit/f02dd5c0f625b25bf3510ba0e9839ca182d930e4))</small>
  
  <img src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/alpha-3-about-graphite.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Alpha 3 release series in the 'About Graphite' menu screenshot" />

- Search functionality by typing in menu lists <small>([#1499](https://github.com/GraphiteEditor/Graphite/pull/1499))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/search-in-menu-lists.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/search-in-menu-lists.mp4" type="video/mp4" />
  	</video>
  </div>

- Improvements to the dynamic input hints of several tools <small>([commit 9f8466](https://github.com/GraphiteEditor/Graphite/commit/9f84661facd545bfdfeaa5d37038abeacc00ee08), [#1667](https://github.com/GraphiteEditor/Graphite/pull/1667), [#1670](https://github.com/GraphiteEditor/Graphite/pull/1670))</small>

- *Solidify Stroke* node for converting a stroke into a filled shape <small>([#1650](https://github.com/GraphiteEditor/Graphite/pull/1650))</small>

- Path tool point selection checkbox for locking an anchor point's handles as colinear <small>([commit 5bca93](https://github.com/GraphiteEditor/Graphite/commit/5bca931813e456e2f6035844c21e77ee590b7728))</small>
  <div class="video-background">
  	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/colinear-handles-checkbox.webm" type="video/webm" />
  		<source src="https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024/colinear-handles-checkbox.mp4" type="video/mp4" />
  	</video>
  </div>

## Fixes

- *Sample Points* node performance issues and bug with open paths <small>([#1542](https://github.com/GraphiteEditor/Graphite/pull/1542), [commit b3e4ca](https://github.com/GraphiteEditor/Graphite/commit/b3e4caec1aa4d7b702ac36638b968cf165780149), [commit 93aa10](https://github.com/GraphiteEditor/Graphite/commit/93aa10a76f96946314d5c5787418522423a76544), [#1546](https://github.com/GraphiteEditor/Graphite/pull/1546))</small>

- Some numerical node data incompatible with others <small>([#1543](https://github.com/GraphiteEditor/Graphite/pull/1543), [commit c14c6f](https://github.com/GraphiteEditor/Graphite/commit/c14c6fbe9345be91d04825fe8364e6639430b5e5), [#1618](https://github.com/GraphiteEditor/Graphite/pull/1618))</small>

- Incorrect placement when pasting an image <small>([commit cd61da](https://github.com/GraphiteEditor/Graphite/commit/cd61daf869780e1302de1a34bca7b79485ca69c3))</small>

- Unable to select upside-down (negative scale) layers <small>([#1560](https://github.com/GraphiteEditor/Graphite/pull/1560))</small>

- Memory leak in a type inference hashmap <small>([#1566](https://github.com/GraphiteEditor/Graphite/pull/1566))</small>

- Non-filled shapes are drawn missing the solid color field from their *Fill* node <small>([#1572](https://github.com/GraphiteEditor/Graphite/pull/1572))</small>

- *Circular Repeat* node crash when it has no input connection <small>([#1571](https://github.com/GraphiteEditor/Graphite/pull/1571))</small>

- Safari rendering bug that occurs with Graphite's floating menus <small>([commit b4dccb](https://github.com/GraphiteEditor/Graphite/commit/b4dccb865540e25a6d859e5284a50a36e361d5ee))</small>

- Performance issue affecting Windows users where every render occurred thrice <small>([commit a7bf6e](https://github.com/GraphiteEditor/Graphite/commit/a7bf6e24599fc3d7dc51699f916ba049758a2081))</small>

- Performance issue with nonfunctional caching of images <small>([#1595](https://github.com/GraphiteEditor/Graphite/pull/1595))</small>

- Numerous history steps created while dragging an input widget every frame <small>([#1584](https://github.com/GraphiteEditor/Graphite/pull/1584), [#1598](https://github.com/GraphiteEditor/Graphite/pull/1598))</small>

- Ugly anti-aliasing in overlays from non-alignment with the pixel grid <small>([#1603](https://github.com/GraphiteEditor/Graphite/pull/1603))</small>

- Dragging nodes in the graph along only one axis causes them to snap back <small>([#1619](https://github.com/GraphiteEditor/Graphite/pull/1619))</small>

- Path tool point coordinates using the wrong coordinate system <small>([#1626](https://github.com/GraphiteEditor/Graphite/pull/1626))</small>

- Layer panel grouping causing unintended reordering and loss of names <small>([#1627](https://github.com/GraphiteEditor/Graphite/pull/1627), [#1645](https://github.com/GraphiteEditor/Graphite/pull/1645), [#1672](https://github.com/GraphiteEditor/Graphite/pull/1672), [#1678](https://github.com/GraphiteEditor/Graphite/pull/1678), [#1637](https://github.com/GraphiteEditor/Graphite/pull/1637))</small>

- Keyboard shortcut modifier keys displayed in the UI with the wrong order <small>([commit 4405e0](https://github.com/GraphiteEditor/Graphite/commit/4405e01f5595c76ff8c1cdb7e6ebf752ab53943c))</small>

- Polygon tool stars and convex polygons drawn with an undesired tilt <small>([#1640](https://github.com/GraphiteEditor/Graphite/pull/1640))</small>

- Dropdown menus not navigable with with keyboard arrow keys <small>([#1630](https://github.com/GraphiteEditor/Graphite/pull/1630), [#1649](https://github.com/GraphiteEditor/Graphite/pull/1649))</small>

- Changes to a tool's options only applying after a subsequent usage <small>([#1646](https://github.com/GraphiteEditor/Graphite/pull/1646))</small>

- Orphaned child layers left behind when a group is ungrouped or deleted <small>([#1655](https://github.com/GraphiteEditor/Graphite/pull/1655))</small>

- Some tools lacking a cancelable interaction by right clicking or hitting the Escape key <small>([#1658](https://github.com/GraphiteEditor/Graphite/pull/1658), [#1664](https://github.com/GraphiteEditor/Graphite/pull/1664), [#1666](https://github.com/GraphiteEditor/Graphite/pull/1666))</small>

- Several related issues related to extraneous undo/redo history steps <small>([#1660](https://github.com/GraphiteEditor/Graphite/pull/1660), [#1668](https://github.com/GraphiteEditor/Graphite/pull/1668), [#1675](https://github.com/GraphiteEditor/Graphite/pull/1675))</small>

- Node graph not updated when switching tabs to a different document <small>([#1691](https://github.com/GraphiteEditor/Graphite/pull/1691))</small>

- Artboard deletion causes its child artwork to also get deleted <small>([#1651](https://github.com/GraphiteEditor/Graphite/pull/1651))</small>

- Path tool's point dragging gets offset when the viewport is panned <small>([#1693](https://github.com/GraphiteEditor/Graphite/pull/1693))</small>

- Incorrect scale-nudging behavior when multiple layers are selected <small>([#1699](https://github.com/GraphiteEditor/Graphite/pull/1699))</small>

- Copy-pasted layers not preserving their hidden/visible state <small>([#1698](https://github.com/GraphiteEditor/Graphite/pull/1698))</small>

- Non-editability of hidden layers that are selected <small>([#1697](https://github.com/GraphiteEditor/Graphite/pull/1697))</small>

## Internal

- Several large refactors and code cleanups <small>([#1565](https://github.com/GraphiteEditor/Graphite/pull/1565), [#1582](https://github.com/GraphiteEditor/Graphite/pull/1582), [#1620](https://github.com/GraphiteEditor/Graphite/pull/1620), [#1695](https://github.com/GraphiteEditor/Graphite/pull/1695), [#1708](https://github.com/GraphiteEditor/Graphite/pull/1708))</small>

- Refactor for the vector format to begin being based around a concept of attributes <small>([#1624](https://github.com/GraphiteEditor/Graphite/pull/1624))</small>

- Preconfigure dev containers for easy containerized development environment setup <small>([commit 99c199](https://github.com/GraphiteEditor/Graphite/commit/99c199a8f64a3557e21f5dc002fbcfb789c40632), [#1636](https://github.com/GraphiteEditor/Graphite/pull/1636))</small>
