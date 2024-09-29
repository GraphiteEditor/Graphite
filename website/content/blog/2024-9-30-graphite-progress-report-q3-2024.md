+++
title = "Graphite progress report (Q3 2024)"
date = 2024-09-30
[extra]
banner = "ToDo"
banner_png = "ToDo"
author = "Keavon Chambers"
summary = "Graphite's Q3 2024 update introduces node graph organisation, non destructive vector editing, and faster boolean operations."
reddit = "ToDo"
twitter = "ToDo"
+++

[Graphite](/), an open source 2D procedural graphics editor, has spent the months of July, August, and September introducing node graph organisation, non destructive vector editing, faster boolean operations, and many more improvements.

In this quater we have continued iterating on the usability of our unqiue node powered vector workflow. As we reach the conclusion of the [Google Summer of Code](/blog/graphite-internships-announcing-participation-in-gsoc-2024/) projects, we now have a polished node graph organisation system, a working RAW image pipeline (not yet in the editor), and a WebGPU based renderer (File -> Preferences -> Vello). Thanks to the students for their hard work on this and to Google's open source team for sponsoring.

<!-- more -->

This is the third of our quarterly progress report blog posts. If you missed the [last one](../graphite-progress-report-q2-2024), be sure to check it out as well. If you'd like to help speed up future progress, please consider [getting involved](/volunteer) with code, QA/bug testing, or art/marketing projects. [Donations](/donate) are also valued, as are [stars of GitHub](https://github.com/GraphiteEditor/Graphite). Follow along and partake in our [Discord community](https://discord.graphite.rs), too.

All Q3 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-07-01&until=2024-09-30) and all noteworthy changes are detailed below.

## Additions

<!--
EDITOR'S NOTE: The grammatical structure of each bullet point should follow the form: "Check out our new... [Bullet point description]"
-->

