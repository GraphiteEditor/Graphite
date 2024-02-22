+++
title = "Student projects"

[extra]
order = 1 # Page number after chapter intro
+++

Graphite offers a number of opportunities for students to contribute by building a self-contained project as part of a structured format. These projects are designed to be completed over several months and are ideal for Google Summer of Code or similar internship programs, solo or group university capstone projects, and other arrangements. Each project has a distinct focus and is a great way to make a meaningful contribution to open source over the length of the program while receiving mentorship and guidance from the Graphite team.

Student projects require adherence to a set schedule with regular check-ins, milestones, and evaluations. The structured setting is designed to provide a supportive environment for students to learn and grow as developers while gaining real-world industry experience from collaborating on a sizable software product and remaining accountable to stakeholders. It's our goal to make sure you succeed!

To date, three student project experiences have been completed successfully. [See below](#successful-past-projects) for summaries to get a feel for what has worked before.

Use this [contributor guide](../..) to start out with the code. Then when you're ready, reach out through [Discord](https://discord.graphite.rs) and use the `#🎓student-projects` channel to discuss and work towards proposing a project with the Graphite core team.

## Google Summer of Code

GSoC is a program offering students a [stipend](https://developers.google.com/open-source/gsoc/help/student-stipends) for successful completion of an internship-style experience with an open source organization. Read about [how it works](https://summerofcode.withgoogle.com/how-it-works/).

Graphite is [participating](https://summerofcode.withgoogle.com/programs/2024/organizations/graphite) in GSoC 2024 this summer and the proposal formulation period is open now until the April 2 deadline (see the full [timeline](https://developers.google.com/open-source/gsoc/timeline)).

### GSoC Proposals

Writing a good proposal is an important first step that demonstrates your understanding of the project and your ability to plan and execute it. A well-defined proposal will set you up for success throughout the rest of the program.

You are encouraged to reference the project idea list below to find several potential projects suited to your experience, interest, and choice of scope. Then, you must reach out to a [core team member](/about#core-team) through Discord to discuss your plan in detail before writing a proposal. This will help you understand the project's scope and requirements. It will also help us understand your background and capabilities to offer you feedback and suggestions for the best outcome in the competitive applicant selection process.

When it comes to writing the proposal, which you will submit to the GSoC application website, we offer some guidelines below:

- **Proposal structure:** Please consult the [Blender GSoC application template](https://developer.blender.org/docs/programs/gsoc/application_template/) as reference for our desired format.
- **Your background:** We're especially interested in your background and experience, so attaching a résumé or CV is optional but highly recommended and will help us understand your capabilities. If able, please also include links to evidence of past open source contributions or personal projects in the bio section of your proposal. Our goal is to help you learn and grow as a productive open source software engineer, not to help you learn to program from scratch, so any such evidence will help us understand your potential as a self-motivated contributor to the open source community.
- **Prior PRs:** If you have made any contributions to Graphite and/or similar open source projects, please include links to your pull requests in your proposal. We put significant extra weight towards applicants who have already made successful contributions to Graphite because this shows your ability to work in a professional capacity with our team and demonstrates your interest in Graphite in particular (as opposed to a shotgun approach to GSoC applications). You can also state that you'll submit your first PRs (we encourage at least two) after the application deadline on [April 2](https://developers.google.com/open-source/gsoc/timeline#april_2_-_1800_utc), but before we make our final selections a couple days prior to [April 24](https://developers.google.com/open-source/gsoc/timeline#april_24_-_1800_utc). We are very likely to reject applicants who have not made meaningful contributions to Graphite by that time.

<!-- TODO: Explain how we want a proposal structured -->

## Project idea list

### Port LibRaw to Rust

*For Graphite to support editing photos from professional digital cameras, it needs a raw decoding/processing library.*

- **Needed Skills:** Rust, C++, binary format parsing
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Possible Mentors:** [Keavon](/about#keavon), [Dennis](/about#dennis), [Hypercube](/about#hypercube)
- **Expected Outcomes:** Develop a Rust library that ports LibRaw functionality to native Rust. A clean, well-structured code base and API. At a minimum, demonstrate the successful end-to-end decoding, debayering, and color space handling of Sony ARW format photos in Graphite. Publish the library to crates.io.

For Graphite to work as a photo editing app, it needs to import raw photos. These contain compressed sensor imagery and metadata in a variety of formats. Sony ARW is the first target, and additional camera brands are stretch goals, while porting all useful functionality would be the best outcome.

Graphite needs a library written in pure Rust with a suitable (non-GPL) license, which does not currently exist in the ecosystem, so we need to create one ourselves. LibRaw is a common C++ library that supports most camera brands and formats and has a compatible license. Since raw formats are rarely documented by camera manufacturers, porting LibRaw to Rust is the most practical approach.

This project involves diving into the LibRaw source code (~44,000 lines), understanding its architecture, and translating its pieces to idiomatic Rust. Because of Rust's differences from C++, this is not likely to be as simple as a 1:1 source code translation. The student should have strong familiarity with the two languages, experience exploring moderately large code bases, working with binary format parsing, and ideally some knowledge of color science.

### Node graph auto layout

*Graphite's graph UI needs a system to automatically arrange layers and nodes given incremental changes to the graph contents.*

- **Needed Skills:** Rust, algorithm design ([constraint solving](https://en.wikipedia.org/wiki/Constraint_satisfaction_problem), [packing](https://en.wikipedia.org/wiki/Packing_problems), [graph drawing](https://en.wikipedia.org/wiki/Graph_drawing))
- **Project Size:** Medium *(GSoC: 175 hours)* or Large *(GSoC: 350 hours)*
- **Difficulty:** Medium
- **Possible Mentors:** [Keavon](/about#keavon)
- **Expected Outcomes:** A system that manages the placement of nodes based on a set of layout constraint rules and incremental updates to the graph topology. It should run efficiently, even with large graphs. It should be robust enough to handle a variety of graph topologies and user interactions, producing organized, useful, and stable layouts.

The Graphite concept is built around a node graph representation of layer stacks, while tools automatically generate and manipulate nodes. When a layer or node is inserted, deleted, moved, or referenced, the graph needs to be reorganized to maintain a clear and useful layout. Users can also interactively expand and collapse groups of nodes which occupies or frees up graph real estate.

Unlike other node editors that are centered around manual graph editing, where users are fully in charge of node placements within one large node network, Graphite's node UI is more oriented towards automatic layout management and viewing just parts of the graph at one time. This means the shown graph toplogy is constantly changing and the layout system needs to cooperatively organize the graph in concert with user actions.

While general graph layout algorithms are complex and struggle to produce good results in other node editors, Graphite's graph topology is more constrained and predictable, which makes it possible to design a layout system that can produce good results. Nodes tend to be organized into rows, and layers into columns (see the image in the project below). This turns the problem into more of a constraint-based, axis-aligned packing problem.

### Shader-driven graph UI rewrite

*Graphite's node graph UI needs to be rewritten using a shader-based rendering system.*

- **Needed Skills:** Rust, WGPU, computer graphics, shader programming
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Possible Mentors:** [Keavon](/about#keavon), [Dennis](/about#dennis)
- **Expected Outcomes:** A reimplemented graph UI that draws nodes, layers, connections, thumbnails, text, etc. with a custom shader UI rendering system written with WGPU.

The current graph UI is implemented using HTML/CSS and SVG, which is too slow for large graphs and lacks the flexibility to create the desired visual effects like frosted glass. The new system should be able to handle thousands of nodes with ease, and it should be able to render all the visual effects that are envisioned in the design mockup:

<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__5.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Node graph UI mockup" data-carousel-image />

The proposed system is a custom immediate mode renderer built with shader programming. At a minimum, it should replicate present graph interaction functionality, but stretch goals would go beyond that to include various graph quality-of-life features.

### Marquee selection masking

*Graphite's raster editing features requires the implementation of Select mode, where users can draw a mask which becomes a marquee (marching ants) selection.*

- **Needed Skills:** Rust, computer graphics
- **Project Size:** Medium *(GSoC: 175 hours)*
- **Difficulty:** Medium
- **Possible Mentors:** [Keavon](/about#keavon), [Hypercube](/about#hypercube)
- **Expected Outcomes:** Complete implementation of Mask mode and its marquee selection. Marching ants visualization shader effect. Integration of selection mask with the node graph and raster editing tools. Useful raster editing workflow.

A central part of the workflow in raster image editors is the selection of portions of the image to constrain manipulations just to the masked areas. Tools such as the circular and rectangular marquee, lasso, and magic wand are used to create masks. Instead of using dedicated tools, Graphite's design reuses the existing vector and raster drawing tools (like Rectangle, Ellipse, Pen, and Fill) to create masks in a dedicated Mask mode. Returning from Mask mode reveals the marching ants selection that constrains further editing operations.

This is a key feature in Graphite's evolution to a fully-featured raster editor.

### Machine learning architecture

*Generative AI and vision ML models will need to run in Graphite's node graph with a Rust-centric, modular, portable, deployable, scalable environment.*

- **Needed Skills:** Machine learning (and potentially: Rust, Python, ONNX, Candle, Burn)
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Possible Mentors:** [Keavon](/about#keavon), [Oliver](https://github.com/otdavies)
- **Expected Outcomes:** Specifics will vary by proposal. In general, a useful end-to-end integration of at least one GenAI or vision model into Graphite's node graph which can run locally and deploy to a server.

AI is filling a rapidly growing role as a tool in the creative process. Graphite's procedural node-based workflow is uniquely suited to leveraging the power and flexibility of machine learning nodes.

[Segment Anything](https://segment-anything.com/) (object segmentation), [Depth Anything](https://depth-anything.github.io/) (depth estimation), and [Stable Diffusion](https://github.com/CompVis/stable-diffusion) (image generation, generative fill, style transfer, etc.) are currently the three models we are most interested in integrating. The challenge is settling on an architecture and tech stack which is well suited for Graphite's requirements.

The approach should be extensible to future models. It needs to run fast and natively on the assorted hardware of local user machines with hardware acceleration. It should be a one-click installation process for users to download and run models without requiring dependencies or environment setup. Ideally, it should allow the more lightweight models to run locally in browsers with WebGPU. It needs to also be deployable to servers in a scalable, cost-viable manner that reuses most of the same code that runs locally. Runtime overhead, cold start times, and memory usage should be minimized for quick, frequent switching between models in a node graph pipeline. The tech stack also needs to be permissively licensed and, as much as possible, Rust-centric so it doesn't add complexity to our Wasm and Tauri build processes. For Stable Diffusion, we need the flexability to track the latest research and extensions to the ecosystem like new base models, checkpoint training, DreamBooth, LoRA, ControlNet, IP-Adapter, etc. and expose these functionalities through modular nodes.

To meet most of these criteria, our current thinking is to distribute and run our models using the [ONNX](https://onnx.ai/) format. This would integrate ONNX runtimes for WebGPU, native (DirectML, CUDA), and GPU cloud providers. The issue with this approach is that these models (particularly Stable Diffusion) aren't available in an ONNX format.

A potential direction for students proposing this project is to reimplement parts of these ML models for ONNX (potentially in [Candle](https://github.com/huggingface/candle) or [Burn](https://burn.dev/) to leverage the Rust ecosystem, if possible).

Another potential direction is to find a portable, modular, lightweight approach for bundling existing Python-based models. It would need to work across simple and complex models with different architectures. License compliance, if GPL code is involved, would be a consideration.

Based on the experience and insight brought to the table by the student, the nature of the project should be defined through preliminary discussions with the mentors and codified in the proposal. Machine learning and MLOps are fields that Graphite's team lack deep expertise in, so we are looking for a knowledgable student who can bring forth a well-researched and well-architected proposal and then execute on it.

### Complex widget layout system

*Graphite's UI needs an upgraded layout system to support more complex and dynamic widget arrangements defined from the backend.*

- **Needed Skills:** Rust, web (Svelte, CSS, TypeScript)
- **Project Size:** Small *(GSoC: 90 hours)* or Medium *(GSoC: 175 hours)*
- **Difficulty:** Medium
- **Possible Mentors:** [Keavon](/about#keavon)
- **Expected Outcomes:** An improved system for defining widget layouts with better control and flexibility over arrangement and dynamic data binding. Reduction in boilerplate and plumbing required to define each new layout. Better control of styling between rows.

The current system for defining the arrangement of widget layouts from the backend, created during a [previous student project](#2022-backend-layout-system), has served us well thus far but has limitations. This project aims to extend the system to better model our evolved requirements.

The present system is very row-centric, which makes it challenging to create multi-row layouts that distribute their widgets across the space in concert with other rows. It also requires manual updates to the backend data model for each widget, which makes dynamic layouts require extra plumbing and room for mistakes. Defining popover and dialog menus is also cumbersome because each requires several new files in the backend architecture.

Students should have a good level of familiarity with Rust design patterns to envision, prototype, propose, and robustly implement a new system that can handle the complexity of Graphite's use cases. The size of this project can vary depending on the proposal's scope and extent of refactoring to these and adjacent systems.

<!-- ### Node data table editor

*The node graph data model for procedural content generation can be thought of as a spreadsheet, which needs a dedicated viewer/editor panel.*

- **Needed Skills:** Rust, web (Svelte, TypeScript)
- **Project Size:** Small *(GSoC: 90 hours)*
- **Difficulty:** Easy-to-medium
- **Possible Mentors:** [Keavon](/about#keavon)
- **Expected Outcomes:** A functional panel in the editor that displays the selected node output data as a spreadsheet across multiple domains. Connection to the graph engine to read and edit the data. Virtual scrolling and efficient transfer of data to the frontend.

The node graph is a powerful tool for procedural content generation, but it can be difficult to understand the data that flows through it. Node data can be represented as a spreadsheet, where each row presents a domain-specific instance (e.g., a point, segment, or face) and each column displays an attribute (like position, color, or radius).

This project involves implementing the frontend as a cleanly-written Svelte component that can display the data in a tabular format, where virtual scrolling lets it efficiently process only the visible portion of the full data table. Help will be provided in building the frontend component and especially its CSS styling, but the student should be familiar with efficient TypeScript and Rust programming to handle both frontend and backend challenges while maintaining a focus on performance. The backend portion will need to integrate with the node engine and surrounding tooling to query the data coming from the selected node.

A larger-scoped version of the project can expand this to focus also on displaying thumbnail previews of data coming from each node's output. -->

### Animation system

*Adding a timeline-based animation system to Graphite would begin realizing the vision as a versatile content creation suite supporting motion graphics.*

- **Needed Skills:** Rust, web (Svelte, CSS, SVG, TypeScript)
- **Project Size:** Medium *(GSoC: 175 hours)* or Large *(GSoC: 350 hours)*
- **Difficulty:** Easy-to-medium
- **Possible Mentors:** [Keavon](/about#keavon)
- **Expected Outcomes:** A timeline panel in the editor that can create and edit keyframes and timing curves for animating data channels used by nodes. Ergonomic experience for keyframing properties. Efficient curve interpolation. Rendering optimizations for relatively smooth animation playback.

A powerful outcome of Graphite's node-driven architecture is the relatively simple ability to vary data parameters over time.

In the frontend, the student will be responsible for hooking up useful animation features into the existing editor UI (like exposing node parameters to the timeline) and building the new timeline panel with both simplified keyframe indicators and editable curves. A scrubbable playhead, playback controls, and other common timeline features should be included.

In the backend, the animation curves need to be evaluated at the playhead position for each animation channel, then fed into the compiled graph. The node graph system (Graphene) needs to be updated so it can accept these time-varying parameters and route them to the appropriate nodes without requiring a full recompilation of the graph each frame. Any other rendering bottlenecks should also be found and optimized, where feasible and reasonably within scope.

### Testing and performance instrumentation infrastructure

*Graphite has many areas that could benefit from better automated testing for bugs and performance regressions.*

- **Needed Skills:** Rust, unit testing
- **Project Size:** Small *(GSoC: 90 hours)* or larger if proposed
- **Difficulty:** Easy
- **Possible Mentors:** [Dennis](/about#dennis), [Hypercube](/about#hypercube)
- **Expected Outcomes:** Specific focus and scope may vary by the student's interests and proposal. In general, a significant increase in the coverage of tests in useful code areas (such as document loading, manipulation, and rendering) and attention towards systems which measure performance metrics and identify bottlenecks and regressions.

Graphite could benefit from better testing coverage in a number of areas, especially end-to-end testing in the tool, document, and node graph systems. This project is about identifying and addressing areas that are lacking and most vulnerable to suffering from regressions. The student will be responsible for identifying areas that could benefit from better testing.

### Architecture visualization

*Infrastructure to generate visualizations of Graphite's system architecture would be a valuable addition to the project's documentation and debugging tools.*

- **Needed Skills:** Rust (especially proc macros)
- **Project Size:** Medium *(GSoC: 175 hours)* or Large *(GSoC: 350 hours)*
- **Difficulty:** Medium
- **Possible Mentors:** [Keavon](/about#keavon), [Dennis](/about#dennis)
- **Expected Outcomes:** A system built from proc macros which can generate useful visualizations of Graphite's system architecture. Depending on proposal scope, this can include static visualizations added to the documentation, dynamic message flow visualizations for debugging, and tools to help identify redundant message traffic.

Graphite's editor architecture, based around a message-passing processing queue, is structured as a hierarchical system of message handlers. Each handler stores its own state, and references to the state data may be passed along to its child handlers they need it.

It is challenging to document the hierarchy of this system as a tree in the documentation because the code is often changing. Generating a visualization would ensure it remains up to date. Additional visualizations could also be generated with greater detail, such as message flow diagrams for each message.

If proposed as part of the project's scope, a runtime component could be added as an extension of the aforementioned documentation visualizations. These would help developers understand and trace the flow of message traffic, essentially becoming a visual debugger for the message system. Instrumentation included with this could help identify message traffic that causes particularly high load, or locate redundant message traffic, to keep Graphite's performance under control. Timing could also be measured for each message and visualized in a custom flame graph. Current debugger tools can't provide this information because the message-passing approach "flattens out" the traditional function call stack.

### Your own idea

*If you have an idea for a project that you think would be a good fit, we'd love to hear it!*

- **Needed Skills:** Varies
- **Project Size:** Varies
- **Difficulty:** Varies
- **Possible Mentors:** Varies
- **Expected Outcomes:** Stated in your proposal.

If none of the projects above suit your interests or experience, we are very open to discussing your own project ideas that could benefit Graphite. You may consult our [task board](https://github.com/orgs/GraphiteEditor/projects/1/views/1) and [roadmap](/features#roadmap) to get a feel for what our current priorities are.

As is the case with all projects, please discuss this with us on Discord to flesh out your idea. Unsolicited proposals that have not been discussed with us will almost certainly be rejected.

<!-- ### PDF import/export -->
<!-- ### ONNX model embedding to run in WebGPU and native (Tauri) and server -->

## Successful past projects

### 2023: Bezier-rs library

Affiliation: University of Waterloo, Ontario, Canada  
Duration: 9 months  
Students: Hannah Li, Rob Nadal, Thomas Cheng, Linda Zheng, Jackie Chen

The student group designed an API for representing and manipulating Bezier curves and paths as a standalone Rust library which was [published to crates.io](https://crates.io/crates/bezier-rs). It now serves as the underlying vector data format used in Graphite, and acts as a testbed for new computational geometry algorithms. The team also built an [interactive web demo catalog](/libraries/bezier-rs/) to showcase many of the algorithms, which are also handily embedded in the library's [documentation](https://docs.rs/bezier-rs/latest/bezier_rs/).

### 2022: Backend layout system

Affiliation: California Polytechnic State University, San Luis Obispo, USA  
Duration: 3 months  
Student: Max Fisher

The student designed and implemented a new system across the editor's frontend and backend which made it possible to define and manage layouts for widgets from the backend and receive input data from those widgets. Previously, all layouts were statically defined in the frontend and extensive plumbing was required to pass data back and forth.

### 2022: Path boolean operations

Affiliation: California Polytechnic State University, San Luis Obispo, USA  
Duration: 3 months  
Student: Caleb Dennis

The student devised and prototyped algorithms for performing boolean operations on paths, such as union, intersection, and difference. These were used as a stopgap during 2022 and 2023 to provide users with a rudimentary boolean operation feature set.
