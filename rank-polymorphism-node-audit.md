# Rank Polymorphism Node Audit

Classification of all 271 live `#[node_macro::node]` definitions (July 2026, master @ 95c1ab81f) for the rank polymorphism refactor. Two additional commented-out definitions (`month`, `day` in animation.rs) are excluded.

## Rubric summary

- Wires carry `Item<T>` (rank 0) or `List<T>` (rank 1). Each input connector declares a **cell rank**; the compiler maps kernels over excess rank (the **frame**), zipping framed connectors (longest-list, last-element repeats) and broadcasting unframed ones. Output rank = kernel output rank + frame rank. Rank-2 results force-flatten (concatenate) until tree spines land.
- Classification is by **intended semantics**, not today's signatures (bulk-converted `List<T>` carries no signal). Element-wise is the default; rank 1 only for genuine whole-list needs.
- **Classes:** `element-wise` (all data connectors rank 0 → rank 0, includes generators), `floor` (rank 1 → rank 1), `reducer` (rank 1 → rank 0), `expander` (rank 0 → rank 1), `mixed` (differing cell ranks), `infrastructure` (exempt).
- **Lazy connectors** (`Context -> T`, one evaluation per frame slot): only for (1) frame-synthesizers, (2) demand-context modifiers (Footprint et al.), and (3) evaluation-control (Memoize, Monitor, Switch — nodes whose purpose is deciding whether/when upstream evaluates).
- **Attrs**: node reads/writes ATTR_* values; authored as an element-wise `Item<T>` kernel with direct attribute access, classified by semantic rank.
- No `Option` outputs. `Default::default()` is the fallback only when a node's own semantics define no answer; nodes may define richer valid domains (negative from-end indexing, clamping) as non-failures.
- Notation: `name: Item<T>` = rank-0 connector, `name: List<T>` = rank-1 connector, `name: Context -> Item<T> (lazy)` = lazy connector. `DEVIATION:` marks observable behavior changes vs. today.

## Summary statistics

| Class | Count | Share (of 271) | Share (of 244 non-infra) |
|---|---|---|---|
| element-wise | 207 | 76% | 85% |
| expander | 12 | 4% | 5% |
| mixed | 11 | 4% | 5% |
| reducer | 7 | 3% | 3% |
| floor | 7 | 3% | 3% |
| infrastructure | 27 | 10% | — |

Counts reflect the resolved decisions below (blob unification reclassified Post Request, String to Bytes, and Image to Bytes as element-wise).

By area: math 57 (100% element-wise), vector 58, graphic 31, text 33, raster 31, transform/repeat/blending 16, generators/bool/brush 15, gcore 24, gstd/render 21, path-bool 1 (counted in generators group).

## Cross-cutting findings

