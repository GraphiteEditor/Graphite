+++
title = "Menu bar"

[extra]
order = 1
+++

The **menu bar** is the series of menus running across the top left of the editor's [**title bar**](../#title-bar). It provides organized access to many actions which are described on this page.

Clicking **File**, **Edit**, **Layer**, **Select**, **View**, **Window**, and **Help** opens a dropdown menu with clickable actions. Pay attention to the keyboard shortcut listed on the right of each row in the dropdown menus. Learning to use them can help speed up your workflow.

## Menu actions reference

The rest of this page is intended as a reference resource. Skip ahead to the next page if this is your first read-through of the manual.

### App button

The **app button** appears as the Graphite [logo](/logo). Clicking it opens the website [home page](/).

### File

The **File menu** lists actions related to file handling:

| | |
|-|-|
| **New…** | <p>Opens the **New Document** dialog for creating a blank canvas in a new editor tab.</p><p><img src="https://static.graphite.art/content/learn/interface/menu-bar/dialog-new-document.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'New Document' dialog" /></p><p><ul><li><strong>Name</strong> determines the initial filename of the new document.</li><li><strong>Infinite Canvas</strong>, if set, skips adding an artboard and thereby starts with a boundless white canvas extending in all directions.</li><li><strong>Dimensions</strong> sets the width and height, in pixels, of the initial artboard. Ignored if *Infinite Canvas* is ticked.</li></ul></p> |
| **Open…** | <p>Opens the operating system file picker dialog for selecting a `.graphite` file from disk to be opened in a new editor tab.</p> |
| **Open Demo Artwork…** | <p>Opens the **Demo Artwork** dialog for loading a choice of premade sample artwork files provided for you to explore. Click the button below each image to open it.</p><p><img src="https://static.graphite.art/content/learn/interface/menu-bar/dialog-demo-artwork__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'Demo Artwork' dialog" /></p> |
| **Close** | <p>Closes the active document. If it has unsaved changes (denoted by the `*` after the file name), you will be asked to save or discard the changes.</p> |
| **Close All** | <p>Closes all open documents. To avoid accidentally losing unsaved work, you will be asked to confirm that you want to proceed which will discard the unsaved changes in all open documents.</p> |
| **Save** | <p>Saves the active document by writing the `.graphite` file to disk. An operating system file download dialog may appear asking where to place it. That dialog will provide an opportunity to save over a previous version of the file, if you wish, by picking the identical name instead of saving another instance with a number after it.</p> |
| **Import…** | <p>Opens the operating system file picker dialog for selecting an image file from disk to be placed as a new bitmap image layer or SVG content into the active document.</p> |
| **Export…** | <p>Opens the **Export** dialog for saving the artwork as a *File Type* of *PNG*, *JPG*, or *SVG*. *Scale Factor* multiplies the content's document scale, so a value of 2 would export 300x400 content as 600x800 pixels. *Bounds* picks what area to render: *All Artwork* uses the bounding box of all layers, *Selection* uses the bounding box of the currently selected layers, and an *Artboard: \[Name\]* uses the bounds of that artboard. *Transparency* exports PNG or SVG files with transparency instead of the artboard background color.<br /><br /><img src="https://static.graphite.art/content/learn/interface/menu-bar/dialog-export.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'Export' dialog" /></p> |
| **Preferences…** | <p>Opens the **Editor Preferences** dialog for configuring Graphite's settings.<br /><br /><img src="https://static.graphite.art/content/learn/interface/menu-bar/dialog-editor-preferences__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="The 'Editor Preferences' dialog" /></p> |

### Edit

The **Edit menu** lists actions related to the editing workflow:

| | |
|-|-|
| **Undo** | <p>Steps back in the history of changes in the active document.</p> |
| **Redo** | <p>Steps forward in the history of changes in the active document.</p> |
| **Cut** | <p>Copies the selected layer(s) to the clipboard, then deletes them.</p> |
| **Copy** | <p>Copies the selected layer(s) to the clipboard.</p> |
| **Paste** | <p>Pastes the copied layer(s) from the clipboard into the document. It will end up directly above the selected layer, or otherwise at the base of the folder structure.</p><p>In the web version of Graphite, your browser will ask for permission to read from your clipboard which you must grant; alternatively, using the hotkey <kbd>Ctrl</kbd><kbd>V</kbd> (macOS: <kbd>⌘</kbd><kbd>V</kbd>) works without the browser needing this permission.</p> |
| **Duplicate** | <p>Creates a copy of the selected layer(s) directly above their original(s) in the layer stack.</p> |
| **Delete** | <p>Removes all selected layers and folders.</p> |
| **Convert to Infinite Canvas** | <p>Replaces all artboards in the document with standard layers. With no artboards present, the document becomes an infinite canvas.</p> |

