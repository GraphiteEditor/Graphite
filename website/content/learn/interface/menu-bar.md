+++
title = "Menu bar"

[extra]
order = 1
+++

The **menu bar** is the series of menus running across the top left of the editor's [**title bar**](../#title-bar). It provides organized access to many actions which are described on this page.

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The menu bar" /></p>

Clicking **File**, **Edit**, **Layer**, **Document**, **View**, and **Help** opens a dropdown menu with clickable actions. Pay attention to the keyboard shortcut listed on the right of each row in the dropdown menus. Learning to use them can help speed up your workflow.

The rest of this page is intended as a reference resource. Skip ahead to the next page if this is your first read-through of the manual.

## App button

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-app-button.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The app button" /></p>

The **app button** shows the [Graphite logo](/logo). Clicking it opens the Graphite website [home page](/).

## File

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-file.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The File menu" /></p>

The **File menu** lists actions related to file handling:

| | |
|-|-|
| New… | Opens the **New Document** dialog for creating a blank canvas in a new editor tab.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/menu-bar/dialog-new-document.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'New Document' dialog" /> |
| Open… | Opens the operating system file picker dialog for selecting a `.graphite` file from disk to be opened in a new editor tab. |
| Open Demo Artwork… | Opens the **Demo Artwork** dialog for loading a choice of premade sample artwork files provided for you to explore. Click the button below each image to open it.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/menu-bar/dialog-demo-artwork.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'Demo Artwork' dialog" /> |
| Close | Closes the active document. If it has unsaved changes (denoted by the `*` after the file name), you will be asked to save or discard the changes. |
| Close All | Closes all open documents. To avoid accidentally losing unsaved work, you will be asked to confirm that you want to proceed which will discard the unsaved changes in all open documents. |
| Save | Saves the active document by writing the `.graphite` file to disk. An operating system file download dialog may appear asking where to place it. That dialog will provide an opportunity to save over a previous version of the file, if you wish, by picking the identical name instead of saving another instance with a number after it. |
| Import… | Opens the operating system file picker dialog for selecting an image file from disk to be placed as a new bitmap image layer into the active document. |
| Export… | Opens the **Export** dialog for saving the artwork as a *File Type* of *PNG*, *JPG*, or *SVG*. *Scale Factor* multiplies the content's document scale, so a value of 2 would export 300x400 content as 600x800 pixels. *Bounds* picks what area to render: *All Artwork* uses the bounding box of all layers, *Selection* uses the bounding box of the currently selected layers, and an *Artboard: \[Name\]* uses the bounds of that artboard. *Transparency* exports the PNG or SVG file with transparency instead of the artboard background color.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/menu-bar/dialog-export.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'Export' dialog" /> |
| Preferences… | Opens the **Editor Preferences** dialog for configuring Graphite's settings.<br /><br /><img src="https://static.graphite.rs/content/learn/interface/menu-bar/dialog-editor-preferences.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'Editor Preferences' dialog" /> |

## Edit

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-edit.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The Edit menu" /></p>

The **Edit menu** lists actions related to the editing workflow:

| | |
|-|-|
| Undo | Steps back in the history of changes in the active document. |
| Redo | Steps forward in the history of changes in the active document. |
| Cut | Copies the selected layer(s) to the clipboard, then deletes them. |
| Copy | Copies the selected layer(s) to the clipboard. |
| Paste | Pastes the copied layer(s) from the clipboard into the document. It will end up beside a selected layer or inside a selected folder, or otherwise at the base of the folder structure.<br /><br />In the web version of Graphite, your browser will ask for permission to read from your clipboard which you must grant; using the hotkey <kbd>Ctrl</kbd><kbd>V</kbd> (macOS: <kbd>⌘</kbd><kbd>V</kbd>) works without browser permission. |

## Layer

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-layer.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The Layer menu" /></p>

The **Layer menu** lists actions related to the layers within a document:

| | |
|-|-|
| Select All | Selects all layers and folders in the document. |
| Deselect All | Deselects everything in the document. |
| Delete Selected | Removes all selected layers and folders. |
| Grab Selected | Begin grabbing the selected layer(s) to translate (move) them around with your cursor's movement. Lock to an axis with <kbd>X</kbd> or <kbd>Y</kbd> then use the number keys to type a pixel distance value. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>. |
| Rotate Selected | Begin rotating the selected layer(s) around their pivot point with your cursor's movement. Use the number keys to type an angle value in degrees. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>. |
| Scale Selected | Begin scaling the selected layer(s) around their pivot point with your cursor's movement. Lock to an axis with <kbd>X</kbd> or <kbd>Y</kbd>. Use the number keys to type a scale multiplier value. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>. |
| Order ><br />Raise to Front | Reorders the selected layer(s) above all other layers within their same folder(s), so they appear in the layer stack and render above those other layers. |
| Order ><br />Raise | Reorders the selected layers(s) up by one in the layer stack, so any layer that was immediately above the selected layer(s) ends up immediately below. |
| Order ><br />Lower | Reorders the selected layers(s) down by one in the layer stack, so any layer that was immediately below the selected layer(s) ends up immediately above. |
| Order ><br />Lower to Back | Reorders the selected layer(s) below all other layers within their same folder(s), so they appear in the layer stack and render below those other layers. |

## Document

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-document.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The Document menu" /></p>

The **Document menu** lists actions related to the document and artwork:

| | |
|-|-|
| Clear Artboards | Removes all artboards from the document, thus enabling an infinite canvas. |

## View

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-view.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The View menu" /></p>

The **View menu** lists actions related to the view of the canvas and viewport:

| | |
|-|-|
| Tilt | Begins tilting the viewport angle based on your mouse movements. Hold <kbd>Ctrl</kbd> to snap to 15° increments. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>. |
| Reset Tilt | Sets the viewport tilt angle back to 0°. |
| Zoom In | Narrows the view to the next whole zoom increment. |
| Zoom Out | Widens the view to the next whole zoom increment. |
| Zoom to Fit Selection | Zooms and frames the viewport to the bounding box of the selected layer(s). |
| Zoom to Fit All | Zooms and frames the viewport to fit all artboards, or all artwork if using infinite canvas. |
| Zoom to 100% | Zooms the viewport in or out to 100% scale, matching 1:1 the scale of the document and viewport. |
| Zoom to 200% | Zooms the viewport in or out to 200% scale, displaying the artwork at twice the actual size. |
| Rulers | Toggles visibility of the rulers shown along the top and left edges of the viewport. |

## Help

<p><img src="https://static.graphite.rs/content/learn/interface/menu-bar/menu-help.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The Help menu" /></p>

The **Help menu** lists actions related to information about Graphite:

| | |
|-|-|
| About Graphite… | Opens the **About Graphite** dialog for displaying release and license information. |
| User Manual | Opens this [user manual](./learn). |
| Report a Bug | Opens a page to file a [new GitHub issue](https://github.com/GraphiteEditor/Graphite/issues/new). |
| Visit on GitHub | Opens the [Graphite GitHub repository](https://github.com/GraphiteEditor/Graphite). |
| *Debug section* | Developer-only actions. |
