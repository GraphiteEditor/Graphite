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

In the (forthcoming) macOS desktop release, the menu bar is absent from the editor window; its functions are instead located in macOS menu bar.

### Document title

In the center, the **document title** displays the name of the active document. That name is given a `*` suffix if the file has unsaved changes. For example, *Painting.graphite** would be unsaved but *Painting.graphite* would have no changes since it was last saved.

<p><img src="https://static.graphite.rs/content/learn/interface/document-title.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The document title" /></p>

### Window buttons

On the right, the **window buttons** provide platform-specific controls for the application window. In the (forthcoming) macOS desktop release, this appears on the left side instead.

| | |
|-|-|
| **Web** | A button to enter fullscreen mode is displayed.<br /><br />The label "*Go fullscreen to access all hotkeys*" indicates that some shortcut keys like <kbd>Ctrl</kbd><kbd>N</kbd> (macOS: <kbd>⌘</kbd><kbd>N</kbd>) are reserved by the web browser and can only be used in fullscreen mode. (An alternative to fullscreen mode: include <kbd>Alt</kbd> in the shortcut combinations for browser-reserved hotkeys.)<br /><br /><img src="https://static.graphite.rs/content/learn/interface/window-buttons-web.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Fullscreen button" /> |
| **Windows<br />& Linux** | The standard window controls are displayed: minimize, maximize/restore down, and close.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/window-buttons-windows-linux.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Minimize/maximize/close window buttons" /> |
| **macOS** | The standard window controls are displayed: close, minimize, and fullscreen. These are located on the left of the title bar.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/window-buttons-macos.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Close/minimize/fullscreen window buttons" /> |

## Workspace

The **workspace** is the editor's main content area. It houses the **panels** packed next to one another. The **gutter** lines between neighboring panels may be dragged to resize them.

<p><img src="https://static.graphite.rs/content/learn/interface/workspace__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The workspace" /></p>

### Panels

Panels are regions of the UI dedicated to a specific purpose. [**Document**](./document-panel), [**Properties**](./properties-panel), and [**Layers**](./layers-panel) are presently the three panel types. Each will be covered later in the chapter.

Each panel name is shown in its **panel tab bar**. Panel tabs provide a quick way to swap between multiple panels occupying the same area (currently only documents support this). Down the road, these tabs will be dockable so the default layout may be customized.

Beneath the panel tab bar, the **panel body** displays the content for its panel type. Each will be described in the following pages.

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
| **Left<br />mouse<br />button** | Left click<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-left-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left click icon" /> | Left click drag<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-left-click-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left click drag icon" /> | Left double-click<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-left-double-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Left double-click icon" /> |
| **Right<br />mouse<br />button** | Right click<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-right-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right click icon" /> | Right click drag<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-right-click-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right click drag icon" /> | Right double-click<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-right-double-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Right double-click icon" /> |
| **Middle<br />mouse<br />button** | Middle click<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-middle-click.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Middle click icon" /> | Middle click drag<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-middle-click-drag.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Middle click drag icon" /> | Scroll up/down<br /><br /><img src="https://static.graphite.rs/content/learn/interface/mouse-input-scroll-up.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Scroll up icon" /> <img src="https://static.graphite.rs/content/learn/interface/mouse-input-scroll-down.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Scroll down icon" /> |
