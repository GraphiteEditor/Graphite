+++
title = "Completed projects"

[extra]
order = 2 # Page number after chapter intro
+++

We keep an archive of our successful student projects from past years to help prospective applicants get a better feel for the types and scope of projects we can support.

## 2025

### GPU-accelerated raster operations

*Raster operations are limited to slow CPU-based fallbacks while GPU shader implementations require further infrastructure engineering.*

Affiliation: GSoC 2025
Duration: 3 months
Student: Firestar99

- [Program project listing](https://summerofcode.withgoogle.com/organizations/graphite/projects/details/TfdLAuN4)
- [Report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/2658)

**Outcomes:** Restructuring of dependencies within the node and data type definitions to allow for `#[no_std]` in the implementations of shader-driven raster nodes. Introduction of a compile-time pipeline for loading and compiling CPU node implementations to shader code using [Rust GPU](https://github.com/Rust-GPU/rust-gpu). Node definition macro changes to declare per-pixel color adjustment nodes as fragment shaders. Upstream improvements to Rust GPU and its build tool, [Cargo GPU](https://github.com/Rust-GPU/cargo-gpu), to support Graphite's use cases while avoiding a need for the rest of the Graphite project adopt the nightly Rust toolchain.

**Background:** Graphite's node graph engine executes and renders graphics by means of defining artwork as procedural node graph programs within the purpose-built Graphene language. Each graphics operation is a node with, at minimum, a CPU implementation which supports compilation along with the rest of the graph into an executable Graphene program for rendering to the screen. A major goal is to share that same CPU implementation for the GPU version that compiles to a GPU shader in order to maintain identical algorithms between versions and avoid the maintenance burden of separate code paths. However, GPU architectures enforce challenging constraints which leaves this goal as yet unrealized. The project must tackle the engineering challenges of setting up a basic shader compilation system and integrate GPU versions of nodes into the Graphite editor and its render pipeline.


### 3 additional projects (summaries coming soon)

See the [program listing](https://summerofcode.withgoogle.com/programs/2025/organizations/graphite) for more details until the other three GSoC 2025 project summaries are added here.

## 2024

### Interactive node graph auto-layout

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

### Rendering performance infrastructure improvements

*Graphite performance is bottlenecked by limitations in the new node graph rendering architecture that needs improvements.*

Affiliation: GSoC 2024  
Duration: 4 months  
Student: Dennis Kobert

- [Program project listing](https://summerofcode.withgoogle.com/programs/2024/projects/v5z2Psnc)
- [Report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1773)

**Outcomes:** A holistic, metrics-driven focus on fixing the many unoptimized areas of Graphite's node graph compilation, execution, and rendering systems. Integration of Vello as an integrated rendering backend. A significant improvement in the performance of the editor, especially in the node graph, and a more stable and predictable performance profile. Benchmarking and profiling tools to measure and visualize performance improvements and regressions.

**Background:** Graphite's node graph system is the backbone of the editor, but it has many performance problems that need to be addressed because the system is relatively immature and performance-impacting shortcuts were taken during its initial development. This project is all about making the node graph system more robust and optimized, which will have a direct impact on the user experience and the editor's overall performance. By the end of the project, the editor should finally feel usable in the majority of user workflows. Vello should be enabled as an alternate render engine that will fully replace the existing SVG-based one in the future, once browser support arrives across major platforms.

### Raw photograph decoding in Rust

*For Graphite to support editing photos from professional digital cameras, it needs a raw decoding/processing library.*

Affiliation: GSoC 2024  
Duration: 5 months  
Student: Elbert Ronnie

- [Program project listing](https://summerofcode.withgoogle.com/programs/2024/projects/2uiwOfz8)
- [Report and weekly updates](https://github.com/GraphiteEditor/Graphite/discussions/1771)
- [Rawkit library](https://crates.io/crates/rawkit)

**Outcomes:** A Rust library that implements raw photo decoding functionality to native Rust. A clean, well-structured codebase and API. At a minimum, demonstrate the successful end-to-end decoding, debayering, and color space handling of Sony ARW format photos in Graphite. Publish the library to crates.io.

**Background:** For Graphite to work as a photo editing app, it needs to import raw photos. These contain compressed sensor imagery and metadata in a variety of formats. Sony ARW is the first target and additional camera brands are stretch goals. Graphite needs a library written in pure Rust with a suitable (non-GPL) license, which does not currently exist in the ecosystem, so we need to create one ourselves.

## 2023

### Bezier-rs library

*Graphite's vector editing features require the implementation of Bezier curve and path manipulation computational geometry algorithms.*

Affiliation: University of Waterloo, Ontario, Canada  
Duration: 9 months  
Students: Hannah Li, Rob Nadal, Thomas Cheng, Linda Zheng, Jackie Chen

- [Bezier-rs library](https://crates.io/crates/bezier-rs)
- [Interactive web demo](https://keavon.github.io/Bezier-rs/)

**Outcomes:** The student group designed an API for representing and manipulating Bezier curves and paths as a standalone Rust library which was published to crates.io. It now serves as the underlying vector data format used in Graphite, and acts as a testbed for new computational geometry algorithms. The team also built an interactive web demo catalog to showcase many of the algorithms, which are also handily embedded in the library's [documentation](https://docs.rs/bezier-rs/latest/bezier_rs/).

## 2022

### Backend layout system

*Graphite's UI needs a system to define and manage layouts for widgets from the backend.*

Affiliation: California Polytechnic State University, San Luis Obispo, USA  
Duration: 3 months  
Student: Max Fisher

**Outcomes:** The student designed and implemented a new system across the editor's frontend and backend which made it possible to define and manage layouts for widgets from the backend and receive input data from those widgets. Previously, all layouts were statically defined in the frontend and extensive plumbing was required to pass data back and forth.

### Path boolean operations

*Graphite's vector editing features require the implementation of boolean operations on paths, such as union, intersection, and difference.*

Affiliation: California Polytechnic State University, San Luis Obispo, USA  
Duration: 3 months  
Student: Caleb Dennis

**Outcomes:** The student devised and prototyped algorithms for performing boolean operations on paths, such as union, intersection, and difference. These were used as a stopgap during 2022 and 2023 to provide users with a rudimentary boolean operation feature set.
