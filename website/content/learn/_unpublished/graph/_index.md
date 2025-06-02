+++
title = "Graph"
template = "book.html"
page_template = "book.html"

[extra]
order = 4
+++

- Opening the graph
- Document graph vs. layer graph, limitations
- Adding nodes
- Connecting nodes

## Overlaid node graph editing

Opening the overlaid node graph shows the structure of nodes and layers that compose the document artwork. It's a more detailed view of what the [Layers](../layers-panel) and [Properties](../properties-panel) panels show.

**Nodes** are the entities with left-to-right input **connectors**.

**Layers** are the larger entities shown with thumbnails and a bottom-to-top direction of data stacking. Their purpose is to composite sources of graphical data on top of one another in a **layer stack**. Layers take input from other nodes or layers via a connector on their left side. When that connector is fed by another layer stack, the Layers panel considers it a **group** because it combines one stack into another parent stack.

Layers and nodes are wired together using **links** which send data between the outputs of nodes to the inputs of others. You can wire up a node by dragging from the output connector of one node to the input connector of its destination node. But note that forming cyclic graphs, where a loop can be traced along the links of a set of nodes, is not permitted. Graphical data flows into the **Output** node which then becomes rendered to the document viewport.

### Node/layer controls

When a layer or node is selected, these buttons will show up on the left side of the control bar:

<p><img src="https://static.graphite.rs/content/learn/interface/document-panel/node-controls-buttons.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="The node/layer controls" /></p>

| | |
|-|-|
| Make<span>&nbsp;</span>Hidden/<br />Make<span>&nbsp;</span>Visible | <p>Toggles the visibility state of the layer or node. This is equivalent to the eye icon button displayed beside each layer. If a node or layer is hidden, it gets bypassed in the data flow. <kbd>Ctrl</kbd><kbd>H</kbd> (macOS: <kbd>⌘</kbd><kbd>H</kbd>) is a shortcut for this toggle that can be used from the graph or viewport.</p> |
| Preview/<br />End<span>&nbsp;</span>Preview | <p>Temporarily moves the graph output away from the Output node and the graph output is instead provided by the previewed node. While previewing, the node is styled with a dashed, brighter border. Ending the preview returns responsibility back to the Output node. This is a handy feature for viewing part of a graph without needing to disconnect the actual Output node and manually restore it later. Clicking a node or layer in the graph while holding <kbd>Alt</kbd> is a shortcut for toggling its preview.</p> |
