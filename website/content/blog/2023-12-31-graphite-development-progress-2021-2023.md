+++
title = "Graphite development progress: 2021-2023"
date = 2023-12-31

[extra]
banner = "https://static.graphite.rs/content/blog/2022-03-12-graphite-a-vision-for-the-future-of-2d-content-creation.avif"
banner_png = "https://static.graphite.rs/content/blog/2022-03-12-graphite-a-vision-for-the-future-of-2d-content-creation.png"
author = "Keavon Chambers"
summary = "An archive of Graphite development history with progress updates from 2021 to 2023."

js = ["/js/youtube-embed.js"]
css = ["/component/youtube-embed.css"]
+++

TODO

<!-- more -->

## 2021

### February 2021

***Watch the [project announcement presentation](https://youtu.be/Ea4Wt_FgEEw?t=564) and first month [project update presentation](https://youtu.be/gqCxt8XL92o?t=392) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#19](https://gamedev.rs/news/019/#graphite):*

After this month's Rust Gamedev Meetup announcement presentation where the Graphite vision has attracted tremendous interest, advice of the community has resulted in shifting the development strategy to focus on an MVP release as soon as possible, which has begun:

- The past year's in-development custom GUI has been shelved in lieu of an interim web GUI. Graphite intends to natively support Windows, Mac, Linux, and web. This change unblocks core application development but means Graphite is web-only until the Rust GUI ecosystem matures. Good progress this month has been made building a [Vue](https://vuejs.org/) web GUI.
- Graphite's MVP will now support only vector editing. This defers the large complexity of the graph render engine required for node-based raster editing. It should be less difficult to first focus on building a vector editor that improves upon the UX of other similar apps.

This image shows the current GUI implementation state of progress:

![Progress on the GUI](/images/2021-02.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/1?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-02-14&until=2021-02-28)

### March 2021

***Watch the [project update presentation](https://youtu.be/XE0lH0tlbBs?t=3661) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#20](https://gamedev.rs/news/020/#graphite):*

Major progress was made building out the core architectural Rust code. A large accomplishment was designing a first draft software architecture diagram:

<details>
<summary>Draft architecture diagram (click to expand)</summary>

<p class="wide">
<img src="/images/2021-03-architecture-diagram.png" alt="Graphite architecture diagram" />
</p>

</details>

The current editor now has functional Select, Rectangle, and Ellipse tools thanks to the newly-added tool state machine and SVG viewport drawing. The UI now also implements tool-related icons and buttons, bringing it closer to parity with the design mockup. The team also set up a Web/Rust-WASM build system, GitHub CI to confirm PRs compile, and put together starter documentation for the codebase, UX design, and manual.

This image shows the latest editor UI and "Graphite" drawn crudely with its new shape tools:

!["Graphite" drawn using the circles and rectangles of the new tool drawing system](/images/2021-03.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/2?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-03-01&until=2021-03-31)

### April 2021

***Watch the [project update presentation](https://youtu.be/6drrul3p_hU?t=3029) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#21](https://gamedev.rs/news/021/#graphite):*

The team size has doubled in the past month — thank you to the new contributors! Since then, systems related to editor tools and data flow were added. The editor now has proper input behavior on the existing Rectangle and Ellipse tools plus the new Shape and Line tools while holding modifier keys. Pen tool implementation has begun, supporting polylines. Shapes are now drawn with live previews.

Additional work has gone into improving render performance, building the color system in the Rust backend, and adding initial support for displaying shapes in the Layers panel.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/3?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-04-01&until=2021-04-30)

### May 2021

***Watch the [project update presentation](https://youtu.be/Wuwxh958P6I?t=2088) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#22](https://gamedev.rs/news/022/#graphite):*

In the past month, new frontend features have mostly closed the gap for a visually complete UI while a major Rust backend refactor took place.

A new frontend system for floating menus was added to draw menus over the UI, like dropdown menu input widgets and popovers to display the new color picker. Also, the application menu bar was built with working buttons for the new Undo and Export SVG actions.

A large refactor in the Rust backend created a simpler communication strategy between all components in the software stack and a standard method of handling user inputs.

This image shows a famous art piece by Piet Mondrian replicated in Graphite with its latest UI:

![Piet Mondrian's artwork replicated in Graphite using the new color picker](/images/2021-05.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/4?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-05-01&until=2021-05-31)

### June 2021

***Watch the [project update presentation](https://youtu.be/0cefGQyZXH4?t=3423) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#23](https://gamedev.rs/news/023/#graphite):*

Since last newsletter, the editor has received the ability to select layers via the Layers panel and by clicking or dragging a box selection in the viewport. Selected layers can be deleted, duplicated, and copy/pasted.

It is now possible to create, edit, and close multiple documents in their own editor tabs. Additional frontend cleanup and polish has also improved many parts of the editor experience.

Lastly, support for transforms was added to the layers and document, paving the way for moving/scaling/rotating layers and the whole document within the viewport.

This artwork, created by Graphite community member BillyDM, won our first art contest:

![Geometric cherry tree artwork](/images/2021-06.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/5?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-06-01&until=2021-06-30)

### July 2021

***Watch the [project update presentation](https://youtu.be/g-QZAVipiuU?t=1906) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#24](https://gamedev.rs/news/024/#graphite):*

In the past month, the editor has gained numerous vector editing features, including moving layers with the keyboard or mouse, filling and copying colors, flipping and aligning selected layers, and setting blend modes and layer opacity.

Scrollbars, rulers, and thumbnails are now functional. Full screen support has been added, along with a hotkey to center the artwork. An options bar with tool-specific settings and actions has been implemented, currently allowing the number of sides of a polygon to be selected. The order of layers can now be changed using hotkeys.

This image by Graphite community member Norgate shows a recreation of Edvard Munch's "The Scream" using the new drawing tools:

![A recreation of "The Scream" in Graphite by Norgate](/images/2021-07.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/6?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-07-01&until=2021-07-31)

### August 2021

***Watch the [project update presentation](https://youtu.be/TH3AErcNcTY?t=3070) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#25](https://gamedev.rs/news/025/#graphite):*

Work has progressed on features for the app and designs for the project website. Crucial user-facing features have been added: saving/opening documents; a bug report dialog for panics; an auto-generated list of dependency license notices; and a new undo/redo system.

The new Path tool shows Bézier anchors/handles (soon to be draggable). Render performance is much better. Scrollbars now work with the infinite canvas. There's a new transform cage around selected shapes, which may be transformed with Blender-inspired <kbd>G</kbd>/<kbd>R</kbd>/<kbd>S</kbd> keys.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/7?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-08-01&until=2021-08-31)

### September & October 2021

***Watch the [project update presentation](https://youtu.be/nLyiLnC5mn4?t=2410) at the Rust Gamedev Meetup.***

Exploratory research has been ongoing for GPU-driven graphics in Rust and its place in the Graphite pipeline.

Several development tooling upgrades have been made, from TypeScript type checking and cutting down unnecessary dependencies to speeding up build time on license notice generation. An internal refactor made multi-document support now utilize unique document IDs.

The editor's ruler measurements now move and scale with the document. Users of incompatible browsers are now visibly told to upgrade. And meanwhile, further progress has been made on the project website design.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/8?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-09-01&until=2021-10-31)

### November 2021

***Watch the [project update presentation](https://youtu.be/S7aoi_4a2uE?t=2538) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#28](https://gamedev.rs/news/028/#graphite):*

Development continues to pick up speed. Design of the project website has continued for its launch soon, alongside the forthcoming alpha release.

The project upgraded to the Rust 2021 edition and made big improvements to the frontend TypeScript and web infrastructure. The editor UI is now fully responsive at small window sizes. Unsaved document tabs display an `*` and warn before closing the window. And the new snapping system helps draw/move shapes aligned with others.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/9?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-11-01&until=2021-11-30)

### December 2021

***Watch the [project update presentation](https://youtu.be/BIMsBFbPV-c?t=1869) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#29](https://gamedev.rs/news/029/#graphite):*

This wraps up a productive month of features and stability improvements. Documents persist page reloads via IndexedDB browser storage. The Layers panel got some love. Vector anchor points can be dragged (beginnings of the Path/Pen tools). Per-tool footer bar hints teach possible user input actions. And a big code cleanup/refactor took place behind the scenes.

Additional new features and QoL improvements: artboards, panel resizing, the Navigate tool, outline view mode, support for touch input and non-Latin keyboards, an *About Graphite* dialog with version info, plus dozens of bugs and crashes were resolved.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/10?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2021-12-01&until=2021-12-31)

## 2022

### January 2022

***Watch the [project update presentation](https://youtu.be/adt63Gqt6yA?t=7143) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#30](https://gamedev.rs/news/030/#graphite):*

As January winds to a close and the project anniversary nears, the team is proud and excited to announce Graphite alpha 1, the minimum viable product release for a web-based vector graphics editor.

After one year in pre-alpha development by Graphite community members, this first milestone of alpha is nearly here.

Graphite alpha 1 launches **Saturday, February 12** together with the project's website.

Work now begins on alpha 2, focused on building the node graph engine and vector renderer.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/11?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-01-01&until=2022-01-31)

### February 2022

***Watch the [project update presentation](https://youtu.be/dQPkyjbd36Y?t=3953) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#31](https://gamedev.rs/news/031/#graphite):*

After [officially launching the alpha](../announcing-graphite-alpha) version early this month, work has progressed in designing the node graph system. Also, the team has spent this month adding polish to the application and continuing work on more website content.

A new editor feature is the Gradient tool which makes it possible to add some colorful pizzazz. This means that finally all vector editing tools are implemented, but some can still use improvement. Additional work has gone into visual changes to help aid in clarity and discoverability for new users.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/12?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-02-01&until=2022-02-28)

### March 2022

***Watch the [project update presentation](https://youtu.be/okWFrfaaADs?t=2936) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#32](https://gamedev.rs/news/032/#graphite):*

With the completion of the initial node graph UX design, work has begun building the frontend and backend systems for the big leap to node-driven vector editing. This works by composing groups of Rust functions together at runtime and/or compile time. [Watch the (brief) talk](https://youtu.be/okWFrfaaADs?t=4014) about how the backend implementation works around challenges imposed by Rust.

New editor features this month include importing bitmap image layers and customizing stroke styling with dashed lines and rounded or beveled corners. The project website is also now mostly content-complete, including new node graph mockups.

This image shows the first full illustration drawn in Graphite, together with its latest UI:

![Vector artwork of Yosemite Valley illustrated in Graphite](/images/2022-03.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/13?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-03-01&until=2022-03-31)

### April 2022

***Watch the [project update presentation](https://youtu.be/XOpZIzmFifk?t=3845) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#33](https://gamedev.rs/news/033/#graphite):*

April's development has focused on further editor features and UX improvements:

- **It's your type:** The Text tool now provides over 1400 fonts with bold/italic styles from the Google Fonts library.

- **Oh snap!:** A refactor and polish pass on the snapping system provides better clarity and consistency. And shapes now have outlines on hover and selection for easier targeting.

- **Have a dialog:** Supported by a refactor that moved dialog layouts into the Rust backend, users can now create new documents of specified sizes and export artwork as PNG/JPG with new *File* menu dialogs.

- **Pack it up:** The web component of the stack was finally upgraded to Webpack 5 which cleans up a mess of outdated dependencies.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/14?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-04-01&until=2022-04-30)

### May 2022

***Watch the [project update presentation](https://youtu.be/drcX3dCS5MY?t=3509) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#34](https://gamedev.rs/news/034/#graphite):*

May's development has focused on project cleanup:

- **Spring cleaning:** The past month's work has focused mostly on technical debt cleanup, documentation, and bug fixes around the frontend. That continues with the Rust backend next month.

- **A radiant gradient:** The Gradient tool now supports radial styles in addition to linear.

- **New blog post:** [Learn about plans](../distributed-computing-in-the-graphene-runtime/) for distributed computing across many CPUs and GPUs with Graphene, the Rust-based node graph engine and renderer that will power Graphite.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/15?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-05-01&until=2022-05-31)

### June 2022

***Watch the [project update presentation](https://youtu.be/mnuchYuR_ck?t=4309) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#35](https://gamedev.rs/news/035/#graphite):*

June's development focused mainly on bug fixes and big under-the-hood changes:

- **Ahead of the curve:** A long-awaited refactor replaces the underlying Bézier curve data structure in alignment with requirements for Pen tool improvements and the upcoming node system.
- **Sending mixed messages:** The internal messaging system was upgraded to sequence the message processing in a more predictable stack-based order. A new subscription-based event broadcaster was integrated as well.
- **Back on the menu:** The application menu bar content definitions were moved from the JS frontend to a permanent home in the Rust backend.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/16?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-06-01&until=2022-06-30)

### July 2022

***Watch the [project update presentation](https://youtu.be/s9kf9HVUKYE?t=2481) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#36](https://gamedev.rs/news/036/#graphite):*

July's development focused on editor-centric refactors upgrading stopgap measures to more robust systems:

- **Making a splash:** The default document is replaced by a welcome splash screen following a refactor allowing for zero open documents.
- **Modifying for Macs:** Input handling supports the nonstandard modifier keys on Mac keyboards, including labels in the UI.
- **Setting a high bar:** The menu bar cleans up actions and supports new ones like *File* > *Import*. Displayed hotkeys are based on the actual key mapping source, varying by OS.
- **Keeping organized:** The editor codebase is restructured to cut away technical debt and create consistency for new contributors and better docs going forward.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/17?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-07-01&until=2022-07-31)

### August 2022

***Watch the [project update presentation](https://youtu.be/QKqqDilZ448?t=1096) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#37](https://gamedev.rs/news/037/#graphite):*

August's development focused on Bézier shape editing and easier layer transformation:

- **The pen is mightier than before:** Bézier shapes gain support for curve extension and shape closing using the Pen tool and inserting points along curves with the Path tool.
- **Front and center:** Layer origins may be set to control the center of rotation and scale using the Transform tool.

Meanwhile, design and architecture work on the Graphene node-based programming language has been well underway. Graphene is the data graph engine that will replace Graphite's tree-based layer system in the next few months and evolve into a raster-and-vector render engine over time.

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/18?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-08-01&until=2022-08-31)

### September & October 2022

***Watch the [project update presentation](https://youtu.be/BS_446HI12I?t=1096) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#39](https://gamedev.rs/news/039/#graphite):*

September and October's development focused on major new features and improvements to make Graphite more useful and usable:

- **Like magic:** Stable Diffusion, the open source AI image generator, is integrated into Graphite as the experimental Imaginate tool. It provides an innovative non-destructive workflow to interactively co-create art with AI.
- **Right on the nodes:** Graphite's node graph engine prototype is finally up and running, now integrated with a node-powered tool that desaturates the underlying artwork. A graph panel will be ready shortly.
- **With flying colors:** The color picker menu gains hex/RGB/HSV controls. Gradients get unlimited color transitions. The Eyedropper tool is rewritten to sample pixels from the viewport. The editor UI gets a color scheme design refresh for improved clarity, plus helpful new input widgets.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/19?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-09-01&until=2022-10-31)

### November 2022

***Watch the [project update presentation](https://youtu.be/Ck2R0yqTLcU?t=2849) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#40](https://gamedev.rs/news/040/#graphite):*

- **Filling in the blanks:** The experimental Imaginate tool gains Inpaint/Outpaint, letting users [replace content](https://youtube.com/watch?v=Ck2R0yqTLcU&t=3269) in masked areas and even [un-crop](https://youtube.com/watch?v=Ck2R0yqTLcU&t=3862s) images, powered by Stable Diffusion.
- **Connecting the dots:** The node graph compositor now [supports interactive editing](https://youtube.com/watch?v=Ck2R0yqTLcU&t=4332), so users can drag nodes and chain together effects. Nodes can be set in the Properties panel or exposed as inputs in the graph.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/20?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-11-01&until=2022-11-30)

### December 2022

***Watch the [project update presentation](https://youtu.be/iSu-9yKsCRY?t=2138) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#41](https://gamedev.rs/news/041/#graphite):*

- **Chain reaction:** The experimental Imaginate feature, an AI image generation workflow powered by Stable Diffusion, becomes a node. Chain together a sequence of fine-tuned generation steps. And explore ideas by branching the graph into new creative directions.
- **Node nurturing:** New features provide polish to the node graph. Nodes can be copy/pasted, hidden, previewed, and linked more easily.
- **Bugs, begone!:** A major effort to improve editor usability fixes dozens of bugs and paper cuts. Boolean shape operations now crash less frequently, the UI no longer slows down badly over time, and undo history is finally fixed.

This video shows a timelapse of creating mixed vector-and-raster art using the latest features:

<div class="youtube-embed aspect-16x9">
	<img data-youtube-embed="JgJvAHQLnXA" src="https://static.graphite.rs/content/index/commander-basstronaut-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite - Vector Editing: &quot;Commander Basstronaut&quot; Artwork (25x Timelapse)" />
</div>

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/21?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2022-12-01&until=2022-12-31)

## 2023

Full-year summary blog post: [**Looking back on 2023 and what's next**](../looking-back-on-2023-and-what-s-next)

### January 2023

***Watch the [project update presentation](https://youtu.be/HTxX-Wm-3R8?t=2010) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#42](https://gamedev.rs/news/042/#graphite):*

- **Picture this:** Imported images are now part of the node graph. The new *Image Frame* node converts bitmap data into a vector rectangle holding the image. This paves the way for other vector data like shapes and text to soon be converted into nodes and composited alongside images.
- **Instant iterations:** Incremental graph compilation avoids recompiling the whole graph each time an edit is made or a value changes. This makes iteration faster and enables caching of intermediate computations for faster rendering.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/22?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-01-01&until=2023-01-31)

### February 2023

***Watch the [project update presentation](https://www.youtube.com/watch?v=UBtflhwgAHw) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#43](https://gamedev.rs/news/043/#graphite):*

- **Shaping up:** Editing shapes is now easier thanks to point selection and manipulation improvements.
- **Deep dive:** The user experience of nested layer selection is improved by introducing "Deepest" and "Shallowest" modes.
- **Scroll settings:** Scroll up-and-down, or zoom in-and-out, at your preference using the new configuration for scroll wheel behavior.
- **Graph growth:** Additional node graph engineering introduces graceful type checking and brings GPU-accelerated compositing closer to realization.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/23?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-02-01&until=2023-02-28)

### March 2023

***Watch the [project update presentation](https://www.youtube.com/watch?v=ZzxTtAuYk04) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#44](https://gamedev.rs/news/044/#graphite):*

- **Vector nodes:** A major refactor moves vector shape layers into the node graph. Now the *shape*, *transform*, *fill*, and *stroke* are all set via nodes in the graph. Text is the final remaining holdout and will be node-ified next, letting the node graph act as the universal layer type.

This image shows the latest vector artwork created in Graphite, with a glimpse at the new node graph shown beneath for the selected layer:

![Valley of Spires - Vector art made in Graphite](/images/2023-03.png)

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/24?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-03-01&until=2023-03-31)

### April 2023

***Watch the [project update presentation](https://www.youtube.com/watch?v=PrYO1gg7hX4) at the Rust Gamedev Meetup.***

*The following update originates from the Rust Gamedev Newsletter [#45](https://gamedev.rs/news/045/#graphite):*

- **Brushing up:** The new Brush tool makes it possible to paint raster-based art.
- **Writing down:** A refactor of the Text tool integrates typographic content in the node graph. Finally, all artwork types are node-based.
- **Showing true colors:** Node graph compositing now uses linear, not gamma, color. Key new color adjustment nodes are added.
- **Laying the groundwork:** Further engineering work prepares the node graph language for GPU execution. And development continues toward in-graph layer stack compositing.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/25?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-04-01&until=2023-04-30)

### May 2023

***Watch the [project update presentation](https://youtu.be/1DiA3OYqvqU?t=1923) at the Rust Gamedev Meetup.***

- **GPU goodness:** The node graph engine now offers experimental GPU-powered execution using [WGPU](https://wgpu.rs/) and WebGPU in Chromium browsers. Computation is now decoupled from the editor for asynchronous, high-performance workflows. Early [rust-gpu](https://rust-gpu.github.io/) shader compilation is also supported.
- **A better brush:** The brush tool is now smoother and more responsive, with real-time performance and new eraser and blend modes for versatile raster editing.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/26?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-05-01&until=2023-05-31)

### June 2023

***Watch the [project update presentation](https://youtu.be/47wamZL5IFw?t=1968) at the Rust Gamedev Meetup.***

- **Menu makeover:** The menu for inserting nodes into the graph has been revamped.
- **Engine engineering:** Further work has gone into the infrastructure of the Graphene node engine.

<br />

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/27?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-06-01&until=2023-06-30)

### July 2023

***Watch the [project update presentation](https://youtu.be/I_7AgjiE9RA?t=2681) at the Rust Gamedev Meetup.***

- **Reach for the stars:** Vector editing is now more pleasant and comes with new support for box selection of points and drawing star shapes.
<!--
# Widgets
Rename PropertyHolder to LayoutHolder										Keavon Chambers			Jul 31 23:42
Remove widgets built by methods on WidgetHolder								Keavon Chambers			Jul 31 23:36
Switch to the widget builder pattern on all remaining layouts (#1346)		0HyperCube				Jul 31 07:21

# Node graph migration
Initial work migrating vector layers to document graph						0hypercube				Jul 30 20:16
Fix warnings introduced by artboard nodes									0hypercube				Jul 30 15:35
Artboard nodes (#1328)														0HyperCube				Jul 27 07:35
Move Imaginate cache into the node											Dennis Kobert			Jul 14 16:40
Fix Imaginate node types to prevent an immediate crash						Dennis Kobert			Jul 14 15:28
Added primary output option in DocumentNodeType (#1275)						Prikshit Gautam			Jul 5 13:06
Graphene CLI + quantization research (#1320)								Dennis Kobert			Jul 4 17:04

# CI and build infrastructure
Reenable CI for forks and remove duplicate build link comment (#1339)		Dennis Kobert			Jul 29 20:06
Update GitHub Actions CI action versions (#1336)							Bruce Mitchener			Jul 29 06:36
Embed git commit hash in the document file (#1325)							Dennis Kobert			Jul 28 22:52
Clear `wasm-bindgen` cache before compilation (#1334)						Dennis Kobert			Jul 28 08:35
Fix clippy lints (#1327)													0HyperCube				Jul 19 16:38

# Tools
Add checkbox for a transparent BG when exporting image (#1344)				Keavon Chambers			Jul 30 10:57
Select tool's shallowest behavior improvements and refactoring (#1108)		Christopher Mendoza		Jul 28 18:23
Add Polygon/Star toggle to Shape tool (#1215)								Leonard Pauli			Jul 4 16:29

# UI polish
Show disabled menu bar entries with no active document (#1332)				Dhruv					Jul 27 11:26
Fix missing popover text (#1329)											0HyperCube				Jul 19 22:04
Dragging selected folder now drags all children (#1279)						Senne Hofman			Jul 4 16:50

# Snapping
Hide snapping overlays when snapping to a point (#1260)						Shouvik Ghosh			Jul 29 05:25
Add Snapping Options to the Snap Dropdown Menu (#1321)						Dhruv					Jul 15 15:07
-->

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/28?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-07-01&until=2023-07-31)

### August 2023

<!--
New nodes: Resample Points, Spline from Points (#1226)											Chase							Aug 30 20:21
New nodes: Logical boolean operations (OR, AND, XOR, NOT) (#1399)								isiko							Aug 30 08:55
Add Path tool options for editing X/Y point coordinates (#1404)									mobile-bungalow					Aug 29 21:41
Add !build command for core team to create build links											Keavon Chambers					Aug 29 15:33
Clean up and suppress Clippy warnings (#1402)													Prikshit Gautam					Aug 28 05:26
Remove console spam (#1400)																		0HyperCube						Aug 28 00:12
New nodes: shape/curve primitives (#1389)														Ezbaze							Aug 27 22:22
Fix demo art shape node (#1401)																	0HyperCube						Aug 26 19:43
Add a jostle hint to the website main carousel													Keavon Chambers					Aug 25 23:56
Reduce website static content file size by a lot												Keavon Chambers					Aug 23 15:36
Fix commit branch when not in pull request														0hypercube						Aug 23 21:54
Fix github CI branch name (#1396)																0HyperCube						Aug 23 16:17
Update wasm-bindgen, syn and wgpu (#1398)														0HyperCube						Aug 23 15:53
Preserve exposed state on copy (#1397)															0HyperCube						Aug 23 13:31
Add demo artwork open links to website															Keavon Chambers					Aug 22 03:41
Add demo artwork																				Keavon Chambers					Aug 22 03:26
Fix node colors; fix spacebar not closing graph													Keavon Chambers					Aug 20 23:32
Fix navigate and rulers																			0hypercube						Aug 21 22:19
Fix artboard tool and remove old artboard code													0hypercube						Aug 21 21:43
Fix gradient tool																				0hypercube						Aug 21 13:03
Fix freehand and spline tool																	0hypercube						Aug 20 13:43
Fix positioning of new layer nodes after rebase													hypercube						Aug 20 08:36
Fix eyedropper tool																				hypercube						Aug 19 12:46
Fix path tool																					hypercube						Aug 19 12:33
Improve the Circular Repeat node																Keavon Chambers					Aug 19 22:46
Fix graph view button hotkey tooltip; fix layer CSS bug											Keavon Chambers					Aug 19 17:04
New node: Pixel Noise (#1267)																	isiko							Aug 19 20:30
Move node graph from panel to overlay on viewport												Keavon Chambers					Aug 19 01:01
Disable parcel cache for the dev server															Dennis Kobert					Aug 19 01:10
Improve the layers UI in the node graph															Keavon Chambers					Aug 18 14:26
Add auto deployment infrastructure for website													Keavon Chambers					Aug 16 00:35
Improve website book sidebar and nav ripple														Keavon Chambers					Aug 15 02:10
Add more advanced math nodes (#1383)															isiko							Aug 14 15:43
Improve responsive design sizing for website													Keavon Chambers					Aug 12 12:48
Fix undo not immediately removing copy/pasted layer (#1381)										xkef							Aug 13 21:47
Don't include the document node path in the stable node id by default (#1388)					Dennis Kobert					Aug 13 12:25
Curves image adjustment node (#1214)															nat-rix							Aug 13 10:07
Fix prod deployment cache issue and analytics templating										Keavon Chambers					Aug 12 13:16
Fix crash from overflowing values given to NumberInput widgets (#1377)							Omar Magdy						Aug 11 12:31
Correct snapping offsets after canvas transformation when using snap-reliant tools (#1370)		Dhruv							Aug 11 14:51
Add more math nodes (#1378)																		isiko							Aug 11 11:11
Grayscale Node respects the alpha chanel (#1380)												Ezbaze							Aug 11 09:45
Remove unwraps in webgpu checking function (#1382)												Dennis Kobert					Aug 11 10:33
Fix website styling bugs caused by Safari														Keavon Chambers					Aug 10 22:59
Revamp the Graphite website (#1265)																Keavon Chambers					Aug 9 09:28
Roll back node dragging smoothing for now due to wire visual bugs								Keavon Chambers					Aug 9 01:10
Add bounding box node (#1376)																	isiko							Aug 8 18:44
Add Circular Repeat Node (#1373)																isiko							Aug 8 18:13
Additional Input and Math Nodes (#1369)															Ezbaze							Aug 8 16:31
Fix shapes only showing up as outlines on document load (#1375)									Dennis Kobert					Aug 8 14:07
Manually check if web gpu support is available (#1372)											Dennis Kobert					Aug 8 14:04
Add config for nix-shell (#1359)																isiko							Aug 8 14:03
Add Vector Repeat Node (#1371)																	Dennis Kobert					Aug 7 14:59
Add hints for Brush tool resizing (#1363)														Omar Magdy						Aug 7 01:32
Decrease graph compilation time significantly (#1368)											Dennis Kobert					Aug 6 21:36
Add a few node graph style improvements															Keavon Chambers					Aug 5 00:55
Layer-based nodes redesign, just the basics so far (#1362)										Keavon Chambers					Aug 4 14:56
Fix BrushCache serailization (#1358)															Dennis Kobert					Aug 4 20:34
Fix tool shelf's scrollbar layout shift															Keavon Chambers					Aug 3 23:33
Fix Text Tool Overwriting Existing Text on Editing Text Layer (#1356)							Dhruv							Aug 3 21:38
Fix stuck panning after releasing space bar but not LMB (#1353)									Ezbaze							Aug 3 06:48
Fix typo in proto.rs (#1348)																	Ikko Eltociear Ashimine			Aug 2 00:59
-->

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/29?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-08-01&until=2023-08-31)

### September & October 2023

<!--
Add comments to help explain Graphene concepts												Keavon Chambers			Tue Oct 24 21:32:42 2023 -0700
Rename 'Grayscale' node to 'Black & White'													Keavon Chambers			Tue Oct 24 21:22:22 2023 -0700
Remove dead code from overlay graph view state tracking										Keavon Chambers			Tue Oct 24 21:09:02 2023 -0700
Create node by dragging link into empty space (#1438)										0HyperCube				Tue Oct 24 21:22:41 2023 +0100
Migrate the Text tool to the document graph (#1435)											0HyperCube				Tue Oct 24 20:55:13 2023 +0100
A few minor lints and docs (#1436)															Yuri Astrakhan			Thu Oct 19 02:33:10 2023 -0400
Move DeleteLayer to come before SelectionChanged on DeletedSelectedLayers (#1417)			Rob Bertram				Wed Oct 18 13:32:10 2023 -0400
Migrate the Select tool to the document graph (#1433)										0HyperCube				Tue Oct 17 18:59:30 2023 +0100
Lay Groundwork for Rust-based SVG rasterization (#1422)										Dennis Kobert			Sat Sep 30 11:43:24 2023 +0200
New node: Mandelbrot (#1421)																Dennis Kobert			Thu Sep 14 11:08:57 2023 +0200
Insert pasted images as layers in document graph (#1418)									Dennis Kobert			Wed Sep 13 17:02:35 2023 +0200
Lay groundwork for adaptive resolution system (#1395)										Dennis Kobert			Wed Sep 6 12:39:21 2023 +0200
Fix crash when a cycle is introduced into the graph (#1427)									Vlad Rakhmanin			Sat Sep 30 11:07:29 2023 +0100
Update `graphene-cli` and fix no-std compilation for `graphene-core` (#1428)				Dennis Kobert			Sat Sep 30 11:20:17 2023 +0200
Make 'Close All Documents' not confirm if all open documents are already saved (#1423)		Vlad Rakhmanin			Thu Sep 21 22:02:18 2023 +0100
Allow toggling smooth/sharp angle from the path tool options bar (#1415)					mobile-bungalow			Mon Sep 11 17:36:08 2023 -0700
Make RadioInput accept optional selected_index												Keavon Chambers			Mon Sep 11 15:40:05 2023 -0700
Rework navigation tool hints and navigation shortcuts (#1419)								mobile-bungalow			Sun Sep 10 15:42:27 2023 -0700
Clean up code for optional node inputs/outputs												Keavon Chambers			Sun Sep 3 05:15:02 2023 -0700
Add 'select all points' method to ShapeState struct (#1386)									Prikshit Gautam			Tue Sep 5 08:05:43 2023 +0530
Remove history step creation from Select tool box selection									Keavon Chambers			Sat Sep 2 15:26:04 2023 -0700
Rename the Value node to Number (#1412)														Keavon Chambers			Sat Sep 2 15:11:03 2023 -0700
Fix deleting the Path node in "Just a Potted Cactus" demo art (#1413)						0HyperCube				Sat Sep 2 22:54:23 2023 +0100
Fix deleting the shape node (#1411)															0HyperCube				Sat Sep 2 12:16:38 2023 +0100
New node: Color Overlay (#1391)																Dhruv					Sat Sep 2 14:52:43 2023 +0530
Reconnect links when deleting a node (#1405)												Dhruv					Sat Sep 2 14:02:37 2023 +0530
Fix regression blocking inputs in the graph													Keavon Chambers			Sat Sep 2 00:19:03 2023 -0700
Add support for handling MMB/RMB double click inputs (#1407)								Keavon Chambers			Fri Sep 1 14:57:03 2023 -0700
-->

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/30?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-09-01&until=2023-10-31)

### November 2023

<!--
Fix missing menu bar from prev commit; fix menu bar items not graying out after closing all docs	Keavon Chambers			Nov 28 04:50
Add option to toggle ruler visibility (#1479)														Bijay Shrestha			Nov 28 17:19
Clean up comments left over from the port to Svelte													Keavon Chambers			Nov 27 04:50
Redesign the ColorButton widget style																Keavon Chambers			Nov 27 04:48
Fix transforms, Brush tool, and G/R/S (#1473)														0HyperCube				Nov 27 04:54
Fix hiding and collapsing layers (#1481)															0HyperCube				Nov 27 02:27
Fix 404 in node-graph README. (#1480)																Iago-lito				Nov 26 19:01
Fix doctest trying to compile text diagram															0hypercube				Nov 26 17:46
Auto-create `frontend/dist` on first build. (#1478)													Iago-lito				Nov 26 16:51
Fix viewport navigation performance by caching graph compilations (#1477)							Dennis Kobert			Nov 26 15:21
Add !build-profiling command in PRs to request builds in profiling mode								Keavon Chambers			Nov 26 05:39
Avoid unnecessary graph sends to the frontend (#1476)												0HyperCube				Nov 25 23:45
Speed up graph view frontend by removing a querySelectorAll hot path (#1475)						0HyperCube				Nov 25 23:25
Add math expression evaluation to NumberInput boxes (#1472)											Keavon Chambers			Nov 25 14:37
Consolidate MenuListButton into TextButton (#1470)													Keavon Chambers			Nov 25 01:56
Improve NumberInput with dragging to change value and escape/right-click to abort (#1469)			Keavon Chambers			Nov 21 17:26
Fix spline tool deleting layer																		0hypercube				Nov 19 13:41
Migrate demo artwork and fix all failing CI tests (#1459)											0HyperCube				Nov 19 01:06
Rename and reorganize several widgets (#1462)														Keavon Chambers			Nov 18 04:34
Redesign ColorInput widget and rename it to ColorButton												Keavon Chambers			Nov 16 18:38
Fix crash on deleting all subpaths (#1460)															0HyperCube				Nov 16 23:42
Fix Bezier-rs interactive demo page build failure													Keavon Chambers			Nov 16 15:21
Restore ESLint and Prettier auto-formatting and CI linting (#1457)									Keavon Chambers			Nov 16 13:12
Fix test compilation and allow the ci to run (#1456)												Dennis Kobert			Nov 14 23:01
Store Input for Monitor Nodes (#1454)																Dennis Kobert			Nov 14 21:17
Allow groups to work with the node graph (#1452)													0HyperCube				Nov 14 17:17
Replace license generator web infra to use Vite														Keavon Chambers			Nov 9 04:41
Fix frontend webmanifest so installing a PWA isn't broken (#1450)									Thomas Steiner			Nov 6 10:35
Fix Rust code lints (#1448)																			Keavon Chambers			Nov 5 13:52
Switch build system to Vite (#1444)																	Matthew Donoughe		Nov 5 06:12
Rename DocumentNodeType to DocumentNodeBlueprint for clarity										Keavon Chambers			Nov 5 01:24
Update funding info on website																		Keavon Chambers			Nov 4 05:52
Remove the Frame tool																				Keavon Chambers			Nov 4 03:01
Improve previewing node data (#1446)																0HyperCube				Nov 4 09:52
-->

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/31?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-11-01&until=2023-11-30)

### December 2023

<!--
Add tutorial 1 to the user manual																Keavon Chambers			Dec 31 05:23
Update the UI screenshots on the website home page												Keavon Chambers			Dec 30 19:04
Redesign the pivot overlay to a yellow crosshair												Keavon Chambers			Dec 30 14:51
Change default Text tool font to Cabin															Keavon Chambers			Dec 30 14:51
Hide the Cull node by integrating it into all generator nodes (#1538)							Keavon Chambers			Dec 30 12:28
Make CopyToPoints node resolution aware (#1536)													Dennis Kobert			Dec 30 12:56
Add graph type error diagnostics to the UI (#1535)												0HyperCube				Dec 29 08:38
Many subtle improvements to the UI design system (#1537)										Keavon Chambers			Dec 28 04:35
Convert u64 IDs to newtypes (#1532)																Keavon Chambers			Dec 22 03:24
Retire layer paths used throughout the code (#1531)												Keavon Chambers			Dec 21 19:32
Additional clean up and bug fixes after migrating document-legacy								Keavon Chambers			Dec 20 18:21
Fix serialising document (#1526)																0HyperCube				Dec 20 22:58
Remove the whole document-legacy crate (#1524)													Keavon Chambers			Dec 20 05:45
Remove all references to legacy layers (#1523)													Keavon Chambers			Dec 19 20:50
Remove most of document-legacy (#1519)															Keavon Chambers			Dec 19 04:36
New overlay system (#1516)																		0HyperCube				Dec 18 11:17
New node: Copy to Points (#1513)																Keavon Chambers			Dec 17 04:17
New node: Noise Pattern (#1518)																	Keavon Chambers			Dec 17 02:06
Add the user manual to the website (#1390)														Keavon Chambers			Dec 14 05:29
Update website for the unified document graph release											Keavon Chambers			Dec 13 19:22
Clean up some document-legacy code																Keavon Chambers			Dec 12 22:37
Fix the Imaginate node from crashing (#1512)													Keavon Chambers			Dec 12 22:39
Bump the document version																		Keavon Chambers			Dec 12 01:29
Fix the blend mode and opacity widgets of the Layers panel (#1506)								0HyperCube				Dec 12 09:27
Fix document becoming unsaved (*) when clicking with Select tool (#1509)						0HyperCube				Dec 11 22:14
Fix deleting all artboards without crashing (#1502)												0HyperCube				Dec 11 21:46
Fix nudge resizing to also work in document space (#1504)										0HyperCube				Dec 11 21:04
Fix nudging and nudge resizing (#1501)															0HyperCube				Dec 11 09:25
Serve the demo artwork in each build (#1494)													0HyperCube				Dec 11 09:06
Fixes for removing artboards; white infinite canvas background (#1497)							0HyperCube				Dec 10 00:17
Support for previewing the layer node (#1496)													0HyperCube				Dec 9 23:54
Add the Image Color Palette node (#1311)														Henry Barreto			Dec 9 20:21
Exporting (#1495)																				0HyperCube				Dec 9 17:11
Fix layers insert mark disappearing (#1493)														0HyperCube				Dec 9 13:34
Temporarily disable the Imaginate tool															Keavon Chambers			Dec 9 05:09
Fix crash when reordering layers (#1492)														0HyperCube				Dec 9 13:08
Fix Properties panel to show selected layers/nodes												Keavon Chambers			Dec 9 03:10
Wrap opacity/blend_mode in alpha_blending struct for graphic elements							Keavon Chambers			Dec 8 20:15
Eliminate GraphicElementData wrapper around GraphicElement										Keavon Chambers			Dec 8 17:18
Rename Raster to Bitmap																			Keavon Chambers			Dec 8 16:18
Rename several node graph structs/fields														Keavon Chambers			Dec 8 15:27
Improve naming of several proto nodes															Keavon Chambers			Dec 8 14:50
Improve auto-linking of layers dragged onto links												Keavon Chambers			Dec 7 16:17
Add viewing/editing layer names, add Blend Mode node, and clean up Layer node (#1489)			Keavon Chambers			Dec 7 15:10
Fix broken CSS due to bug in Cloudflare minifier												Keavon Chambers			Dec 7 14:34
Stop Ctrl+A from selecting artboards															Keavon Chambers			Dec 6 14:39
Make the tool shelf adapt to multiple columns and improve panel scrollbars						Keavon Chambers			Dec 6 01:47
Redesign the Layers panel																		Keavon Chambers			Dec 6 01:32
Clean up .graphite file serde encoding															Keavon Chambers			Dec 6 01:39
Improve navigation footer bar hints																Keavon Chambers			Dec 6 01:44
Improve text transforms (#1487)																	0HyperCube				Dec 4 22:57
Refactor Graphite dependency management (#1455)													Dennis Kobert			Dec 4 12:39
Fix how transforms work with footprints and remove a redundant transforms field (#1484)			Dennis Kobert			Dec 3 23:17
Fix graph UI links getting tangled when switching document tabs (#1483)							0HyperCube				Dec 3 10:19
-->

- [List of closed issues this month](https://github.com/GraphiteEditor/Graphite/milestone/32?closed=1)

- [List of code commits completed this month](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2023-12-01&until=2023-12-31)

## 2024 (addendum)

Full-year summary blog post: [**Year in review: 2024 highlights and a peek at 2025**](../year-in-review-2024-highlights-and-a-peek-at-2025)

For completeness, the following is compiled as an addendum to the 2021-2023 development summaries presented above. For the year 2024, we explored breaking down the progress updates into detailed quarterly reports filled with visual feature demonstrations, each published as individual blog posts. They are linked below for further reading.

### Q1 (January - March 2024)

![](https://static.graphite.rs/content/blog/2024-05-09-graphite-progress-report-q1-2024__2.avif)

Blog post: [**Graphite progress report (Q1 2024)**](../graphite-progress-report-q1-2024/)

Further details by month:

- January: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/33?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-01-01&until=2024-01-31) • [update presentation](https://youtu.be/lYHWuORYUOU?t=4179)

- February: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/34?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-02-01&until=2024-02-29) • [update presentation](https://youtu.be/4RiTqzgRSFE?t=2410)

- March: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/35?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-03-01&until=2024-03-31)

### Q2 (April - June 2024)

![](https://static.graphite.rs/content/blog/2024-07-31-graphite-progress-report-q2-2024.avif)

Blog post: [**Graphite progress report (Q2 2024)**](../graphite-progress-report-q2-2024/)

Further details by month:

- April: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/36?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-04-01&until=2024-04-30)

- May: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/37?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-05-01&until=2024-05-31)

- June: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/38?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-06-01&until=2024-06-30)

### Q3 (July - September 2024)

![](https://static.graphite.rs/content/blog/2024-10-15-graphite-progress-report-q3-2024.avif)

Blog post: [**Graphite progress report (Q3 2024)**](../graphite-progress-report-q3-2024/)

Further details by month:

- July: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/39?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-07-01&until=2024-07-31)

- August: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/40?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-08-01&until=2024-08-31)

- September: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/41?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-09-01&until=2024-09-30)

### Q4 (October - December 2024)

![](https://static.graphite.rs/content/blog/2025-03-31-graphite-progress-report-q4-2024.avif)

Blog post: [**Graphite progress report (Q4 2024)**](../graphite-progress-report-q4-2024/)

Further details by month:

- October & November: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/42?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-10-01&until=2024-11-30)

- December: [closed issues](https://github.com/GraphiteEditor/Graphite/milestone/43?closed=1) • [code commits](https://github.com/GraphiteEditor/Graphite/commits/master/?since=2024-12-01&until=2024-12-31)
