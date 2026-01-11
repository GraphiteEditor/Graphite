+++
title = "Fill"

[extra]
order = 2
css = ["/page/user-manual/node.css"]
+++

Applies a fill style to the vector content, giving an appearance to the area within the interior of the geometry.

## Interface

### Inputs

| Parameter | Details | Possible Types |
|:-|:-|:-|
| Content | <p>The content with vector paths to apply the fill style to.</p><p><em>Primary Input</em></p> | `Table<Graphic>`<br />`Table<Vector>` |
| Fill | <p>The fill to paint the path with.</p><p><em>Default:&nbsp;<span style="padding-right: 100px; background: black; border: 2px solid var(--color-fog)"></span></em></p> | `Fill`<br />`Gradient`<br />`Table<Color>`<br />`Table<GradientStops>` |

### Outputs

| Product | Details | Possible Types |
|:-|:-|:-|
| Result | <p>The vector content with the fill style applied.</p><p><em>Primary Output</em></p> | `Table<Vector>`<br />`Table<Graphic>` |
