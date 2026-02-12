+++
title = "Student projects"
template = "book.html"
page_template = "book.html"

[extra]
order = 4 # Chapter number
+++

Graphite offers a number of opportunities for students to contribute by building a self-contained project as part of a structured format. These projects are designed to be completed over several months and are ideal for Google Summer of Code or similar internship programs, solo or group university capstone projects, and other arrangements. Each project has a distinct focus and is a great way to make a meaningful contribution to open source over the length of the program while receiving mentorship and guidance from the Graphite team.

Student projects require adherence to a set schedule with regular check-ins, milestones, and evaluations. The structured setting is designed to provide a supportive environment for students to learn and grow as developers while gaining real-world industry experience from collaborating on a sizable software product and remaining accountable to stakeholders. It's our goal to make sure you succeed!

Use this [contributor guide](..) to start out with the code. Then when you're ready, reach out through [Discord](https://discord.graphite.art) and use the `#🎓student-projects` channel to discuss and work towards proposing a project with the Graphite core team.

## AI contribution policy

Be sure to familiarize yourself with our [AI contribution policy](../starting-a-task/ai-contribution-policy) before getting involved with the Graphite code base. Proposals also must not be written by AI or else they will be rejected.

## Google Summer of Code

GSoC is a program offering students a [stipend](https://developers.google.com/open-source/gsoc/help/student-stipends) for successful completion of an internship-style experience with an open source organization. Read about [how it works](https://summerofcode.withgoogle.com/how-it-works/).

Graphite participated in GSoC [2024](https://summerofcode.withgoogle.com/programs/2024/organizations/graphite) and [2025](https://summerofcode.withgoogle.com/programs/2025/organizations/graphite) and we anticipate doing so again in [2026](https://developers.google.com/open-source/gsoc/timeline) if our organization is accepted back. We accept year-round contributions; getting involved early is a great way to have a head start and stand out in your application in the upcoming program.

### Writing a proposal

Writing a good proposal is an important step that demonstrates your understanding of the project and your ability to think ahead and execute it. A well-defined plan will set you up for success throughout the rest of the program.

<details>
<summary>For proposal writing guidelines and requirements: click here</summary>

You are encouraged to reference the project idea list below to find several potential projects suited to your experience, interest, and choice of scope. Then, you must reach out to a [core team member](/about#core-team) through Discord to discuss your plan in detail before writing a proposal. This will help you understand the project's scope and requirements and develop a detailed timeline for your expected summer-long work schedule. Importantly, it will also help us understand your background and capabilities to offer you feedback and suggestions for the best outcome in the competitive applicant selection process.

When it comes to writing the proposal, which you will submit to the GSoC application website, we offer some guidelines below:

- **Proposal structure:** Please consult the [Blender GSoC application template](https://developer.blender.org/docs/programs/gsoc/application_template/) as reference for our desired format. For project ideas already listed below, omit the "Benefits" section. Remember: don't waste your—and our—time restating information that we already know, like background info about Graphite or our tech stack; we just want to hear your thoughts and plans about what you uniquely bring to the table and how you'll execute the project. Proposals should be utilitarian, not formal, while also demonstrating your professional communication skills. Using an LLM to write your proposal won't be to your advantage.
- **Experience:** We're especially interested in your background and work experience, so attaching a résumé or CV is an optional but recommended way to help us understand your capabilities. If able, please also include links to past open source contributions or personal projects in the bio section. Our goal is to provide an environment for you to learn and grow as a productive software engineer and team collaborator, not to help you learn the basics of coding, so any included work examples will help us understand your potential as a self-motivated contributor to the open source community.
- **Work timeline:** Your goal is to write a proposal that inspires confidence in your ability to successfully complete the project, which means understanding in detail what's involved at a technical level and how you plan to tackle it. A detailed work timeline is the most important written part of your proposal. It should be broken into weekly milestones with a couple sentences of technical detail. The summary in the project idea list below doesn't give enough information to develop a timeline, so you'll need to discuss this with the core team on Discord.
- **Prior PRs:** The largest factor in our selection decision will be the quality and extent of your prior contributions to Graphite made during the proposal formulation period (or before, if applicable). Include a link to `https://github.com/GraphiteEditor/Graphite/commits?author=YOUR_GITHUB_USERNAME` in your proposal and feel free to write up a summary of what you've contributed and learned from the process. You may also keep contributing during the month after applications close, before we've finalized our selections, for those additional PRs to be considered.

</details>

## Project idea list

Projects listed below vary considerably in their required skills and technical background. Some are very research-heavy and are only suited for students with years of self-motivated learning and project development in adjacent topics. Others have a more general focus and are approachable to a wider range of students. Please pay close attention to the "Needed Skills" and "Difficulty" indicators so you don't waste your opportunity applying to a project we don't think you're a good fit for.

<!--
- System for nodes displaying gizmos to update their parameters
- Category of tools for repeating, mirroring, patterning, and manipulating objects ("recipes")
- Text improvements (formatting spans, flows between text areas, text-on-path)
- Feature-complete SVG import and rendering support
-->

### Graphene language bidirectional type inference

*Graphene needs to implement a more powerful type system so a generic type may be inferred based on surrounding context of the type's usage constraints.*

- **Possible Mentors:** [Dennis](https://github.com/truedoctor)
- **Needed Skills:** Rust, type theory, programming languages theory, past experience implementing such a system
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Expected Outcomes:** A complete implementation to upgrade the current limited type inference system. The new system should work like Rust's, where variables of unknown types can be given a type satisfying the later usages of the variable.

Consider a node with a generic input parameter which is connected to a node supplying a concrete type. As long as the type is one that satisfies the constraints of the generic parameter, this is valid. The current system checks for this single-directional constraint. But many cases arise where this is insufficient. For example, if the generic parameter is used in multiple places with different constraints, the system needs to be able to infer a type that satisfies all of those constraints.

<details>
<summary>For additional technical details: click here</summary>

Read more about [HM type inference](https://en.wikipedia.org/wiki/Hindley%E2%80%93Milner_type_system), a powerful (but potentially more complex than necessary) model. See also the [GitHub issue](https://github.com/GraphiteEditor/Graphite/issues/2350) describing this, where you can ask questions if needed. This is an advanced topic and only suitable for individuals who have already implemented a similar system in a programming language or compiler project before.

</details>

### Node equivalence rewriting

*A sequence of nodes may perform operations on data that can be expressed using fewer equivalent nodes, and users may often wish to perform such simplifications.*

- **Possible Mentors:** [Dennis](https://github.com/truedoctor)
- **Needed Skills:** Rust, graph theory, algorithm design
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Expected Outcomes:** A system for classifying and tracking data transformations symbolically within the DAG of the node graph. A system for applying rewrite rules to selected portions of the graph to produce an equivalent graph with fewer nodes. Integration with the editor to allow users to apply simplifications to selected nodes, especially to transforms and geometry.

Oftentimes, node graphs contain redundant steps that collectively perform a simpler operation. For example, two Transform nodes may produce the same result as a single Transform node with the combined transformation. Or a node that generates a star shape, then a Path node that applies a differential modification to its geometry, may be equivalent to a single Path node that produces the same geometry in one step. This project is about architecting and integrating a system for tracking classes of data transformations, like transforms or geometric modifications or appearance changes, and allowing the user to select the redundant nodes to collapse or "bake" them into a simpler graph with identical output. This is sort of like selecting the terms of a math expression and applying algebraic simplification rules to reduce it to its simplified form.

<details>
<summary>For additional technical details: click here</summary>

This is best for someone with an interest towards graph theory and compiler optimization topics like [E-graphs](https://en.wikipedia.org/wiki/E-graph). Additional detail is provided in the [GitHub issue](https://github.com/GraphiteEditor/Graphite/issues/2021) including some introductory explanation about E-graphs from a Rust crate that implements them, [egg](https://egraphs-good.github.io/).

</details>

### Machine learning architecture

*AI/ML image/vision models for content editing will need to run in Graphite's node graph with a Rust-centric, modular, portable, deployable, scalable environment.*

- **Possible Mentors:** [Oliver](https://github.com/otdavies)
- **Needed Skills:** Machine learning (and potentially: Rust, Python, ONNX, Burn)
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Hard
- **Expected Outcomes:** Specifics will vary by proposal. In general, a useful end-to-end integration of at least one image model into Graphite's node graph which can run both locally and deployed to a hosting provider server.

AI/ML is filling a rapidly growing role in the industry as a tool in some creative processes. Graphite's procedural node-based workflow is uniquely suited to leveraging the power and flexibility of AI nodes.

[Segment Anything 2](https://ai.meta.com/research/sam2/) (object segmentation) and [Depth Anything 3](https://github.com/ByteDance-Seed/Depth-Anything-3) (depth estimation) are currently the models we are most [interested in integrating](https://github.com/GraphiteEditor/Graphite/issues/1694). The challenge is settling on an architecture and tech stack which is well suited for Graphite's requirements.

<details>
<summary>For additional technical details: click here</summary>

The approach should be extensible to future models. It needs to run fast and natively on the assorted hardware of local user machines with hardware acceleration. It should be a one-click installation process for users to download and run models without requiring dependencies or environment setup. Ideally, it should allow the more lightweight models to run locally in browsers with WebGPU. It needs to also be deployable to servers in a scalable, cost-viable manner that reuses most of the same code that runs locally. Runtime overhead, cold start times, and memory usage should be minimized for quick, frequent switching between models in a node graph pipeline. The tech stack also needs to be permissively licensed and, as much as possible, Rust-centric so it doesn't add complexity to our Wasm and desktop build processes.

To meet most of these criteria, our current thinking is to distribute and run our models using the [ONNX](https://onnx.ai/) format. This would integrate ONNX runtimes for WebGPU, native, and GPU cloud providers. One challenge is that many of the best-performing models are not packaged in ONNX format, but this approach also allows for direct implementation of model architectures in Rust.

[Burn](https://burn.dev/) is Rust's most promising and advanced machine learning framework, and in addition to Rust model implementations, it also [supports](https://github.com/tracel-ai/burn-onnx) ONNX model loading for conversion into its native format.

Another potential direction is to find a portable, modular, lightweight approach for bundling existing Python-based models. It would need to work across simple and complex models with different architectures. License compliance, if GPL code is involved, would be a consideration.

Based on the experience and insight brought to the table by the student, the nature of the project should be defined through preliminary discussions with the mentors and codified in the proposal. Machine learning and MLOps are fields that Graphite's team lack deep expertise in, so we are looking for a knowledgable student who can bring forth a well-researched and well-architected proposal and then execute on it.

</details>

### Generalized graphical data rendering representation

*Rendering graphical content like colors, gradients, patterns, and whole other layers needs to be possible in a more flexible way that can target the fills and strokes of vector shapes.*

- **Possible Mentors:** [Keavon](https://github.com/keavon)
- **Needed Skills:** Rust, SVG
- **Project Size:** Medium or Large *(GSoC: 175 or 350 hours)*
- **Difficulty:** Medium
- **Expected Outcomes:** Improved SVG and Vello renderer implementations that can handle a wider variety of paint types and effects. Support for every combination of paint type with its application to fills, strokes, and full-canvas drawing. Inclusion of the specified paint source types in the graphical data model and appropriate nodes for generating and handling such data.

Presently, Graphite has a limited methodology for defining what gets painted when rendering vector shape fills and strokes. Solid colors and spatially positioned gradients are supported for fills, but only solid colors for strokes. Also, gradients cannot be painted across the entire canvas, and patterns do not exist at all yet. This project involves refactoring the renderer and data model to support a more generalized representation of paint sources that can be applied to fills, strokes, and entire layers. It deprecates the current solid/gradient/none selection for fills and solid/none selection for strokes in favor supporting anything that could be painted as a layer.

<details>
<summary>For additional technical details: click here</summary>

An extended description and a list of child issues is available in the [GitHub issue](https://github.com/GraphiteEditor/Graphite/issues/2779). A large-sized project would likely include support for the polyfilled gradient types described in the sub-issues of [this task](https://github.com/GraphiteEditor/Graphite/issues/2304).

</details>

<!-- ### Advanced text layout and typography

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/1105)

### Brush engine

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/1297)

### Advanced color management

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- Add support for HDR/WCG and/or CMYK and alternate color spaces/models
- Requires an experienced understanding of color science

### SVG with raster effects

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- The SVG spec supports a number of filters and other raster effects, and we currently only implement a small subset.
- Add support for the rest of the SVG spec, including filters, masks, and other raster effects.
- Allow roundtrip import and export of SVG files with these features.
- Import, render (through SVG and Vello), and export of [filters like these](https://codepen.io/miXTim/pen/ZErggMQ).

### Snapping system overhaul

*This is a newly added project pending a full written overview. Come ask on Discord for details.*

- [See the GitHub issue.](https://github.com/GraphiteEditor/Graphite/issues/2352)

### Tooling polishing and gizmo additions

*This is a newly added project pending a full written overview. Come ask on Discord for details.* -->

### Marquee selection masking

*Graphite's raster editing features requires the implementation of Select mode, where users can draw a mask which becomes a marquee (marching ants) selection.*

- **Possible Mentors:** [Keavon](/about#keavon)
- **Needed Skills:** Rust, computer graphics
- **Project Size:** Large *(GSoC: 350 hours)*
- **Difficulty:** Medium
- **Expected Outcomes:** Complete implementation of Mask mode and its marquee selection. Marching ants visualization shader effect. Integration of selection mask with the node graph and raster editing tools. Useful raster editing workflow.

A central part of the workflow in raster image editors is the selection of portions of the image to constrain manipulations just to the masked areas. Tools such as the circular and rectangular marquee, lasso, and magic wand are used to create masks. Instead of using dedicated tools, Graphite's design reuses the existing vector and raster drawing tools (like Rectangle, Ellipse, Pen, and Fill) to create masks in a dedicated Mask mode. Returning from Mask mode reveals the marching ants selection that constrains further editing operations.

This is a key feature in Graphite's evolution to a fully-featured raster editor.

### Testing and performance instrumentation

*Graphite has many areas that could benefit from better automated testing for bugs and performance regressions.*

- **Possible Mentors:** [Dennis](/about#dennis)
- **Needed Skills:** Rust, unit testing
- **Project Size:** Small *(GSoC: 90 hours)* or larger if proposed
- **Difficulty:** Easy
- **Expected Outcomes:** Specific focus and scope may vary by the student's interests and proposal. In general, a significant increase in the coverage of tests in useful code areas (such as document loading, tool manipulation, and rendering) and attention towards systems which measure performance metrics and identify bottlenecks and regressions.

Graphite could benefit from better testing coverage in a number of areas, especially end-to-end testing in the tool, document, and node graph systems. This project is about identifying and addressing areas that are lacking and most vulnerable to suffering from regressions. The student will be responsible for identifying areas that could benefit from better testing.

### Your own idea

*If you have an idea for a project that you think would be a good fit, we'd love to hear it!*

- **Possible Mentors:** Varies
- **Needed Skills:** Varies
- **Project Size:** Varies
- **Difficulty:** Varies
- **Expected Outcomes:** Stated in your proposal.

If none of the projects above suit your interests or experience, we are very open to discussing your own project ideas that could benefit Graphite. You may consult our [task board](https://github.com/orgs/GraphiteEditor/projects/1/views/1) and [roadmap](/features#roadmap) to get a feel for what our current priorities are.

As is the case with all projects, please discuss this with us on Discord to flesh out your idea. Unsolicited proposals that have not been discussed with us will almost certainly be rejected.

