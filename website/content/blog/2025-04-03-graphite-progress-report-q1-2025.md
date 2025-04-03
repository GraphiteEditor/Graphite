+++
title = "Graphite progress report (Q1 2025)"
date = 2025-04-03
[extra]
banner = "…"
banner_png = "…"
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q1 2025 update introduces experimental animations and improved vector editing"
reddit = "…"
twitter = "…"
css = ["/component/demo-artwork.css"]
+++

[Graphite](/), a new open source 2D procedural graphics editor, has spent January-March introducing animations, new nodes, and improved vector editing.

Through these first three months of 2025, we are delighted to have seen many contributions both from new and recurrent contributors; a big thank you for making this ambitious project more of a reality. If you are interested in getting involved or just following development, see the [contributor guide](/volunteer/guide) and join [our Discord](https://discord.graphite.rs).

All Q1 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2025-01-01&until=2025-03-31) and all noteworthy changes are detailed below.


<!-- more -->


## Additions

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->

- Add Path tool support for G/R/S rotation and scaling with a single selected handle <small>([#2180](https://github.com/GraphiteEditor/Graphite/pull/2180))</small>

- Change Spline tool behavior to use Path and Spline from Points nodes instead of legacy Spline node <small>([#2200](https://github.com/GraphiteEditor/Graphite/pull/2200))</small>

- Add handle visualization during point insertion in the Path tool <small>([#2197](https://github.com/GraphiteEditor/Graphite/pull/2197))</small>

- Polish user-created subgraph nodes: imports in the Properties panel; reorder/delete/rename imports/exports <small>([#2105](https://github.com/GraphiteEditor/Graphite/pull/2105))</small>

- Add visualization overlays to G/R/S <small>([#2195](https://github.com/GraphiteEditor/Graphite/pull/2195))</small>

- Implement extending, joining, and creating new subpaths with the Spline tool <small>([#2203](https://github.com/GraphiteEditor/Graphite/pull/2203))</small>

- Give the current snapping target layer(s) an outline <small>([#2224](https://github.com/GraphiteEditor/Graphite/pull/2224))</small>

- Experimental vector meshes <small>([#2223](https://github.com/GraphiteEditor/Graphite/pull/2223))</small>

- Add an editor preference for touched/enclosed/directional based selection <small>([#2156](https://github.com/GraphiteEditor/Graphite/pull/2156))</small>

- Add G/R/S to the Pen tool to control the outgoing segment handle <small>([#2211](https://github.com/GraphiteEditor/Graphite/pull/2211))</small>

- Make the transform cage show/hide resize grips as space allows <small>([#2209](https://github.com/GraphiteEditor/Graphite/pull/2209))</small>

- Add joining of path endpoints with Ctrl+J in the Path tool <small>([#2227](https://github.com/GraphiteEditor/Graphite/pull/2227))</small>

- Add lasso selection mode <small>([#2235](https://github.com/GraphiteEditor/Graphite/pull/2235))</small>

- Add support for skewing the transform cage by Ctrl-dragging its edges <small>([#2251](https://github.com/GraphiteEditor/Graphite/pull/2251))</small>

- Make primitive shape tools no longer draw transformed unit shapes <small>([#2236](https://github.com/GraphiteEditor/Graphite/pull/2236))</small>

- Add pin and cut icons; improve menu bar shortcut labels to choose shortest <small>([commit 0ec91bf](https://github.com/GraphiteEditor/Graphite/commit/0ec91bfe01a7ca3a97cf4c7f073ccb13731b11d8))</small>

- Add new icons to all menu bar entries <small>([commit 0037f51](https://github.com/GraphiteEditor/Graphite/commit/0037f5158ce01c4376e18f844e5b220115dbdf99))</small>

- Add a "Reverse" command to the "Order" menu <small>([#2273](https://github.com/GraphiteEditor/Graphite/pull/2273))</small>

- Add Pen and Path tool modes to avoid showing all handles <small>([#2264](https://github.com/GraphiteEditor/Graphite/pull/2264), [commit 7bbbf7f](https://github.com/GraphiteEditor/Graphite/commit/7bbbf7fa7cf5b9d71584bb936023e1ee644e316f))</small>

- Add the style of right-angle grid-aligned wires in the graph <small>([#2182](https://github.com/GraphiteEditor/Graphite/pull/2182))</small>

- Implement merging pairs of {paths, splines} with the Pen and Spline tools <small>([#2269](https://github.com/GraphiteEditor/Graphite/pull/2269), [#2292](https://github.com/GraphiteEditor/Graphite/pull/2292), [#2319](https://github.com/GraphiteEditor/Graphite/pull/2319))</small>

- Add the compass rose translation gizmo to the transform cage <small>([#2277](https://github.com/GraphiteEditor/Graphite/pull/2277))</small>

- Add the Select Parent command <small>([#2329](https://github.com/GraphiteEditor/Graphite/pull/2329))</small>

- Add draggable skew triangles to the transform cage <small>([#2300](https://github.com/GraphiteEditor/Graphite/pull/2300))</small>

- Enable free movement of transform cage edge during skew when Ctrl is held <small>([#2358](https://github.com/GraphiteEditor/Graphite/pull/2358))</small>

- Allow the Line tool to drag start and end points of line layers <small>([#2278](https://github.com/GraphiteEditor/Graphite/pull/2278))</small>

- Add recursive folder expand/collapse to the Layers panel <small>([#2419](https://github.com/GraphiteEditor/Graphite/pull/2419))</small>


- Add sizing gizmos to the Text tool's text area <small>([#2176](https://github.com/GraphiteEditor/Graphite/pull/2176))</small>

- New node: Merge by Distance <small>([#2307](https://github.com/GraphiteEditor/Graphite/pull/2307))</small>

- Add Path tool support for dragging along an axis when Shift is held <small>([#2449](https://github.com/GraphiteEditor/Graphite/pull/2449))</small>

- Add the Spreadsheet panel to inspect node output data <small>([#2442](https://github.com/GraphiteEditor/Graphite/pull/2442))</small>

- Experimental animation support <small>([#2443](https://github.com/GraphiteEditor/Graphite/pull/2443), [commit 08a4b69](https://github.com/GraphiteEditor/Graphite/commit/08a4b69948d9f2b8cdaf1a47caa30db7f3029aab), [#2471](https://github.com/GraphiteEditor/Graphite/pull/2471))</small>

- New nodes: Mirror, Round Corners, Box Warp, Remove/Generate Handles, Spatial Merge by Distance <small>([#2448](https://github.com/GraphiteEditor/Graphite/pull/2448))</small>


## Fixes

- Fix isometric dotted grid and avoid antialiasing on dashed line overlays <small>([commit 1c880da](https://github.com/GraphiteEditor/Graphite/commit/1c880daea2c0e67c80eaf45cd4024cf58c1ff1bf))</small>

- Improve snapping with better snap target names, tooltips, cleaner overlay labels, code cleanup <small>([commit 07601a5](https://github.com/GraphiteEditor/Graphite/commit/07601a5c6c4c67ffc2d6f08471a67029e6321aa3))</small>

- Fix drawing new layers not being put next to nested selected layers <small>([commit 51d1c4e](https://github.com/GraphiteEditor/Graphite/commit/51d1c4eeacec6e4baf095fae8c5095b768391e93))</small>

- Remove the Pen tool's anchor square under the cursor when drawing a not-yet-placed segment <small>([commit 3582126](https://github.com/GraphiteEditor/Graphite/commit/3582126dedd747fd8441aba0e4eb9acb9594a9fd))</small>

- Make the Text tool delete empty text layers when clicking away wth LMB <small>([#2192](https://github.com/GraphiteEditor/Graphite/pull/2192))</small>

- Update the bounding box snapping modes to use Align with Edges for edges and alignment <small>([#2185](https://github.com/GraphiteEditor/Graphite/pull/2185))</small>

- Remove trailing zeros in rendered SVG path output <small>([commit 1e62af8](https://github.com/GraphiteEditor/Graphite/commit/1e62af88cd746674ea879ef17cd9423ed94e0b6a))</small>

- Restore Pen tool undo/redo and fix incorrect triggering of undo when changing tools <small>([#2193](https://github.com/GraphiteEditor/Graphite/pull/2193))</small>

- Improve older document upgrading compatibility and make node type errors clearer <small>([#2201](https://github.com/GraphiteEditor/Graphite/pull/2201))</small>

- Retain transforms of layers when transferred between transformed groups <small>([#2212](https://github.com/GraphiteEditor/Graphite/pull/2212))</small>

- In Path tool when dragging a handle, make Alt recover the opposing handle if it's not cubic rather than zero-length <small>([#2196](https://github.com/GraphiteEditor/Graphite/pull/2196))</small>

- Remove useful line following cursor in snap overlays during constrained line drawing <small>([#2206](https://github.com/GraphiteEditor/Graphite/pull/2206))</small>

- Make the document auto-save system initially restore the last-viewed tab before loading the rest <small>([#2194](https://github.com/GraphiteEditor/Graphite/pull/2194))</small>

- Fix blurry overlay rendering when the pixel display ratio isn't 100% <small>([#2204](https://github.com/GraphiteEditor/Graphite/pull/2204))</small>

- Fix regressions from #2105 (more control over subgraph imports/exports handling) <small>([#2213](https://github.com/GraphiteEditor/Graphite/pull/2213))</small>

- Fix regression where Pen tool resumes segment placement after returning from another tool <small>([#2234](https://github.com/GraphiteEditor/Graphite/pull/2234))</small>

- Fix a minor regression in monitor nodes with VectorData <small>([#2237](https://github.com/GraphiteEditor/Graphite/pull/2237))</small>

- Fix regression causing pasted images to have a zero-size transform <small>([#2238](https://github.com/GraphiteEditor/Graphite/pull/2238))</small>

- Fix Document > Clear Artboards so it doesn't also clear everything else <small>([#2177](https://github.com/GraphiteEditor/Graphite/pull/2177))</small>

- Avoid adding an unnecessary Transform node with the TransformChange message <small>([commit 303c1d4](https://github.com/GraphiteEditor/Graphite/commit/303c1d45f89def4c1fc0833edd3b5cfceca7e5d3))</small>

- Polish the G/R/S feature behavior, visualizations, and hints <small>([#2229](https://github.com/GraphiteEditor/Graphite/pull/2229))</small>

- Make the transform cage resize about the pivot when Alt is pressed <small>([#2226](https://github.com/GraphiteEditor/Graphite/pull/2226))</small>

- Group layers with Ctrl+G into independent groups if they're spread across artboards <small>([#2239](https://github.com/GraphiteEditor/Graphite/pull/2239))</small>

- Fix demo artwork <small>([commit 5fedd5c](https://github.com/GraphiteEditor/Graphite/commit/5fedd5c234bece5206c668af316c61d26c15eb3c))</small>

- Fix Path tool issue where the selected points could be dragged from afar within the layer interior <small>([#2260](https://github.com/GraphiteEditor/Graphite/pull/2260))</small>

- Improve the Pen tool's colinearity and equidistance controls <small>([#2242](https://github.com/GraphiteEditor/Graphite/pull/2242))</small>

- Make the Select tool avoid updating hints just when clicking but not dragging <small>([#2248](https://github.com/GraphiteEditor/Graphite/pull/2248))</small>

- Fix shallow select mode not allowing a Ctrl-click select deepest if the target's ancestor is already selected <small>([#2247](https://github.com/GraphiteEditor/Graphite/pull/2247))</small>

- Fix copied/duplicated selected layers getting misordered <small>([#2257](https://github.com/GraphiteEditor/Graphite/pull/2257))</small>

- Make joining path endpoints across layers work to merge the two layers <small>([#2245](https://github.com/GraphiteEditor/Graphite/pull/2245))</small>

- Fix some number input widgets becoming selected after dragging left/right in Firefox <small>([#2250](https://github.com/GraphiteEditor/Graphite/pull/2250))</small>

- Further polishing of G/R/S visualization and features <small>([#2243](https://github.com/GraphiteEditor/Graphite/pull/2243))</small>

- Standardize increment snapping to use the Shift key <small>([commit f13efd2](https://github.com/GraphiteEditor/Graphite/commit/f13efd2d06082d9e4d580bd7660eff8d106f9e86))</small>

- Polish and add aborting to several input widgets: no Esc closing parent menus; color picker axis align; repeat on arrow buttons <small>([#2276](https://github.com/GraphiteEditor/Graphite/pull/2276))</small>

- Fix crash when ungrouping a direct child of the root in debug mode <small>([#2241](https://github.com/GraphiteEditor/Graphite/pull/2241))</small>

- Fix scale transform being applied when drawing shapes while zoomed in <small>([#2286](https://github.com/GraphiteEditor/Graphite/pull/2286))</small>

- Fix unresolved types in graph wires when repeatedly undoing and redoing <small>([#2283](https://github.com/GraphiteEditor/Graphite/pull/2283))</small>

- Fixed minor issues related to frontier selection visibility in the Pen/Path tools <small>([#2291](https://github.com/GraphiteEditor/Graphite/pull/2291))</small>

- Improve grab/rotate/scale handling of pan/tilt/zoom <small>([#2285](https://github.com/GraphiteEditor/Graphite/pull/2285))</small>

- Make the Pen tool use Ctrl to lock the angle of handles such that they maintain colinearity <small>([#2284](https://github.com/GraphiteEditor/Graphite/pull/2284))</small>

- Fix self-chaining of transforms; fix compass rose getting offset when rotating a layer <small>([#2296](https://github.com/GraphiteEditor/Graphite/pull/2296))</small>

- Fix regressions introduced in #2282 with the compass rose feature <small>([#2298](https://github.com/GraphiteEditor/Graphite/pull/2298))</small>

- Make the Pen tool extend an endpoint by starting with a colinear, equidistant handle <small>([#2295](https://github.com/GraphiteEditor/Graphite/pull/2295))</small>

- Fix transform cage bug where aborted resize/rotate after drag is used for next resize/rotate <small>([#2308](https://github.com/GraphiteEditor/Graphite/pull/2308))</small>

- Make grid-aligned node graph wires an experimental feature disabled by default <small>([commit 5115a05](https://github.com/GraphiteEditor/Graphite/commit/5115a05c5bd2dd4171f68cecc3b863c0efff12ad))</small>

- Limit the Sample to Points node's spacing value to prevent freezing when 0 <small>([commit 390574d](https://github.com/GraphiteEditor/Graphite/commit/390574d5c6f9282bf474d2d5bedb319dd6464d7d))</small>

- Fix incorrect Properties panel widget types for proto nodes <small>([#2323](https://github.com/GraphiteEditor/Graphite/pull/2323))</small>

- Add feature for switching existing boolean ops to another type in the Select tool <small>([#2322](https://github.com/GraphiteEditor/Graphite/pull/2322))</small>

- Fix duplicates not all being selected after Ctrl+D <small>([#2324](https://github.com/GraphiteEditor/Graphite/pull/2324))</small>

- Add a workaround to prevent nudge resizing from giving lines a NaN scale <small>([#2331](https://github.com/GraphiteEditor/Graphite/pull/2331))</small>

- Make Ctrl+D duplication interleave each layer like Alt+drag duplication already does <small>([#2328](https://github.com/GraphiteEditor/Graphite/pull/2328))</small>

- Fix several minor Pen and Path tool bugs <small>([#2327](https://github.com/GraphiteEditor/Graphite/pull/2327))</small>

- Make it easier to resize short/narrow edges of the transform cage without corners taking precedence <small>([#2320](https://github.com/GraphiteEditor/Graphite/pull/2320))</small>

- Fix bug introduced in #2276 causing number inputs to abort on any keyboard input, not just Esc <small>([commit bc6e762](https://github.com/GraphiteEditor/Graphite/commit/bc6e76208daef2f8a031ede23f0425ecafcd17de))</small>

- Remove checks from append_subpath to improve vector editing performance <small>([#2190](https://github.com/GraphiteEditor/Graphite/pull/2190))</small>

- Fix wrong node parameter widgets, attempt 2 at #2323 <small>([commit e41471c](https://github.com/GraphiteEditor/Graphite/commit/e41471c088619474f275acae4da797ae8deba157))</small>

- Fix wrong node parameter widgets, attempt 3 at #2323  <small>([#2334](https://github.com/GraphiteEditor/Graphite/pull/2334))</small>

- Fix perf regression from updating the hints bar every frame <small>([#2360](https://github.com/GraphiteEditor/Graphite/pull/2360))</small>

- Fix Vello rendering the infinite canvas without a white background color <small>([#2361](https://github.com/GraphiteEditor/Graphite/pull/2361))</small>

- Fix Pen tool click-dragging from handle causing opposite colinear handle to rotate with drag <small>([#2338](https://github.com/GraphiteEditor/Graphite/pull/2338))</small>


- Fix chaining GRS commands so they work smoothly and don't add intermediate undo steps <small>([#2297](https://github.com/GraphiteEditor/Graphite/pull/2297))</small>

- Fix document upgrades to work recursively within subgraph nodes <small>([#2369](https://github.com/GraphiteEditor/Graphite/pull/2369))</small>

- Fix regresion from #2265 causing an extra default artboard to show up <small>([commit 22a900b](https://github.com/GraphiteEditor/Graphite/commit/22a900b35e0de0440eecca9321b7174d223f91ef))</small>

- Disable the Path tool's "Colinear Handles" checkbox when no interior anchors are selected <small>([#2339](https://github.com/GraphiteEditor/Graphite/pull/2339))</small>


- Fix noise pattern parameter issue <small>([#2412](https://github.com/GraphiteEditor/Graphite/pull/2412))</small>

- Fix fill tool on raster image temporarily breaks the graph <small>([#2398](https://github.com/GraphiteEditor/Graphite/pull/2398))</small>

- Fix inconsistent stroke width in 'Outline' view mode <small>([#2417](https://github.com/GraphiteEditor/Graphite/pull/2417))</small>


- In the Pen tool, make Space drag the whole manipulator group while dragging a handle <small>([#2416](https://github.com/GraphiteEditor/Graphite/pull/2416))</small>

- Fix quick measuring of skewed and rotated layers by using the viewport space AABB <small>([#2396](https://github.com/GraphiteEditor/Graphite/pull/2396))</small>


- Fix autosaved document ID being incorrectly added to browser storage at the wrong time without its document data <small>([#2426](https://github.com/GraphiteEditor/Graphite/pull/2426))</small>


- Fix constrained snap when dragging by a compass rose axis and fix that axis line's jiggling <small>([#2333](https://github.com/GraphiteEditor/Graphite/pull/2333))</small>

- Make the Transform node's skew parameter input actually in degrees <small>([#2431](https://github.com/GraphiteEditor/Graphite/pull/2431))</small>

- Fix drawing tools to work in viewport space instead of document space <small>([#2438](https://github.com/GraphiteEditor/Graphite/pull/2438))</small>

- Make the Select tool box-select the deepest individual layers or their common shared parent <small>([#2424](https://github.com/GraphiteEditor/Graphite/pull/2424))</small>

- Fix Select tool resizing with Shift held allowing the constrained aspect ratio to change when snapping <small>([#2441](https://github.com/GraphiteEditor/Graphite/pull/2441))</small>

- Fix Pen tool so it cancels (not confirms) a handle drag in which setting colinearity moves the other handle <small>([#2439](https://github.com/GraphiteEditor/Graphite/pull/2439))</small>

- Refactor GRS messages and fix regression in chained GRS operations <small>([#2450](https://github.com/GraphiteEditor/Graphite/pull/2450))</small>

- Avoid crashing when a document can't be loaded <small>([#2453](https://github.com/GraphiteEditor/Graphite/pull/2453))</small>

- Refactor many usages of Color to natively store linear not gamma <small>([#2457](https://github.com/GraphiteEditor/Graphite/pull/2457))</small>

- Improve Text tool resize/drag behavior <small>([#2428](https://github.com/GraphiteEditor/Graphite/pull/2428))</small>

- Fix device pixel ratio being tied to the document by moving it from overlays to portfolio <small>([commit 4e418bb](https://github.com/GraphiteEditor/Graphite/commit/4e418bbfe1ecc415a6ff924c0392c865d1411a73))</small>

- Fix the Into nodes, which were broken but unused except in GPU nodes <small>([#2480](https://github.com/GraphiteEditor/Graphite/pull/2480))</small>

- Work around unwrap crash <small>([commit 158f18d](https://github.com/GraphiteEditor/Graphite/commit/158f18df0d8421785d8b949e03253dd479af22c7))</small>

## Internal

- Rename "options/top bar" terminology to "control bar" and update comments <small>([commit 9eb544d](https://github.com/GraphiteEditor/Graphite/commit/9eb544df740a6c9188c2255b623f0a716bd64fb3))</small>

- Bezier-rs: Add method to check subpath insideness <small>([#2183](https://github.com/GraphiteEditor/Graphite/pull/2183))</small>

- Remove blob URL dead code and clean up more frontend code <small>([#2199](https://github.com/GraphiteEditor/Graphite/pull/2199))</small>

- Add marbled mandelbrot as unpublished demo art <small>([commit 9a25555](https://github.com/GraphiteEditor/Graphite/commit/9a25555732e90a6cad386097b957975694b31538))</small>

- Instance tables
  - Instance tables refactor part 1: wrap graphical data in the new Instances<T> struct <small>([#2230](https://github.com/GraphiteEditor/Graphite/pull/2230))</small>

  - Instance tables refactor part 2: move the transform and alpha_blending fields up a level <small>([#2249](https://github.com/GraphiteEditor/Graphite/pull/2249))</small>

  - Instance tables refactor part 3: flatten ImageFrame<P> in lieu of Image<P> <small>([#2256](https://github.com/GraphiteEditor/Graphite/pull/2256))</small>

  - Instance tables refactor part 4: replace ArtboardGroups with multi-row Instances<Artboard> <small>([#2265](https://github.com/GraphiteEditor/Graphite/pull/2265))</small>

  - Instance tables refactor part 5: unwrap GraphicGroup as multi-row Instance<GraphicElement> tables and move up transforms <small>([#2363](https://github.com/GraphiteEditor/Graphite/pull/2363))</small>


- Improve naming for variables from the lasso selection feature <small>([#2244](https://github.com/GraphiteEditor/Graphite/pull/2244))</small>

- Update most Rust dependencies <small>([#2259](https://github.com/GraphiteEditor/Graphite/pull/2259))</small>

- Code cleanup around the input mapper system <small>([commit 0cda8e2](https://github.com/GraphiteEditor/Graphite/commit/0cda8e2bb41fbc75dbc9597fc5e567475f7eaef1))</small>

- Add upgrade script to convert "Spline" node to "Path" -> "Spline from Points" <small>([#2274](https://github.com/GraphiteEditor/Graphite/pull/2274))</small>

- Update some UI components to polish the frontend <small>([commit 2c88bee](https://github.com/GraphiteEditor/Graphite/commit/2c88bee0ee42c3ead8095cbc8460ba531a9120f8))</small>

- Reorganize the menu bar and add additional commands to it <small>([commit ddb0c8c](https://github.com/GraphiteEditor/Graphite/commit/ddb0c8c2496b99a559fd9b045b86c256b501a3a7))</small>

- Replace Footprint/() call arguments with dynamically-bound Contexts <small>([#2232](https://github.com/GraphiteEditor/Graphite/pull/2232))</small>

- Tidy up NodeNetworkInterface, replacing &[] root network arguments with methods for the document network <small>([#2393](https://github.com/GraphiteEditor/Graphite/pull/2393))</small>

- Update dependencies throughout the project <small>([#2401](https://github.com/GraphiteEditor/Graphite/pull/2401))</small>

- Allow printing proto graph in graphite-cli (#2388) <small>([commit 85fac63](https://github.com/GraphiteEditor/Graphite/commit/85fac63bb264f281e63017df677c8e212e8cb63a))</small>

- Upgrade to the Rust 2024 edition <small>([#2367](https://github.com/GraphiteEditor/Graphite/pull/2367))</small>

- Remove subtyping for () from node graph type system <small>([#2418](https://github.com/GraphiteEditor/Graphite/pull/2418))</small>



## Testing

- Add tests to the Ellipse, Artboard, and Fill tools <small>([#2181](https://github.com/GraphiteEditor/Graphite/pull/2181))</small>

- Add tests for GRS transform cancellation <small>([#2467](https://github.com/GraphiteEditor/Graphite/pull/2467))</small>

- Add test for chained GRS transformations <small>([#2475](https://github.com/GraphiteEditor/Graphite/pull/2475))</small>

- Add tests for document panning, zooming, and rotating <small>([#2492](https://github.com/GraphiteEditor/Graphite/pull/2492))</small>



## Website

- Comprehensively update user manual and contributor guide, add Adam to core team <small>([commit 93a60da](https://github.com/GraphiteEditor/Graphite/commit/93a60daa24e200c2b68a07815cd79dbdfa29457d))</small>

- Reduce website loading times and related code cleanup <small>([commit 68e6bec](https://github.com/GraphiteEditor/Graphite/commit/68e6bec9b5647a267f239944a7be4cf9337615a7))</small>

- More website loading speed and code improvements <small>([commit ae2637e](https://github.com/GraphiteEditor/Graphite/commit/ae2637e08e674d51b8bc3c383dee7f661240f337))</small>

- Add Stipe donations to the website and polish other pages <small>([commit b7907bc](https://github.com/GraphiteEditor/Graphite/commit/b7907bc96f02f421d1514f9ea66662010512bfe5))</small>

- Add the 2024 yearly report to the blog <small>([commit ab724d8](https://github.com/GraphiteEditor/Graphite/commit/ab724d8b007c97a16c4084029f1f30e2579a8c4e))</small>

- Improve readability of the donation page <small>([commit b36521e](https://github.com/GraphiteEditor/Graphite/commit/b36521e5888f1b6eeb30e8ed18f335939ef04758))</small>

- Update content on the website hoe page, roadmap, and donate page <small>([commit eada1eb](https://github.com/GraphiteEditor/Graphite/commit/eada1eba54854769dad8df3fe00ac56a3d84d53d))</small>

- Update website with improved student project details <small>([commit 1700c3a](https://github.com/GraphiteEditor/Graphite/commit/1700c3a6505173ec2c5a60abdc81bf38277f5cfd))</small>

- Declare the start of the Alpha 4 release series <small>([commit fb13d58](https://github.com/GraphiteEditor/Graphite/commit/fb13d58767ced5d88399cf1e96c714e792fa7e15))</small>

- Update student projects page of the website <small>([commit 3e56113](https://github.com/GraphiteEditor/Graphite/commit/3e56113c78a6b7ba168f2dd8ec58f30a77cf7aed))</small>
