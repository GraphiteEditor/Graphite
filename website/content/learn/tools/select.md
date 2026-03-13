+++
title = "Select tool"
template = "book.html"
page_template = "book.html"

[extra]
order = 1
+++

The **Select tool** is the primary tool for selecting and transforming content. It lets you pick layers, move them around, and resize, rotate, skew, and flip them using interactive bounding box handles or keyboard-driven transforms. Activate it by clicking its icon in the tool shelf or pressing <kbd>V</kbd> (for the letter's arrow shape that resembles the tool icon).

<!-- TODO: Screenshot of the Select tool active with a layer selected, showing the transform cage bounding box with its handles visible -->

## Tool options

The tool's control bar across the top of the viewport provides controls that apply to the Select tool.

<!-- TODO: Screenshot of the Select tool's full control bar -->

### Selection mode

The **selection mode** dropdown controls which layer gets chosen when you click on a stack of overlapping layers.

| Mode | Behavior |
|:-|:-|
| **Shallow Select** | Clicking picks the shallowest (outermost) matching layer. Double-clicking drills down into the hierarchy one level at a time. Press <kbd>Shift</kbd><kbd>Esc</kbd> (or *Select* > *Select Parent* in the menu bar) to go back up one level to the parent. |
| **Deep Select** | Clicking immediately picks the deepest (innermost) layer beneath the cursor. |

Regardless of mode, holding <kbd>Ctrl</kbd> (macOS: <kbd>⌘</kbd>) while clicking always selects the deepest layer directly.

### Pivot

The **pivot** is the point in space used by the tools for rotation and scale manipulation. When enabled, it is visualized by a gizmo drawn at that point. Specifically, it is the center point for:

- Rotating with the transform cage rotation handles
- Scaling with the transform cage edge/corner handles when <kbd>Alt</kbd> is held
- Rotating and scaling via the G/R/S keyboard-driven transforms

It is important to understand that the pivot is purely a **tool-level concept**; it is not stored as part of the layer's data. When the pivot does not coincide with the layer's own local origin, the tool achieves the correct visual result by combining a rotation or scale with a compensating translation, so that the layer appears to transform around the pivot. The layer's actual transform (as seen in its Transform node) reflects only the net result of that math, not the pivot itself. Editing a layer's Transform node values directly always operates around the layer's own local origin, regardless of where the pivot gizmo is placed.

<!-- TODO: Screenshot showing the pivot gizmo (circle) on-canvas positioned away from a layer's center, with an arrow or annotation illustrating that rotation happens around it -->

The **pivot gizmo** checkbox toggles the on-canvas circle indicator. When disabled, the center of selection bounds is used as the pivot. When enabled, the **pivot type** dropdown chooses what determines the pivot's location:

| Type | Meaning |
|:-|:-|
| **Custom Pivot** | <p>The pivot is a freely draggable point. Drag the on-canvas gizmo to place it anywhere. The **9-point reference grid** widget <!-- TODO: Inline image of the 9-point reference grid widget --> snaps it to one of nine reference positions on the bounding box: the four edge midpoints, the four corners, or the center.</p><p>By default the custom pivot is not persistent; it resets to the last-chosen reference point on the 9-point widget whenever the selection is replaced with a different set of layers. The **pin** button <!-- TODO: Inline image of the pin button icon --> changes this behavior, keeping the pivot fixed at its current canvas position regardless of selection changes.</p> |
| **Origin (Average Point)** | The pivot is automatically placed at the average of all selected layers' origins. It cannot be moved manually. |
| **Origin (Active Object)** | The pivot is placed at the origin of the most recently selected layer. It cannot be moved manually. |

### Alignment

Six **alignment buttons** become active when two or more layers are selected:

<!-- TODO: Screenshot of the six alignment buttons in the control bar -->

- Align left edges
- Align horizontal centers
- Align right edges
- Align top edges
- Align vertical centers
- Align bottom edges

Each button aligns all selected layers relative to the combined bounding box of the entire selection. This means a center alignment, for example, places every object at the midpoint between the outermost edges of the whole group, not the center of any individual object.

A useful consequence of this is a two-step technique for centering objects onto the largest one: first apply an edge alignment (e.g. align left edges), which collapses all objects so their edges coincide and shrinks the combined bounding box down to the size of the widest or tallest object. Then apply a center alignment; because the combined bounds now match the largest object, everything centers onto it rather than onto some point in empty space between them.

To align the selected object(s) with an artboard, include the artboard in the selection by <kbd>Ctrl</kbd>-clicking (macOS: <kbd>⌘</kbd>-clicking) the artboard in the Layers panel.

### Flip and turn

Two **flip buttons** <!-- TODO: Inline images of the flip horizontal and flip vertical button icons --> mirror the selection horizontally or vertically. Two **turn buttons** <!-- TODO: Inline images of the turn −90° and turn +90° button icons --> rotate the selection by exactly −90° or +90°. Like alignment, both operations are performed relative to the combined bounding box of the entire selection, so all selected layers move together as a group rather than each being flipped or rotated independently in place.

### Boolean operations

Boolean operations combine or subtract the filled regions of vector shapes to produce new geometry. Five operations are available:

| Operation | Result |
|:-|:-|
| <!-- TODO: BooleanUnion icon --> **Union** | Merges all shapes into one, keeping the area covered by any of them. When applied to a single self-intersecting path, all enclosed regions are filled regardless of how many times the path winds over them. |
| <!-- TODO: BooleanSubtractFront icon --> **Subtract Front** | Removes the area of the frontmost shape from all shapes behind it. |
| <!-- TODO: BooleanSubtractBack icon --> **Subtract Back** | Removes the area of the backmost shape from all shapes in front of it. |
| <!-- TODO: BooleanIntersect icon --> **Intersect** | Keeps only the area where all shapes overlap, discarding everything else. |
| <!-- TODO: BooleanDifference icon --> **Difference** | Keeps only the areas covered by an odd number of overlapping shapes. |

<!-- TODO: Diagram showing two overlapping shapes and the visual result of each of the five boolean operations applied to them -->

**Union** and **Difference** are also useful when applied to a single self-intersecting path. Union fills every enclosed region regardless of winding, producing a fully solid shape. Difference applies the even-odd rule, alternating between filled and unfilled regions each time the path crosses itself, which can produce rings, cutouts, or other effects from a single complex path.

Clicking a boolean operation button wraps the selected layers into a new group and adds a **Boolean Operation node** to that group's node graph. The operation is non-destructive; the original shapes remain intact inside the group and can be selected, moved, reshaped, or reordered at any time. The boolean result updates live as the contents change.

If a group already has a Boolean Operation node applied, clicking any of the five buttons switches to that operation instead of creating a new one. This makes it easy to try each option to find the desired result.

## Selecting layers

### Clicking

Click any unselected layer to select it and deselect everything else. The clicked layer gets highlighted with a bounding box.

| Input | Result |
|:-|:-|
| **Click** | Select the layer under the cursor |
| **<kbd>Shift</kbd> Click** | Add/remove the clicked layer to/from the current selection |
| **<kbd>Alt</kbd> Click** | Remove the clicked layer from the current selection |
| **<kbd>Ctrl</kbd> Click<br />(macOS:<br /><kbd>⌘</kbd> Click)** | If the [selection mode](#selection-mode) is set to the default *Shallow Select*, this overrides it and selects the most deeply-nested layer beneath the cursor for that click |
| **Double-Click** | If the [selection mode](#selection-mode) is set to the default *Shallow Select*, this drills down into the layer grouped within the current one |
| **Click an Empty Area** | Deselect everything; alternatively, <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>A</kbd> (macOS: <kbd>Shift</kbd><kbd>⌘</kbd><kbd>A</kbd>) |

### Box/lasso selection

Drag on an empty area of the canvas to draw a rectangular selection box or freehand lasso shape. Releasing the mouse finalizes the selection.

| Input | Result |
|:-|:-|
| **Drag** | Rectangular box select |
| **<kbd>Ctrl</kbd> Drag** | Draw a freehand **lasso** polygon instead of a rectangle |
| **<kbd>Alt</kbd> Drag** | Remove layers inside the box from the current selection |
| **<kbd>Shift</kbd> Drag** | Add layers inside the box to the current selection |

Whether a layer must be fully enclosed by the selection area or merely touched by it is controlled by a preference in *File* > *Preferences* (macOS: *Graphite* > *Preferences*) under the **Selection** setting:

| Mode | Behavior |
|:-|:-|
| **Touched** *(default)* | Selects any layer that the selection area touches or overlaps. |
| **Enclosed** | Selects only layers fully contained within the selection area. |
| **Directional** | Automatically chooses based on drag direction: dragging rightward uses *Enclosed*, dragging leftward uses *Touched*. |

<!-- TODO: Diagram showing two side-by-side examples: a left-to-right drag selecting only fully enclosed layers (dashed box outline), and a right-to-left drag selecting any touched layers (solid box outline) — label these as Directional mode behavior -->

<!-- TODO: Un-hide this when it's fixed; currently it only works if the Text layer was just selected but doesn't work at any point after that -->
<!-- ### Entering text editing

If exactly one text layer is selected, pressing <kbd>Enter</kbd> opens it in the Text tool for editing. -->

## Transform cage

Selected layers are surrounded by the **transform cage**: a bounding box with interactive handles for moving, resizing, rotating, skewing, and more.

<!-- TODO: Annotated diagram of the transform cage showing the edge midpoint resize handles, corner resize handles, skew triangles beside each edge midpoint, and the rotation zone just outside each corner -->

### Moving

To move a layer, click and drag it. Or with an existing set of selected layers, drag any of them to move them in unison. When offset, a visualization displays the offset distance from the initial drag position.

If the clickable area of the intended layer makes targeting difficult, the circular ring of the **transform dial** offers another method of dragging. It appears while placing your pointer within the transform cage and viewing it in a sufficiently zoomed viewport to fit the dial on screen. Red (X) and green (Y) arrows on the dial may also be dragged to translate the selection along the object's local axes.

<!-- TODO: Screenshot of the transform dial showing the central ring and the red/green axis arrows inside a transform cage -->

| Input | Result |
|:-|:-|
| **Drag** | Move freely |
| **<kbd>Shift</kbd> Drag** | Constrain movement to the horizontal or vertical axis, whichever is dominant |
| **<kbd>Alt</kbd> Drag** | Duplicate the selection and move the copy, leaving the original in place |

When dragging with <kbd>Alt</kbd> held, a duplicate is created and moved instead of the original. While still dragging, pressing <kbd>Ctrl</kbd><kbd>D</kbd> (macOS: <kbd>⌘</kbd><kbd>D</kbd>) places copies at the current position.

### Resizing

Drag an **edge midpoint handle** or **corner handle** of the transform cage to resize the selection.

| Input | Result |
|:-|:-|
| **Drag an Edge<br />or Corner Handle** | Resize the selected objects while the opposite edge or corner stays fixed |
| **<kbd>Alt</kbd> Drag** | Scale around the center or [pivot point](#pivot) instead of the opposite edge or corner |
| **<kbd>Shift</kbd> Drag** | Preserve the initial aspect ratio |

### Rotating

Hover just **outside** a transform cage corner until the pointer changes to a rotation cursor, then drag to rotate the selection around the pivot point.

| Input | Result |
|:-|:-|
| **Drag** | Rotate freely |
| **<kbd>Shift</kbd> Drag** | Snap rotation to 15° angle multiples of the initial rotation |

### Skewing

**Skew handles** appear as small triangles on either side of each edge midpoint when the edge appears large enough on screen. Drag one to shear the selection along that edge.

<!-- TODO: Close-up of the skew triangles visible beside an edge midpoint handle -->

| Input | Result |
|:-|:-|
| **Drag a Skew<br />Triangle** | Skew along the dragged edge |
| **<kbd>Ctrl</kbd> Drag** | Allow free movement by dragging the edge anywhere |

## Keyboard transforms (GRS)

While the Select tool is active and layers are selected, pressing <kbd>G</kbd> (**grab**), <kbd>R</kbd> (**rotate**), or <kbd>S</kbd> (**scale**) begins a keyboard-driven transform. Moving the pointer then applies the transform interactively.

<!-- TODO: Link to the shared GRS documentation page once written -->

## Nudging

Arrow keys move or resize the selected layers in small increments without starting a drag.

| Input | Result |
|:-|:-|
| **Arrows** | Move the selection by 1 px |
| **<kbd>Shift</kbd> Arrows** | Move the selection by 10 px |
| **<kbd>Alt</kbd> Arrows** | Resize the selection by moving the bottom/right edges of its bounding box |
| **<kbd>Ctrl</kbd><kbd>Alt</kbd> Arrows** | Resize the selection by moving the top/left edges of its bounding box |

## Quick measurement

Press and hold the <kbd>Alt</kbd> key anytime while hovering the pointer over an unselected layer to display the offset distances, in pixels, between the selection bounding box and that hovered layer's bounds.

<!-- TODO: Screenshot of the quick measurement overlay showing distance lines and values between a selected layer and a hovered layer -->
