+++
title = "Document panel"

[extra]
order = 2
+++

The **Document panel** is the main content area where the artwork is displayed and edited using **tools** within the **viewport**. It's also where the **node graph** can be overlaid by pressing <kbd>Ctrl</kbd><kbd>Space</kbd>.

The viewport is for interactive, visual editing of the **canvas**. The node graph is where you can inspect the underlying structure of the document and edit it in a more technical manner when the need arises.

There is one instance of the Document panel per open document file. Each has its own tab labeled with its file name. When a document has unsaved changes, an `*` is included at the end of the name.

The Document panel is composed of three main areas:

- The **control bar** runs across the top of the panel and shows tool and viewport controls.
- The **tool shelf** is the vertical bar that runs down the left of the panel showing the editing tools and working colors.
- The **viewport** fills most of the panel and contains a view of the canvas which can be interactively edited using the tools.

## Control bar

Here is where you control your interaction with the document via active tool and view options.

<!--
### Editing modes

Only the default mode is currently implemented. Others will be added in the future and this dropdown is a placeholder for that.

| | |
|-|-|
| <img src="https://static.graphite.rs/content/learn/interface/document-panel/editing-modes__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="The editing modes dropdown menu" /> | The default, **Design Mode**, is for directly editing the artwork.<br /><br />Once implemented, **Select Mode** will be where marquee selections are made to constrain the active tool's edits to a masked area of choice.<br /><br />Once implemented, **Guide Mode** will be for creating guides and constraint systems used for alignment and constraint-based layout. |
-->

### Tool controls

The left side of the control bar has controls pertaining to the active tool. It's empty for some tools.

<p><img src="https://static.graphite.rs/content/learn/interface/document-panel/tool-options__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example of the tool controls for the Select tool" /></p>

The example above shows the Select tool controls. Because the Select tool deals with selecting and transforming layers, its controls include:
- The selection mode to determine if the deepest or shallowest layer in a nested hierarchy is selected when clicked.
- The point about which the **transform cage** is centered.
- Actions to align and distribute the selected layers amongst themselves.
- Actions to flip the selected layers horizontally or vertically.
- Actions to cut or combine selected layers with themselves using boolean operations.

Depending on which tool is active, these will change to reflect the pertinent options.

### Viewport controls

The right side of the control bar has controls related to the active document and viewport.

<p><img src="https://static.graphite.rs/content/learn/interface/document-panel/viewport-options__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The viewport controls" /></p>

