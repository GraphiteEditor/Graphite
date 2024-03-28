+++
title = "Graphite Progress #1: Jan to March"
date = 2024-03-31
[extra]
banner = "https://static.graphite.rs/content/index/gui-demo-red-dress.avif"
banner_png = "https://static.graphite.rs/content/index/gui-demo-red-dress.png"
author = "0hypercube"
+++


[Graphite](/) is a new 2D vector graphics editor bringing a modern, nondestructive approach to creative tooling with node based procedural generation. The project is currently in the Alpha 3 milestone, with a focus on streamlining the creation procedural vector artwork. See the [roadmap](/features#roadmap) for a more in depth summary of the goals for Alpha 3.

Over the last three months we are delighted to have seen many contributions both from new and recurrent contributors, including substantial interest in the [Google Summer of Code](/blog/graphite-internships-announcing-participation-in-gsoc-2024/) program. If you are interested in getting involved, have a look at the [student projects page](/volunteer/guide/projects/student-projects/).

## Added

- [12e16b9](https://github.com/GraphiteEditor/Graphite/commit/12e16b9a4efe40fcf779c34f83d3df8b3e3542f3) 'Red Dress' demo art showcasing dynamic poisson disk sampling using the node graph.
- [#1594](https://github.com/GraphiteEditor/Graphite/pull/1594) Extending paths with freehand tool
  <div class="video-background"><video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback src="/2024-02-22-graphite-progress-1-jan-march/extend_freehand.webm"></div>
- [#1625](https://github.com/GraphiteEditor/Graphite/pull/1625) Autopanning when dragging outside of viewport
  <div class="video-background"><video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback src="/2024-02-22-graphite-progress-1-jan-march/autoscroll.webm"></div>
- [#1650](https://github.com/GraphiteEditor/Graphite/pull/1650) Solidify stroke node
  <div class="video-background"><video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback src="/2024-02-22-graphite-progress-1-jan-march/solidify_stroke.webm"></div>
- [#1593](https://github.com/GraphiteEditor/Graphite/pull/1593) Break path with Ctrl+Shift+Delete
- [#1586](https://github.com/GraphiteEditor/Graphite/pull/1586) Poisson disk sampling node
  <div class="video-background"><video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback src="/2024-02-22-graphite-progress-1-jan-march/poisson_disk.webm"></div>
- [#1579](https://github.com/GraphiteEditor/Graphite/pull/1579) SVG import
- [#1576](https://github.com/GraphiteEditor/Graphite/pull/1576) Morph node
- [#1521](https://github.com/GraphiteEditor/Graphite/pull/1521) Snapping system
  <div class="video-background"><video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback src="/2024-02-22-graphite-progress-1-jan-march/snapping.webm"></div>
- [#1664](https://github.com/GraphiteEditor/Graphite/pull/1664) Cancel more operations with right click
- [#1618](https://github.com/GraphiteEditor/Graphite/pull/1618) Vector2 node
- [#1592](https://github.com/GraphiteEditor/Graphite/pull/1592) Random scale and rotation for copy to points
  <div class="video-background"><video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback src="/2024-02-22-graphite-progress-1-jan-march/random_scale_rotation.webm"></div>


## Bug fixes

- [#1678](https://github.com/GraphiteEditor/Graphite/pull/1678) Ordering when grouping layers
- [#1651](https://github.com/GraphiteEditor/Graphite/pull/1651) Artboard deletion improvements
- [#1691](https://github.com/GraphiteEditor/Graphite/pull/1691) Graph view overlay updates when switching documents
- [#1693](https://github.com/GraphiteEditor/Graphite/pull/1693) Scrolling whilst dragging with path tool
- [#1670](https://github.com/GraphiteEditor/Graphite/pull/1670) Input hints
- [#1587](https://github.com/GraphiteEditor/Graphite/pull/1587) Pen tool undo
- [#1584](https://github.com/GraphiteEditor/Graphite/pull/1584) Undo when dragging widgets
- [#1614](https://github.com/GraphiteEditor/Graphite/pull/1614) Fix subtract node


## Internal improvments

- [#1624](https://github.com/GraphiteEditor/Graphite/pull/1624) Attribute based vector format in the node graph similar to blender
- [15931d0](https://github.com/GraphiteEditor/Graphite/commit/15931d06b1de870b2b6043a506bfe47b3fd84781) Caching in demo art
- [1bf62d9](https://github.com/GraphiteEditor/Graphite/commit/1bf62d92c23f87e2ccff75905dae055f0c7d76f9) Testing demo art
- [822b25c](https://github.com/GraphiteEditor/Graphite/commit/822b25ceb6590d2a7355179a8031edbdc3d29ac5) Update frontend dependencies


A big thanks to all of the contributors. If you are interested in getting involved or just following development, see the [contributor guide](/volunteer/guide) and join the [discord guild](https://discord.graphite.rs).