- Nondestructive vector editing <small>([#1676](https://github.com/GraphiteEditor/Graphite/pull/1676))</small>

- Basic artboard snapping <small>([#1734](https://github.com/GraphiteEditor/Graphite/pull/1734))</small>


- Draggable upstream nodes feature <small>([#1812](https://github.com/GraphiteEditor/Graphite/pull/1812))</small>

- Stack-based Boolean Operation layer node <small>([#1813](https://github.com/GraphiteEditor/Graphite/pull/1813))</small>

- Scope API <small>([#1814](https://github.com/GraphiteEditor/Graphite/pull/1814))</small>

- Graph breadcrumb trail button <small>([commit 8e774ef](https://github.com/GraphiteEditor/Graphite/commit/8e774efe9dae51c0c1db2928e22c6de5b93d0584))</small>

- Integrate Vello for vector rendering <small>([#1802](https://github.com/GraphiteEditor/Graphite/pull/1802))</small>

- Add caching to boolean operations <small>([commit 6ecb173](https://github.com/GraphiteEditor/Graphite/commit/6ecb173c1c06807f13a859ef90b7d7f43af042be))</small>

- Gradient rendering with Vello <small>([#1865](https://github.com/GraphiteEditor/Graphite/pull/1865))</small>

- Memoize hashing <small>([#1876](https://github.com/GraphiteEditor/Graphite/pull/1876))</small>

- Artboard names in the export dialog with persistant settings <small>([commit 4d3e459](https://github.com/GraphiteEditor/Graphite/commit/4d3e459f1f52f04be81727fe5946010bcabeb1eb))</small>

- "Painted Dreams" demo artwork (and retire Just a Potted Cactus) <small>([commit 80cf486](https://github.com/GraphiteEditor/Graphite/commit/80cf486790d9300510904622924162a9b4017b1e))</small>

- Layer node chains, import/export edge connectors, and refactor graph editing to go through a NodeNetworkInterface <small>([#1794](https://github.com/GraphiteEditor/Graphite/pull/1794))</small>

- Distribute and algin snapping <small>([#1793](https://github.com/GraphiteEditor/Graphite/pull/1793))</small>

- Quick measure overlays with Alt pressed <small>([#1894](https://github.com/GraphiteEditor/Graphite/pull/1894))</small>

- Seed parameters to all nodes with RNG <small>([commit c39032a](https://github.com/GraphiteEditor/Graphite/commit/c39032ab548d4f33d18a399c64b77d3d6f4ecd45))</small>

- New nodes: Blend Colors, Percentage Value <small>([commit d7546fb](https://github.com/GraphiteEditor/Graphite/commit/d7546fb18310490d5ce10d406e7a3faaf1ae98fe))</small>

- Raw-rs: add post-processing steps <small>([#1923](https://github.com/GraphiteEditor/Graphite/pull/1923))</small>

- Raw-rs: use camera white balance when available <small>([#1941](https://github.com/GraphiteEditor/Graphite/pull/1941))</small>

- Add path-bool library <small>([#1952](https://github.com/GraphiteEditor/Graphite/pull/1952))</small>

- Add drag-and-drop and copy-paste file importing/opening throughout the UI <small>([#2012](https://github.com/GraphiteEditor/Graphite/pull/2012))</small>

# Fixes

- Fix rulers showing in the wrong spot when initially opening a document <small>([#1801](https://github.com/GraphiteEditor/Graphite/pull/1801))</small>

- Restore backwards compatibility broken with #1750 to prepare next release <small>([commit 621f469](https://github.com/GraphiteEditor/Graphite/commit/621f469a150d4a14c86ba9be87df82aae5dded74))</small>

- Fix multi-subpath boolean operations <small>([#1804](https://github.com/GraphiteEditor/Graphite/pull/1804))</small>

- Fix bug causing some node outputs to fail connecting wires to other inputs <small>([commit 84d7262](https://github.com/GraphiteEditor/Graphite/commit/84d72621e545ba7743e38d46ce1607133bb13461))</small>

- Fix 'Zoom with Scroll' not working after a page refresh <small>([#1807](https://github.com/GraphiteEditor/Graphite/pull/1807))</small>

- Fix Pen and Freehand tool path extension <small>([#1809](https://github.com/GraphiteEditor/Graphite/pull/1809))</small>

- Reorganize cargo dependencies and upgrade most of them <small>([#1815](https://github.com/GraphiteEditor/Graphite/pull/1815))</small>

- Fix a lot of Clippy warnings <small>([#1808](https://github.com/GraphiteEditor/Graphite/pull/1808))</small>

- Disable snapping by default as a bug mitigation, and assorted cleanup <small>([commit a17ed68](https://github.com/GraphiteEditor/Graphite/commit/a17ed68008ab9d16f929fac2d23fc98367d7e151))</small>


- Fix snapping bug where snapped-together shapes couldn't be moved <small>([#1818](https://github.com/GraphiteEditor/Graphite/pull/1818))</small>

- Apply the graphic group transform in the boolean node <small>([#1825](https://github.com/GraphiteEditor/Graphite/pull/1825))</small>

- Remove artboard from graphic element <small>([#1824](https://github.com/GraphiteEditor/Graphite/pull/1824))</small>

- Insert transform node on groups <small>([#1829](https://github.com/GraphiteEditor/Graphite/pull/1829))</small>

- Finish removing GraphicElement::Artboard started in #1824 <small>([#1830](https://github.com/GraphiteEditor/Graphite/pull/1830))</small>

- Simplify build process by auto-installing npm packages and simplify the contributor guide <small>([#1799](https://github.com/GraphiteEditor/Graphite/pull/1799))</small>

- Restore functionality of GPU infrastructure <small>([#1797](https://github.com/GraphiteEditor/Graphite/pull/1797))</small>

- Fix Rust-Analyzer conflicts with build targets and other compile speed issues <small>([commit 857bc77](https://github.com/GraphiteEditor/Graphite/commit/857bc772de8c5731a2eaf7f442beb573ccf8ca4c))</small>

- Update dependencies and lock files <small>([#1841](https://github.com/GraphiteEditor/Graphite/pull/1841))</small>

- Fix breakage of shallow select mode in the Select tool <small>([#1843](https://github.com/GraphiteEditor/Graphite/pull/1843))</small>

- Fix graph not being resent after Ctrl+D node duplication <small>([#1852](https://github.com/GraphiteEditor/Graphite/pull/1852))</small>

- Disable Vello renderer checkbox in preferences if browser doesn't support WebGPU <small>([#1844](https://github.com/GraphiteEditor/Graphite/pull/1844))</small>

- Fix Layers panel UI not updating on selection change and expand/collapsing <small>([#1847](https://github.com/GraphiteEditor/Graphite/pull/1847))</small>

- Fix, document, and update npm dependencies and tooling; fix Bezier-rs demos not building <small>([#1857](https://github.com/GraphiteEditor/Graphite/pull/1857))</small>

- Fix spelling in several code comments <small>([#1860](https://github.com/GraphiteEditor/Graphite/pull/1860))</small>

- Fix rendering of non closed paths and improve bbox calculation <small>([#1859](https://github.com/GraphiteEditor/Graphite/pull/1859))</small>

- Serialize documents with images in linear space instead of sRGB <small>([#1856](https://github.com/GraphiteEditor/Graphite/pull/1856))</small>

- Fix grid overlay color showing as none <small>([commit a4a5139](https://github.com/GraphiteEditor/Graphite/commit/a4a513911dac29ee5c6546842be194b1513f2cec))</small>

- Fix viewport bounds getting out of sync at times, like when toggling rulers <small>([commit 0617759](https://github.com/GraphiteEditor/Graphite/commit/06177597ae8ef52f7a273875d6afb63fb09ec3c8))</small>

- Sandbox node graph execution on native targets and attempt recovery from panics on Wasm <small>([#1846](https://github.com/GraphiteEditor/Graphite/pull/1846))</small>

- Fix Vello rendering of transforms for nested graphic groups <small>([#1871](https://github.com/GraphiteEditor/Graphite/pull/1871))</small>

- Apply opacity and blend modes to individual layers in Vello <small>([#1874](https://github.com/GraphiteEditor/Graphite/pull/1874))</small>

- Improve rendered SVG output syntax for better compatibility and terseness <small>([#1880](https://github.com/GraphiteEditor/Graphite/pull/1880))</small>

- Apply correct stroke styling with Vello <small>([#1875](https://github.com/GraphiteEditor/Graphite/pull/1875))</small>

- Blog post: Graphite progress report (Q2 2024) <small>([#1805](https://github.com/GraphiteEditor/Graphite/pull/1805))</small>

- Use a coarse bounding box to avoid a detailed check for intersection when clicking artwork <small>([#1887](https://github.com/GraphiteEditor/Graphite/pull/1887))</small>

- Switch to MSAA for Vello rendering for better anti-aliasing <small>([#1899](https://github.com/GraphiteEditor/Graphite/pull/1899))</small>

- Make Vello render groups with a blending stack only when necessary <small>([#1905](https://github.com/GraphiteEditor/Graphite/pull/1905))</small>

- Restructure window state management and fix Vello canvas not resizing with viewport <small>([#1900](https://github.com/GraphiteEditor/Graphite/pull/1900))</small>

- Correctly set the dimensions of the SVG foreignObject container for the Vello canvas <small>([#1907](https://github.com/GraphiteEditor/Graphite/pull/1907))</small>

- Set Vello to render an explicit background color behind artboards <small>([#1902](https://github.com/GraphiteEditor/Graphite/pull/1902))</small>

- Integrate raw WGPU textures into Vello rendering pipeline <small>([#1897](https://github.com/GraphiteEditor/Graphite/pull/1897))</small>

- Fix regressions from network interface PR <small>([#1906](https://github.com/GraphiteEditor/Graphite/pull/1906))</small>

- Restore the Imaginate node with the full node graph architecture (but a flaky deadlock remains) <small>([#1908](https://github.com/GraphiteEditor/Graphite/pull/1908))</small>

- Refactor document node type lookup function to fix performance degradation over time <small>([#1878](https://github.com/GraphiteEditor/Graphite/pull/1878))</small>

- Make noise generation resolution aware <small>([#1909](https://github.com/GraphiteEditor/Graphite/pull/1909))</small>

- Fix render disappearing while panning when using Vello <small>([#1915](https://github.com/GraphiteEditor/Graphite/pull/1915))</small>

- Fix node graph type errors not being shown <small>([#1917](https://github.com/GraphiteEditor/Graphite/pull/1917))</small>

- Add 'Gradient Map' adjustment node <small>([commit 501b562](https://github.com/GraphiteEditor/Graphite/commit/501b562d0f17b91a54a14c83a4a0f90a1e03e369))</small>

- Add colors to all nodes in a graph, even if disconnected, and properly display hidden network imports <small>([#1921](https://github.com/GraphiteEditor/Graphite/pull/1921))</small>

- Add grid snapping to graph imports/exports; improve layer panel drag into/between insertion; better preserve graph space on reordering <small>([#1911](https://github.com/GraphiteEditor/Graphite/pull/1911))</small>

- Respect 'Clip' on Artboards when exporting "All Artworks". <small>([#1916](https://github.com/GraphiteEditor/Graphite/pull/1916))</small>

- Raw-rs: Add preprocessing and demosaicing steps <small>([#1796](https://github.com/GraphiteEditor/Graphite/pull/1796))</small>

- Format demo artwork's nodes in layer chains <small>([commit e28e841](https://github.com/GraphiteEditor/Graphite/commit/e28e841e3bef401c19f5ddcf01c68fedd6b5c9c3))</small>

- Add profiling metrics for, and speed up, Graphene graph compilation <small>([#1924](https://github.com/GraphiteEditor/Graphite/pull/1924))</small>

- Improve layer panel positioning for upstream nodes <small>([#1928](https://github.com/GraphiteEditor/Graphite/pull/1928))</small>

- Post Clippy warnings as PR comments <small>([#1926](https://github.com/GraphiteEditor/Graphite/pull/1926))</small>

- Fix all Clippy warnings <small>([#1936](https://github.com/GraphiteEditor/Graphite/pull/1936))</small>

- Add a profiling action to CI which comments on PRs with notable demo art performance variances <small>([#1925](https://github.com/GraphiteEditor/Graphite/pull/1925))</small>

- Add a profiling action to CI which comments on PRs with notable demo art performance variances (#1925) <small>([commit 12ebc6f](https://github.com/GraphiteEditor/Graphite/commit/12ebc6f97269791cb935d17d1f6de59b271a240a))</small>

- New node: Assign Colors <small>([#1938](https://github.com/GraphiteEditor/Graphite/pull/1938))</small>

- Separate the Merge node from the Boolean Operation node <small>([#1933](https://github.com/GraphiteEditor/Graphite/pull/1933))</small>

- Rename document_node_types.rs to document_node_definitions.rs <small>([commit 6a2b0d7](https://github.com/GraphiteEditor/Graphite/commit/6a2b0d74dc51b786f1d8765508043e3272b82faa))</small>

- Improve profiling CI action's comment output text <small>([#1939](https://github.com/GraphiteEditor/Graphite/pull/1939))</small>

- Recategorize the node catalog <small>([commit e647ca9](https://github.com/GraphiteEditor/Graphite/commit/e647ca9f91a5e823137122126fe9e980f65d62ea))</small>

- Revamp the website <small>([commit 5d74178](https://github.com/GraphiteEditor/Graphite/commit/5d74178f5f417afdd70abec596d38f22d490240a))</small>

- Fix website base template escaping <small>([commit 98ab069](https://github.com/GraphiteEditor/Graphite/commit/98ab069a1703e89cea031fedffc55c3bf4191b5d))</small>

- Revamp the website more <small>([commit 40fd447](https://github.com/GraphiteEditor/Graphite/commit/40fd4473a784ba24fc3105f8da56baacccf2dcf5))</small>

- Add shifting of layers in stacks as blocks that collide and bump other layers/nodes <small>([#1940](https://github.com/GraphiteEditor/Graphite/pull/1940))</small>

- Add self-hosted build asset deployment to GitHub releases in the CI action <small>([commit f2493d5](https://github.com/GraphiteEditor/Graphite/commit/f2493d5308ace728c3525d51609c3a2efcbdc139))</small>

- Implement node path insertion at compile time <small>([#1947](https://github.com/GraphiteEditor/Graphite/pull/1947))</small>

- Improve layer positioning in graph upon reordering; improve history system; add selection history <small>([#1945](https://github.com/GraphiteEditor/Graphite/pull/1945))</small>

- Fix crash from gradients with bounds of zero <small>([#1950](https://github.com/GraphiteEditor/Graphite/pull/1950))</small>

- Update various content on the website <small>([commit fb7d597](https://github.com/GraphiteEditor/Graphite/commit/fb7d5970b38c61fc1a35aeefd7028858c96f5ff2))</small>

- Fix broken SVG importing and crash when exporting <small>([#1953](https://github.com/GraphiteEditor/Graphite/pull/1953))</small>

- Switch attribute-based vector data from referencing PointIds to indexes in the points table <small>([#1949](https://github.com/GraphiteEditor/Graphite/pull/1949))</small>

- Bezier-rs: Fix crash when outlining a small bézier <small>([#1958](https://github.com/GraphiteEditor/Graphite/pull/1958))</small>

- Bezier-rs: Fix crash when outlining a small bézier (#1958) <small>([commit bf5019d](https://github.com/GraphiteEditor/Graphite/commit/bf5019db7b52120bbb255adae78ee416c96a39b4))</small>

- Fix invalid segment crash when disolving point loop <small>([#1959](https://github.com/GraphiteEditor/Graphite/pull/1959))</small>

- Refactor navigation metadata <small>([#1956](https://github.com/GraphiteEditor/Graphite/pull/1956))</small>

- Raw-rs: Flip and rotate image based on camera orientation <small>([#1954](https://github.com/GraphiteEditor/Graphite/pull/1954))</small>

- Make the Clippy Check CI action not comment on draft PRs <small>([commit a93dcb2](https://github.com/GraphiteEditor/Graphite/commit/a93dcb2776027bfcb385ae9dfddff249bdfd896b))</small>

- Set integer ruler intervals when zoomed in <small>([#1966](https://github.com/GraphiteEditor/Graphite/pull/1966))</small>

- Fix some typos in the node graph code <small>([#1970](https://github.com/GraphiteEditor/Graphite/pull/1970))</small>

- Fix some typos in the node graph code (#1970) <small>([commit 507210b](https://github.com/GraphiteEditor/Graphite/commit/507210b961ba620f8908da3ce3bf26518fda4a4b))</small>

- Make the node graph use a document node's default type <small>([#1965](https://github.com/GraphiteEditor/Graphite/pull/1965))</small>

- Make CI collapse previous PR comments with profiling benchmark deltas <small>([#1974](https://github.com/GraphiteEditor/Graphite/pull/1974))</small>

- Make the primitive shape tools avoid setting a negative transform scale <small>([#1973](https://github.com/GraphiteEditor/Graphite/pull/1973))</small>

- Nudge only the shallowest selected layers to avoid amplified translation <small>([#1975](https://github.com/GraphiteEditor/Graphite/pull/1975))</small>

- Improve select tool click targets <small>([#1976](https://github.com/GraphiteEditor/Graphite/pull/1976))</small>

- Correctly apply transforms to vector data and strokes <small>([#1977](https://github.com/GraphiteEditor/Graphite/pull/1977))</small>

- Fix types of inputs to nodes with a nested network implementation <small>([#1978](https://github.com/GraphiteEditor/Graphite/pull/1978))</small>

- Fix click targets (in, e.g., the boolean node) by resolving footprints from render output <small>([#1946](https://github.com/GraphiteEditor/Graphite/pull/1946))</small>

- Refactor the node macro and simply most of the node implementations <small>([#1942](https://github.com/GraphiteEditor/Graphite/pull/1942))</small>

- Raw-rs: run tests in parallel <small>([#1968](https://github.com/GraphiteEditor/Graphite/pull/1968))</small>

- Update Cargo.lock to fix CI <small>([#1994](https://github.com/GraphiteEditor/Graphite/pull/1994))</small>

- Box TaggedValue::VectorModification <small>([#1995](https://github.com/GraphiteEditor/Graphite/pull/1995))</small>

- Fix blend modes and opacity on raster data <small>([#1996](https://github.com/GraphiteEditor/Graphite/pull/1996))</small>

- Use overlays to draw artboard names <small>([#1981](https://github.com/GraphiteEditor/Graphite/pull/1981))</small>

- Fix Graphene CLI crash <small>([#1993](https://github.com/GraphiteEditor/Graphite/pull/1993))</small>

- Improve node macro and add more diagnostics <small>([#1999](https://github.com/GraphiteEditor/Graphite/pull/1999))</small>

- Fix compilation on nightly <small>([#2001](https://github.com/GraphiteEditor/Graphite/pull/2001))</small>

- Improve type compatibility and clean up new node macro usages <small>([#2002](https://github.com/GraphiteEditor/Graphite/pull/2002))</small>

- Path Bool library code cleanup <small>([#2000](https://github.com/GraphiteEditor/Graphite/pull/2000))</small>

- Fix broken Opacity slider in Layers panel <small>([#2004](https://github.com/GraphiteEditor/Graphite/pull/2004))</small>

- Upgrade the third-party library license generation <small>([commit 14de67c](https://github.com/GraphiteEditor/Graphite/commit/14de67c5a78a1e8369a8b023da441237a44e48cb))</small>

- Clean up web code errors and make CI enforce them <small>([commit 1ee5ffb](https://github.com/GraphiteEditor/Graphite/commit/1ee5ffbbe8a64da5a83da889dd16d41c3463c332))</small>

- Upgrade web dependencies <small>([commit aa03dc8](https://github.com/GraphiteEditor/Graphite/commit/aa03dc8278859c5be9159603f771913568f02567))</small>

- Remove serde from Bezier-rs web demos to reduce Wasm size <small>([commit 0b0169a](https://github.com/GraphiteEditor/Graphite/commit/0b0169a415453b9d9910d765f0538ecd67b618c0))</small>

- New node: Dehaze <small>([#1882](https://github.com/GraphiteEditor/Graphite/pull/1882))</small>

- Add manually-runnable benchmarks for runtime profiling <small>([#2005](https://github.com/GraphiteEditor/Graphite/pull/2005))</small>

- Replace terminology "primary" with "call argument" and "parameter" with "secondary input" <small>([commit c738b4a](https://github.com/GraphiteEditor/Graphite/commit/c738b4a1f9a309f3ab12d2259ba8631402d73da6))</small>

- Fix many regressions introduced mostly in #1946 <small>([#1986](https://github.com/GraphiteEditor/Graphite/pull/1986))</small>
