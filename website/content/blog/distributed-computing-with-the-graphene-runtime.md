+++
title = "Distributed computing with the Graphene runtime"
date = 2022-04-18

[extra]
banner = "2022-03-12-graphite-a-vision-for-the-future-of-2d-content-creation.png"
author = "Keavon Chambers"
+++

Graphite is a professional 2D graphics editor for photo editing, image manipulation, graphic design, illustration, data visualization, automation, and technical art. It is designed to run on a variety of machines, from mobile hardware like iPads or web browsers on midrange laptops up to beefy workstations with dozens of CPU cores and multiple GPUs. To provide a responsive user experience, its architecture is made to support the use of cloud computation to make up for deficiencies in local compute power, even providing some productivity improvements for high-end workstation users thanks to the wide scalability of distributed rendering.

# Node-based editing

A core feature is Graphite's reliance on procedural content generation using a node graph system called Graphene. In traditional editors like Photoshop and Gimp, certain operations like "blur the image" modify the image pixels permanently, destroying the original (unblurred) image information.

Graphite is a *nondestructive* editor. Its approach is to store the "blur" operation as a step in the creation process. All editing steps made by the user are encoded as operations, such as: import or resize an image, draw with a paintbrush, select an area with a certain color, combine two geometric shapes, etc. Operations are functions that process information and are called *nodes*. For example, the "Blur" node takes an image and a strength value and outputs a blurred version of the image. More advanced, machine learning-powered nodes may do things like image synthesis or style transfer. Many nodes perform a wide variety of image editing operations and these are connected together into a directed acyclic graph where the final output is the pixels drawn to the screen or saved to an image file.

Many nodes process raster image data, but others work on data types like vector shapes, numbers, strings, colors, and large data tables (like imports from a spreadsheet, database, or CSV file). Some nodes perform visual operations like blur while others modify data, like performing regex matching on strings or sorting tables of information. For example, a CSV file might be imported, cleaned up, processed, then fed into the visual nodes which render it in the form of a chart. Different nodes may take microseconds, milliseconds, seconds, or occasionally even minutes to run. Most should not take more than a few seconds, and those which take so long should run infrequently. During normal operations, hundreds of milliseconds should be the worst case for ordinary nodes that run frequently. Caching is used heavily to minimize the need for recomputation.

## Node authorship

The nodes that process the data can be computationally expensive. The goal is for the editor to ordinarily run and render (mostly) in real-time in order to be interactive. Because operations are arranged in a directed acyclic graph rather than a sequential list, there is opportunity to run many stages of the computation and rendering in parallel.

Nodes are implemented by us as part of a built-in library, and by some users who may choose to write code using a built-in development environment. Nodes can be written in Rust to target the CPU with the Rust compilation toolchain (made conveniently accessible for users). The same CPU Rust code can often be reused for authoring GPU compute shaders via the [rust-gpu](https://github.com/EmbarkStudios/rust-gpu) compiler.

## Sandboxing

For security and portability, user-authored nodes are compiled into WebAssembly (WASM) modules and run in a sandbox. Built-in nodes run natively, and are not sandboxed, for better performance (except when the entire editor is running in a web browser). When running in a distributed compute cluster on cloud machines, the infrastructure provider may be able to offer sandboxing to sufficiently address the security concerns of running untrusted code in order to safely use the native (non-WASM) versions of nodes.

# The Graphene distributed runtime

In the product architecture, Graphene is a distributed runtime environment for quickly processing data in the node graph by utilizing a pool of CPU and GPU compute resources available on local and networked machines. Jobs are run where latency, speed, and bandwidth availability will be most likely to provide a responsive user experience.

## Scheduler

If users are running offline, their CPU threads and GPU (or multiple GPUs) are assigned jobs by the local Graphene master scheduler. If running online, some jobs are performed locally while others are run in the cloud, an on-premise compute cluster, or just a spare computer on the same network. The scheduler generally prioritizes keeping quicker, latency-sensitive jobs local to the client machine or LAN while allowing slower, compute-intensive jobs to usually run on the cloud. Each networked cluster (such as the local machine, an on-prem render farm, and the cloud data center) runs a master scheduler that commands the available compute resources. The multiple master schedulers (of which one may be authoritative) must cooperatively plan the high-level allocation of resources in their respective domains while avoiding excessive chatter over the internet about the minutiae of each job invocation.

## Cache manager

Working together with the scheduler, the Graphene cache manager stores intermediate node evaluation results and intelligently evicts them when space is limited. If the user changes the node graph partially downstream, it can reuse the upstream cached data but will need to recompute changed downstream operations. When rendering raster imagery, areas of the image are broken down into tiles at a certain resolution (document zoom depth) and cached on a per-tile basis. Tiles are used because the whole document is too large to render all at once every time changes occur, since some parts may be outside the confines of the current viewport, and because changes may only invalidate portions of the document so only those tiles need to re-render.

## Progressive enhancement

The scheduler and cache manager work in lockstep to utilize available compute and cache storage resources on the local machine or cluster in order to minimize latency for the user. Immediate feedback is needed when, for example, drawing with a paintbrush tool. Sometimes, nodes can be run with quality flags. Many nodes are be implemented using several different algorithms that produce faster results of worse quality. This means it runs once with a quick and ugly result, then runs again later to render a higher quality version, and potentially several times subsequently to improve the visual fidelity when the scheduler has spare compute resources and time. Anti-aliasing, for example, will usually pop in to replace aliased renders after a few seconds of waiting.

## Batched execution

It is important to reduce the overhead between executions. Sometimes, Graphene will predict, or observe during runtime, the frequent execution of code paths (or rather, node paths) with significant sequential (not parallel) execution steps. These are good candidates for optimization by reducing the overhead between each execution. In these cases, Graphene will batch multiple sequentially-run nodes by recompiling them such that they are inlined as one execution unit. This can happen for both CPU-based programs and GPU-based compute shaders. This is conceptually similar to how just-in-time (JIT) compilers can predict, or observe at runtime, frequently-run code paths in order to apply compiler optimizations where they are most impactful.

## Data locality

When dealing with sequential chains of nodes, if they haven't been recompiled as a batched execution unit, the Graphene scheduler may also frequently prioritize grouping together a set of related nodes to be run together on the same machine for memory and cache locality. Sequential nodes allocated to the same physical hardware can avoid the overhead of copying data over the internet, or between machines in the cluster, or between RAM and VRAM, or from RAM to the CPU cache. Graphene should recognize when a certain intermediate result already lives in RAM or VRAM and prioritize using that CPU or GPU to compute the tasks which rely on that data. But when that's not possible, we need a fast architecture to transfer data between machines. For GPUs, it might be possible to use a DMA (Direct Memory Access) strategy like Microsoft's new DirectStorage API for DirectX and transfer data into or out of VRAM straight to SSDs or networked file systems. Efficiently transferring the final result, and maybe occasionally intermediate cache results, between networks (like the cloud and client) with latency and bandwidth considerations is also important.
