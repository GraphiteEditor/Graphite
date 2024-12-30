+++
title = "Graphite progress report (Q4 2024)"
date = 2024-12-30
[extra]
banner = "..."
banner_png = "..."
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q4 2024 update includes a multitude of bug fixes and quality of life improvements."
reddit = "..."
twitter = "..."
+++

[Graphite](/), a new open source 2D procedural graphics editor, has spent October-December on a multitude of smaller quality-of-life features and bug fixes, making Graphite a usable alternative to vector graphics software such as Inkscape for the web.

All Q4 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-10-01&until=2024-12-31) and all noteworthy changes are detailed below.

<!-- more -->

This is the forth in our series of quarterly progress reports (congratulations on a full year @Keavon). If you missed the [last one](../graphite-progress-report-q3-2024), be sure to check it out as well. If you'd like to help speed up future progress, please consider [getting involved](/volunteer) with code, QA/bug testing, or art/marketing projects. [Donations](/donate) are also valued, as are [stars of GitHub](https://github.com/GraphiteEditor/Graphite). Follow along and partake in our [Discord community](https://discord.graphite.rs), too.

The new *todo!()* artwork shown here ...


<div class="demo-artwork">
	<a href="...">
		<img src="..." onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of ..." />
	</a>
	<p>
		<span>
			<em>todo!()</em>
		</span>
		<br />
		<span>
			<a href="...">Open this artwork</a> to<br />explore it yourself.
		</span>
	</p>
</div>

