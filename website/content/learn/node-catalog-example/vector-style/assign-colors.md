+++
title = "Assign Colors"

[extra]
order = 1
css = ["/page/user-manual/node.css"]
+++

Uniquely sets the fill and/or stroke style of every vector element to individual colors sampled along a chosen gradient.

## Interface

### Inputs

| Parameter | Details | Possible Types |
|:-|:-|:-|
| Content | <p>The content with vector paths to apply the fill and/or stroke style to.</p><p><em>Primary Input</em></p> | `Table<Graphic>`<br />`Table<Vector>` |
| Fill | <p>Whether to style the fill.</p><p><em>Default: `true`</em></p> | `bool` |
| Stroke | <p>Whether to style the stroke.</p><p><em>Default: `false`</em></p> | `bool` |
| Gradient | <p>The range of colors to select from.</p><p><em>Default:&nbsp;<span style="padding-right: 100px; background: linear-gradient(to right, black, white); border: 2px solid var(--color-fog)"></span></em></p> | `GradientStops` |
| Reverse | <p>Whether to reverse the gradient.</p><p><em>Default: `false`</em></p> | `bool` |
| Randomize | <p>Whether to randomize the color selection for each element from throughout the gradient.</p><p><em>Default: `false`</em></p> | `bool` |
| Seed | <p>The seed used for randomization.</p><p>Seed to determine unique variations on the randomized color selection.</p><p><em>Default: `0`</em></p> | `SeedValue` |
| Repeat Every | <p>The number of elements to span across the gradient before repeating. A 0 value will span the entire gradient once.</p><p><em>Default: `0`</em></p> | `u32` |

### Outputs

| Product | Details | Possible Types |
|:-|:-|:-|
| Result | <p>The vector content with the fill and/or stroke styles applied.</p><p><em>Primary Output</em></p> | `Table<Graphic>`<br />`Table<Vector>` |
