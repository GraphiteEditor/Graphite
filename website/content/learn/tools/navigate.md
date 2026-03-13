+++
title = "Navigate tool"
template = "book.html"
page_template = "book.html"

[extra]
order = 3
+++

The **Navigate tool** provides dedicated controls for zooming and tilting the view of the canvas within the viewport. Most of these navigation actions are also available as global shortcuts that work while any other tool is active, which are documented as "Any tool" alongside the tool-specific controls below. Activate it by clicking its icon in the tool shelf or pressing <kbd>Z</kbd> (for "zoom").

## Panning

Panning shifts the visible portion of the canvas within the viewport.

### Free panning

| Input | Result | Any tool |
|:-|:-|:-:|
| **<kbd>Space</kbd> Drag**<br /><br />**Middle-Click Drag**<br /><br />**Trackpad Scroll Gesture** | Pan the canvas | ✓ |

### Directional scroll panning

| Input | Result | Any tool |
|:-|:-|:-:|
| **Scroll Wheel** | Pan vertically<br />(default, see [preference](#scroll-wheel-preference)) | ✓ |
| **<kbd>Shift</kbd> Scroll Wheel** | Pan horizontally | ✓ |

### Paginated scroll panning

| Input | Result | Any tool |
|:-|:-|:-:|
| **<kbd>Page Up</kbd>** | (↑) Pan up by one viewport height | ✓ |
| **<kbd>Page Down</kbd>** | (↓) Pan down by one viewport height | ✓ |
| **<kbd>Shift</kbd> <kbd>Page Up</kbd>** | (←) Pan left by one viewport width | ✓ |
| **<kbd>Shift</kbd> <kbd>Page Down</kbd>** | (→) Pan right by one viewport width | ✓ |

## Zooming

Zooming magnifies or reduces the visible portion of the canvas within the viewport.

### Free zooming

| Input | Result | Any tool |
|:-|:-|:-:|
| **<kbd>Ctrl</kbd> Scroll Wheel<br /><br />Trackpad Pinch Gesture** | Zoom toward or away from the pointer<br />(default, see [preference](#scroll-wheel-preference)) | ✓ |

### Step zooming

| Input | Result | Any tool |
|:-|:-|:-:|
| **Click** | Zoom in a step toward the pointer | |
| **<kbd>Shift</kbd> Click** | Zoom out a step from the pointer | |
| **<kbd>Ctrl</kbd><kbd>+</kbd><br />(macOS: <kbd>⌘</kbd><kbd>+</kbd>)** | Zoom in a step toward the viewport center | ✓ |
| **<kbd>Ctrl</kbd><kbd>-</kbd><br />(macOS: <kbd>⌘</kbd><kbd>-</kbd>)** | Zoom out a step from the viewport center | ✓ |

### Smooth zooming

| Input | Result | Any tool |
|:-|:-|:-:|
| **Drag** | Drag up to zoom in or down to zoom out | |
| **<kbd>Ctrl</kbd> Middle-Click Drag** | Drag up to zoom in or down to zoom out | ✓ |

Hold <kbd>Shift</kbd> to snap to preset zoom levels while dragging.

### Preset zooming

| Input | Result | Any tool |
|:-|:-|:-:|
| **<kbd>.</kbd>** | Fit the current selection in the viewport | ✓ |
| **<kbd>Ctrl</kbd><kbd>0</kbd>** | Fit the entire document in the viewport center | ✓ |
| **<kbd>Ctrl</kbd><kbd>1</kbd><br />(macOS: <kbd>⌘</kbd><kbd>1</kbd>)** | Zoom to 100% | ✓ |
| **<kbd>Ctrl</kbd><kbd>2</kbd><br />(macOS: <kbd>⌘</kbd><kbd>2</kbd>)** | Zoom to 200% | ✓ |

#### Scroll wheel preference

By default, the scroll wheel pans the canvas and <kbd>Ctrl</kbd> Scroll zooms. The *Zoom with Scroll* setting located in *File* > *Preferences* lets you swap this so the scroll wheel zooms and <kbd>Ctrl</kbd> Scroll pans vertically. This is not recommended if a trackpad is in use because it swaps two-finger scroll and pinch gestures.

#### Zoom rate preference

The *Zoom Rate* setting located in *File* > *Preferences* controls the speed of magnification while zooming with the scroll wheel (or trackpad scroll or pinch gesture). Relative to a default of 50, lower values reduce the zoom speed while higher values increase it.

## Tilting

Tilting rotates the entire canvas within the viewport. This does not rotate the actual document content, just the current view of it.

| Input | Result | Any tool |
|:-|:-|:-:|
| **<kbd>Alt</kbd> Drag** | Tilt the canvas | |
| **<kbd>Alt</kbd> Middle-Click Drag**<br /><br />**<kbd>Alt</kbd> <kbd>Space</kbd> Drag** | Tilt the canvas | ✓ |

Hold <kbd>Shift</kbd> to snap to 15° increments while dragging.

## Flipping

Flipping reflects the entire canvas within the viewport. Like tilting, this does not affect the actual document content, and it can be toggled off anytime to return to the normal view.

Use *View* > *Flip* to activate the reflected view mode. While active, an icon to un-flip appears beside the viewport zoom percentage at the right of the control bar. This works with any tool active.
