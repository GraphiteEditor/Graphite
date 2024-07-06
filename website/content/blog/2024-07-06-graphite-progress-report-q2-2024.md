+++
title = "Graphite progress report (Q2 2024)"
date = 2024-07-06
[extra]
banner = "https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024.avif"
banner_png = "https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024.png"
author = "Keavon Chambers & Hypercube"
summary = "Graphite's Q2 2024 update introduces boolean operations, layer locking, the centroid node, and a dot grid."
+++

[Graphite](/) is a new 2D vector graphics editor bringing a modern, nondestructive approach to creative workflows with node-based procedural generation. The project is currently three years into development, with a focus on streamlining the creative process for procedural vector artwork. See the [roadmap](/features#roadmap) for a more in-depth summary of the goals for 2024 and beyond.

<!-- more -->

This is the second of our quarterly blog posts. If you missed [the first one](../graphite-progress-report-q1-2024), be sure to check it out after this.

In the past three months, we are delighted to have seen many contributions both from new and recurrent contributors, including substantial contributions from [Google Summer of Code](/blog/graphite-internships-announcing-participation-in-gsoc-2024/) students. We would like to send a big thanks to all of the contributors who made this progress happen. If you are interested in getting involved or just following development, see the [contributor guide](/volunteer/guide) and join [our Discord](https://discord.graphite.rs).

All Q2 2024 commits may be [viewed in this list](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-04-01&until=2024-06-30) and all noteworthy changes are detailed below.


## New editor features

- Add layer locking feature <small>([#1702](https://github.com/GraphiteEditor/Graphite/pull/1702))</small>

- Add corner rounding to the Rectangle node <small>([#1648](https://github.com/GraphiteEditor/Graphite/pull/1648))</small>

- Added fine-grained choices to Snapping options popover <small>([#1730](https://github.com/GraphiteEditor/Graphite/pull/1730))</small>

- Add rotation to Repeat node <small>([commit 72ba4dd](https://github.com/GraphiteEditor/Graphite/commit/72ba4ddfe421c0e17930ad1c2be85be2c69e04ea))</small>

- DropdownInput preview support and ColorButton history improvements <small>([#1598](https://github.com/GraphiteEditor/Graphite/pull/1598))</small>

- Bezier-rs: Add calculations for area and centroid of subpaths <small>([#1729](https://github.com/GraphiteEditor/Graphite/pull/1729))</small>

- Dot grid <small>([#1709](https://github.com/GraphiteEditor/Graphite/pull/1709))</small>

- Add grid color customization and choice to display as dots <small>([#1743](https://github.com/GraphiteEditor/Graphite/pull/1743))</small>

- Add Isometric Fountain demo artwork <small>([commit 6b0822d](https://github.com/GraphiteEditor/Graphite/commit/6b0822d31890b6699c4533c3e828da0e9e8c9490))</small>

- Add Area and Centroid nodes <small>([#1749](https://github.com/GraphiteEditor/Graphite/pull/1749))</small>

- Add boolean operations <small>([#1759](https://github.com/GraphiteEditor/Graphite/pull/1759))</small>

- Move gradient picking into the color picker <small>([#1778](https://github.com/GraphiteEditor/Graphite/pull/1778))</small>

- Add artboard displayed names in the viewport <small>([#1795](https://github.com/GraphiteEditor/Graphite/pull/1795))</small>

## Google Summer of Code projects

### Raster improvements

[TrueDoctor](https://github.com/truedoctor)'s Google Summer of Code project. See the [GitHub discussion thread](https://github.com/GraphiteEditor/Graphite/discussions/1773) for more details.

- New node: Rasterize <small>([#1755](https://github.com/GraphiteEditor/Graphite/pull/1755))</small>


### Node graph UI improvements 

[AdamGerhant](https://github.com/adamgerhant)'s Google Summer of Code project. See the [GitHub discussion thread](https://github.com/GraphiteEditor/Graphite/discussions/1769) for more details.

- Generalize layers as merge nodes to enable adjustment layers <small>([#1712](https://github.com/GraphiteEditor/Graphite/pull/1712))</small>

- Code cleanup and refactor for generalized layers <small>([#1738](https://github.com/GraphiteEditor/Graphite/pull/1738))</small>

- Enable Merge nodes to take vector data inputs from the bottom, not just left <small>([commit 244c8ad](https://github.com/GraphiteEditor/Graphite/commit/244c8ad10ad79c9ca4fcdb21260c5d5dc60b3a21))</small>

- Node network subgraph editing <small>([#1750](https://github.com/GraphiteEditor/Graphite/pull/1750))</small>

- Migrate node graph UI interaction from frontend to backend <small>([#1768](https://github.com/GraphiteEditor/Graphite/pull/1768))</small>

- Code cleanup after migrating node graph interaction to the backend <small>([#1790](https://github.com/GraphiteEditor/Graphite/pull/1790))</small>


### RAW image importing

[ElbertRonnie](https://github.com/elbertronnie)'s Google Summer of Code project. The `raw-rs` library is not yet used in the editor yet (but will hopefully be integrated soon). See the [GitHub discussion thread](https://github.com/GraphiteEditor/Graphite/discussions/1771) for more details.

- Create new library Raw-rs including a basic TIFF decoder <small>([#1757](https://github.com/GraphiteEditor/Graphite/pull/1757))</small>

- Run Raw-rs tests only on CI <small>([#1776](https://github.com/GraphiteEditor/Graphite/pull/1776))</small>

- Raw-rs: make decoder for ARW1 and ARW2 formats <small>([#1775](https://github.com/GraphiteEditor/Graphite/pull/1775))</small>


## Fixes

- Polish up the Layers panel design <small>([commit 938a688](https://github.com/GraphiteEditor/Graphite/commit/938a688fa08511ed4a5f3c3d5bcfa26f4bd1eb47))</small>

- Update cargo dependencies to fix security advisories <small>([commit 0f43a25](https://github.com/GraphiteEditor/Graphite/commit/0f43a254aff266dc7b4f3fc554e25b21514b64cc))</small>

- Update contributor guide installation instructions for Fedora-based Linux OSs <small>([#1718](https://github.com/GraphiteEditor/Graphite/pull/1718))</small>

- Add alpha to Extract Channel node and remove Extract Alpha node <small>([#1731](https://github.com/GraphiteEditor/Graphite/pull/1731))</small>

- Fix interactive outlining of layers within the Select tool's box selection <small>([#1727](https://github.com/GraphiteEditor/Graphite/pull/1727))</small>

- Insert duplicated layers directly above their selected source layers <small>([#1726](https://github.com/GraphiteEditor/Graphite/pull/1726))</small>

- Fix crash when Pen tool's in-progress point snaps along an angle with its previous anchor <small>([#1701](https://github.com/GraphiteEditor/Graphite/pull/1701))</small>

- Update roadmap with new features and icons <small>([commit 6a1a145](https://github.com/GraphiteEditor/Graphite/commit/6a1a145d190887f65f1f851282bd86c8836f85a7))</small>

- Polish and fix small bugs with tilt and zoom navigation <small>([commit 597c96a](https://github.com/GraphiteEditor/Graphite/commit/597c96a7db06fe44dbd4dc170511c011c7239073))</small>

- Remove editor instances concept and clean up JS interop code <small>([commit 19eb6ce](https://github.com/GraphiteEditor/Graphite/commit/19eb6ce0ab10065ec6acd6e49edd2f072729fc77))</small>

- Fix shallow/deep selection <small>([#1725](https://github.com/GraphiteEditor/Graphite/pull/1725))</small>

- Add initial Graphene docs to the contributor guide <small>([#1686](https://github.com/GraphiteEditor/Graphite/pull/1686))</small>

- Fix duplicate selection when holding Alt and dragging with Select tool <small>([#1739](https://github.com/GraphiteEditor/Graphite/pull/1739))</small>

- Store overlays, snapping, and grid state in saved documents and toggle them with hotkeys <small>([commit 7845302](https://github.com/GraphiteEditor/Graphite/commit/7845302c50705332711b6b18b4bdfe28a2f3c306))</small>

- Fix breakage of About Graphite dialog from editor instances refactor <small>([commit 1ce3d59](https://github.com/GraphiteEditor/Graphite/commit/1ce3d59e0f39e6733c7fa170af00ef59fd10ffd9))</small>

- Add visibility and delete buttons to node sections in the Properties panel <small>([commit 07fd2c2](https://github.com/GraphiteEditor/Graphite/commit/07fd2c27827e4a91ad238d790d41396a33ef2389))</small>

- Loosen the Graphene type system to allow contravariant function arguments <small>([#1740](https://github.com/GraphiteEditor/Graphite/pull/1740))</small>

- Fix image loading and remove resolve_empty_stacks() function <small>([#1746](https://github.com/GraphiteEditor/Graphite/pull/1746))</small>

- Improve backwards compatability robustness of serde-based document format <small>([commit de84e39](https://github.com/GraphiteEditor/Graphite/commit/de84e39c4ec6c191f73702a51be0a5ec9c662642))</small>

- Fix artboards not being included in Export menu's bounds list <small>([#1748](https://github.com/GraphiteEditor/Graphite/pull/1748))</small>

- Fix and refactor website meta tags for better SEO and social media previews <small>([commit d0c493c](https://github.com/GraphiteEditor/Graphite/commit/d0c493cdb6867763d507a24f12d92f2617385114))</small>

- Update Git Attributes for JSON syntax highlighting in `*.graphite` files <small>([#1752](https://github.com/GraphiteEditor/Graphite/pull/1752))</small>

- Update the website layout with narrower default width and better typography <small>([#1753](https://github.com/GraphiteEditor/Graphite/pull/1753))</small>

- Fix off by one subpath and unrelated crash <small>([#1754](https://github.com/GraphiteEditor/Graphite/pull/1754))</small>

- Fix primitive tool shapes appearing at document origin before dragging; fix Ctrl+0 recenter shifting <small>([#1751](https://github.com/GraphiteEditor/Graphite/pull/1751))</small>

- Always update source node in borrow tree <small>([#1758](https://github.com/GraphiteEditor/Graphite/pull/1758))</small>

- Fix tool drawing start point offset when an artboard has a transform <small>([#1763](https://github.com/GraphiteEditor/Graphite/pull/1763))</small>

- Update website roadmap <small>([commit e4d3faa](https://github.com/GraphiteEditor/Graphite/commit/e4d3faa52af42173eaf67b2dc5c2df5d6e6f23a8))</small>

- Fix crash and clean up frontend -> backend input handling code <small>([#1770](https://github.com/GraphiteEditor/Graphite/pull/1770))</small>

- Fix bug with Path tool point insertion often not working <small>([commit cf01f52](https://github.com/GraphiteEditor/Graphite/commit/cf01f522a8b57bfd625f15a10ae6b2812ed4c0a4))</small>

- Fix Poisson-Disk Points node transform of input shape <small>([#1784](https://github.com/GraphiteEditor/Graphite/pull/1784))</small>

- Fix clicking a selected anchor not deselecting all other selected points <small>([#1782](https://github.com/GraphiteEditor/Graphite/pull/1782))</small>

- Arrange layers in top level <small>([#1786](https://github.com/GraphiteEditor/Graphite/pull/1786))</small>

- Fix PathStyle::clear_fill() doctest in style.rs <small>([#1791](https://github.com/GraphiteEditor/Graphite/pull/1791))</small>

- Fix bounding boxes of VectorData with rotations in several nodes <small>([#1792](https://github.com/GraphiteEditor/Graphite/pull/1792))</small>
