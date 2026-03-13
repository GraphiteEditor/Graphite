+++
title = "Artboard tool"
template = "book.html"
page_template = "book.html"

[extra]
order = 2
+++

The **Artboard tool** creates and manages artboards: named rectangular regions that define the bounds of exportable artwork areas on the canvas. Activate it by clicking its icon in the tool shelf.

<!-- TODO: Screenshot of the Artboard tool active with one artboard created and selected, showing its transform cage and label -->

## Creating artboards

Click and drag on any empty area of the canvas to draw a new artboard. When the mouse is released, the artboard is created with a white background and given a default label.

<!-- TODO: Screenshot showing an artboard being drawn with the dashed preview rectangle visible -->

| Input | Result |
|:-|:-|
| **Drag** | Create a new artboard with the dragged dimensions |
| **<kbd>Shift</kbd> Drag** | Constrain the new artboard to a square |
| **<kbd>Alt</kbd> Drag** | Draw from the center outward rather than from corner to corner |

## Transform cage

When an artboard is selected, its **transform cage** appears around it. This works similarly to the Select tool's transform cage but is limited to moving and resizing; artboards cannot be rotated or skewed.

<!-- TODO: Annotated diagram of an artboard's transform cage showing the edge and corner midpoint handles -->

### Selecting

Click any artboard to select it, which displays its transform cage and makes its properties editable. Only one can be selected at a time.

| Input | Result |
|:-|:-|
| **Click an Artboard** | Select that artboard |
| **<kbd>Delete</kbd> or <kbd>Backspace</kbd>** | Delete the selected artboard, including its inner artwork contents |


### Moving

Click and drag anywhere in the artboard to move it. The artboard's inner artwork contents move with it, maintaining relative positions to the artboard container.

| Input | Result |
|:-|:-|
| **Drag** | Move the artboard freely |
| **<kbd>Shift</kbd> Drag** | Constrain movement to the horizontal or vertical axis (whichever is dominant) |

### Resizing

Drag an **edge midpoint handle** or **corner handle** on the transform cage to resize the artboard. Its inner artwork contents are repositioned to match the new bounds if the top/left edges are dragged.

| Input | Result |
|:-|:-|
| **Drag an Edge or<br />Corner Handle** | Resize the artboard while the opposite edge or corner stays fixed |
| **<kbd>Alt</kbd> Drag** | Resize from the center rather than the opposite edge or corner |
| **<kbd>Shift</kbd> Drag** | Preserve the original aspect ratio |

## Nudging

Arrow keys move or resize the artboard in small increments without starting a drag.

| Input | Result |
|:-|:-|
| **Arrows** | Move the artboard by 1 px |
| **<kbd>Shift</kbd> Arrows** | Move the artboard by 10 px |
| **<kbd>Alt</kbd> Arrows** | Resize the artboard by moving its bottom/right edges |
| **<kbd>Ctrl</kbd><kbd>Alt</kbd> Arrows** | Resize the artboard by moving its top/left edges |

## Snapping

While creating, moving, or resizing an artboard, it snaps to nearby artboards, layers, grid lines, and alignment guides. The edge midpoints, corners, and center of the artboard are all used as snap points.

## Quick measurement

Press and hold the <kbd>Alt</kbd> key anytime while hovering the pointer over an unselected artboard to display the offset distances, in pixels, between that and the selected artboard.

<!-- TODO: Screenshot of the quick measurement overlay showing distance lines and values between a selected artboard and a hovered layer -->