1. **The element-wise default holds overwhelmingly.** Every math node, every raster adjustment, every string operation, nearly every vector modifier, and all generators classify as rank 0. The true rank-1 floors number in the single digits (Assign Colors, Pack Strips, Extend, the Flatten family) plus a handful of reducers and expanders.
2. **A third lazy category is needed: evaluation-control.** Memoize's entire purpose is skipping upstream evaluation on cache hit; Monitor is compiler-inserted plumbing; Switch's short-circuiting branches are the user-facing member of this category. None meets the two authored-node lazy criteria, and none should — they warrant an explicit exemption alongside frame-synthesizers and demand-context modifiers.
3. **The demand-context-modifier criterion generalizes beyond Footprint.** Quantize Real Time / Quantize Animation Time rewrite the time in context before upstream evaluates; Area/Centroid reset the Footprint to default for resolution-independence. All are legitimate lazy connectors under "modifies context before upstream evaluates."
4. **A recurring deviation family: hidden whole-list aggregation.** Many measure-style nodes silently reduce across the list today (Count Points, Path Length, Area, Centroid, Point Inside, Dimensions, Sample Gradient, Image Color Palette, plus the progression-family subpath flattening in Cut Path / Position on Path / Tangent on Path / Morph's path). All are reclassified element-wise; recovering the old aggregate behavior requires composing with explicit reducers.
5. **New companion nodes needed** to recover composability lost by removing hidden aggregation: generic numeric reducers (Sum, Average, Minimum, Maximum over a `List`), boolean reducers (Any, All), a Filter/Cull node (`List<T> + List<bool> mask`), and a Sort node. Count Elements already exists as the list-length reducer. Also: a Corners node (`List<f64> -> Item<Corners<f64>>` via CSS shorthand expansion) and a Separate Glyphs expander (split out of Text to Vector).
6. **Failure-case pattern:** several generators return an empty `List` on failure today (Hex to Color, QR Code, Image decode, Noise Pattern / Mandelbrot offscreen). Rank-0 output cannot be empty; these become `Default::default()` values. If "absent" must be representable, that's the future exception/invalid-signal channel, not empty lists.
7. **Byte-blob representation split (gstd), RESOLVED:** `Arc<[u8]>` rank-0 blobs vs. `List<u8>` per-byte lists were used inconsistently. Unify on a rank-0 blob type, turning String to Bytes, Image to Bytes, and Post Request's body into plain element-wise connectors.
8. **Evidence the refactor is needed:** Black & White's bare `tint: Color` parameter — exactly the rank-0 connector shape this refactor prescribes — currently causes a type error that hides the node (in-code TODO). The three "apply once to the list's parent" TODOs in blending are fixed outright by rank-0 wires.

## Resolved decisions (Keavon, July 2026)

1. **Switch keeps short-circuiting.** Its branches stay lazy (`Context -> Item<T>`), classified under the evaluation-control lazy category. Condition is rank-0 eager; when framed by a `List<bool>`, each frame slot evaluates only its taken branch (per-element short-circuiting), with memoization collapsing repeated evaluations of context-independent upstreams.
2. **Raster zip-mismatch cases adopt clean rank semantics.** Plural raster data isn't used in the wild; past ad-hoc behaviors are removed: Mix uses longest-zip; Combine Channels uses longest-zip with default-raster slots for mismatched dimensions; Mask passes the image through on degenerate stencils.
3. **Rectangle corner radii become a `Corners<f64>` value type** following CSS expansion rules (reusable wherever four-value expansion applies). A new Corners node converts `List<f64> -> Item<Corners<f64>>` via the shorthand rules.
4. **Centroid is element-wise** like the other measure nodes. Group centroid composes as Flatten Path + Centroid (identical recipe to Bounding Box); the flag only applied to aggregating per-shape outputs directly, which needs area weights.
5. **Empty-input passthroughs dropped** (Gradient Map / Fill / Stroke): rank-0 default semantics apply; connectors get sensible `#[default(...)]` values where useful.
6. **Attach Attribute merges into Write Attribute** (both new/undocumented; no real-artwork usage).
7. **Nodes define their own valid domains.** Negative from-end indexing and clamping are legitimate node semantics, not failures; `Default::default()` applies only when the node itself has no defined answer. Index Points keeps its behavior unchanged.
8. **Some / Unwrap Option / Size Of deleted** — vestigial and unused.
9. **Byte-blob unification adopted:** a rank-0 blob type replaces `List<u8>`; Post Request's body, String to Bytes, and Image to Bytes become element-wise.
10. **Dash patterns become a `DashPattern` value type** (amending the Stroke row's rank-1 `List<f64>` cell): `List<T>` is reserved for frames and attribute-carrying collections; compound values whose inner elements never hold attributes (dash sequences, corner radii) are value types, keeping their connectors rank 0 and frameable. Cascades to the Vec-looking TaggedValue variants.
11. **Text to Vector splits in two:** the compound-path-per-string node (element-wise) and a Separate Glyphs expander.

## Deletions, merges, and splits

| Node | Action | Reason |
|---|---|---|
| Copy to Points | DELETE | Transform broadcasting + assign/spread family (N×M decomposed) |
| Repeat on Points | DELETE | Transform broadcasting replaces it |
| Map | DELETE/demote to legacy | Compiler framing is exactly this node |
| Map String, Read String | DEPRECATE | Subsumed by framing over rank-0 string kernels |
| Attach Attribute | MERGE into Write Attribute | Identical once Write Attribute's value is an eager zip |
| Extract Element | MERGE into Index Elements | "Bare element" distinction dissolves when Item carries attributes |
| As u32 / As u64 / As f64 | DELETE | Existing TODOs slate them for Passthrough replacement |
| Some / Unwrap Option / Size Of | DELETE | Vestigial and unused (resolution 8) |
| Text to Vector (glyph mode) | SPLIT | `separate_glyphs` becomes a Separate Glyphs expander node (resolution 10) |
| Upload Texture | DELETE | Doc comment already deprecates in favor of Convert |
| To Graphic | Possibly subsume | Auto-conversion between graphical Item types would cover it |
| Legacy Layer Extend | DELETE | Existing TODO; document-upgrade shim |
| Brightness/Contrast Classic | Keep hidden | PSD interop; see raster flags |

---

## Math (57 nodes — all element-wise)

All in `node-graph/nodes/math/src/lib.rs`. Zero rank-1 connectors in the entire crate; every node zips/broadcasts. Attributes now flow through math nodes (fixing today's attribute-dropping hole).

| Node | Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Math | 39 | `operand_a: Item<T>, expression: Item<String>, operand_b: Item<T> -> Item<T>` | element-wise | no | — | Parse/eval failure -> 0 (already default-safe); per-element expressions want parse caching |
| Add | 84 | `augend: Item<A>, addend: Item<B> -> Item<A::Output>` | element-wise | no | — | |
| Subtract | 98 | `minuend: Item<A>, subtrahend: Item<B> -> Item<A::Output>` | element-wise | no | — | |
| Multiply | 112 | `multiplier: Item<A>, multiplicand: Item<B> -> Item<A::Output>` | element-wise | no | — | Includes DAffine2×DAffine2, DAffine2×DVec2 |
| Divide | 129 | `numerator: Item<A>, denominator: Item<B> -> Item<A::Output>` | element-wise | no | — | Zero denominator -> default (already) |
| Reciprocal | 152 | `value: Item<T> -> Item<T>` | element-wise | no | — | 0 -> 0 (already) |
| Modulo | 165 | `numerator: Item<A>, modulus: Item<B>, always_positive: Item<bool> -> Item<A::Output>` | element-wise | no | — | |
| Exponent | 183 | `base: Item<T>, power: Item<T> -> Item<T::Output>` | element-wise | no | — | |
| Root | 200 | `radicand: Item<T>, degree: Item<T> -> Item<T>` | element-wise | no | — | Degree <= 0 -> 0 (already) |
| Logarithm | 225 | `value: Item<T>, base: Item<T> -> Item<T>` | element-wise | no | — | |
| Sine | 248 | `theta: Item<T>, radians: Item<bool> -> Item<T>` | element-wise | no | — | |
| Cosine | 261 | `theta: Item<T>, radians: Item<bool> -> Item<T>` | element-wise | no | — | |
| Tangent | 274 | `theta: Item<T>, radians: Item<bool> -> Item<T>` | element-wise | no | — | |
| Sine Inverse | 287 | `value: Item<T>, radians: Item<bool> -> Item<T>` | element-wise | no | — | |
| Cosine Inverse | 301 | `value: Item<T>, radians: Item<bool> -> Item<T>` | element-wise | no | — | |
| Tangent Inverse | 319 | `value: Item<T>, radians: Item<bool> -> Item<T::Output>` | element-wise | no | — | DVec2 impl is atan2 |
| Remap | 357 | `value: Item<U>, input_min: Item<U>, input_max: Item<U>, output_min: Item<U>, output_max: Item<U>, clamped: Item<bool> -> Item<U>` | element-wise | no | — | |
| Random | 405 | `seed: Item<u64>, min: Item<f64>, max: Item<f64> -> Item<f64>` | element-wise | no | — | Vary seed per element (e.g. via index) for per-element variation |
| As u32 | 425 | `value: Item<u32> -> Item<u32>` | element-wise | no | — | DELETE (existing TODO) |
| As u64 | 432 | `value: Item<u64> -> Item<u64>` | element-wise | no | — | DELETE (existing TODO) |
| As f64 | 439 | `value: Item<f64> -> Item<f64>` | element-wise | no | — | DELETE (existing TODO) |
| Round | 445 | `value: Item<T> -> Item<T>` | element-wise | no | — | |
| Floor | 456 | `value: Item<T> -> Item<T>` | element-wise | no | — | |
| Ceiling | 467 | `value: Item<T> -> Item<T>` | element-wise | no | — | |
| Absolute Value | 507 | `value: Item<T> -> Item<T>` | element-wise | no | — | |
| Min | 518 | `value: Item<T>, other_value: Item<T> -> Item<T>` | element-wise | no | — | Pairwise; list minimum = future reducer node |
| Max | 532 | `value: Item<T>, other_value: Item<T> -> Item<T>` | element-wise | no | — | Pairwise; list maximum = future reducer node |
| Clamp | 546 | `value: Item<T>, min: Item<T>, max: Item<T> -> Item<T>` | element-wise | no | — | |
| Greatest Common Divisor | 571 | `value: Item<T>, other_value: Item<T> -> Item<T>` | element-wise | no | — | |
| Least Common Multiple | 591 | `value: Item<T>, other_value: Item<T> -> Item<T>` | element-wise | no | — | |
| Less Than | 646 | `value: Item<T>, other_value: Item<T>, or_equal: Item<bool> -> Item<bool>` | element-wise | no | — | |
| Greater Than | 663 | `value: Item<T>, other_value: Item<T>, or_equal: Item<bool> -> Item<bool>` | element-wise | no | — | |
| Equals | 679 | `value: Item<T>, other_value: Item<T> -> Item<bool>` | element-wise | no | — | |
| Not Equals | 693 | `value: Item<T>, other_value: Item<T> -> Item<bool>` | element-wise | no | — | |
| Logical Or | 707 | `value: Item<bool>, other_value: Item<bool> -> Item<bool>` | element-wise | no | — | |
| Logical And | 720 | `value: Item<bool>, other_value: Item<bool> -> Item<bool>` | element-wise | no | — | |
| Logical Not | 733 | `input: Item<bool> -> Item<bool>` | element-wise | no | — | |
| Switch | 743 | `condition: Item<bool>, if_true: Context -> Item<T> (lazy), if_false: Context -> Item<T> (lazy) -> Item<T>` | element-wise | branches (evaluation-control) | — | Short-circuits: only the taken branch evaluates, per frame slot when framed (resolution 1) |
| Bool Value | 790 | `bool: Item<bool> -> Item<bool>` | element-wise | no | — | Value generator |
| Number Value | 796 | `number: Item<f64> -> Item<f64>` | element-wise | no | — | Value generator |
| Percentage Value | 802 | `percentage: Item<f64> -> Item<f64>` | element-wise | no | — | Value generator |
| Vec2 Value | 808 | `x: Item<f64>, y: Item<f64> -> Item<DVec2>` | element-wise | no | — | Value generator |
| Color Value | 814 | `color: Item<Color> -> Item<Color>` | element-wise | no | — | Current `List<Color>` wrapper is bulk-conversion noise |
| RGBA to Color | 820 | `red: Item<f64>, green: Item<f64>, blue: Item<f64>, alpha: Item<f64> -> Item<Color>` | element-wise | no | — | Drop `new_from_element` wrapper |
| HSVA to Color | 832 | `hue: Item<f64>, saturation: Item<f64>, value: Item<f64>, alpha: Item<f64> -> Item<Color>` | element-wise | no | — | Drop wrapper |
| HSLA to Color | 843 | `hue: Item<f64>, saturation: Item<f64>, lightness: Item<f64>, alpha: Item<f64> -> Item<Color>` | element-wise | no | — | Drop wrapper |
| Hex to Color | 854 | `hex_code: Item<String> -> Item<Color>` | element-wise | no | — | DEVIATION: invalid input today yields empty list ("no color"); rank 0 yields `Color::default()` (finding 6) |
| Gradient Value | 863 | `gradient: Item<GradientStops> -> Item<GradientStops>` | element-wise | no | — | Value generator |
| Gradient Type | 869 | `gradient: Item<GradientStops>, gradient_type: Item<GradientType> -> Item<GradientStops>` | element-wise | no | attrs | Writes ATTR_GRADIENT_TYPE; whole-list loop becomes the frame; type can now vary per element |
| Spread Method | 878 | `gradient: Item<GradientStops>, spread_method: Item<GradientSpreadMethod> -> Item<GradientStops>` | element-wise | no | attrs | Writes ATTR_SPREAD_METHOD; same as above |
| Sample Gradient | 887 | `gradient: Item<GradientStops>, position: Item<f64> -> Item<Color>` | element-wise | no | — | DEVIATION: today samples only `element(0)`, silently truncating multi-gradient lists |
| Footprint Value | 897 | `transform: Item<DAffine2>, resolution: Item<DVec2> -> Item<Footprint>` | element-wise | no | — | Constructs a Footprint value; not a demand-context modifier |
| Dot Product | 911 | `vector_a: Item<DVec2>, vector_b: Item<DVec2>, normalize: Item<bool> -> Item<f64>` | element-wise | no | — | |
| Angle Between | 932 | `vector_a: Item<DVec2>, vector_b: Item<DVec2>, radians: Item<bool> -> Item<f64>` | element-wise | no | — | |
| Angle To | 954 | `observer: Item<T>, target: Item<U>, radians: Item<bool> -> Item<f64>` | element-wise | no | — | |
| Length | 976 | `vector: Item<DVec2> -> Item<f64>` | element-wise | no | — | TODO rename to Magnitude |
| Normalize | 984 | `vector: Item<DVec2> -> Item<DVec2>` | element-wise | no | — | |

## Vector (43 nodes in vector_nodes.rs + vector_modification_nodes.rs)

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Assign Colors | vector_nodes.rs:73 | `content: List<Vector>, fill: Item<bool>, stroke: Item<bool>, gradient: Item<GradientStops>, reverse: Item<bool>, randomize: Item<bool>, seed: Item<SeedValue>, repeat_every: Item<u32> -> List<Vector>` | floor | no | attrs | Genuine floor (spreads by index over list length). Gradient today reads only row 0 -> Item. Assign/spread family direction per assign-transform-nodes branch |
| Fill | vector_nodes.rs:138 | `content: Item<Vector>, fill: Item<Fill> -> Item<Vector>` | element-wise | no | no | DEVIATION: `List<Color>` fill today collapses to one fill; zip gives per-element fills (resolution 5: adopted). Backup-color connectors are UI-state stash |
| Stroke | vector_nodes.rs:197 | `content: Item<Vector>, color: Item<Color>, weight: Item<f64>, align: Item<StrokeAlign>, cap: Item<StrokeCap>, join: Item<StrokeJoin>, miter_limit: Item<f64>, paint_order: Item<PaintOrder>, dash_lengths: List<f64>, dash_offset: Item<f64> -> Item<Vector>` | mixed | no | attrs | `dash_lengths` is one whole dash pattern (genuine rank 1). DEVIATION: color today uses `element(0)` for all elements |
| Copy to Points | vector_nodes.rs:256 | DELETE | mixed | — | attrs | N×M points×content cross-product, decomposed by Transform broadcast + assign/spread nodes |
| Round Corners | vector_nodes.rs:336 | `source: Item<Vector>, radius: Item<f64>, roundness: Item<f64>, edge_length_limit: Item<f64>, min_angle_threshold: Item<f64> -> Item<Vector>` | element-wise | no | attrs | World-space via ATTR_TRANSFORM |
| Merge by Distance | vector_nodes.rs:450 | `content: Item<Vector>, distance: Item<f64>, algorithm: Item<MergeByDistanceAlgorithm> -> Item<Vector>` | element-wise | no | attrs | Merging is within one element only |
| Extrude | vector_nodes.rs:675 | `source: Item<Vector>, direction: Item<DVec2>, joining_algorithm: Item<ExtrudeJoiningAlgorithm> -> Item<Vector>` | element-wise | no | no | |
| Box Warp | vector_nodes.rs:683 | `content: Item<Vector>, rectangle: Item<Vector> -> Item<Vector>` | element-wise | no | attrs | DEVIATION: today only rectangle element 0 used; zip warps content[i] by rectangle[i] |
| Pack Strips | vector_nodes.rs:768 | `elements: List<T>, separation: Item<f64>, strip_max_length: Item<f64>, strip_direction: Item<RowsOrColumns> -> List<T>` | floor | no | attrs | Genuine whole-list layout (BFDH sort + reorder) |
| Auto-Tangents | vector_nodes.rs:889 | `source: Item<Vector>, spread: Item<f64>, preserve_existing: Item<bool> -> Item<Vector>` | element-wise | no | attrs | |
| Bounding Box | vector_nodes.rs:1043 | `content: Item<Vector> -> Item<Vector>` | element-wise | no | no | Already per-element today (no deviation). Union = Combine + Bounding Box |
| Dimensions | vector_nodes.rs:1068 | `content: Item<Vector> -> Item<DVec2>` | element-wise | no | attrs | DEVIATION: today unions all elements' boxes; per-shape now. Empty -> DVec2::ZERO |
| As Vector | vector_nodes.rs:1078 | passthrough | infrastructure | no | no | Type assertion identity |
| Points to Polyline | vector_nodes.rs:1084 | `points: Item<Vector>, closed: Item<bool> -> Item<Vector>` | element-wise | no | no | |
| Offset Path | vector_nodes.rs:1112 | `content: Item<Vector>, distance: Item<f64>, join: Item<StrokeJoin>, miter_limit: Item<f64> -> Item<Vector>` | element-wise | no | attrs | |
| Solidify Stroke | vector_nodes.rs:1156 | `content: Item<Vector> -> List<Vector>` | expander | no | attrs | 1-2 rows (fill + outlined stroke). DEVIATION: today also flattens `List<Graphic>` input; becomes upstream Flatten. Framed output force-flattens (settled rule) |
| Separate Subpaths | vector_nodes.rs:1267 | `content: Item<Vector> -> List<Vector>` | expander | no | no | One row per subpath; framed output force-flattens |
| Path is Closed | vector_nodes.rs:1298 | `content: Item<Vector>, index: Item<f64> -> Item<bool>` | element-wise | no | no | DEVIATION: index today counts subpaths across ALL elements; now scoped per element (old behavior = Flatten Path first) |
| Map Points | vector_nodes.rs:1313 | `content: Item<Vector>, mapped: Context -> DVec2 (lazy) -> Item<Vector>` | element-wise | yes (frame-synth) | no | Loop over points is invented by the node (index+position in context). Minor DEVIATION: point index restarts per element |
| Flatten Path | vector_nodes.rs:1331 | `content: List<Vector> -> Item<Vector>` | reducer | no | attrs | Settled reducer. In-code TODO already plans Flatten + per-element Combine Paths split |
| Sample Polyline | vector_nodes.rs:1373 | `content: Item<Vector>, spacing: Item<PointSpacingType>, separation: Item<f64>, quantity: Item<u32>, start_offset: Item<f64>, stop_offset: Item<f64>, adaptive_spacing: Item<bool> -> Item<Vector>` | element-wise | no | attrs | Settled element-wise; memoized |
| Simplify | vector_nodes.rs:1459 | `content: Item<Vector>, tolerance: Item<f64> -> Item<Vector>` | element-wise | no | attrs | |
| Decimate | vector_nodes.rs:1503 | `content: Item<Vector>, tolerance: Item<f64> -> Item<Vector>` | element-wise | no | attrs | |
| Cut Path | vector_nodes.rs:1631 | `content: Item<Vector>, progression: Item<f64>, reverse: Item<bool>, parameterized_distance: Item<bool> -> Item<Vector>` | element-wise | no | no | DEVIATION: progression's whole part today indexes subpaths flattened across ALL elements; now per element |
| Cut Segments | vector_nodes.rs:1683 | `content: Item<Vector> -> Item<Vector>` | element-wise | no | no | Already clean per-element |
| Position on Path | vector_nodes.rs:1741 | `content: Item<Vector>, progression: Item<f64>, reverse: Item<bool>, parameterized_distance: Item<bool> -> Item<DVec2>` | element-wise | no | attrs | Same cross-element subpath DEVIATION as Cut Path |
| Tangent on Path | vector_nodes.rs:1779 | `content: Item<Vector>, progression: Item<f64>, reverse: Item<bool>, parameterized_distance: Item<bool>, radians: Item<bool> -> Item<f64>` | element-wise | no | attrs | Same DEVIATION |
| Scatter Points | vector_nodes.rs:1827 | `content: Item<Vector>, separation: Item<f64>, seed: Item<u32> -> Item<Vector>` | element-wise | no | no | DEVIATION: RNG today threads one stream across elements; per-element re-seeds (as Jitter Points already does) |
| Spline | vector_nodes.rs:1876 | `content: Item<Vector> -> Item<Vector>` | element-wise | no | no | DEVIATION: today filter_maps away pointless elements (hidden filtering); now passes them through unchanged |
| Jitter Points | vector_nodes.rs:1976 | `content: Item<Vector>, max_distance: Item<f64>, seed: Item<u32>, along_normals: Item<bool> -> Item<Vector>` | element-wise | no | attrs | RNG already per-element; behavior preserved |
| Offset Points | vector_nodes.rs:2026 | `content: Item<Vector>, distance: Item<f64> -> Item<Vector>` | element-wise | no | attrs | |
| Morph | vector_nodes.rs:2063 | `content: List<Vector>, progression: Item<f64>, reverse: Item<bool>, distribution: Item<InterpolationDistribution>, path: Item<Vector> -> Item<Vector>` | mixed | no | attrs | Content genuinely spread-by-index (rank 1). DEVIATION: control `path` today flattens subpaths across its list; now one element |
| Bevel | vector_nodes.rs:2870 | `source: Item<Vector>, distance: Item<f64> -> Item<Vector>` | element-wise | no | attrs | |
| Close Path | vector_nodes.rs:2883 | `source: Item<Vector> -> Item<Vector>` | element-wise | no | no | |
| Point Inside | vector_nodes.rs:2894 | `source: Item<Vector>, point: Item<DVec2> -> Item<bool>` | element-wise | no | attrs | DEVIATION: today ORs containment across all elements; per-shape now, "any" = future Any reducer |
| Count Elements | vector_nodes.rs:2904 | `content: List<T> -> Item<f64>` | reducer | no | no | Genuine list-length reducer (ListDyn, any element type) |
| Count Points | vector_nodes.rs:2909 | `content: Item<Vector> -> Item<f64>` | element-wise | no | no | DEVIATION: today sums across elements; per-shape now, total = Sum reducer |
| Index Points | vector_nodes.rs:2916 | `content: List<Vector>, index: Item<f64> -> Item<DVec2>` | mixed | no | no | Negative from-end indexing and clamping are node-defined valid semantics; behavior unchanged (resolution 7) |
| Path Length | vector_nodes.rs:2950 | `source: Item<Vector> -> Item<f64>` | element-wise | no | attrs | DEVIATION: today sums perimeters across elements |
| Area | vector_nodes.rs:2969 | `content: Context -> Item<Vector> (lazy) -> Item<f64>` | element-wise | yes (context modifier) | attrs | Resets Footprint before upstream eval (justified). DEVIATION: today sums areas across elements |
| Centroid | vector_nodes.rs:2983 | `content: Context -> Item<Vector> (lazy), centroid_type: Item<CentroidType> -> Item<DVec2>` | element-wise | yes (context modifier) | attrs | Same Footprint-reset pattern. DEVIATION: today one weighted centroid across all; group centroid = Flatten Path + Centroid, same recipe as Bounding Box (resolution 4) |
| Path Modify | vector_modification_nodes.rs:9 | (editor-internal) | infrastructure | no | attrs | Hidden node backing Pen/Path tool edits; exempt |
| Apply Transform | vector_modification_nodes.rs:37 | `vector: Item<Vector> -> Item<Vector>` | element-wise | no | attrs | Bakes ATTR_TRANSFORM into geometry, resets to identity |

## Generators, Boolean, Brush (15 nodes)

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Circle | generator_nodes.rs:59 | `radius: Item<f64> -> Item<Vector>` | element-wise | no | no | Generator emits rank 0; `List<f64>` radii frames into `List<Vector>` |
| Arc | generator_nodes.rs:72 | `radius: Item<f64>, start_angle: Item<f64>, sweep_angle: Item<f64>, arc_type: Item<ArcType> -> Item<Vector>` | element-wise | no | no | |
| Spiral | generator_nodes.rs:98 | `spiral_type: Item<SpiralType>, turns: Item<f64>, start_angle: Item<f64>, inner_radius: Item<f64>, outer_radius: Item<f64>, angular_resolution: Item<f64> -> Item<Vector>` | element-wise | no | no | |
| Ellipse | generator_nodes.rs:120 | `radius_x: Item<f64>, radius_y: Item<f64> -> Item<Vector>` | element-wise | no | no | |
| Rectangle | generator_nodes.rs:148 | `width: Item<f64>, height: Item<f64>, corner_radius: Item<Corners<f64>>, clamped: Item<bool> -> Item<Vector>` | element-wise | no | no | DEVIATION: `List<f64>` CSS-shorthand mode replaced by the `Corners<f64>` value type + new Corners conversion node (resolution 3) |
| Regular Polygon | generator_nodes.rs:166 | `sides: Item<u64>, radius: Item<f64> -> Item<Vector>` | element-wise | no | no | |
| Star | generator_nodes.rs:184 | `sides: Item<u64>, radius_1: Item<f64>, radius_2: Item<f64> -> Item<Vector>` | element-wise | no | no | |
| QR Code | generator_nodes.rs:223 | `text: Item<String>, has_size: Item<bool>, size: Item<f64>, error_correction: Item<QRCodeErrorCorrectionLevel>, individual_squares: Item<bool> -> Item<Vector>` | element-wise | no | no | DEVIATION: encode failure -> `Item::default()` instead of empty list |
| Arrow | generator_nodes.rs:278 | `arrow_to: Item<DVec2>, shaft_width: Item<f64>, head_width: Item<f64>, head_length: Item<f64> -> Item<Vector>` | element-wise | no | no | |
| Line | generator_nodes.rs:290 | `line_to: Item<DVec2> -> Item<Vector>` | element-wise | no | no | |
| Grid | generator_nodes.rs:310 | `grid_type: Item<GridType>, spacing: Item<DVec2>, columns: Item<u32>, rows: Item<u32>, angles: Item<DVec2> -> Item<Vector>` | element-wise | no | no | Emits one mesh element |
| Boolean Operation | path-bool/lib.rs:26 | `content: List<Graphic>, operation: Item<BooleanOperation> -> Item<Vector>` | reducer | no | attrs | Genuinely order-dependent whole-list; output drops from 1-item List to Item (existing TODO acknowledges) |
| Brush Stamp Generator | brush/brush.rs:66 | (skip_impl internal) | infrastructure | no | no | Would be element-wise if exposed |
| Blit | brush/brush.rs:86 | (skip_impl internal) | infrastructure | flag | attrs | `positions` is a whole-list fold; `BlendFn` Node connector fails lazy criteria (low stakes, internal) |
| Brush | brush/brush.rs:191 | `background: Item<Raster<CPU>>, trace: List<BrushStroke>, cache: #[data] -> Item<Raster<CPU>>` | mixed | no | attrs | `trace` genuinely sequential rank 1. DEVIATION: today paints only background element 0 (code TODO resolved by framing) |

## Graphic / list manipulation (31 nodes)

This is the list-manipulation core, so rank-1 density is legitimately high here — each rank-1 connector is a genuine whole-list need.

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Index Elements | graphic.rs:16 | `list: List<T>, index: Item<i64> -> Item<T>` | mixed | no | — | Out-of-bounds -> default (already). A List of indices frames into a gather node |
| Omit Element | graphic.rs:47 | `list: List<T>, index: Item<i64> -> List<T>` | mixed | no | — | Filtering = rank 1. Out-of-range -> unchanged (keep) |
| Extract Element | graphic.rs:77 | MERGE into Index Elements | mixed | no | — | "Bare element" distinction dissolves with Item |
| Map | graphic.rs:111 | DELETE/demote | infrastructure | — | — | Compiler framing subsumes it |
| Mirror | graphic.rs:146 | `content: List<T>, relative_to_bounds: Item<ReferencePoint>, offset: Item<f64>, angle: Item<f64>, keep_original: Item<bool> -> List<T>` | mixed | no | attrs | Whole-list bounds + duplication (up to 2N out) |
| Path of Subgraph | graphic.rs:218 | (editor plumbing) | infrastructure | no | — | `List<NodeId>` is a single path value |
| Write Attribute | graphic.rs:230 | `content: Item<T>, name: Item<String>, value: Item<AttributeValueDyn> -> Item<T>` | element-wise | no (was lazy) | attrs | Settled: lazy per-index value becomes eager rank-0 zip |
| Attach Attribute | graphic.rs:267 | MERGE into Write Attribute (confirmed) | element-wise | no | attrs | DEVIATION: wrap-around for short source becomes repeat-last (resolution 6) |
| Read Attribute (×12: Vector, Number, Bool, String, Transform, Color, Blend Mode, Gradient Type, Spread Method, Gradient Stops, Artboard, Raster) | graphic.rs:301-482 | `content: Item<Dyn>, name: Item<String> -> Item<X>` | element-wise | no | attrs | DEVIATION: today skips missing values (shorter output); now emits default per slot, preserving length. Could collapse to one generic node (orthogonal) |
| Extend | graphic.rs:498 | `base: List<T>, new: List<T> -> List<T>` | floor | no | — | Settled: genuine concatenation |
| Legacy Layer Extend | graphic.rs:518 | DELETE | infrastructure | no | attrs | Document-upgrade shim (existing TODO) |
| Wrap Graphic | graphic.rs:545 | `content: List<T> -> Item<Graphic>` | reducer | no | — | Grouping. DAffine2/DVec2 coercion impls worth revisiting |
| To Graphic | graphic.rs:566 | `content: Item<T> -> Item<Graphic>` | element-wise | no | — | DEVIATION: today wraps whole list into ONE Graphic; now per-item conversion. Whole-list grouping = Wrap Graphic. May be subsumed by auto-conversion |
| Flatten Graphic | graphic.rs:584 | `content: List<Graphic>, fully_flatten: Item<bool> -> List<Graphic>` | mixed | no | attrs | Explicit nesting reduction; composes parent transforms into children |
| Flatten Vector | graphic.rs:621 | `content: List<Graphic> -> List<Vector>` | floor | no | attrs | Deep flatten + type filter; ATTR_EDITOR_MERGED_LAYERS hack has removal TODO |
| Flatten Raster | graphic.rs:654 | `content: List<Graphic> -> List<Raster<CPU>>` | floor | no | — | |
| Flatten Color | graphic.rs:660 | `content: List<Graphic> -> List<Color>` | floor | no | — | |
| Flatten Gradient | graphic.rs:666 | `content: List<Graphic> -> List<GradientStops>` | floor | no | — | |
| Colors to Gradient | graphic.rs:672 | `colors: List<Color> -> Item<GradientStops>` | reducer | no | — | Spread by index/length. Output drops to Item |
| Create Artboard | artboard.rs:12 | `content: Context -> List<Graphic> (lazy), location: Item<DVec2>, dimensions: Item<DVec2>, background: Item<Color>, clip: Item<bool> -> Item<Artboard>` | mixed | yes (context modifier) | attrs | Translates Footprint before upstream eval (justified). DEVIATION: background List<Color> element-0 + WHITE fallback becomes Item<Color> with default |

## Text (33 nodes)

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| String Value | text/lib.rs:187 | `string: Item<String> -> Item<String>` | element-wise | no | no | Value generator |
| As String | text/lib.rs:193 | `value: Item<String> -> Item<String>` | element-wise | no | no | Debug passthrough |
| String Concatenate | text/lib.rs:199 | `first: Item<String>, second: Item<String> -> Item<String>` | element-wise | no | no | Two lists zip pairwise (not N×M) |
| String Replace | text/lib.rs:205 | `string: Item<String>, from: Item<String>, to: Item<String> -> Item<String>` | element-wise | no | no | |
| String Slice | text/lib.rs:213 | `string: Item<String>, start: Item<f64>, end: Item<f64> -> Item<String>` | element-wise | no | no | Grapheme-indexed within one string |
| String Truncate | text/lib.rs:236 | `string: Item<String>, length: Item<u32>, suffix: Item<String> -> Item<String>` | element-wise | no | no | |
| Format Number | text/lib.rs:264 | `number: Item<f64>, decimal_places: Item<u32>, decimal_separator: Item<String>, fixed_decimals: Item<bool>, use_thousands_separator: Item<bool>, thousands_separator: Item<String>, start_at_10000: Item<bool> -> Item<String>` | element-wise | no | no | |
| String to Number | text/lib.rs:362 | `string: Item<String>, fallback: Item<f64> -> Item<f64>` | element-wise | no | no | Explicit fallback connector (better than Default) |
| String Trim | text/lib.rs:374 | `string: Item<String>, start: Item<bool>, end: Item<bool> -> Item<String>` | element-wise | no | no | |
| String Escape | text/lib.rs:398 | `string: Item<String>, unescape: Item<bool> -> Item<String>` | element-wise | no | no | |
| String Reverse | text/lib.rs:411 | `string: Item<String> -> Item<String>` | element-wise | no | no | Graphemes within one string, not list order |
| String Repeat | text/lib.rs:421 | `string: Item<String>, count: Item<u32>, separator: Item<String>, separator_escaping: Item<bool> -> Item<String>` | element-wise | no | no | One joined string, not a frame |
| String Pad | text/lib.rs:453 | `string: Item<String>, length: Item<u32>, padding: Item<String>, up_to: Item<String>, from_end: Item<bool> -> Item<String>` | element-wise | no | no | |
| String Contains | text/lib.rs:517 | `string: Item<String>, substring: Item<String>, at_start: Item<bool>, at_end: Item<bool> -> Item<bool>` | element-wise | no | no | |
| String Find Index | text/lib.rs:538 | `string: Item<String>, substring: Item<String>, from_end: Item<bool> -> Item<f64>` | element-wise | no | no | -1 not-found sentinel (kept) |
| String Occurrences | text/lib.rs:567 | `string: Item<String>, substring: Item<String>, overlapping: Item<bool> -> Item<f64>` | element-wise | no | no | |
| String Capitalization | text/lib.rs:636 | `string: Item<String>, capitalization: Item<StringCapitalization>, use_joiner: Item<bool>, joiner: Item<String> -> Item<String>` | element-wise | no | no | |
| String Length | text/lib.rs:723 | `string: Item<String> -> Item<f64>` | element-wise | no | no | Graphemes within one string; list length = Count Elements |
| String Split | text/lib.rs:731 | `string: Item<String>, delimiter: Item<String>, delimiter_escaping: Item<bool> -> List<String>` | expander | no | no | |
| String Join | text/lib.rs:752 | `strings: List<String>, separator: Item<String>, separator_escaping: Item<bool> -> Item<String>` | reducer | no | no | Genuine whole-list |
| Map String | text/lib.rs:771 | DEPRECATE | infrastructure | — | no | Subsumed by framing |
| Read String | text/lib.rs:794 | DEPRECATE | infrastructure | — | no | Vararg pair of Map String |
| Serialize | text/lib.rs:803 | `value: Item<T> -> Item<String>` | element-wise | no | no | Debug node |
| Regex Contains | regex.rs:7 | `string: Item<String>, pattern: Item<String>, case_insensitive: Item<bool>, multiline: Item<bool>, at_start: Item<bool>, at_end: Item<bool> -> Item<bool>` | element-wise | no | no | Invalid pattern -> false |
| Regex Replace | regex.rs:45 | `string: Item<String>, pattern: Item<String>, replacement: Item<String>, replace_all: Item<bool>, case_insensitive: Item<bool>, multiline: Item<bool> -> Item<String>` | element-wise | no | no | Invalid pattern -> unchanged |
| Regex Find | regex.rs:87 | `string: Item<String>, pattern: Item<String>, match_index: Item<f64>, case_insensitive: Item<bool>, multiline: Item<bool> -> List<String>` | expander | no | attrs | Writes ATTR_START/END/NAME; empty list on no match |
| Regex Find All | regex.rs:158 | `string: Item<String>, pattern: Item<String>, case_insensitive: Item<bool>, multiline: Item<bool> -> List<String>` | expander | no | attrs | |
| Regex Split | regex.rs:201 | `string: Item<String>, pattern: Item<String>, case_insensitive: Item<bool>, multiline: Item<bool> -> List<String>` | expander | no | no | |
| Format JSON | json.rs:13 | `json: Item<String>, compact: Item<bool>, multi_line: Item<bool>, indent: Item<String>, break_length: Item<u32>, break_nested: Item<bool> -> Item<String>` | element-wise | no | no | Invalid JSON -> unchanged |
| Query JSON | json.rs:187 | `json: Item<String>, path: Item<String>, unquote_strings: Item<bool> -> Item<String>` | element-wise | no | no | Empty string on no match |
| Query JSON All | json.rs:225 | `json: Item<String>, path: Item<String>, unquote_strings: Item<bool> -> List<String>` | expander | no | attrs | Writes ATTR_TYPE |
| Text | gstd/text.rs:12 | `text: Item<String>, font: Item<Resource>, size: Item<f64>, line_height: Item<f64>, letter_spacing: Item<f64>, letter_tilt: Item<f64>, has_max_width: Item<bool>, max_width: Item<f64>, has_max_height: Item<bool>, max_height: Item<f64>, align: Item<TextAlign> -> Item<String>` | element-wise | no | attrs | DEVIATION: single-element List output is bulk-conversion residue; one styled Item with typographic attributes |
| Text to Vector | gstd/text.rs:96 | `strings: Item<String> -> Item<Vector>` | element-wise | no | attrs | SPLIT: separate_glyphs mode becomes a new Separate Glyphs expander node (resolution 10) |

## Raster (31 nodes)

`T` ranges over `Raster<CPU> | Color | GradientStops`; per-pixel/per-stop looping stays inside the `Adjust`/`Blend` traits, whose impls move from `List<X>` down to bare `X`. GPU (`shader_node`) machinery unchanged; kernel params stay bare for uniform compatibility.

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Luminance | adjustments.rs:52 | `input: Item<T>, luminance_calc: Item<LuminanceCalculation> -> Item<T>` | element-wise | no | no | |
| Gamma Correction | adjustments.rs:77 | `input: Item<T>, gamma: Item<f32>, inverse: Item<bool> -> Item<T>` | element-wise | no | no | |
| Extract Channel | adjustments.rs:98 | `input: Item<T>, channel: Item<RedGreenBlueAlpha> -> Item<T>` | element-wise | no | no | |
| Make Opaque | adjustments.rs:122 | `input: Item<T> -> Item<T>` | element-wise | no | no | |
| Brightness/Contrast Classic | adjustments.rs:144 | `input: Item<T>, brightness: Item<f32>, contrast: Item<f32> -> Item<T>` | element-wise | no | no | Hidden; kept for GPU |
| Brightness/Contrast | adjustments.rs:175 | `input: Item<T>, brightness: Item<f32>, contrast: Item<f32>, use_classic: Item<bool> -> Item<T>` | element-wise | no | no | `use_classic` hidden mode kept for PSD interop |
| Levels | adjustments.rs:256 | `image: Item<T>, shadows/midtones/highlights/output min/max: Item<f32> -> Item<T>` | element-wise | no | no | |
| Black & White | adjustments.rs:335 | `image: Item<T>, tint: Item<Color>, 6× channel weights: Item<f32> -> Item<T>` | element-wise | no | no | Bare `tint: Color` currently type-errors (finding 8); refactor unblocks it |
| Hue/Saturation | adjustments.rs:412 | `input: Item<T>, hue_shift: Item<f32>, saturation_shift: Item<f32>, lightness_shift: Item<f32> -> Item<T>` | element-wise | no | no | |
| Invert | adjustments.rs:444 | `input: Item<T> -> Item<T>` | element-wise | no | no | |
| Threshold | adjustments.rs:465 | `image: Item<T>, min_luminance: Item<f32>, max_luminance: Item<f32>, luminance_calc: Item<LuminanceCalculation> -> Item<T>` | element-wise | no | no | |
| Vibrance | adjustments.rs:511 | `image: Item<T>, vibrance: Item<f32> -> Item<T>` | element-wise | no | no | |
| Channel Mixer | adjustments.rs:713 | `image: Item<T>, monochrome: Item<bool>, 16× weights: Item<f32> -> Item<T>` | element-wise | no | no | `_output_channel` display-only |
| Selective Color | adjustments.rs:845 | `image: Item<T>, mode: Item<RelativeAbsolute>, 36× offsets: Item<f32> -> Item<T>` | element-wise | no | no | `_colors` display-only |
| Posterize | adjustments.rs:989 | `input: Item<T>, levels: Item<u32> -> Item<T>` | element-wise | no | no | |
| Exposure | adjustments.rs:1019 | `input: Item<T>, exposure: Item<f32>, offset: Item<f32>, gamma_correction: Item<f32> -> Item<T>` | element-wise | no | no | |
| Sample Image | std_nodes.rs:33 | `image_frame: Item<Raster<CPU>> -> Item<Raster<CPU>>` | element-wise | no | attrs | DEVIATION: today filter_maps away offscreen elements (hidden filtering); now passes through |
| Combine Channels | std_nodes.rs:97 | `red/green/blue/alpha: Item<Raster<CPU>> ×4 -> Item<Raster<CPU>>` | element-wise | no | attrs | DEVIATION: today pads short lists with zero/one fills and drops mismatched rows; resolved: longest-zip, mismatched dimensions -> default raster slot (resolution 2) |
| Mask | std_nodes.rs:178 | `image: Item<Raster<CPU>>, stencil: Item<Raster<CPU>> -> Item<Raster<CPU>>` | element-wise | no | attrs | Zip answers the code's own multi-stencil TODO. DEVIATION resolved: degenerate stencil passes the image through (resolution 2) |
| Extend Image to Bounds | std_nodes.rs:229 | `image: Item<Raster<CPU>>, bounds: Item<DAffine2> -> Item<Raster<CPU>>` | element-wise | no | attrs | |
| Empty Image | std_nodes.rs:277 | `transform: Item<DAffine2>, color: Item<Color> -> Item<Raster<CPU>>` | element-wise | no | attrs | WHITE fallback becomes `#[default(Color::WHITE)]` |
| Image | std_nodes.rs:292 | `resource: Item<Resource> -> Item<Raster<CPU>>` | element-wise | no | no | DEVIATION: decode failure -> `Raster::default()` instead of empty list |
| Noise Pattern | std_nodes.rs:316 | `clip: Item<bool>, seed: Item<u32>, scale: Item<f64>, + 12 noise params: Item<...> -> Item<Raster<CPU>>` | element-wise | no | attrs | Reads footprint eagerly (not a modifier). DEVIATION: offscreen -> default raster instead of empty list |
| Mandelbrot | std_nodes.rs:475 | `-> Item<Raster<CPU>>` | element-wise | no | attrs | Same footprint read + empty-list DEVIATION |
| Blur | filter.rs:89 | `image_frame: Item<Raster<CPU>>, radius: Item<f64>, box_blur: Item<bool>, gamma: Item<bool> -> Item<Raster<CPU>>` | element-wise | no | no | Already clean per-element |
| Median Filter | filter.rs:125 | `image_frame: Item<Raster<CPU>>, radius: Item<f64> -> Item<Raster<CPU>>` | element-wise | no | no | |
| Mix | blending_nodes.rs:144 | `over: Item<T>, under: Item<T>, blend_mode: Item<BlendMode>, opacity: Item<f32> -> Item<T>` | element-wise | no | no | DEVIATION resolved: longest-zip replaces today's min-length zip with surplus passthrough/drop (resolution 2) |
| Color Overlay | blending_nodes.rs:168 | `image: Item<T>, color: Item<Color>, blend_mode: Item<BlendMode>, opacity: Item<f32> -> Item<T>` | element-wise | no | no | |
| Image Color Palette | image_color_palette.rs:6 | `image: Item<Raster<CPU>>, count: Item<u32> -> List<Color>` | expander | no | no | DEVIATION: today pools all elements into one histogram; per-image palettes now, framed output force-flattens |
| Gradient Map | gradient_map.rs:12 | `image: Item<T>, gradient: Item<GradientStops>, reverse: Item<bool> -> Item<T>` | element-wise | no | no | DEVIATION resolved: default gradient applies; element(0) + empty-passthrough behavior dropped (resolution 5) |
| Dehaze | dehaze.rs:10 | `image_frame: Item<Raster<CPU>>, strength: Item<f64> -> Item<Raster<CPU>>` | element-wise | no | no | |

## Transform, Repeat, Blending (16 nodes)

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Transform | transform_nodes.rs:14 | `content: Context -> Item<T> (lazy), translation: Item<DVec2>, rotation: Item<f64>, scale: Item<DVec2>, skew: Item<DVec2> -> Item<T>` | element-wise | content (footprint modifier) | attrs | THE broadcast node (replaces Repeat on Points). DAffine2/DVec2 value implementations need an eager value-kernel sibling |
| Reset Transform | transform_nodes.rs:57 | `content: Item<T>, reset_translation: Item<bool>, reset_rotation: Item<bool>, reset_scale: Item<bool> -> Item<T>` | element-wise | no | attrs | Implemented as an Item kernel |
| Replace Transform | transform_nodes.rs:95 | `content: Item<T>, transform: Item<DAffine2> -> Item<T>` | element-wise | no | attrs | Implemented as an Item kernel |
| Extract Transform | transform_nodes.rs:117 | `content: Item<Dyn> -> Item<DAffine2>` | element-wise | no | attrs | DEVIATION: today reads only element 0; framing yields per-element transforms (TODO #2982 anticipated this) |
| Invert Transform | transform_nodes.rs:123 | `transform: Item<DAffine2> -> Item<DAffine2>` | element-wise | no | no | |
| Decompose Translation | transform_nodes.rs:129 | `transform: Item<DAffine2> -> Item<DVec2>` | element-wise | no | no | |
| Decompose Rotation | transform_nodes.rs:135 | `transform: Item<DAffine2> -> Item<f64>` | element-wise | no | no | |
| Decompose Scale | transform_nodes.rs:143 | `transform: Item<DAffine2>, scale_type: Item<ScaleType> -> Item<DVec2>` | element-wise | no | no | |
| Decompose Skew | transform_nodes.rs:152 | `transform: Item<DAffine2> -> Item<f64>` | element-wise | no | no | |
| Repeat | repeat_nodes.rs:12 | `content: Context -> Item<T> (lazy), count: Item<u32>, reverse: Item<bool> -> List<T>` | expander | content (frame-synth) | no | DEVIATION: multi-element upstream today flattens count×N per iteration; framed version groups per element (same total, different order) |
| Repeat Array | repeat_nodes.rs:48 | `content: Context -> Item<T> (lazy), direction: Item<DVec2>, angle: Item<f64>, count: Item<u32> -> List<T>` | expander | content (frame-synth) | attrs | Composes per-copy ATTR_TRANSFORM. Same flattening DEVIATION |
| Repeat Radial | repeat_nodes.rs:96 | `content: Context -> Item<T> (lazy), start_angle: Item<f64>, radius: Item<f64>, count: Item<u32> -> List<T>` | expander | content (frame-synth) | attrs | Same family |
| Repeat on Points | repeat_nodes.rs:142 | DELETE | expander | — | attrs | Transform broadcasting replaces it; classified for the record only |
| Blend Mode | blending/lib.rs:185 | `content: Item<T>, blend_mode: Item<BlendMode> -> Item<T>` | element-wise | no | attrs | Implemented as an Item kernel; "apply to the parent" TODO fixed |
| Opacity | blending/lib.rs:209 | `content: Item<T>, has_opacity: Item<bool>, opacity: Item<f64>, has_fill: Item<bool>, fill: Item<f64> -> Item<T>` | element-wise | no | attrs | Implemented as an Item kernel; missing attribute treated as the 1.0 implicit default, NOT f64::default() |
| Clipping Mask | blending/lib.rs:251 | `content: Item<T>, clip: Item<bool> -> Item<T>` | element-wise | no | attrs | Implemented as an Item kernel; same TODO fixed |

## Gcore (24 nodes)

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Read Graphic | context.rs:10 | `-> Item<Graphic>` | element-wise | no | no | Vararg generator. DEVIATION: today returns singleton `List<Graphic>`; binding site (`List::new_from_item`) confirms one-element-per-iteration intent |
| Read Vector | context.rs:18 | `-> Item<Vector>` | element-wise | no | no | Same |
| Read Raster | context.rs:26 | `-> Item<Raster<CPU>>` | element-wise | no | no | Same |
| Read Color | context.rs:34 | `-> Item<Color>` | element-wise | no | no | Same |
| Read Gradient | context.rs:42 | `-> Item<GradientStops>` | element-wise | no | no | Same |
| Read Position | context.rs:50 | `loop_level: Item<u32> -> Item<DVec2>` | element-wise | no | no | Context-driven generator |
| Read Index | context.rs:67 | `loop_level: Item<u32> -> Item<f64>` | element-wise | no | no | Context-driven generator |
| Real Time | animation.rs:32 | `component: Item<RealTimeMode> -> Item<f64>` | element-wise | no | no | Variation from context time |
| Animation Time | animation.rs:53 | `rate: Item<f64> -> Item<f64>` | element-wise | no | no | |
| Quantize Real Time | animation.rs:64 | `value: Context -> T (lazy), quantum: Item<f64> -> T` | infrastructure | yes (context modifier) | no | Rewrites time in context before upstream eval (finding 3) |
| Quantize Animation Time | animation.rs:104 | `value: Context -> T (lazy), quantum: Item<f64> -> T` | infrastructure | yes (context modifier) | no | Same |
| Pointer Position | animation.rs:143 | `-> Item<DVec2>` | element-wise | no | no | |
| Log to Console | debug.rs:8 | `value: Item<T> -> Item<T>` | element-wise | no | no | Rank 0 means one log line per element when framed (debatable but consistent) |
| Size Of | debug.rs:16 | DELETE | element-wise | no | no | Vestigial and unused (resolution 8) |
| Some | debug.rs:22 | DELETE | element-wise | no | no | Vestigial; existed to make Option values, which wires ban (resolution 8) |
| Unwrap Option | debug.rs:28 | DELETE | element-wise | no | no | Vestigial; paired with Some (resolution 8) |
| Clone | debug.rs:34 | (by-reference wire test) | infrastructure | no | no | By-reference connectors are a question mark under Item/List wires |
| Passthrough | ops.rs:9 | `content: Item<T> -> Item<T>` | element-wise | no | no | Identity at any rank |
| Into | ops.rs:14 | `value: Item<T> -> Item<O>` | element-wise | no | no | Compiler-inserted conversion |
| Convert | ops.rs:19 | `value: Item<T> -> Item<O>` | element-wise | no | no | Value conversion; eager footprint read |
| Memoize | memo.rs:13 | (evaluation-control) | infrastructure | yes (exempt category) | no | Purpose IS skipping upstream evaluation (finding 2) |
| Monitor | memo.rs:39 | (evaluation-control) | infrastructure | yes (exempt category) | no | Compiler-inserted introspection tap |
| Extract XY | extract_xy.rs:9 | `vector: Item<DVec2>, axis: Item<XY> -> Item<f64>` | element-wise | no | no | Textbook rank-0 kernel |
| Context Modification | context_modification.rs:15 | (compiler-inserted) | infrastructure | yes (context modifier) | no | Strips context before upstream eval |

## Gstd / render pipeline (21 nodes)

| Node | File:Line | Proposed connectors -> output | Class | Lazy | Attrs | Notes |
|---|---|---|---|---|---|---|
| Get Request | platform_application_io.rs:49 | `url: Item<String>, discard_result: Item<bool>, headers: Item<String> -> Item<String>` | element-wise | no | no | Framing a URL list issues N requests |
| Post Request | platform_application_io.rs:82 | `url: Item<String>, body: Item<Blob>, discard_result: Item<bool>, headers: Item<String> -> Item<String>` | element-wise | no | no | Blob unification adopted (resolution 9) |
| String to Bytes | platform_application_io.rs:119 | `string: Item<String> -> Item<Blob>` | element-wise | no | no | Blob unification adopted (resolution 9) |
| Image to Bytes | platform_application_io.rs:125 | `image: Item<Raster<CPU>> -> Item<Blob>` | element-wise | no | no | Blob unification adopted (resolution 9). DEVIATION: today reads only element(0) |
| Load Resource | platform_application_io.rs:140 | `url: Item<String> -> Item<Arc<[u8]>>` | element-wise | no | no | Already rank-0 blob; failure -> empty placeholder (matches Default) |
| Decode Image | platform_application_io.rs:164 | `data: Item<Arc<[u8]>> -> Item<Raster<CPU>>` | element-wise | no | no | DEVIATION: decode failure -> `Raster::default()` instead of empty list |
| Create Canvas | platform_application_io.rs:188 | (wasm surface factory) | infrastructure | no | no | |
| Rasterize | platform_application_io.rs:195 | `data: List<T>, footprint: Item<Footprint>, canvas: (infra) -> Item<Raster<CPU>>` | reducer | no | attrs | Whole scene -> one raster. DEVIATION: degenerate footprint -> default raster instead of empty list |
| Editor API | platform_application_io.rs:265 | (scope) | infrastructure | no | no | |
| Resource | platform_application_io.rs:270 | `hash: Item<ResourceHash> -> Item<Resource>` | element-wise | no | no | DEVIATION: panics on missing today; becomes `Resource::default()` + logged error (no-panic policy) |
| Wgpu Executor | platform_application_io.rs:278 | (scope) | infrastructure | no | no | |
| Try Wgpu Executor | platform_application_io.rs:288 | (scope) | infrastructure | no | no | Option on infra scope wire: exempt from no-Option rule |
| Render Intermediate | render_node.rs:26 | (render sink) | infrastructure | flag | no | Lazy connector fails both criteria as written but plausibly inherits evaluation-control justification from the sink chain |
| Render | render_node.rs:77 | (render sink) | infrastructure | no | no | |
| Create Context | render_node.rs:150 | (context synthesizer) | infrastructure | yes (context modifier) | no | Synthesizes footprint/time/pointer/varargs before upstream eval |
| Render Pixel Preview | render_pixel_preview.rs:11 | (render sink) | infrastructure | yes (context modifier) | no | Modifies Footprint for logical-resolution upstream |
| Pixel Preview Pipeline | render_pixel_preview.rs:78 | (pipeline warm-up) | infrastructure | no | no | |
| Render Background | render_background.rs:15 | (final composite) | infrastructure | no | no | |
| Composite Background Pipeline | render_background.rs:121 | (pipeline warm-up) | infrastructure | no | no | |
| Render Output Cache | render_cache.rs:327 | (tile cache) | infrastructure | yes (context modifier + frame-synth) | no | Re-evaluates upstream per missing tile with synthesized region Footprints |
| Upload Texture | texture_conversion.rs:252 | DELETE (deprecated for Convert) | element-wise | no | no | Per-element CPU->GPU upload preserving attributes — exactly what framing provides |
