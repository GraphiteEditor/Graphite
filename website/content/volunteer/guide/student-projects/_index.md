+++
title = "Student projects"
template = "book.html"
page_template = "book.html"

[extra]
order = 4 # Chapter number
+++

Graphite offers a number of opportunities for students to contribute by building a self-contained project as part of a structured format. These projects are designed to be completed over several months and are ideal for Google Summer of Code or similar internship programs, solo or group university capstone projects, and other arrangements. Each project has a distinct focus and is a great way to make a meaningful contribution to open source over the length of the program while receiving mentorship and guidance from the Graphite team.

Student projects require adherence to a set schedule with regular check-ins, milestones, and evaluations. The structured setting is designed to provide a supportive environment for students to learn and grow as developers while gaining real-world industry experience from collaborating on a sizable software product and remaining accountable to stakeholders. It's our goal to make sure you succeed!

Use this [contributor guide](..) to start out with the code. Then when you're ready, reach out through [Discord](https://discord.graphite.rs) and use the `#🎓student-projects` channel to discuss and work towards proposing a project with the Graphite core team.

## Google Summer of Code

GSoC is a program offering students a [stipend](https://developers.google.com/open-source/gsoc/help/student-stipends) for successful completion of an internship-style experience with an open source organization. Read about [how it works](https://summerofcode.withgoogle.com/how-it-works/).

<!-- Graphite [participated in GSoC 2024](https://summerofcode.withgoogle.com/programs/2024/organizations/graphite) and we anticipate doing so again in 2025 if our organization's application is accepted. Getting involved early is a great way to have a head start and stand out in your application. -->
Graphite is [participating again in GSoC 2025](https://summerofcode.withgoogle.com/programs/2025/organizations/graphite). Applications closed [April 8](https://developers.google.com/open-source/gsoc/timeline). We accept year-round contributions; getting involved early is a great way to have a head start and stand out in your application in next year's program.

### Writing a proposal

Writing a good proposal is an important step that demonstrates your understanding of the project and your ability to think ahead and execute it. A well-defined plan will set you up for success throughout the rest of the program.

<details>
<summary>For proposal writing guidelines and requirements: click here</summary>

You are encouraged to reference the project idea list below to find several potential projects suited to your experience, interest, and choice of scope. Then, you must reach out to a [core team member](/about#core-team) through Discord to discuss your plan in detail before writing a proposal. This will help you understand the project's scope and requirements and develop a detailed timeline for your expected summer-long work schedule. Importantly, it will also help us understand your background and capabilities to offer you feedback and suggestions for the best outcome in the competitive applicant selection process.

When it comes to writing the proposal, which you will submit to the GSoC application website, we offer some guidelines below:

- **Proposal structure:** Please consult the [Blender GSoC application template](https://developer.blender.org/docs/programs/gsoc/application_template/) as reference for our desired format. For project ideas already listed below, omit the "Benefits" section. Remember: don't waste your—and our—time restating information that we already know, like background info about Graphite or our tech stack; we just want to hear your thoughts and plans about what you uniquely bring to the table and how you'll execute the project. Proposals should be utilitarian, not formal, while also demonstrating your professional communication skills. Using an LLM to write your proposal won't be to your advantage.
- **Experience:** We're especially interested in your background and work experience, so attaching a résumé or CV is an optional but recommended way to help us understand your capabilities. If able, please also include links to past open source contributions or personal projects in the bio section. Our goal is to provide an environment for you to learn and grow as a productive software engineer and team collaborator, not to help you learn the basics of coding, so any included work examples will help us understand your potential as a self-motivated contributor to the open source community.
- **Work timeline:** Your goal is to write a proposal that inspires confidence in your ability to successfully complete the project, which means understanding in detail what's involved at a technical level and how you plan to tackle it. A detailed work timeline is the most important written part of your proposal. It should be broken into weekly or bi-weekly milestones with a couple sentences of technical detail. The summary in the project idea list below doesn't give enough information to develop a timeline, so you'll need to discuss this with the core team on Discord.
- **Prior PRs:** The largest factor in our selection decision will be the quality and extent of your prior contributions to Graphite made during the proposal formulation period (or before, if applicable). Include a link to `https://github.com/GraphiteEditor/Graphite/commits?author=YOUR_GITHUB_USERNAME` in your proposal and feel free to write up a summary of what you've contributed and learned from the process. You may also keep contributing during the month after applications close, before we've finalized our selections, for those additional PRs to be considered.

</details>

## Project idea list

### Compilers, graphics, and theory

These projects are more advanced but are highest priority for Graphite's development. We will be aiming to find standout candidates for at least one of these projects this year. If your background suits the projects listed in this section, you are likely to have better odds applying to these compared to the more general projects further below.

#### Graphene language/compiler development

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/2350)
- Best for someone with an aptitude or focus on programming languages, compilers, and type system theory.

#### GPU-accelerated rendering pipeline within the compiler/runtime/engine

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Build out infrastructure in the language/compiler/runtime/engine using [rust-gpu](https://github.com/Rust-GPU/rust-gpu) and/or [CubeCL](https://github.com/tracel-ai/cubecl).
- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/2168)
- Best for someone with both an aptitude for low-level graphics programming (experience in one of WGPU, Vulkan, OpenGL, etc.) and an interest in compilers and programming languages.

#### Node equivalence rewriting

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/2021)
- Best for someone with an interest towards graph theory and compiler optimization topics like [E-graphs](https://en.wikipedia.org/wiki/E-graph).

#### Machine learning architecture

*Generative AI and vision ML models will need to run in Graphite's node graph with a Rust-centric, modular, portable, deployable, scalable environment.*

- **Possible Mentors:** [Oliver](https://github.com/otdavies)
- **Needed Skills:** Machine learning (and potentially: Rust, Python, ONNX, Candle, Burn)
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Expected Outcomes:** Specifics will vary by proposal. In general, a useful end-to-end integration of at least one GenAI or vision model into Graphite's node graph which can run both locally and deployed to a hosting provider server.

AI/ML is filling a rapidly growing role as a tool in the creative process. Graphite's procedural node-based workflow is uniquely suited to leveraging the power and flexibility of AI nodes.

[Segment Anything 2](https://ai.meta.com/sam2/) (object segmentation), [Depth Anything](https://depth-anything.github.io/) (depth estimation), and [Stable Diffusion](https://github.com/CompVis/stable-diffusion) (image generation, generative fill, style transfer, etc.) are currently the three models we are most [interested in integrating](https://github.com/GraphiteEditor/Graphite/issues/1694). The challenge is settling on an architecture and tech stack which is well suited for Graphite's requirements.

<details>
<summary>For additional technical details: click here</summary>

The approach should be extensible to future models. It needs to run fast and natively on the assorted hardware of local user machines with hardware acceleration. It should be a one-click installation process for users to download and run models without requiring dependencies or environment setup. Ideally, it should allow the more lightweight models to run locally in browsers with WebGPU. It needs to also be deployable to servers in a scalable, cost-viable manner that reuses most of the same code that runs locally. Runtime overhead, cold start times, and memory usage should be minimized for quick, frequent switching between models in a node graph pipeline. The tech stack also needs to be permissively licensed and, as much as possible, Rust-centric so it doesn't add complexity to our Wasm and desktop build processes. For Stable Diffusion, we need the flexability to track the latest research and extensions to the ecosystem like new base models, checkpoint training, DreamBooth, LoRA, ControlNet, IP-Adapter, etc. and expose these functionalities through modular nodes.

To meet most of these criteria, our current thinking is to distribute and run our models using the [ONNX](https://onnx.ai/) format. This would integrate ONNX runtimes for WebGPU, native (DirectML, CUDA), and GPU cloud providers. The issue with this approach is that these models (particularly Stable Diffusion) aren't available in an ONNX format.

A potential direction for students proposing this project is to reimplement parts of these ML models for ONNX (potentially in [Candle](https://github.com/huggingface/candle) or [Burn](https://burn.dev/) to leverage the Rust ecosystem, if possible).

Another potential direction is to find a portable, modular, lightweight approach for bundling existing Python-based models. It would need to work across simple and complex models with different architectures. License compliance, if GPL code is involved, would be a consideration.

Based on the experience and insight brought to the table by the student, the nature of the project should be defined through preliminary discussions with the mentors and codified in the proposal. Machine learning and MLOps are fields that Graphite's team lack deep expertise in, so we are looking for a knowledgable student who can bring forth a well-researched and well-architected proposal and then execute on it.

</details>

### Native development

#### Graphite desktop app engineering

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

### Rendering and graphics

Several of these require a good understanding of computer graphics rendering techniques and algorithms. Experience in game development and writing your own rendering engines is a plus.

#### Mesh vector rendering

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- See the GitHub issues: [fills](https://github.com/GraphiteEditor/Graphite/issues/2309) and [strokes](https://github.com/GraphiteEditor/Graphite/issues/2310).

#### Support paints for strokes and fills

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Refactor and upgrade our renderer to cleanly handle paints for fills and strokes.
- This includes [gradient rendering](https://github.com/GraphiteEditor/Graphite/issues/2304) polyfills
- May include other rendering features like [stroke alignment](https://github.com/GraphiteEditor/Graphite/issues/2268) polyfills

#### Advanced text layout and typography

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/1105)

#### PDF and/or DXF import/export

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Scope and viability depends on the state of available libraries.

#### Traditional brush engine

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/1297)

#### Procedural brush engine

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [Read this thesis for background](https://digitalcommons.calpoly.edu/theses/2653/), chapter 3 onwards.

#### Advanced color management

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Add support for HDR/WCG and/or CMYK and alternate color spaces/models
- Requires an experienced understanding of color science

#### Image processing algorithms for photography

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

#### New graphics nodes

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Research and implement image processing (raster) or geometry (vector) nodes that you propose or we suggest in our discussions with you.
- Example of one such node: [Text on path](https://github.com/GraphiteEditor/Graphite/issues/978).

#### SVG with raster effects

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- The SVG spec supports a number of filters and other raster effects, and we currently only implement a small subset.
- Add support for the rest of the SVG spec, including filters, masks, and other raster effects.
- Allow roundtrip import and export of SVG files with these features.
- Import, render (through SVG and Vello), and export of [filters like these](https://codepen.io/miXTim/pen/ZErggMQ).

### Editor tooling

#### Snapping system overhaul

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/2352)

#### Advanced vector editing tool modes

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Add modes for segment editing, mesh vector, and more. Discuss with us on Discord to decide on the scope of the project.

#### Tooling polishing and gizmo additions

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

#### Marquee selection masking

*Graphite's raster editing features requires the implementation of Select mode, where users can draw a mask which becomes a marquee (marching ants) selection.*

- **Possible Mentors:** [Keavon](/about#keavon)
- **Needed Skills:** Rust, computer graphics
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Medium
- **Expected Outcomes:** Complete implementation of Mask mode and its marquee selection. Marching ants visualization shader effect. Integration of selection mask with the node graph and raster editing tools. Useful raster editing workflow.

A central part of the workflow in raster image editors is the selection of portions of the image to constrain manipulations just to the masked areas. Tools such as the circular and rectangular marquee, lasso, and magic wand are used to create masks. Instead of using dedicated tools, Graphite's design reuses the existing vector and raster drawing tools (like Rectangle, Ellipse, Pen, and Fill) to create masks in a dedicated Mask mode. Returning from Mask mode reveals the marching ants selection that constrains further editing operations.

This is a key feature in Graphite's evolution to a fully-featured raster editor.

### Refactors and infrastructure

#### Complex widget layout system

*Graphite's UI needs an upgraded layout system to support more complex and dynamic widget arrangements defined from the backend.*

- **Possible Mentors:** [Keavon](/about#keavon)
- **Needed Skills:** Rust, web (Svelte, CSS, TypeScript)
- **Project Size:** Small *(GSoC: 90 hours)* or Medium *(GSoC: 175 hours)*
- **Difficulty:** Medium
- **Expected Outcomes:** An improved system for defining widget layouts with better control and flexibility over arrangement and dynamic data binding. Reduction in boilerplate and plumbing required to define each new layout. Better control of styling between rows.

The current system for defining the arrangement of widget layouts from the backend, created during a [previous student project](#2022-backend-layout-system), has served us well thus far but has limitations. This project aims to extend the system to better model our evolved requirements.

Students should have a good level of familiarity with Rust design patterns to envision, prototype, propose, and robustly implement a new system that can handle the complexity of Graphite's use cases. The size of this project can vary depending on the proposal's scope and extent of refactoring to these and adjacent systems.

<details>
<summary>For additional technical details: click here</summary>

The present system is very row-centric, which makes it challenging to create multi-row layouts that distribute their widgets across the space in concert with other rows. It also requires manual updates to the backend data model for each widget, which makes dynamic layouts require extra plumbing and room for mistakes. Defining popover and dialog menus is also cumbersome because each requires several new files in the backend architecture.

</details>

#### Testing and performance instrumentation

*Graphite has many areas that could benefit from better automated testing for bugs and performance regressions.*

- **Possible Mentors:** [Dennis](/about#dennis)
- **Needed Skills:** Rust, unit testing
- **Project Size:** Small *(GSoC: 90 hours)* or larger if proposed
- **Difficulty:** Easy
- **Expected Outcomes:** Specific focus and scope may vary by the student's interests and proposal. In general, a significant increase in the coverage of tests in useful code areas (such as document loading, tool manipulation, and rendering) and attention towards systems which measure performance metrics and identify bottlenecks and regressions.

Graphite could benefit from better testing coverage in a number of areas, especially end-to-end testing in the tool, document, and node graph systems. This project is about identifying and addressing areas that are lacking and most vulnerable to suffering from regressions. The student will be responsible for identifying areas that could benefit from better testing.

#### Architecture visualization

*Infrastructure to generate visualizations of Graphite's system architecture would be a valuable addition to the project's documentation and debugging tools.*

- **Possible Mentors:** [Keavon](/about#keavon), [Dennis](/about#dennis)
- **Needed Skills:** Rust (especially proc macros)
- **Project Size:** Small *(GSoC: 90 hours)* or larger if proposed
- **Difficulty:** Medium
- **Expected Outcomes:** A system built from proc macros which can generate useful visualizations of Graphite's system architecture. Depending on proposal scope, this can include static visualizations added to the documentation, dynamic message flow visualizations for debugging, and tools to help identify redundant message traffic.

Graphite's editor architecture, based around a message-passing processing queue, is structured as a hierarchical system of message handlers. Each handler stores its own state, and references to the state data may be passed along to its child handlers that need it.

It is challenging to document the hierarchy of this system as a tree in the documentation because the code is often changing. Generating a visualization would ensure it remains up to date. Additional visualizations could also be generated with greater detail, such as message flow diagrams for each message.

<details>
<summary>For additional technical details: click here</summary>

If proposed as part of the project's scope, a runtime component could be added as an extension of the aforementioned documentation visualizations. These would help developers understand and trace the flow of message traffic, essentially becoming a visual debugger for the message system. Instrumentation included with this could help identify message traffic that causes particularly high load, or locate redundant message traffic, to keep Graphite's performance under control. Timing could also be measured for each message and visualized in a custom flame graph. Current debugger tools can't provide this information because the message-passing approach "flattens out" the traditional function call stack.

</details>

### Other

#### Your own idea

*If you have an idea for a project that you think would be a good fit, we'd love to hear it!*

- **Possible Mentors:** Varies
- **Needed Skills:** Varies
- **Project Size:** Varies
- **Difficulty:** Varies
- **Expected Outcomes:** Stated in your proposal.

If none of the projects above suit your interests or experience, we are very open to discussing your own project ideas that could benefit Graphite. You may consult our [task board](https://github.com/orgs/GraphiteEditor/projects/1/views/1) and [roadmap](/features#roadmap) to get a feel for what our current priorities are.

As is the case with all projects, please discuss this with us on Discord to flesh out your idea. Unsolicited proposals that have not been discussed with us will almost certainly be rejected.

## Successful past projects

### 2024: Interactive node graph auto-layout

*Graphite's graph UI needs a system to automatically arrange layers and nodes given incremental changes to the graph contents.*

Affiliation: GSoC 2024  
Duration: 3 months  
Student: Adam Gerhant

- [Program project listing](https://summerofcode.withgoogle.com/programs/2024/projects/gvbBoCpT)
- [Report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1769)

**Outcomes:** A system that manages the placement of nodes based on a set of layout constraint rules and incremental updates to the graph topology. It should run efficiently, even with large graphs. It should be robust enough to handle a variety of graph topologies and user interactions, producing organized, useful, and stable layouts.

**Background:** The Graphite concept is built around a node graph representation of layer stacks, while tools automatically generate and manipulate nodes. When a layer or node is inserted, deleted, moved, or referenced, the graph needs to be reorganized to maintain a clear and useful layout. Users can also interactively expand and collapse groups of nodes which occupies or frees up graph real estate.

Unlike other node editors that are centered around manual graph editing, where users are fully in charge of node placements within one large node network, Graphite's node UI is more oriented towards automatic layout management and viewing just parts of the graph at one time. This means the shown graph topology is constantly changing and the layout system needs to cooperatively organize the graph in concert with user actions.

While general graph layout algorithms are complex and struggle to produce good results in other node editors, Graphite's graph topology is more constrained and predictable, which makes it possible to design a layout system that can produce good results. Nodes tend to be organized into rows, and layers into columns. This turns the problem into more of a constraint-based, axis-aligned packing problem.

### 2024: Rendering performance infrastructure improvements

*Graphite performance is bottlenecked by limitations in the new node graph rendering architecture that needs improvements.*

Affiliation: GSoC 2024  
Duration: 4 months  
Student: Dennis Kobert

- [Program project listing](https://summerofcode.withgoogle.com/programs/2024/projects/v5z2Psnc)
- [Report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1773)

**Outcomes:** A holistic, metrics-driven focus on fixing the many unoptimized areas of Graphite's node graph compilation, execution, and rendering systems. Integration of Vello as an integrated rendering backend. A significant improvement in the performance of the editor, especially in the node graph, and a more stable and predictable performance profile. Benchmarking and profiling tools to measure and visualize performance improvements and regressions.

**Background:** Graphite's node graph system is the backbone of the editor, but it has many performance problems that need to be addressed because the system is relatively immature and performance-impacting shortcuts were taken during its initial development. This project is all about making the node graph system more robust and optimized, which will have a direct impact on the user experience and the editor's overall performance. By the end of the project, the editor should finally feel usable in the majority of user workflows. Vello should be enabled as an alternate render engine that will fully replace the existing SVG-based one in the future, once browser support arrives across major platforms.

### 2024: Raw photograph decoding in Rust

*For Graphite to support editing photos from professional digital cameras, it needs a raw decoding/processing library.*

Affiliation: GSoC 2024  
Duration: 5 months  
Student: Elbert Ronnie

- [Program project listing](https://summerofcode.withgoogle.com/programs/2024/projects/2uiwOfz8)
- [Report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1771)
- [Rawkit library](https://crates.io/crates/rawkit)

**Outcomes:** A Rust library that implements raw photo decoding functionality to native Rust. A clean, well-structured codebase and API. At a minimum, demonstrate the successful end-to-end decoding, debayering, and color space handling of Sony ARW format photos in Graphite. Publish the library to crates.io.

**Background:** For Graphite to work as a photo editing app, it needs to import raw photos. These contain compressed sensor imagery and metadata in a variety of formats. Sony ARW is the first target and additional camera brands are stretch goals. Graphite needs a library written in pure Rust with a suitable (non-GPL) license, which does not currently exist in the ecosystem, so we need to create one ourselves.

### 2023: Bezier-rs library

*Graphite's vector editing features require the implementation of Bezier curve and path manipulation computational geometry algorithms.*

Affiliation: University of Waterloo, Ontario, Canada  
Duration: 9 months  
Students: Hannah Li, Rob Nadal, Thomas Cheng, Linda Zheng, Jackie Chen

- [Bezier-rs library](https://crates.io/crates/bezier-rs)
- [Interactive web demo](https://keavon.github.io/Bezier-rs/)

**Outcomes:** The student group designed an API for representing and manipulating Bezier curves and paths as a standalone Rust library which was published to crates.io. It now serves as the underlying vector data format used in Graphite, and acts as a testbed for new computational geometry algorithms. The team also built an interactive web demo catalog to showcase many of the algorithms, which are also handily embedded in the library's [documentation](https://docs.rs/bezier-rs/latest/bezier_rs/).

### 2022: Backend layout system

*Graphite's UI needs a system to define and manage layouts for widgets from the backend.*

Affiliation: California Polytechnic State University, San Luis Obispo, USA  
Duration: 3 months  
Student: Max Fisher

**Outcomes:** The student designed and implemented a new system across the editor's frontend and backend which made it possible to define and manage layouts for widgets from the backend and receive input data from those widgets. Previously, all layouts were statically defined in the frontend and extensive plumbing was required to pass data back and forth.

### 2022: Path boolean operations

*Graphite's vector editing features require the implementation of boolean operations on paths, such as union, intersection, and difference.*

Affiliation: California Polytechnic State University, San Luis Obispo, USA  
Duration: 3 months  
Student: Caleb Dennis

**Outcomes:** The student devised and prototyped algorithms for performing boolean operations on paths, such as union, intersection, and difference. These were used as a stopgap during 2022 and 2023 to provide users with a rudimentary boolean operation feature set.
