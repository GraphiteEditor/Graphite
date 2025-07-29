+++
title = "Graphene"
template = "book.html"
page_template = "book.html"

[extra]
order = 5 # Chapter number
+++

Graphene is the node graph engine that powers the Graphite editor.

It's hard to describe in one sentence precisely what Graphene is, because it's a technology that serves several roles when viewed from different angles. But to get a feel for what it encompasses, here is a list of some of its purposes:

- Render engine
- Runtime environment
- Procedural data processor
- Node-based scripting system
- Compiled programming language
- Compiler toolchain built around `rustc`

## Background

### Artwork as a program

Artwork created in Graphite is represented as a node graph that generates the graphical content authored by the user. This document is essentially source code for a program in the Graphene language. Modifying the graph (like adding a layer, changing a node's parameter, or updating a node's data every frame while interactively drawing a shape) changes the actual program that generates and renders the artwork. This program must be recompiled and executed every frame a change is made.

Nodes are functions that run algorithms related to graphical operations. Some may read bitmap images from disk, others may generate procedural patterns, and more may be used for compositing and blending. Vector nodes can also produce shapes, alter their geometry, and apply styling and effects. Put together, a full document is built from just its interconnected nodes— producing a complete work of art generated entirely with algorithms and data.

### Graph executors as programming languages

Every node-based application needs to run its node graph to compute the resulting data. Execution occurs in an order that depends on the shape of the graph so that every node has the data it needs to compute its output.

A procedural graph executor, in its basic form, is a simple system that executes functions in the appropriate order. It feeds information between nodes and caches that data for reuse between executions so that only changed branches of the graph have to be computed again. The system, as described, is the approach commonly used by virtually all node-based apps.

Crucially, the execution flow is handled at runtime so there is some overhead during every run. By analogy to programming languages, this traditional execution model acts like an interpreted language. But interpreted languages are famously slow, and we don't want Graphite leaving performance on the table.

In designing Graphene, we decided to take a more advanced approach that could yield many of the benefits of a compiled language— code inlining, compiler optimizations, and a philosophy of offloading invariant enforcement to the type system. Instead of building a simple graph interpreter where functions (nodes) are run as user input changes, we designed a system that dynamically executes the graph with a variable degree of pre-compiled optimizations where bits and pieces are recompiled and patched in while the user modifies the artwork (and graph) every frame. Thereby, Graphene can dynamically range between an interpreted language, a JIT-optimized language, and a fully compiled language.

## Technical overview

### The latency/performance tradeoff

While working in Graphite, multiple needs arise for speed in different contexts. While making interactive changes, the user needs feedback as quickly as possible. While panning and zooming the canvas or playing an animation, the user cares about smoothness and responsiveness. When procedural artwork is exported as a standalone program that processes data at runtime (like as part of an image processing web server or embedded within a game engine), performance is the sole concern.

This sliding scale of latency/performance concerns maps directly to programming language concepts. Interpreted languages run immediately, but with slow runtime performance. JIT-optimized languages also run nearly without delay, but with less overhead than an interpreter since it can dynamically balance its effort towards optimizing and executing code. Compiled languages take upfront time to compile, but run with less overhead. A choice of optimization levels can be applied to further trade initial compilation time for runtime performance.

We designed Graphene to operate in all three regimes:

| Regime      | Usage                                                                 |
|:------------|:----------------------------------------------------------------------|
| Interpreted | While editing. Simple and currently the only mode that's implemented. |
| JIT         | While editing. Dynamically bridges the gap between both other regimes by selectively substituting branches of the graph with interpreted and compiled nodes to keep latency low and work towards higher execution performance. |
| Compiled    | When exported. The entire graph is compiled as a standalone program.  |

### Building upon the Rust compiler