## Additions

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->
- Add line height and character spacing to the Text node <small>([#2016](https://github.com/GraphiteEditor/Graphite/pull/2016))</small>
- Add support for pinning nodes in the Properties panel <small>([commit e6d8c47](https://github.com/GraphiteEditor/Graphite/commit/e6d8c4743d2aff15985c929df2cc7381a61908a0))</small>
- New demo artwork: "Changing Seasons" <small>([commit fa6b5f2](https://github.com/GraphiteEditor/Graphite/commit/fa6b5f298adf395362e1aaa2c07be89fa89eaee2))</small>
- New node: Offset Path <small>([#2030](https://github.com/GraphiteEditor/Graphite/pull/2030))</small>
- Make Copy to Points and (Circular) Repeat and nodes output group data, and add flattening nodes <small>([#2011](https://github.com/GraphiteEditor/Graphite/pull/2011))</small>
- Allow the Fill and Stroke nodes to work on groups <small>([#2046](https://github.com/GraphiteEditor/Graphite/pull/2046))</small>
- Add switch node and fix log to console node <small>([#2064](https://github.com/GraphiteEditor/Graphite/pull/2064))</small>
- New node: Bevel <small>([#2067](https://github.com/GraphiteEditor/Graphite/pull/2067))</small>
- Add a node insertion button and layer renaming from the Properties panel <small>([#2072](https://github.com/GraphiteEditor/Graphite/pull/2072))</small>
- In the Path tool, make Space shift the anchor while dragging handles <small>([#2065](https://github.com/GraphiteEditor/Graphite/pull/2065))</small>
- Add Path tool support for the Tab key swapping to dragging the opposite handle <small>([#2058](https://github.com/GraphiteEditor/Graphite/pull/2058))</small>
- Allow the Pen tool to connect layers by their endpoints, merging into a single layer <small>([#2076](https://github.com/GraphiteEditor/Graphite/pull/2076))</small>
- New nodes: "Clamp", "To U32", and "To U64" <small>([#2087](https://github.com/GraphiteEditor/Graphite/pull/2087))</small>
- New node: Jitter Points <small>([commit 7d86bf4](https://github.com/GraphiteEditor/Graphite/commit/7d86bf4abf7edfe6a5d021075e050614bee07c13))</small>
- Improve the node graph with revamped top bar and disabling tools when graph is open <small>([#2093](https://github.com/GraphiteEditor/Graphite/pull/2093))</small>
- Add merging nodes into a subgraph with Ctrl+M and basic subgraph signature customization <small>([#2097](https://github.com/GraphiteEditor/Graphite/pull/2097))</small>
- New node: Dot Product <small>([#2126](https://github.com/GraphiteEditor/Graphite/pull/2126))</small>
- New node: Math <small>([#2121](https://github.com/GraphiteEditor/Graphite/pull/2121))</small>
- Add Freehand tool drawing new subpaths on an existing layer with Shift held <small>([commit ed119ad](https://github.com/GraphiteEditor/Graphite/commit/ed119ad3d799030dbc488ccfc8ca9ad057eeff2c))</small>
- Automatically place layers into the artboard they're drawn inside of <small>([#2110](https://github.com/GraphiteEditor/Graphite/pull/2110))</small>
- Add more actions to the Layer menu bar entries <small>([commit feba874](https://github.com/GraphiteEditor/Graphite/commit/feba87449bb490e47df6f267576bec5ab4238dc3))</small>
- Add selection removal to the Select tool's box select (Ctrl+Shift modifier) <small>([#2162](https://github.com/GraphiteEditor/Graphite/pull/2162))</small>


## Fixes
- Make Upload Texture node resolution-aware <small>([#2018](https://github.com/GraphiteEditor/Graphite/pull/2018))</small>
- Always close subpaths before applying boolean ops <small>([#2014](https://github.com/GraphiteEditor/Graphite/pull/2014))</small>
- Fix double clicking anchor to convert between smooth and sharp <small>([#2023](https://github.com/GraphiteEditor/Graphite/pull/2023))</small>
- Fix faulty contravariance checking <small>([#2025](https://github.com/GraphiteEditor/Graphite/pull/2025))</small>
- Fix the Scatter Points node <small>([commit 7a56af0](https://github.com/GraphiteEditor/Graphite/commit/7a56af01efc82460e780c78b008a52487972a7eb))</small>
- Update Wasm dependencies to fix WebGPU spec change crash in Firefox with Vello <small>([#2027](https://github.com/GraphiteEditor/Graphite/pull/2027))</small>
- Factor in artboard clipping to the click target x-ray function <small>([#2028](https://github.com/GraphiteEditor/Graphite/pull/2028))</small>
- Fix Ctrl+H layer hiding and Ctrl+L layer locking only working with the graph open <small>([#2029](https://github.com/GraphiteEditor/Graphite/pull/2029))</small>
- Improve text overlay styling and fix artboard label positioning bug <small>([#2032](https://github.com/GraphiteEditor/Graphite/pull/2032))</small>
- Fix crash from empty document with no artboards introduced in #2028 <small>([#2036](https://github.com/GraphiteEditor/Graphite/pull/2036))</small>
- Fix brush tool broken by #2011 <small>([#2045](https://github.com/GraphiteEditor/Graphite/pull/2045))</small>
- Allow multiple top output wires to come from layers <small>([#2049](https://github.com/GraphiteEditor/Graphite/pull/2049))</small>
- Fix various crashes and bugs <small>([#2075](https://github.com/GraphiteEditor/Graphite/pull/2075))</small>
- Fix gradient render transforms with Vello <small>([#2059](https://github.com/GraphiteEditor/Graphite/pull/2059))</small>
- Fix alignment snapping not preserving aspect ratio when Shift is held <small>([#2062](https://github.com/GraphiteEditor/Graphite/pull/2062))</small>
- Fix Text tool clearing text when hitting Escape by changing it to commit the edit instead <small>([#2052](https://github.com/GraphiteEditor/Graphite/pull/2052))</small>
- Allow the Path tool to edit an upstream path even if there's a type conversion midway <small>([#2055](https://github.com/GraphiteEditor/Graphite/pull/2055))</small>
- Fix layer name text input <small>([#2081](https://github.com/GraphiteEditor/Graphite/pull/2081))</small>
- Fix NumberInput widget not being reactive to changes to the unit <small>([#2080](https://github.com/GraphiteEditor/Graphite/pull/2080))</small>
- Improve Sample Points, Scatter Points, and Splines from Points to include segments and work with subpaths <small>([#2085](https://github.com/GraphiteEditor/Graphite/pull/2085))</small>
- Avoid sometimes breaking the selected layer upon switching away from the Select tool <small>([commit 8d3da83](https://github.com/GraphiteEditor/Graphite/commit/8d3da83606c23366d2688602afbc0917e7224e68))</small>
- Hide the left border notch in layers when a wire isn't entering from the layer's left <small>([commit 12ca060](https://github.com/GraphiteEditor/Graphite/commit/12ca06035cd7463ed895671ff7eebe53fde655c6))</small>
- Fix point nudging to work in document space <small>([#2095](https://github.com/GraphiteEditor/Graphite/pull/2095))</small>
- Fix Bevel node crash with zero-length segments <small>([#2096](https://github.com/GraphiteEditor/Graphite/pull/2096))</small>
- Fix the spline node algorithm to be continuous across start/end points <small>([#2092](https://github.com/GraphiteEditor/Graphite/pull/2092))</small>
- Improve nudging when tilted and add Artboard tool nudge resizing; disable menu bar entries when no layer is selected <small>([#2098](https://github.com/GraphiteEditor/Graphite/pull/2098))</small>
- Add Brush tool warning; move font list loading to document creation time <small>([commit de366f9](https://github.com/GraphiteEditor/Graphite/commit/de366f951424fcdf4463a419db3fa659910fabfd))</small>
- Make the Pen tool only append new paths when Shift is held <small>([#2102](https://github.com/GraphiteEditor/Graphite/pull/2102))</small>
- Make Pen tool always snap to endpoint anchors, even when snapping is off <small>([#2107](https://github.com/GraphiteEditor/Graphite/pull/2107))</small>
- Fix crash when upgrading a document with a Modulo node from 3 commits ago <small>([commit 4c4d559](https://github.com/GraphiteEditor/Graphite/commit/4c4d559d97b4d131d2777c0aab19590531ae47a9))</small>
- Clean up editor preferences dialog <small>([commit 99cf8f0](https://github.com/GraphiteEditor/Graphite/commit/99cf8f0c4f91a051b59fc2c9e5cc6c7417bdd74b))</small>
- Refactor collection of snap targets <small>([#2114](https://github.com/GraphiteEditor/Graphite/pull/2114))</small>
- Remove Double-Click Behavior for Switching to Path Tool on Non-Path Layers <small>([#2116](https://github.com/GraphiteEditor/Graphite/pull/2116))</small>
- Disabling colinear state when both the handles are selected and moved <small>([#2120](https://github.com/GraphiteEditor/Graphite/pull/2120))</small>
- Fix bitmap bounding box <small>([#2122](https://github.com/GraphiteEditor/Graphite/pull/2122))</small>
- Fixes shortcut key not showed in File->Close <small>([#2135](https://github.com/GraphiteEditor/Graphite/pull/2135))</small>
- Fix SVG `viewBox` capitalization in renderer <small>([#2131](https://github.com/GraphiteEditor/Graphite/pull/2131))</small>
- Tidy up the UI with clearer Layers panel selection marks and removal of most "coming soon" UI elements <small>([commit 1264ea8](https://github.com/GraphiteEditor/Graphite/commit/1264ea8246cbb06e0602a93be983762ab17adf30))</small>
- Improve issues with selection history <small>([#2138](https://github.com/GraphiteEditor/Graphite/pull/2138))</small>
- Fix transform cage rotation abort causing broken state upon next transformation <small>([#2149](https://github.com/GraphiteEditor/Graphite/pull/2149))</small>
- Make Path tool deselect all points on single-click, and select all on double-click, of shape's fill <small>([#2148](https://github.com/GraphiteEditor/Graphite/pull/2148))</small>
- Fix Select tool's box selection not being able to extend a selection with shift <small>([#2157](https://github.com/GraphiteEditor/Graphite/pull/2157))</small>
- Fix several bugged vector-related nodes <small>([commit b81f483](https://github.com/GraphiteEditor/Graphite/commit/b81f48385afc8c9c27820ffe8d5953529f89b7bd))</small>
- Improve quick measurement overlays across all possible arrangement scenarios <small>([#2147](https://github.com/GraphiteEditor/Graphite/pull/2147))</small>
- Improve quick measurement overlays with better number alignment and decimal rounding <small>([#2155](https://github.com/GraphiteEditor/Graphite/pull/2155))</small>
- Improve Text tool click targets on text layers to use the text box <small>([#2145](https://github.com/GraphiteEditor/Graphite/pull/2145))</small>

## Internal
- Simplify the Bezier-rs interactive web demo code <small>([#2020](https://github.com/GraphiteEditor/Graphite/pull/2020))</small>
- Fix/suppress new Clippy warnings introduced in Rust 1.82 <small>([commit a395fbf](https://github.com/GraphiteEditor/Graphite/commit/a395fbf0637c23b8b05a7c451ff7b8421587b655))</small>
- Raw-rs: Refactor to run multiple steps in a single loop <small>([#1972](https://github.com/GraphiteEditor/Graphite/pull/1972))</small>
- Raw-rs: Fix naming convention of matrices <small>([#2071](https://github.com/GraphiteEditor/Graphite/pull/2071))</small>
- Raw-rs: Remove from workspace <small>([#2066](https://github.com/GraphiteEditor/Graphite/pull/2066))</small>
- Clean up old usages of `NodeId(generate_uuid())` by replacing it with `NodeId::new()` <small>([#2009](https://github.com/GraphiteEditor/Graphite/pull/2009))</small>
- Fix Bezier-rs web demos failing to run wasm-opt in CI <small>([commit 4df7803](https://github.com/GraphiteEditor/Graphite/commit/4df780391c0cbb87b12812d0249ed9b62e2740e4))</small>
- Raw-rs: Remove fortuples dependency <small>([#2082](https://github.com/GraphiteEditor/Graphite/pull/2082))</small>
- Rename Raw-rs to Rawkit <small>([#2088](https://github.com/GraphiteEditor/Graphite/pull/2088))</small>
- Parse description from node doc comments <small>([#2089](https://github.com/GraphiteEditor/Graphite/pull/2089))</small>
- Upgrade and document the math operation nodes <small>([commit d649052](https://github.com/GraphiteEditor/Graphite/commit/d649052255c10c15754c3a3707f2edf996d2468d))</small>
- Bezier-rs: Make rectangle constructor produce linear segments <small>([#2109](https://github.com/GraphiteEditor/Graphite/pull/2109))</small>
- Add math-parser library <small>([#2033](https://github.com/GraphiteEditor/Graphite/pull/2033))</small>
- Fix clippy lints <small>([#2119](https://github.com/GraphiteEditor/Graphite/pull/2119))</small>
- Simplify the implementation of the message buffering <small>([#2123](https://github.com/GraphiteEditor/Graphite/pull/2123))</small>
- Update some dependencies <small>([#2134](https://github.com/GraphiteEditor/Graphite/pull/2134))</small>
- Parse doc comments on node parameters <small>([#2163](https://github.com/GraphiteEditor/Graphite/pull/2163))</small>

## Website
- Blog post: Graphite progress report (Q3 2024) <small>([#2013](https://github.com/GraphiteEditor/Graphite/pull/2013))</small>
- Update the screenshots on the website home page carousel <small>([commit f892687](https://github.com/GraphiteEditor/Graphite/commit/f89268757614bda5f949144f84ccc79bd33ddec2))</small>
- Update the website roadmap and other details <small>([commit d7a271f](https://github.com/GraphiteEditor/Graphite/commit/d7a271f67595863835e80e58636fead9b1fe0a1d))</small>


## Announcements


### Community art contest

There were no submissions to the Q4 art contest. Post your creations in the `#🎨art-showcase` channel in [our Discord](https://discord.graphite.rs) before the end of March to be featured in the Q1 progress report.
