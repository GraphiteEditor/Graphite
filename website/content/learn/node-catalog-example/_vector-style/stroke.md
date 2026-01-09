+++
title = "Stroke"

[extra]
order = 2
+++

<style>
table tr td:last-child code {
	white-space: nowrap;
}
</style>

Applies a stroke style to the vector content, giving an appearance to the area within the outline of the geometry.

### Inputs

| Parameter | Details | Possible Types |
|:-|:-|:-|
| Content | <p>The content with vector paths to apply the stroke style to.</p><p><em>Primary Input</em></p> | `Table<Vector>`<br />`Table<Graphic>` |
| Color | <p>The stroke color.</p><p><em>Default: <span style="padding-right: 100px; background: black; border: 2px solid var(--color-fog)">&nbsp;</span></em></p> | `Table<Color>` |
| Weight | <p>The stroke thickness.</p><p><em>Default: `2 px`</em></p> | `f64` |
| Align | <p>The alignment of stroke to the path's centerline or (for closed shapes) the inside or outside of the shape.</p><p><em>Default: `Center`</em></p> | `StrokeAlign` |
| Cap | <p>The shape of the stroke at open endpoints.</p><p><em>Default: `Butt`</em></p> | `StrokeCap` |
| Join | <p>The curvature of the bent stroke at sharp corners.</p><p><em>Default: `Miter`</em></p> | `StrokeJoin` |
| Miter Limit | <p>The threshold for when a miter-joined stroke is converted to a bevel-joined stroke when a sharp angle becomes pointier than this ratio.</p><p><em>Default: `4`</em></p> | `f64` |
| Paint Order | <p>The order to paint the stroke on top of the fill, or the fill on top of the stroke.</p><p><em>Default: `StrokeAbove`</em></p> | `PaintOrder` |
| Dash Lengths | <p>The stroke dash lengths. Each length forms a distance in a pattern where the first length is a dash, the second is a gap, and so on. If the list is an odd length, the pattern repeats with solid-gap roles reversed.</p><p><em>Default: `[]`</em></p> | `Vec<f64>`<br />`f64`<br />`String` |
| Dash Offset | <p>The phase offset distance from the starting point of the dash pattern.</p><p><em>Default: `0 px`</em></p> | `f64` |

### Outputs

| Product | Details | Possible Types |
|:-|:-|:-|
| Result | <p>The vector content with the stroke style applied.</p><p><em>Primary Output</em></p> | `Table<Vector>`<br />`Table<Graphic>` |

### Context

Not context-aware.