### Layer

The **Layer menu** lists actions related to the layers within a document:

| | |
|-|-|
| **New** | <p>Creates a new layer in the active document. It will end up directly above the selected layer, or otherwise at the base of the folder structure.</p> |
| **Group** | <p>Creates a new folder in place of the selected layer(s), then moves them into that folder.</p> |
| **Ungroup** | <p>Removes the selected folder(s), moving their contents up one level in the layer stack.</p> |
| **Hide/Show** | <p>Toggles visibility of the selected layer(s), including or excluding them from rendering as part of the artwork.</p> |
| **Lock/Unlock** | <p>Toggles the locked state of the selected layer(s), preventing them from being selected by tools in the viewport.</p> |
| **Grab** | <p>Begin grabbing the selected layer(s) to translate (move) them around with your cursor's movement. Lock to an axis with <kbd>X</kbd> or <kbd>Y</kbd> then use the number keys to type a pixel distance value. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>.</p> |
| **Rotate** | <p>Begin rotating the selected layer(s) around their pivot point with your cursor's movement. Use the number keys to type an angle value in degrees. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>.</p> |
| **Scale** | <p>Begin scaling the selected layer(s) around their pivot point with your cursor's movement. Lock to an axis with <kbd>X</kbd> or <kbd>Y</kbd>. Use the number keys to type a scale multiplier value. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>.</p> |
| **Arrange ><br />Raise to Front** | <p>Reorders the selected layer(s) above all others within their same folder(s), so they appear in the layer stack and render above those other layers.</p> |
| **Arrange ><br />Raise** | <p>Reorders the selected layers(s) up by one in the layer stack, so any layer that was immediately above the selected layer(s) ends up immediately below.</p> |
| **Arrange ><br />Lower** | <p>Reorders the selected layers(s) down by one in the layer stack, so any layer that was immediately below the selected layer(s) ends up immediately above.</p> |
| **Arrange ><br />Lower to Back** | <p>Reorders the selected layer(s) below all others within their same folder(s), so they appear in the layer stack and render below those other layers.</p> |
| **Arrange ><br />Reverse** | <p>Reorders the selected layers by swapping their positions from top to bottom. Reversal applies amongst each set of selected sibling layers (those with a shared parent).</p> |
| **Align ><br />Align Left** | <p>Moves the selected layer(s) so their left edges line up with the leftmost edge of the selection's bounding box.</p> |
| **Align ><br />Align Horizontal Center** | <p>Moves the selected layer(s) so their horizontal centers line up with the horizontal center of the selection's bounding box.</p> |
| **Align ><br />Align Right** | <p>Moves the selected layer(s) so their right edges line up with the rightmost edge of the selection's bounding box.</p> |
| **Align ><br />Align Top** | <p>Moves the selected layer(s) so their top edges line up with the topmost edge of the selection's bounding box.</p> |
| **Align ><br />Align Vertical Center** | <p>Moves the selected layer(s) so their vertical centers line up with the vertical center of the selection's bounding box.</p> |
| **Align ><br />Align Bottom** | <p>Moves the selected layer(s) so their bottom edges line up with the bottommost edge of the selection's bounding box.</p> |
| **Flip ><br />Flip Horizontal** | <p>Reflects the selected layer(s) horizontally within the selection's bounding box.</p> |
| **Flip ><br />Flip Vertical** | <p>Reflects the selected layer(s) vertically within the selection's bounding box.</p> |
| **Turn ><br />Turn -90°** | <p>Rotates the selected layer(s) a quarter turn counterclockwise about the selection's bounding box center.</p> |
| **Turn ><br />Turn 90°** | <p>Rotates the selected layer(s) a quarter turn clockwise about the selection's bounding box center.</p> |
| **Boolean ><br />Union** | <p>Combines all paths of the selected vector layer(s) while cutting out overlapping areas (even the interiors of a single path)
| **Boolean ><br />Subtract Front** | <p>Cuts overlapping areas out from the last of the selected vector layers.</p> |
| **Boolean ><br />Subtract Back** | <p>Cuts overlapping areas out from the first of the selected vector layers.</p> |
| **Boolean ><br />Intersect** | <p>Cuts away all but the overlapping areas shared by every path of the selected vector layer(s).</p> |
| **Boolean ><br />Difference** | <p>Cuts away the overlapping areas shared by every path of the selected vector layer(s), leaving only the non-overlapping areas.</p> |
| **Make Path Editable** | <p>Applies a path edit operation (the **Path node**) to the selected vector layer, capturing the geometry after other nondestructive operations to enable its direct modification by the **Path** and **Pen** tools.</p> |

