+++
title = "Interface"
template = "book.html"
page_template = "book.html"

[extra]
order = 2
+++

This chapter formally introduces the concepts and terminology for the user interface (UI) of the Graphite editor. You may skip to the next chapter if you're familiar with the general layout and terms used in industry-standard graphics editors.

## Title bar

The bar running across the top of the editor is called the **title bar**. In the (forthcoming) desktop release of Graphite, this acts as the draggable window frame.

<!-- <p><img src="https://static.graphite.rs/content/learn/interface/title-bar.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The title bar" /></p> -->

### Menu bar

On the left, the [**menu bar**](./menu-bar) provides quick access to many editor, document, and artwork related controls. Its functions are covered in detail on the next page.

<!-- <p><img src="https://static.graphite.rs/content/learn/interface/menu-bar.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The menu bar" /></p> -->

<!-- In the (forthcoming) macOS desktop release, the menu bar is absent from the editor window; its functions are instead located in macOS menu bar. -->

### Window buttons

On the right (and on Mac, the left), the **window buttons** provide platform-specific controls for the application.

<!-- In the (forthcoming) macOS desktop release, this appears on the left side instead. -->

| | |
|-|-|
| **Web** | <p>Fullscreen:</p><p><img src="https://static.graphite.rs/content/learn/interface/window-buttons-web__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Fullscreen button" /></p> |
| **Windows** | <p>Minimize, maximize/restore down, close:</p><p><img src="https://static.graphite.rs/content/learn/interface/window-buttons-windows.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Minimize/maximize/close window buttons" /></p> |
| **Linux** | <p>Minimize, maximize/unmaximize, close:</p><p><img src="https://static.graphite.rs/content/learn/interface/window-buttons-linux.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Minimize/maximize/close window buttons" /></p> |
| **macOS** | <p>Close, minimize, fullscreen:</p><p><img src="https://static.graphite.rs/content/learn/interface/window-buttons-macos.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Close/minimize/fullscreen window buttons" /></p> |

## Workspace

The **workspace** is the editor's main content area, filled with **panels** arranged next to one another. The **gutter** lines, located between neighboring panels, may be dragged to resize them.

<p><img src="https://static.graphite.rs/content/learn/interface/workspace__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The workspace" /></p>

### Panels

Panels are regions of the UI dedicated to a specific purpose. [**Document**](./document-panel), Properties, and Layers are presently the three panel types.

Each panel name is shown in its **panel header**. Panel tabs offer a quick way to swap between multiple panels occupying the same area (currently only documents support this).

Down the road, these tabs will be dockable so the default layout may be customized.

Beneath the panel header, the **panel content** displays the content for its panel type. Each will be described in the following pages.

## Status bar

The bar running across the bottom of the editor is called the **status bar**.

<p><img src="https://static.graphite.rs/content/learn/interface/status-bar__2.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Status bar" /></p>

### Input hints

The **input hints** are presently the only occupant of the status bar. They indicate what common keyboard and mouse inputs are valid in the current context. Hints change with each active tool as well as with the current interaction state. Keep a frequent eye on the hints to discover more features as you work.

Hints with a **`+`** mean that adding the indicated modifier key will change the base action. For example: in the following action, dragging with left-click held down will zoom the canvas; then additionally holding the <kbd>Ctrl</kbd> key will make the zoom action snap to whole increments.

<p><img src="https://static.graphite.rs/content/learn/interface/input-hints-plus.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example hint" /></p>

Hints with a **`/`** mean that either indicated input combination can be used to trigger the same action. For example: in the following action, either holding the space bar while dragging with the left mouse button held down, or just dragging with the middle mouse button held down, will both pan around the document in the viewport.

<p><img src="https://static.graphite.rs/content/learn/interface/input-hints-slash.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example hint" /></p>

The following chart describes each icon representing the mouse inputs you can perform so a hint's prescribed action occurs.

| | Clicks | Drags | Others |
|-|:-:|:-:|:-:|
| **Left<br />mouse<br />button** | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-left-click.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left click icon" /><br /><br />Left click | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-left-click-drag.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left click drag icon" /><br /><br />Left click drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-left-double-click.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left double-click icon" /><br /><br />Left double-click |
| **Right<br />mouse<br />button** | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-right-click.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right click icon" /><br /><br />Right click | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-right-click-drag.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right click drag icon" /><br /><br />Right click drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-right-double-click.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right double-click icon" /><br /><br />Right double-click |
| **Middle<br />mouse<br />button** | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-middle-click.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Middle click icon" /><br /><br />Middle click | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-middle-click-drag.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Middle click drag icon" /><br /><br />Middle click drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-scroll.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Scroll up/down icons" /><br /><br />Scroll up/down |
| **No<br />mouse<br />button** | | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-drag.avif" onload="this.width = this.naturalWidth / 2" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Drag icon" /><br /><br />Drag | |
