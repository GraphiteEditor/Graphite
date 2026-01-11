+++
title = "Vector: Style"
template = "book.html"
page_template = "book.html"

[extra]
order = 1
css = ["/page/user-manual/node-category.css"]
+++

Nodes in this category apply styling effects to vector graphics, such as controlling stroke (outline) and fill properties.

## Nodes

| Node | Details | Possible Types |
|:-|:-|:-|
| [Assign Colors](./assign-colors) | <p>Uniquely sets the fill and/or stroke style of every vector element to individual colors sampled along a chosen gradient.</p> | `Table<Vector> → Table<Vector>`<br />`Table<Graphic> → Table<Graphic>` |
| [Fill](./fill) | <p>Applies a fill style to the vector content, giving an appearance to the area within the interior of the geometry.</p> | `Table<Vector> → Table<Vector>`<br />`Table<Graphic> → Table<Graphic>` |
| [Stroke](./stroke) | <p>Applies a stroke style to the vector content, giving an appearance to the area within the outline of the geometry.</p> | `Table<Vector> → Table<Vector>`<br />`Table<Graphic> → Table<Graphic>` |