| | |
|-|-|
| Overlays | <p>When checked (default), overlays are shown. When unchecked, they are hidden. Overlays are the temporary contextual visualizations (like bounding boxes and vector manipulators) that are usually blue and appear atop the viewport when using tools.</p> |
| Snapping | <p>When checked (default), drawing and dragging shapes and vector points means they will snap to other areas of geometric interest like corners or anchor points. When unchecked, the selection moves freely.<br /><br />Fine-grained options are available by clicking the overflow button to access its options popover menu. Each option has a tooltip explaining what it does by hovering the cursor over it.</p><p><img src="https://static.graphite.rs/content/learn/interface/document-panel/snapping-popover__5.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="Snapping options popover menu" /></p><p>Snapping options relating to **Bounding Boxes**:</p><p><ul><li>**Align with Edges**: Snaps to horizontal/vertical alignment with the edges of any layer's bounding box.</li><li>**Corner Points**: Snaps to the four corners of any layer's bounding box.</li><li>**Center Points**: Snaps to the center point of any layer's bounding box.</li><li>**Edge Midpoints**: Snaps to any of the four points at the middle of the edges of any layer's bounding box.</li><li>**Distribute Evenly**: Snaps to a consistent distance offset established by the bounding boxes of nearby layers (due to a bug, **Corner Points** and **Center Points** must be enabled).</li></ul></p><p>Snapping options relating to **Paths**:</p><p><ul><li>**Align with Anchor Points**: Snaps to horizontal/vertical alignment with the anchor points of any vector path.</li><li>**Anchor Points**: Snaps to the anchor point of any vector path.</li><li>**Line Midpoints**: Snaps to the point at the middle of any straight line segment of a vector path.</li><li>**Path Intersection Points**: Snaps to any points where vector paths intersect.</li><li>**Along Paths**: Snaps along the length of any vector path.</li><li>**Normal to Paths**: Snaps a line to a point perpendicular to a vector path (due to a bug, **Intersections of Paths** must be enabled).</li><li>**Tangent to Paths**: Snaps a line to a point tangent to a vector path (due to a bug, **Intersections of Paths** must be enabled).</li></ul></p> |
| Grid | <p>When checked (off by default), grid lines are shown and snapping to them becomes active. The initial grid scale is 1 document unit, helping you draw pixel-perfect artwork.</p><ul><li><p>**Type** sets whether the grid pattern is made of squares or triangles.</p><p>**Rectangular** is a pattern of horizontal and vertical lines:</p><p><img src="https://static.graphite.rs/content/learn/interface/document-panel/grid-rectangular-popover__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="Snapping options popover menu" /></p><p>It has one option unique to this mode:</p><ul><li>**Spacing** is the width and height of the rectangle grid cells.</li></ul><p>**Isometric** is a pattern of triangles:</p><p><img src="https://static.graphite.rs/content/learn/interface/document-panel/grid-isometric-popover__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="Snapping options popover menu" /></p><p>It has two options unique to this mode:</p><ul><li>**Y Spacing** is the height between vertical repetitions of the grid.</li><li>**Angles** is the slant of the upward and downward sloped grid lines.</li></ul></li><li>**Display** gives control over the appearance of the grid. The **Display as dotted grid** checkbox (off by default) replaces the solid lines with dots at their intersection points.</li><li>**Origin** is the position in the canvas where the repeating grid pattern begins from. If you need an offset for the grid where an intersection occurs at a specific location, set those coordinates.</li></ul> |
| Render Mode | <p>**Normal** (default): The artwork is rendered normally.</p><p>**Outline**: The artwork is rendered as a wireframe.</p><p>**Pixel Preview**: **Not implemented yet.** The artwork is rendered as it would appear when exported as a bitmap image at 100% scale regardless of the viewport zoom level.</p><p>**SVG Preview**: **Not implemented yet.** The artwork is rendered as it would appear when exported as an SVG image.</p> |
| Zoom In | <p>Zooms the viewport in to the next whole increment.</p> |
| Zoom Out | <p>Zooms the viewport out to the next whole increment.</p> |
| Reset Tilt and Zoom to 100% | <p>Resets the viewport tilt to 0°. Resets the viewport zoom to 100% which matches the canvas and viewport pixel scale 1:1.</p> |
| Viewport Zoom | <p>Indicates the current zoom level of the viewport and allows precise values to be chosen.</p> |
| Viewport Tilt | <p>Hidden except when the viewport is tilted (use the *View* > *Tilt* menu action). Indicates the current tilt angle of the viewport and allows precise values to be chosen.</p> |
| Node Graph | <p>Toggles the visibility of the overlaid node graph.</p> |

## Tool shelf

This narrow bar runs vertically down the left side of the Document panel beside the viewport.

### Tools

Located at the top of the tool shelf area, the **tool shelf** provides a selection of **tools** for interactively editing the artwork.

| | |
|-|-|
| <img src="https://static.graphite.rs/content/learn/interface/document-panel/tool-shelf__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="The tool shelf" /> | <p>The tool shelf is split into three sections: the **general tools** (gray icons), **vector tools** (blue icons), and **raster tools** (orange icons).</p><p><ul><li>**General tools** are used for assorted editing tasks within the viewport.</li><li>**Vector tools** are used for drawing and editing vector shapes, paths, and text.</li><li>**Raster tools** are used for drawing and editing raster image content. The grayed out icons are placeholders for upcoming tools.</li></ul></p> |

### Working colors

The **working colors** are the two colors used by the active tool.

| | |
|-|-|
| <img src="https://static.graphite.rs/content/learn/interface/document-panel/working-colors.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The working colors" /> | <p>The upper circle is the **primary color**. The lower circle is the **secondary color**.</p><p>There are two buttons located underneath: **Swap** which reverses the current color choices, and **Reset** which restores the primary color to black and the secondary color to white.</p> |

The tool controls (above the viewport) for some of the tools offer choices for using the primary and secondary colors. For example, the vector drawing tools have **Fill** and **Stroke** options that use the current secondary and primary colors, respectively, as defaults:

<p><img src="https://static.graphite.rs/content/learn/interface/document-panel/tool-options-fill-stroke-colors__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" onload="this.width = this.naturalWidth / 2" alt="The Fill and Stroke controls for a vector tool's controls" /></p>

These options each allow choices of being driven by the primary working color, secondary working color, or a custom color set just for that tool.

## Viewport

The **viewport** is the view into the canvas. It lets you interact visually with the artwork content. It also displays temporary contextual **overlays** atop the content to assist with your editing.

The viewport is also surrounded by **scrollbars** and **rulers** around its edges. Scrollbars, located along the bottom/right edges, allow scrolling the artwork to show different parts in the viewport. Rulers, located along the top/left edges, offer dimensional information for the area of the canvas that's within view. They can be hidden with the *View* > *Rulers* toggleable menu option.
