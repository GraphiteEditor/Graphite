+++
title = "Features and limitations"

[extra]
order = 1
+++

Bear in mind that Graphite is alpha software, meaning it is actively changing and improving.

## Current capabilities

A lot is planned on the future [roadmap](/features#roadmap), but here's an overview of the concepts behind the selection of workflows you can currently use in Graphite.

### Vector illustration and graphic design

| | |
|-|-|
| <p>Vector editing is the core competency of the Graphite editor at this stage in its development. That means you can create shape-based vector artwork and designs with the available tools.</p><p>Primitive geometry like rectangles and ellipses can be drawn and, as desired, modified into more complex shapes using the Path tool. Fully organic shapes may also be created from scratch with the Pen tool. They can then be given colors and gradients to add visual style. This cactus is an example of the style of artwork you can create with vector graphics.</p> | <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/cactus-vector-art.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example vector artwork of a potted cactus" style="max-width: unset" /> |

### Procedural design

A procedural content generation workflow lets you describe *how* a creative decision becomes a visual outcome rather than doing it all yourself. For example, copying a shape 25 times around the inside of a circle would be tedious work if done by hand but it's easy for the computer to do it. And if you decide 10 instances may look better than 25, or you want to change the copied shape, or you opt for a different radial separation, it's easy to just update a numerical parameter. That saves you from laboriously placing every shape all over again. You're able to build a *procedure* that the computer carries out on your behalf.

The aforementioned example takes the form of the <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" style="vertical-align: middle" alt="Circular Repeat" /> node which is represented as this box-shaped entity with colored *connectors* on either end. *Nodes* encode certain operations (or functions) in the procedure that generates your artwork. Once you've drawn some content, you can see the nodes which generate it by opening the *node graph* with the <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/node-graph-button.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" style="vertical-align: middle" alt="'Node Graph' button" /> button located to the top right of the viewport. This example may have a node setup which looks like this:

<p><img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/path-fill-circular-repeat-layer.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Path node, Fill node, Circular Repeat node in a sequence feeding into the Untitled Layer" /></p>

Starting from the left, the <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/path-node.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" style="vertical-align: middle" alt="Path" /> node generates some geometry (in this case, drawn using the *Pen* tool). Next, the vector path data feeds through the <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/fill-node__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" style="vertical-align: middle" alt="Fill" /> node to apply a blue color. At this point, the path data looks like so:

<p><img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/blue-arch-shape.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /></p>

Next, that is fed into the <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" style="vertical-align: middle" alt="Circular Repeat" /> node which has several parameters you can modify and get different output data based on your choices, like in these examples:

<style class="table-1-style">
.table-1-style + table {
	width: auto;
}

.table-1-style + table td {
	vertical-align: middle;
	text-align: center;
}
</style>

| | |
|-|-|
| <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-parameters-1__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> | <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-output-1.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> |
| <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-parameters-2__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> | <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-output-2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> |
| <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-parameters-3__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> | <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-output-3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> |
| <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-parameters-4__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> | <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-output-4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" /> |

<!-- TODO: Rename "Angle Offset" to "Start Angle" and redo the screenshots which show that -->
The node's properties offer controls over settings like *Angle Offset* (what angle to start at), *Radius* (distance from the center), and *Instances* (how many copies to distribute). These parameters can also be exposed into the graph so they are driven by the calculated numerical outputs of other nodes instead of values you pick by hand.

### Raster compositing

Raster image editing is a growing capability that will develop over time into the central focus of Graphite. Raster imagery is composed of pixels which are grids of color that can represent anything visual, like paintings and photographs. The current feature set lets you import images, manipulate them using the node-based compositor, and apply nondestructive global effects like color adjustment filters.

A prototype <img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/brush-tool-icon.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="" style="vertical-align: bottom" /> Brush tool exists letting you draw simple doodles and sketches. However it is very limited in its capabilities and there are multiple bugs and performance issues with the feature. It can be used in a basic capacity, but don't expect to paint anything too impressive using raster brushes quite yet. The tool will be fully rewritten in the future.

## Status and limitations

Please make yourself aware of these factors to better understand and work around the rough edges in today's Graphite editor.

### Evolving document format

Saved documents will eventually fail to render in future versions of the Graphite editor because of code changes. Since node implementations and other systems are in flux, file format stability isn't possible yet during this alpha stage of development. If a file opens but there's a rendering error, you may need to open the node graph and replace outdated nodes by creating new ones near the site of an error. Later in the development roadmap, a redesigned file format with a `.gdd` (Graphite Design Document) extension will replace `.graphite` files and it will be built with seamless backwards-compatability in mind.

### Limited raster tooling

While you can import bitmap images, apply image effects in the node graph, and draw brush strokes, there is not much tooling yet to make the overall raster workflow that useful. Marquee selection is an upcoming feature slated for later in 2025 which will significantly improve the utility of raster editing in Graphite.

Hardware accelerated rendering, to offload pixel processing from the CPU to GPU, is also planned for 2025. It will drastically improve the performance of working with millions of pixels.

### Performance bottlenecks

Graphite has several temporary performance bottlenecks that currently yield poor responsiveness when working with raster content, complex vector artwork, and large procedural node graphs. This is especially impactful for raster content. It also currently applies to large volumes of vector data, such as paragraphs worth of text (which is represented as vector paths).

Each of these limitations will be resolved by finishing the implementations of incomplete systems that impose slowdowns in their current forms. For example, certain opportunities for node graph caching are not operational and GPU-accelerated rendering isn't enabled yet.

Performance will be a high-priority focus throughout 2025.

### Best-effort Safari support

Old versions of Safari lack support for the web standards Graphite is built upon. The latest version of the browser still won't run Graphite as well as Chrome and you may encounter extra bugs because we have limited resources to regularly test for Safari issues. Feel free to file issues only if you're using the latest Safari version and find a bug that isn't present in Chrome.

The latest Chrome, Edge, or Opera browser is recommended for the best-supported experience. Firefox works reasonably well, with only some minor loss of quality-of-life features. Brave is likely to encounter issues due to its aggressive degradation of web standards.
