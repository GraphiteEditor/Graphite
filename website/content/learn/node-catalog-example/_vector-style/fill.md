+++
title = "Fill"

[extra]
order = 1
+++

<style>
table tr td:last-child code {
	white-space: nowrap;
}
</style>

Applies a fill style to the vector content, giving an appearance to the area within the interior of the geometry.

### Inputs

| Parameter | Details | Possible Types |
|:-|:-|:-|
| Content | <p>The content with vector paths to apply the fill style to.</p><p><em>Primary Input</em></p> | `Table<Vector>`<br />`Table<Graphic>` |
| Fill | <p>The fill to paint the path with.</p><p><em>Default: <span style="padding-right: 100px; background: black; border: 2px solid var(--color-fog)">&nbsp;</span></em></p> | `Fill`<br />`Table<Color>`<br />`Table<GradientStops>`<br />`Gradient` |

### Outputs

| Product | Details | Possible Types |
|:-|:-|:-|
| Result | <p>The vector content with the fill style applied.</p><p><em>Primary Output</em></p> | `Table<Vector>`<br />`Table<Graphic>` |

### Context

Not context-aware.
