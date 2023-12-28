+++
title = "Features and limitations"

[extra]
order = 1
+++

Please keep in mind that Graphite is alpha software, meaning it is actively changing and improving. Remember to save your work frequently because crashes are not unheard of.

## Current capabilities

### Vector illustration and graphic design

Vector editing is the core competency of the Graphite editor at this stage in its development. That means you can create graphic designs and shape-based vector artwork with the tools on offer, like this cactus:

<p><img src="https://static.graphite.rs/content/index/just-a-potted-cactus-thumbnail.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example vector artwork of a potted cactus" /></p>

Primitive geometry like rectangles and ellipses can be drawn and, as desired, modified into more complex shapes using the Path tool. Fully organic shapes may also be created from scratch with the Pen tool. They can then be given colors and gradients to add visual style.

### Raster compositing

Raster image editing is a growing capability that will develop over time into the central focus of Graphite. Raster imagery is composed of pixels which are grids of color that can represent anything visual, like paintings and photographs. The current feature set lets you import images, manipulate them using the node-based compositor, and apply nondestructive effects like color adjustment filters.

A prototype Brush tool exists letting you draw simple doodles and sketches. However it is very limited in its capabilities and there are multiple bugs and performance issues with the feature. It can be used in a limited capacity, but don't expect to paint anything too impressive using raster brushes quite yet.

The raster-based Imaginate feature enables you to synthesize artwork using generative AI based on text descriptions. With it, you can also nondestructively modify your vector art and imported images. You can inpaint (or outpaint) the content in a specific masked part of an image or use it to touch up quick-and-dirty compositions.

### Procedural design

Procedural content generation workflows let you describe *how* a creative decision becomes a visual outcome rather than doing it all yourself. For example, copying a shape 50 times around the inside of a circle would be a lot of work if done by hand but it's easy for the computer to do it. And if you decide you prefer 60 instead of 50 instances, or you want to change the copied shape, or you opt for a different circle radius, you can avoid doing even more manual work by editing the parameters instead of doing all the laborious changes yourself each time. You're able to build a *procedure* that the computer carries out on your behalf.

The aforementioned example takes the form of the *Circular Repeat* node:

<p><img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width /= 2" alt="Circular Repeat node" /></p>

Nodes are boxes that encode a certain operation (or function) in the procedure that generates your artwork. On the left, this node takes the input of your shape to be duplicated. On the right, the modified data (with the repeated shape) is the output. Links are wired from the outputs of nodes to the inputs of others, left to right, in the *node graph* which can be accessed by clicking this button located in the bottom left corner of the Graphite editor:

<p><img src="https://static.graphite.rs/content/learn/interface/document-panel/graph-view-button-while-closed.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Open node graph button" /></p>

The node's properties give additional controls over settings like *Angle Offset* (what angle to start at), *Radius* (how large the circle pattern should be), and *Count* (how many copies to make):

<p><img src="https://static.graphite.rs/content/learn/introduction/features-and-limitations/circular-repeat-node-parameters.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width /= 2" alt="Circular Repeat node parameters" /></p>


These parameters can also be exposed into the graph so they are driven by the calculated numerical outputs of other nodes instead of values you pick by hand.

## Status and limitations

Please make yourself aware of these factors to better understand and work around the rough edges in today's Graphite editor.

### Unstable document format

Artwork you save as a `.graphite` document file will eventually fail to open in future versions of the Graphite editor because of code changes. Since the implementations are in flux for many systems, file format stability isn't possible yet during this alpha stage of development. A redesigned file format with a `.gdd` (Graphite Design Document) extension will replace `.graphite` files and it will be built with backwards-compatability in mind.

Sometimes an error will appear when opening an outdated document. Other times, it may open but result in a crash or broken functionality when editing. With some technical know-how, it might be possible to manually edit the JSON serialization format, but this is not officially supported.

<!-- To open an outdated file, [look here](https://github.com/GraphiteEditor/Graphite/deployments/activity_log?environment=graphite-editor+%28Production%29) for the previous version of the Graphite editor that was published before the date you saved the document. Click "View deployment" to open it. -->

### No vector import

While you can export your artwork as an SVG file for use elsewhere, there is not support yet for importing an SVG file to be edited.

### Unstable node graph interactions

The node graph implementation was completed very recently. There are still a number of bugs and unexpected limitations that can arise. If the graph isn't updating, it may have become invalid. You can check if there are errors by opening the JavaScript console with the <kbd>F12</kbd> key, and if you see node graph evaluation errors, undo your changes to before the unsupported graph edit.

### No snapping system

Previous versions of Graphite had a mediocre snapping system for helping you draw with precise alignment between elements in your artwork. To accommodate implementing the node graph, this code had to be removed because it conflicted. An improved version of the feature will be [rebuilt](https://github.com/GraphiteEditor/Graphite/issues/1206) in the near future.

### Limited Safari support

Old versions of Safari lack the minimum web standards features Graphite requires to run. The latest version of the browser still won't run Graphite as well as Chrome and you may encounter extra bugs because we have limited resources to regularly test for Safari issues.

The latest Chrome or Chromium-based browser is recommended for the best-supported experience, although Firefox works with only some minor feature degradations.
