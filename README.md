<a href="https://graphite.rs/">
<picture>
<source media="(prefers-color-scheme: dark)" srcset="https://static.graphite.rs/readme/graphite-readme-logo-dark-theme.svg">
<source media="(prefers-color-scheme: light)" srcset="https://static.graphite.rs/readme/graphite-readme-logo-light-theme.svg">
<img alt="Graphite logo" src="https://static.graphite.rs/readme/graphite-readme-logo-dark-theme.svg">
</picture>
</a>

<h1 align="center">Redefining state-of-the-art graphics editing.</h1>

Graphite is an in-development raster and vector 2D graphics editor that is free and open source. It is designed to be powered by a node graph compositing engine that supercharges your layer stack, providing a completely non-destructive editing experience.

Right now, Graphite is a lightweight vector graphics editor [available for alpha testing](https://editor.graphite.rs) in your web browser. The node system is in the prototype stage and will be usable in later 2022, while raster graphics editing and a native desktop application will come next year.

Learn more on the [project website.](https://graphite.rs/)

⭐ Please star this GitHub repository to help build momentum. ⭐

## Discord

If the Graphite project strikes your fancy, [join our Discord community](https://discord.graphite.rs) to chat with the community and development team. You're invited to stop by just to lurk, ask questions, offer suggestions, or get involved in the project. We are seeking collaborators to help design and develop the software and this is where we communicate.

## Contributing

We need Rust and web developers! See [instructions here](https://graphite.rs/contribute/) for getting started.

We are also in search of artists to create beautiful sample work in Graphite and illustrations for the website and social media. Please [get in touch](https://graphite.rs/contact/) if you are able to help out.

## Design

The #1 priority for the Graphite software is providing a delightful user experience. Unlike some other open source applications, the UI, UX, and product design is not an afterthought, but a central guiding light in the software development process.

Below are some mockups demonstrating future goals for the user interface once nodes and raster photo editing is possible.

![Graphite raster UI viewport mockup](https://static.graphite.rs/content/index/gui-mockup-viewport.png)

*Viewport interface mockup showcasing a photo editing project that utilizes Graphite's raster graphics pipeline, one of the upcoming roadmap milestones. Raster editing is not yet supported.*

![Graphite raster UI node graph mockup](https://static.graphite.rs/content/index/gui-mockup-nodes.png)

*Node graph mockup demonstrating how the layers directly correspond to nodes. Thick vertical (upward) lines represent compositing stacks and horizontal (rightward) links represent data flow connections.*

## Roadmap

Graphite does not use formal version numbers because of the constant rate of feature development and continuous release cycle. Changes can be tracked by [commit hash](https://github.com/GraphiteEditor/Graphite/commits/master). Development is broken into [monthly sprints](https://github.com/GraphiteEditor/Graphite/milestones) and less frequent release series based on major technology readiness milestones:

- Alpha Milestone 1 is the current release series available at [editor.graphite.rs](https://editor.graphite.rs) which encompasses minimum-viable-product (MVP) vector editing features. Features and improvements are continually added and deployed. Regrettably, file format stability isn't guaranteed at this stage since it would prohibitively hinder the pace of development.

- Alpha Milestone 2 is the next release series. It will introduce the node graph system for procedural vector editing. Late summer is the optimistic target.

- Alpha Milestone 3 will probably focus on switching to an in-house vector graphics render engine built on [wgpu](https://wgpu.rs/).

- Alpha Milestone 4 will probably introduce raster compositing.

- Beta versions will follow once basic procedural vector and raster editing is fully supported. File format stability, authoring + sharing custom nodes/extensions, and a downloadable native desktop client will be included during or before Beta.

- RAW photo editing, advanced color handling, automation and batch processing, and procedural painting workflows will be added during further Beta development.
