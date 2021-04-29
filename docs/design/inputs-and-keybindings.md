# Inputs and keybindings

## Input categories

- Keyboard
	- Modifier keys (<kbd>Ctrl</kbd>, <kbd>Shift</kbd>, <kbd>Alt</kbd>)
	- Letter keys (<kbd>A</kbd>–<kbd>Z</kbd>)
	- Number keys (<kbd>0</kbd>–<kbd>9</kbd>)
	- Left edge keys (<kbd>Escape</kbd>, <kbd>\`</kbd>, <kbd>Tab</kbd>)
	- Right edge keys (<kbd>Backspace</kbd>, <kbd>\\</kbd>, <kbd>Return</kbd>, <kbd>/</kbd>)
	- Space bar (<kbd>⎵</kbd>)
	- Symbol pair keys (<kbd>-</kbd>/<kbd>=</kbd>, <kbd>\[</kbd>/<kbd>\]</kbd>, <kbd>;</kbd>/<kbd>'</kbd>, <kbd>,</kbd>/<kbd>.</kbd>)
	- Function keys (<kbd>F1</kbd>–<kbd>F12</kbd>)
	- Navigation keys (<kbd>Insert</kbd>/<kbd>Delete</kbd>, <kbd>Home</kbd>/<kbd>End</kbd>, <kbd>Page Up</kbd>/<kbd>Page Down</kbd>)
	- Arrow keys (<kbd>↑</kbd>, <kbd>→</kbd>, <kbd>↓</kbd>, <kbd>←</kbd>)
	- Numpad <kbd>Enter</kbd> should be equivalent to <kbd>Ctrl</kbd><kbd>Return</kbd> (<kbd>Enter</kbd>)
	- Other numpad keys should reflect their main keyboard counterparts
- Mouse
	- Cursor movement
	- LMB
	- RMB
	- MMB
	- Vertical scroll wheel up/down
	- Horizontal scroll wheel (less common)
	- Forward navigation (less common)
	- Backward navigation (less common)
- Tablet
	- Hover movement
	- Stroke movement
		- With pressure
		- With tilt
		- With angle (less common)
	- Lower stylus button
	- Upper stylus button
	- Eraser hover movement (less common)
	- Eraser stroke movement (less common)
- Touch
	- Tap
	- Drag
	- Pinch
	- Rotate
	- Multiple fingers (1–4)

## Document-specific commands

- <kbd>Ctrl</kbd><kbd>S</kbd> Save document.
- <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>S</kbd> Save as.
- <kbd>Ctrl</kbd><kbd>E</kbd> Export.

## Panel-specific commands

### Document Panel

#### Viewport navigation

<kbd>Ctrl</kbd><kbd>-</kbd> Zoom out to the next discrete increment.
<kbd>Ctrl</kbd><kbd>=</kbd>/<kbd>Ctrl</kbd><kbd>+</kbd> Zoom in to the next discrete increment.
<kbd>Ctrl</kbd><kbd>0</kbd> Zoom to show the entire canvas.
<kbd>Ctrl</kbd><kbd>`</kbd> Zoom to show the entire selection.
<kbd>Ctrl</kbd><kbd>1</kbd> Zoom to 100% scale.
<kbd>Ctrl</kbd><kbd>2</kbd> Zoom to 200% scale.

#### Selection-specific

- <kbd>H</kbd> Hide/show selection, equivalent to turning off the eye icon on every selected layer
- <kbd>Alt</kbd><kbd>H</kbd> Show hidden direct children of the selection, equivalent to turning on the eye icon on every direct child layer of the selected layers
- <kbd>X</kbd> Delete selection (with confirmation)
- <kbd>Ctrl</kbd><kbd>I</kbd> Invert selected, by applying an Invert node.

#### Masking

- <kbd>Tab</kbd> Enter/exit Mask Mode.

#### Working colors

- <kbd>Shift</kbd><kbd>X</kbd> Swap the primary and secondary working colors.
- <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>X</kbd> Reset the primary and secondary working colors to black and white.

#### Tool shelf

| Tool            | Graphite     | Photoshop                                  | Illustrator                   | XD               | Affinity Designer             | Inkscape | Gimp     |
| --------------- | ------------ | ------------------------------------------ | ----------------------------- | ---------------- | ----------------------------- | -------- | -------- |
Select Tool       | <kbd>V</kbd> | **<kbd>V</kbd>**                           | <kbd>V</kbd>                  | <kbd>V</kbd>     | <kbd>V</kbd>                  |          |          |
Crop Tool         | <kbd> </kbd> | <kbd>C</kbd>                               | <kbd>Shift</kbd><kbd>O</kbd>  | <kbd>A</kbd>     |                               |          |          |
Navigate Tool     | <kbd>Z</kbd> | **<kbd>Z</kbd>**/<kbd>H</kbd>/<kbd>R</kbd> | **<kbd>Z</kbd>**/<kbd>H</kbd> | **<kbd>Z</kbd>** | **<kbd>Z</kbd>**/<kbd>H</kbd> |          |          |
Eyedropper Tool   | <kbd>I</kbd> | **<kbd>I</kbd>**                           | **<kbd>I</kbd>**              |                  | **<kbd>I</kbd>**              |          |          |
Text Tool         | <kbd>T</kbd> | **<kbd>T</kbd>**                           | **<kbd>T</kbd>**              | **<kbd>T</kbd>** | **<kbd>T</kbd>**              |          |          |
Fill Tool         | <kbd>F</kbd> | <kbd>G</kbd>                               |                               |                  | <kbd>G</kbd>                  |          |          |
Gradient Tool     | <kbd>H</kbd> | <kbd>G</kbd>                               | <kbd>G</kbd>                  |                  | <kbd>G</kbd>                  |          |          |
Brush Tool        | <kbd>B</kbd> | **<kbd>B</kbd>**                           | **<kbd>B</kbd>**              |                  | **<kbd>B</kbd>**              |          |          |
Heal Tool         | <kbd>J</kbd> | **<kbd>J</kbd>**                           |                               |                  |                               |          |          |
Clone Tool        | <kbd>C</kbd> | <kbd>S</kbd>                               |                               |                  |                               |          |          |
Patch Tool        | <kbd> </kbd> | <kbd>J</kbd>                               |                               |                  |                               |          |          |
Detail Tool       | <kbd>D</kbd> |                                            |                               |                  |                               |          |          |
Relight Tool      | <kbd>O</kbd> | **<kbd>O</kbd>**                           |                               |                  |                               |          |          |
Path Tool         | <kbd>A</kbd> | **<kbd>A</kbd>**                           | **<kbd>A</kbd>**              |                  | **<kbd>A</kbd>**              |          |          |
Pen Tool          | <kbd>P</kbd> | **<kbd>P</kbd>**                           | **<kbd>P</kbd>**              | **<kbd>P</kbd>** | **<kbd>P</kbd>**              |          |          |
Freehand Tool     | <kbd>N</kbd> | <kbd>P</kbd>                               | **<kbd>N</kbd>**              |                  | **<kbd>N</kbd>**              |          |          |
Spline Tool       | <kbd> </kbd> | <kbd>P</kbd>                               | <kbd>Shift</kbd><kbd>~</kbd>  |                  | <kbd>P</kbd>                  |          |          |
Line Tool         | <kbd>L</kbd> | <kbd>U</kbd>                               | <kbd>\\</kbd>                 | **<kbd>L</kbd>** | <kbd>P</kbd>                  |          |          |
Rectangle Tool    | <kbd>M</kbd> | <kbd>U</kbd>/**<kbd>M</kbd>**              | **<kbd>M</kbd>**              | <kbd>R</kbd>     | **<kbd>M</kbd>**              |          |          |
Ellipse Tool      | <kbd>E</kbd> | <kbd>U</kbd>/<kbd>M</kbd>                  | <kbd>L</kbd>                  | **<kbd>E</kbd>** | <kbd>M</kbd>                  |          |          |
Shape Tool        | <kbd>Y</kbd> | <kbd>U</kbd>                               |                               | **<kbd>Y</kbd>** |                               |          |          |

#### Tool-specific keys

Excluding mouse inputs and modifier keys.

##### Select Tool

- <kbd>G</kbd> Grab (translate) the selected items. Hit X or Y to constrain to that global axis, hit X or Y again to constrain to that local axis. Type a number to move along that axis by that many pixels (`px` or `pixel` or `pixels`), or type a unit suffix.
- <kbd>R</kbd> Rotate the selected items. Type a number to rotate by that many degrees (`°` or `deg` or `degree` or `degrees`), or type a unit suffix (`turn` or `turns` or `rev` or `revs`, `rad` or `radians`, `min` or `minutes` or `'`, `sec` or `seconds` or `"`, `grad`).
- <kbd>S</kbd> Scale the selected items. Hit X or Y to constrain to that global axis, hit X or Y again to constrain to that local axis. Type a number to scale along that axis by that factor (`fac` or `factor`), or type a unit suffix (`%`).

##### Crop Tool

##### Navigate Tool

##### Eyedropper Tool

##### Text Tool

##### Fill Tool

##### Gradient Tool

##### Brush Tool

##### Heal Tool

##### Clone Tool

##### Patch Tool

##### Detail Tool

##### Relight Tool

##### Path Tool

##### Pen Tool

##### Freehand Tool

##### Spline Tool

##### Line Tool

##### Rectangle Tool

##### Ellipse Tool

##### Shape Tool
