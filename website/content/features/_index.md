+++
title = "Features and roadmap"
template = "page.html"
+++

<section class="section-row">
<div class="section">

# Features and roadmap.

The current version of Graphite provides tools for designing vector art with Bezier curves, similar to tools like Inkscape, Illustrator, and Affinity Designer. These creations may be exported to SVG, JPG, or PNG formats. External images may be imported and placed in the document as a layer (but not directly edited yet).

The present priority is building the node graph system and integrating it with the existing vector editing toolset. Once ready, work will shift to building a raster-based render engine. More advanced features will build off those core capabilities.

Short-term feature development at a granular level is tracked in the [Task Board](https://github.com/GraphiteEditor/Graphite/projects/1) on GitHub. Check that out to see what's coming down the pipeline during monthly sprints. Graphite does not use formal version numbers because of the constant rate of feature development and continuous release cycle. Changes can be tracked by [commit hash](https://github.com/GraphiteEditor/Graphite/commits/master) and progress divided into [monthly sprints](https://github.com/GraphiteEditor/Graphite/milestones). The hosted web app deploys a [recent commit](https://github.com/GraphiteEditor/Graphite/releases/tag/latest-stable) from the past week or two.

## Milestones

Release series are announced based on major technology readiness milestones. Following a year of pre-alpha development, alpha milestone 1 was [announced](https://graphite.rs/blog/announcing-graphite-alpha/) and work has continued under that banner while progressing towards the features of the second milestone release.

- Alpha Milestone 1 is the current release series available at [editor.graphite.rs](https://editor.graphite.rs) which encompasses minimum-viable-product (MVP) vector editing features. Features and improvements are continually added and deployed. Regrettably, file format stability isn't guaranteed at this stage since it would prohibitively hinder the pace of development.

- Alpha Milestone 2 is the next release series. It will introduce the node graph system for procedural vector editing. This is expected to be ready before the end of 2022.

- Alpha Milestone 3 will probably focus on switching to an in-house vector graphics render engine built on [wgpu](https://wgpu.rs/).

- Alpha Milestone 4 will probably introduce raster compositing.

- Beta versions will follow once basic procedural vector and raster editing is fully supported. File format stability, authoring + sharing custom nodes/extensions, and a downloadable native desktop client will be included during or before Beta.

- RAW photo editing, advanced color handling, automation and batch processing, and procedural painting workflows will be added during further Beta development.

## Planned capabilities

Below is an incomplete list of planned features and longer-term aspirations.

Short Term:
- Node graph and layer tree
- Procedural generation
- Importing SVG files

Medium Term:
- Mixed vector and raster workflow
- Compositing engine
- Resolution-agnostic rendering
- RAW photo editing
- HDR/WCG color handling
- Data viz/graph/chart creation
- Data-driven template replacement
- Advanced typesetting
- Procedural painting
- CAD-like constraint solver
- Real-time collaborative editing
- Custom node scripting
- Asset manager and store
- Batch conversion and processing
- Portable render engine
- Localization/internationalization
- Keyboardless touch and stylus controls
- Native desktop application

Long Term:
- Physically-based painting
- Motion graphics and animation
- Live video compositing
- Animated SVG authorship
- Distributed rendering system

</div>
</section>