### Select

The **Select menu** lists actions related to the selection of layers within a document:

| | |
|-|-|
| **Select All** | <p>Selects all layers and folders in the document.</p> |
| **Deselect All** | <p>Deselects everything in the document.</p> |
| **Select Parent** | <p>Selects the parent folder(s) of the currently selected layer(s).</p> |
| **Previous Selection** | <p>Goes back to the previously selected set of layers or nodes in the selection history.</p><p>If the side of your mouse has navigation buttons, you can use the back button as a shortcut (not supported in Firefox).</p> |
| **Next Selection** | <p>Goes forward to the next selected set of layers or nodes in the selection history.</p><p>If the side of your mouse has navigation buttons, you can use the forward button as a shortcut (not supported in Firefox).</p> |

### View

The **View menu** lists actions related to the view of the canvas within the viewport:

| | |
|-|-|
| **Tilt** | <p>Begins tilting the viewport angle based on your mouse movements.</p><p>While tilting, hold <kbd>Shift</kbd> to snap to 15° increments. Confirm with a left click or <kbd>Enter</kbd>. Cancel with a right click or <kbd>Esc</kbd>.</p> |
| **Reset Tilt** | <p>Sets the viewport tilt angle back to 0°.</p> |
| **Zoom In** | <p>Narrows the view to the next whole zoom increment, such as:</p><p>25%, 33.33%, 40%, 50%, 66.67%, 80%, 100%, 125%, 160%, 200%, 250%, 320%, 400%, 500%</p> |
| **Zoom Out** | <p>Widens the view to the next whole zoom increment, such as above.</p> |
| **Zoom to Selection** | <p>Zooms and frames the viewport to the bounding box of the selected layer(s).</p> |
| **Zoom to Fit** | <p>Zooms and frames the viewport to fit all artboards, or all artwork if using infinite canvas.</p> |
| **Zoom to 100%** | <p>Zooms the viewport in or out to 100% scale, making the document and viewport scales match 1:1.</p> |
| **Zoom to 200%** | <p>Zooms the viewport in or out to 200% scale, displaying the artwork at twice the actual size.</p> |
| **Flip** | <p>Mirrors the viewport horizontally, flipping the view of the artwork until deactivated.</p> |
| **Rulers** | <p>Toggles visibility of the rulers along the top/left edges of the viewport.</p> |

### Window

The **Window menu** lists actions related to the visibility of workspace panels within the application window:

| | |
|-|-|
| **Properties** | <p>Toggles visibility of the **Properties panel** on the upper-right side of the workspace. It is used to inspect and edit the values of graphics operation (node) parameters. Selected layers or nodes display their parametric controls in this panel.</p> |
| **Layers** | <p>Toggles visibility of the **Layers panel** on the lower-right side of the workspace. It is used to organize and select the artboards and layers that form the structure of a document.</p> |
| **Data** | <p>Toggles visibility of the **Data panel** on the lower-left side of the workspace. It is used to introspect data flow from the output of a selected node for technical debugging of content generated within the node graph.</p> |

### Help

The **Help menu** lists actions related to information about Graphite:

| | |
|-|-|
| **About Graphite…** | <p>Opens the **About Graphite** dialog for displaying release and license information. You can check it for the release date of the current editor version.</p> |
| **User Manual** | <p>Opens this [user manual](./learn).</p> |
| **Donate to Graphite** | <p>Opens the Graphite [development fund](/donate) page where you can contribute financially to support ongoing development of the project.</p> |
| **Report a Bug** | <p>Opens a page to file a [new GitHub issue](https://github.com/GraphiteEditor/Graphite/issues/new).</p> |
| **Visit on GitHub** | <p>Opens the [Graphite GitHub repository](https://github.com/GraphiteEditor/Graphite).</p> |
| **Developer Debug** | <p>A section with developer-only actions. Users should ignore these.</p> |
