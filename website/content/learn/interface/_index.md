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

<p><img src="https://static.graphite.rs/content/learn/interface/title-bar.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The title bar" /></p>

### Menu bar

On the left, the [**menu bar**](./menu-bar) provides quick access to many editor, document, and artwork related controls. Its functions are covered in detail on the next page.

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The menu bar" /></p>

<!-- In the (forthcoming) macOS desktop release, the menu bar is absent from the editor window; its functions are instead located in macOS menu bar. -->

### Document title

In the center, the **document title** displays the name of the active document. That name is given a `*` suffix if the file has unsaved changes. For example, *Painting.graphite** would be unsaved but *Painting.graphite* would have no changes following its last save.

<p><img src="https://static.graphite.rs/content/learn/interface/document-title.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The document title" /></p>

### Window buttons

On the right, the **window buttons** provide platform-specific controls for the application.

<!-- In the (forthcoming) macOS desktop release, this appears on the left side instead. -->

| | |
|-|-|
| **Web** | <p>A button to enter fullscreen mode is displayed.</p><p>The label "*Go fullscreen to access all hotkeys*" indicates that some shortcut keys like <kbd>Ctrl</kbd><kbd>N</kbd> (macOS: <kbd>⌘</kbd><kbd>N</kbd>) are reserved by the web browser and can only be used in fullscreen mode. (An alternative to going fullscreen: include <kbd>Alt</kbd> in the shortcut combinations for browser-reserved hotkeys.)</p><p><img src="https://static.graphite.rs/content/learn/interface/window-buttons-web.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Fullscreen button" /></p> |
<!-- | **Windows<br />& Linux** | The standard window controls are displayed: minimize, maximize/restore down, and close.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/window-buttons-windows-linux.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Minimize/maximize/close window buttons" /> | -->
<!-- | **macOS** | The standard window controls are displayed: close, minimize, and fullscreen. These are located on the left of the title bar.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/window-buttons-macos.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Close/minimize/fullscreen window buttons" /> | -->

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

<p><img src="https://static.graphite.rs/content/learn/interface/status-bar__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Status bar" /></p>

### Input hints

The **input hints** are presently the only occupant of the status bar. They indicate what common keyboard and mouse inputs are valid in the current context. Hints change with each active tool as well as with the current interaction state. Keep a frequent eye on the hints to discover more features as you work.

Hints with a **`+`** mean that adding the indicated modifier key will change the base action. For example: in the following action, dragging with left-click held down will zoom the canvas; then additionally holding the <kbd>Ctrl</kbd> key will make the zoom action snap to whole increments.

<p><img src="https://static.graphite.rs/content/learn/interface/input-hints-plus.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example hint" /></p>

Hints with a **`/`** mean that either indicated input combination can be used to trigger the same action. For example: in the following action, either holding the space bar while dragging with the left mouse button held down, or just dragging with the middle mouse button held down, will both pan around the document in the viewport.

<p><img src="https://static.graphite.rs/content/learn/interface/input-hints-slash.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Example hint" /></p>

The following chart describes each icon representing the mouse inputs you can perform so a hint's prescribed action occurs.

| | Clicks | Drags | Others |
|-|:-:|:-:|:-:|
| | | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Drag icon" /><br style="line-height: 4" />Drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-stationary.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Mouse kept stationary icon" /><br style="line-height: 4" />Stationary |
| **Left<br />mouse<br />button** | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-left-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left click icon" /><br style="line-height: 4" />Left click | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-left-click-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left click drag icon" /><br style="line-height: 4" />Left click drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-left-double-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left double-click icon" /><br style="line-height: 4" />Left double-click |
| **Right<br />mouse<br />button** | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-right-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right click icon" /><br style="line-height: 4" />Right click | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-right-click-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right click drag icon" /><br style="line-height: 4" />Right click drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-right-double-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right double-click icon" /><br style="line-height: 4" />Right double-click |
| **Middle<br />mouse<br />button** | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-middle-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Middle click icon" /><br style="line-height: 4" />Middle click | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-middle-click-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Middle click drag icon" /><br style="line-height: 4" />Middle click drag | <img src="https://static.graphite.rs/content/learn/interface/mouse-icon-scroll.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Scroll up/down icons" /><br style="line-height: 4" />Scroll up/down |
