# Adding a gizmo to a node

A gizmo is a draggable handle drawn on the canvas that edits one node input. This directory holds the
machinery for them. Adding one to a node usually means writing a table entry, not a file.

## How it fits together

```
gizmo_registry.rs    which parameters get gizmos, declared as data
generic_gizmos/      the mechanics: hit-testing, hover/drag, overlays, writing the input
gizmo_behaviors.rs   the shape-specific half, and the only place node geometry belongs
gizmo_manager.rs     picks the right handler for the selected layer
```

The generic layer always owns the hover/drag state machine, arbitration between overlapping gizmos,
cursor feedback, and the write to the graph. You supply what is genuinely particular to your node, and
often that is nothing at all.

## The whole job, when the parameter is a length

The Heart's radius is the smallest complete example. It is one entry and no code:

```rust
const HEART_GIZMOS: &[GizmoInfo] = &[GizmoInfo {
    parameter_index: heart::RadiusInput::INDEX,
    gizmo_type: GizmoType::Slider,
    name: "Radius",
    min: Some(0.),
    max: None,
    behavior: GizmoBehavior::NONE,
    position_hint: PositionHint::ParameterDerived,
}];
```

Then register the node so the manager can find it:

```rust
fn registered_gizmo_nodes() -> Vec<(ProtoNodeIdentifier, &'static [GizmoInfo])> {
    vec![
        // ...
        (generator_nodes::heart::IDENTIFIER, HEART_GIZMOS),
    ]
}
```

That gives you a handle sitting `radius` out along the local +X axis, discoverable at rest, draggable,
clamped, undoable. If your parameter is a length measured from the layer's origin, stop here.

### Which `gizmo_type`

- `Slider` — an `f64`, dragged along a ray. The default and the one most parameters want.
- `Dial` — a `u32` count, stepped by horizontal drag. Sides, points, rows.
- `Angle` — an angle in degrees. Runs on the slider's machinery, so it expects a custom `drag`.
- `Position` — **not implemented.** Declaring it silently produces no gizmo.

A declaration that supplies its own `drag` is hosted by the slider whatever it declares, because the
dial's step-drag is exactly what such a node is replacing.

## When the default is not enough

Everything below is optional and defaulted. Reach for a hook only when the default is wrong, and put
the function in `gizmo_behaviors.rs` rather than in the generic layer.

| Hook | Use it when |
|---|---|
| `handle_positions` | the handle does not belong on the +X axis — a star's radius is grabbable at every vertex |
| `hover_distances` | what you grab is not a point — a grid's rows are grabbed anywhere along an edge |
| `drag` | reading a distance along a ray is the wrong question — a spiral winds, an arc sweeps |
| `snap_targets` | the drag should settle onto values derived from the node's other inputs |
| `overlay` | the shape draws something of its own: an outline, a guide, ticks |
| `draws_own_handle` | your overlay already draws the thing being grabbed, so the generic handle would double it |
| `extended_target` | what you grab is a region, so an overlapping point handle should outrank it |
| `angle_deadzone` | a rotational drag needs a jitter guard near the origin |

A worked example, from `POLYGON_RADIUS`. A regular polygon's radius reaches every corner, so every
corner is a grab point:

```rust
fn polygon_radius_handles(context: &GizmoContext, value: f64) -> Vec<DVec2> {
    let Some((sides, _)) = extract_polygon_parameters(Some(context.layer), context.document) else {
        return Vec::new();
    };

    (0..sides)
        .map(|vertex| {
            let angle = ((vertex as f64) * TAU) / (sides as f64);
            DVec2::new(value * angle.sin(), -value * angle.cos())
        })
        .collect()
}
```

The drag then runs along the ray through whichever corner was taken hold of, and `context.handle_index`
tells your overlay which one that is.

### Writing a `drag`

Return every input the motion implies, not just the one you declared. A spiral's turns cannot change
without its outer radius following, or the spiral tightens as it grows:

```rust
fn spiral_turns_drag(context: &GizmoContext, drag: &mut DragInput) -> DragWrites {
    // ... read the starting values out of `drag.initial_parameters`
    DragWrites::inputs(vec![
        (TurnsInput.into(), TaggedValue::F64(new_turns)),
        (OuterRadiusInput.into(), TaggedValue::F64(new_outer_radius)),
    ])
}
```

Three things worth knowing about `DragInput`:

- **It is mutable.** A gesture that reaches a limit and re-anchors rather than stopping — an arc dragged
  past a full sweep hands over to its other endpoint — rewrites the baseline the rest of the drag is
  measured against.
- **`initial_parameters` is the node as it was when the drag began.** Read from it, not from the
  document: by the second frame the live values are the ones you already wrote.
- **`DragWrites` can carry a transform.** A control that repositions the shape as it resizes it needs
  one; a grid grown from its top edge has to move up as it gains a row, or the edge slides out from
  under the cursor.

## The invariant

A gizmo never mutates geometry. It writes a node input and re-runs the graph, then re-reads its own
position from the value it just wrote. Every edit path — gizmo, Properties panel, API — converges on the
same write, which is why a value changed in the panel moves the canvas handle for free. The grid's
transform is the one exception, and it moves the layer rather than the geometry.

## Things that will catch you

- **`INDEX` counts from the node's primary input**, so the first real parameter is `1`. Use the generated
  symbol (`heart::RadiusInput::INDEX`) rather than a literal, and a node gaining an input will not
  silently repoint your gizmo at the wrong one.
- **Respect the node's `#[hard(..)]` range.** Writing outside it does not clamp — it produces geometry the
  renderer cannot draw. A heart with a cleavage deeper than its shoulders are high crosses its own lobes
  and vanishes entirely.
- **A normalized parameter needs a `drag`.** The default writes a distance in document units straight
  through, which is meaningless for a fraction-of-the-radius parameter.
- **The transform cage sits on top of the obvious grab points.** Its corner and edge handles land where a
  circle's radius or an arc's endpoint invites the cursor, and it wins the press. Test away from them.
- **The bounding-box `PositionHint` variants are inert.** Every migrated shape derives its handle from a
  parameter, so `BoundingBoxCenter` and friends currently fall through to the +X axis.
- **Two overlapping handles are not ranked by distance alone.** A gizmo grabbed along a region reports how
  far the cursor is from that region, which is near zero everywhere along it; a point handle reports its
  real distance. Comparing those two numbers gives the region every grab. Mark the region one
  `extended_target: true` and the point wins outright — this is what makes an arc's sweep endpoints
  reachable at all, since they sit on the very circumference its radius is grabbed along.
- **Nothing is drawn at rest unless something asks for it.** A slider with no overlay marks its grab
  points; one that supplies an overlay is expected to draw its own resting state.

## Testing

Registry declarations are cheap to assert directly — see the tests at the bottom of `gizmo_registry.rs`,
which check that each node exposes what it should and that behaviors carrying handles or drags actually
have them. Pure helpers are worth extracting and testing on their own; `nearest_snap_target` in
`generic_slider_gizmo.rs` is the pattern.

None of that catches a gizmo that is drawn in the wrong place or drags the wrong way. Run the editor and
grab the handle. Interaction code is exactly where tests pass and the control still feels wrong.