Nodes are functions written in Rust and every node has precompiled bytecode that ships with Graphite for use in the interpreted regime. The graph `input` → `A` → `B` → `C` → `output` is equivalent to the Rust statement `let output = C(B(A(input)));`. Graphene can either execute `A`, `B`, and `C` sequentially in its interpreted regime, or its JIT and compiled regimes can generate that Rust statement and compile it with the Rust compiler, `rustc`. The inlined and optimized bytecode can then be substituted for those three nodes in the JIT regime.

Graphene figures out which branches of the graph to compile and substitute as part of the JIT process while the user is authoring content in Graphite. While editing the graph, as changes occur to specific nodes, their surrounding graph branches drop back down to using the slower interpreted nodes. Then the JIT system works its way back up to faster execution over time by gradually compiling and swapping in larger optimized parts of the overall graph.

The fully compiled regime is used only when the user exports the procedural artwork as a standalone program. For example, a CLI program may read a string input argument (like a name) and procedurally generate an output image file (like a birthday card).

### Compile server

The three regimes have thus far been only a description of the eventual architecture direction. The interpreted regime is currently the only mode implemented in Graphene. The other two will require access to `rustc` which will necessitate the compile server that we will finish building and then publicly host for Graphite users in the future. Users of the desktop version of Graphite will be able to use an embedded `rustc` if the user has opted to download the Rust toolchain while installing Graphite.

Without a compile server, all the nodes are precompiled when Graphite is built. The node registry (in the file `node_registry.rs`) currently exists to allow the interpreted executor to find the Rust functions that correspond to each node with its appropriate type signature. Nodes support generics, so it's currently necessary to list every forseeable concrete type signature in the registry until the compile server can generate bytecode for less common type combinations on-the-fly.

### GPU compute shaders

Further building upon the Rust compiler toolchain, we employ the [`rust-gpu`](https://github.com/EmbarkStudios/rust-gpu) compiler backend for `rustc` which generates compute shaders that get executed on the GPU. This means we can write the same code to implement nodes that run on both CPU and GPU. (Although in practice, some nodes may need GPU-specific versions suited for the architectural limitations of GPU programming.) And we don't have to use a separate shader language!

### A language within a language

While Graphene is a programming language, it is also foundationally built upon the Rust language. We don't just use the Rust compiler, but we also employ its type system, traits, data structures, standard library, and crate ecosystem. The data that flows between nodes are Rust types (like structs, enums, tuples, primitives, and collections). Graphene's generic type system uses Rust's trait definitions in its enforcement of type safety and type inference.

### Graphene language concepts

Since Graphene is fundamentally a programming language, throughout this documentation we will use analogies which correlate Graphene concepts with their counterparts from traditional programming language theory. Here is an at-a-glance overview:

| Graphene concept  | Programming language concept         |
|:------------------|:-------------------------------------|
| Node              | Function                             |
| Graphite editor   | IDE/text editor                      |
| Document          | Source code                          |
| Graph/network     | Abstract syntax tree (AST)           |
| Graph compilation | Linking/JIT optimization/compilation |
| Graph execution   | Program execution                    |

<!-- Our philosophy of building (bootstrapping) our own higher-level language features from the language itself -->
<!-- Call arguments, construction arguments, `.eval()`, recompiling when construction argument values are updated but not when call argument data changes -->
<!-- Compose nodes and automatic/manual composition -->
<!-- Extract/inject nodes and metaprogramming -->
<!-- Cache nodes and stable node IDs -->
<!-- Graph rewriting step (currently used only to remove Identity nodes),
	 at various points in the compilation process,
	 based on rules akin to an optimizing compiler -->
<!-- Borrow tree -->
<!-- Document nodes, proto nodes, and networks (must be: acyclic) -->
<!-- Lambdas -->
<!-- Graph compilation process -->
<!-- The compilation server -->
<!-- Code structure overview -->
<!-- Guide for implementing a node -->
<!-- The `Node` trait -->
<!-- Generics, type inference, type erasure, and the node registry -->
<!-- Monitor nodes -->
