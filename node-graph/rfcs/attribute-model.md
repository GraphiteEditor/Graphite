# Summary

Give every item flowing through the graph a set of named, typed attributes
next to its primary `element` value. Attributes are stored as a packed
record whose layout the compiler computes at graph compile time. Nodes
declare their attribute reads and writes in their signatures, and the
compiler resolves every access to a byte offset during wiring, so there is
no name lookup at runtime. Storage and batch results are per-attribute
columns. The contiguous record only exists as a per-lane view, assembled
into buffers the compiler assigns. All of the machinery that could
corrupt a layout is generated code, so getting it wrong is a type error or
a graph compile error rather than undefined behavior.

# Motivation

Attributes currently exist as string-keyed pairs of boxed trait objects
carried inside `List<T>`:

```rs
pub struct List<T> {
	element: Vec<T>,
	attributes: Vec<(String, Box<dyn AnyAttributeValue>)>,
}
```

Every access does a string comparison and a downcast, every value is
boxed, and merging eagerly pads missing attributes with materialized
defaults. On a ten-node chain with eight attributes over 64k items this
costs us around 500ns per item. The design described here measures
between 3.5 and 47ns on the same workload, depending on the execution
mode, and the cost is mostly independent of the attribute count.

There is also a cost at compile time and in the node catalog. Because
attributes ride inside `List<T>`, a node that touches a property needs
per-type traits (`MultiplyAlpha` and kin) and an implementations list
enumerating every carrier type. Each row monomorphizes, adding a new
carrier type means editing every one of these lists, and the duplicated
instantiations show up in the build size. The blending nodes also carry
a TODO ("find a way to make this apply once to the list's parent rather
than applying to each item") that the current representation cannot
express at all: opacity on a group and opacity on each member composite
differently once members overlap, so the difference is semantic, and
there is currently nowhere to put it.

The requirements, briefly. Attributes are named with strings and work
for all types, and users can author read/write nodes with custom names.
A node placed before vs. after a structure node affects different
nesting levels. Items whose element types agree can merge regardless of
their attribute sets, with missing values filled from name-specific
defaults. Names resolve at graph compile time, with a dynamic escape
hatch for runtime-shaped data. A wire without attributes costs what a
plain wire costs, and an attribute that is constant across a domain
costs one slot rather than one per element. Batch access is the case to
optimize, and scalar access should not require a second representation
with conversions between the two.

# Guide-level explanation

## What an attribute is

An item is a primary value (the `element`, which determines the wire's
type and colour) plus a set of named attributes that flow along with it.
A node can read, add, or overwrite one attribute without touching the
element and without knowing which other attributes exist. Lists carry
attributes at every nesting level, so an attribute on a group is a
different thing from the same attribute on the group's members.

## Declaring an attribute

An attribute name is declared once, as a marker type:

```rs
#[attribute(name = "opacity", default = 1.)]
pub struct Opacity;
```

This fixes the name, the value type, and the name-specific default
(opacity should default to fully opaque, not to `f64::default()`). The
registry collects the declarations into a census, and a misspelled name
in a document can be diagnosed with a nearest-match suggestion. For
names declared in code, one name belongs to one marker, so a name can
never mean two different types. For user-supplied names, the marker
fixes the value type and the default in code, while the name itself
arrives as a constant text input on the document node. It joins the name
table at graph compile time, which is where every resolution happens
anyway, and two user-supplied names colliding at different value types
is a graph compile error naming both nodes.

## Reading and writing attributes

A node declares its attribute io in its signature. The opacity node
becomes:

```rs
/// Modifies the opacity of the input by multiplying the existing value by this percentage.
#[node_macro::node(category("Blending"))]
fn opacity<T>(
	_: impl Ctx,
	element: T,
	/// How visible the content should be, from 100% (fully opaque) to 0% (fully transparent).
	#[default(100.)]
	factor: Percentage,
	opacity: Attr<Opacity>,
) -> (T, Attr<Opacity>) {
	(element, Attr(*opacity * factor / 100.))
}
```

`Attr<Opacity>` as a parameter is a read (it yields the declared default
if nothing upstream wrote the attribute), in the return tuple it is a
write, and the same marker on both sides is a modify. The first
parameter after the context is the primary input, and an unbounded
generic `element: T` that is returned in the first tuple position means
"I pass the element through unchanged". The compiler lowers this to a
byte copy (often to nothing, see below), and a single compiled instance
covers every element type, with no trait bounds and no implementations
list. A node that actually computes on the element uses a concrete type
or a bound instead and monomorphizes per its implementations list.
`_: ()` means "no primary input".

## Levels: before vs. after a structure node

Where a node sits in the chain decides which nesting level it affects.
Applying the opacity node to a shape and then repeating it gives every
copy its own opacity. Repeating first and then applying opacity sets one
value on the whole group, which composites differently where copies
overlap. The node's code is identical in both cases. Reads and writes
bind to the top level of the wire at the node's position in the chain,
and Repeat pushed a level in one of the two arrangements. Reaching an
inner level from outside is an explicit map/enter construct, so "set on
the parent" and "map over the children" are visibly different graphs.

## Structure nodes

A node that produces a list declares the new level's extent and writes
per-copy values:

```rs
/// Instances the content a number of times, spaced by the direction vector.
#[node_macro::node(category("Repeat"), level_extent = count)]
fn repeat<T>(
	ctx: impl Ctx + ExtractIndex,
	element: T,
	#[default(1)]
	#[hard(1..)]
	count: u32,
	#[default(100., 100.)]
	direction: DVec2,
	transform: Attr<Transform>,
) -> List<(T, Attr<Transform>)> {
	let offset = direction * ctx.innermost_index() as f64;
	emit(element, Attr(DAffine2::from_translation(offset) * *transform))
}
```

The body is one lane of the declared list: the kernel reads its own copy
index and produces that copy's values. `level_extent = count` declares
the size of the new level, and the compiler derives the structural parts
from that one declaration: the multiplication with the carrier's extent,
and the index decomposition that routes an output lane to the right copy
and content item. The `emit(...)` tail marks the one-lane form and
doubles as the tuple constructor, and it is optional. A list return without
a `level_extent` is the store form, whose body produces the whole level
at once. That form is for nodes like string splitting, where the extent
cannot be known without running.

## Merging

Merging concatenates. The merged attribute set is the union of the
inputs', and an attribute missing on one side is filled with its declared
default for that side's items, so the result is rectangular in every
attribute. A scalar input contributes one item. When lists are combined,
each input's own top-level attributes are pushed down onto that input's
items (composing by the attribute's declared rule where one exists,
otherwise the inner value wins), and the merged list starts with an empty
top level. If the user wants to keep the groups as groups, they wrap
explicitly instead.

## Selecting

Switch takes two lazy inputs and returns one of them, for any carrier,
without an implementations list:

```rs
/// Evaluates either the "If True" or "If False" input branch based on the condition.
#[node_macro::node(category("Math: Logic"))]
fn switch<T>(
	ctx: impl Ctx,
	_: (),
	condition: bool,
	#[expose]
	if_true: impl Node<Context<'_>, Output = T>,
	#[expose]
	if_false: impl Node<Context<'_>, Output = T>,
) -> T {
	if condition { if_true.eval(ctx) } else { if_false.eval(ctx) }
}
```

An unbounded generic on a lazy input means the whole record flows
through. Evaluating a branch yields an opaque value carrying its record,
and whatever value the kernel returns is the output, element and
attributes together. Kernels can evaluate several inputs, hold the
results side by side, and pick among them with any logic, so fallback,
N-way multiplexers, and per-lane data-driven selection are the same
two-line pattern rather than new node kinds. The branches may carry
different attribute sets. The output carries their union, filled with
defaults per branch. A lazy input with a concrete output type is an
ordinary value input: its value flows, the attributes on its wire do
not.

The rest of the authoring surface composes. Categories, per-parameter
doc comments, `#[default]`, `#[hard]`, `#[expose]`, widget overrides, and
the kernel dialects (`Result<_, Interrupt>` with `?`, `GPoll` returns,
async sources) all compose with the forms above.

# Reference-level explanation

## Records and layouts

A record is the element at offset 0 plus one field per written attribute,
aligned to the widest field. Since the element comes first, a pointer to
the record is also a valid pointer to the element. Element-only
consumers are wired without adaptation, the wire keeps the element's type
and colour, and the registry stays keyed on element types.

A wire's layout is the set of all attributes written in its upstream
cone, in a canonical order (descending alignment, then size, then name
and level), computed at graph compile time. Some consequences:

- Layout identity is captured by stable node ids, because the write set
  is part of the hashed upstream cone. An instance that survives an
  incremental recompile cannot meet a changed layout.
- Reads resolve to `Option<offset>` at wiring. Present means a field
  access, and absent means the macro emits the default constant. Writes
  always resolve. The runtime does no name lookup, no hashing, and no
  downcasting. A resolved read costs the same as a native struct field
  access (0.43ns).
- Writes that are never read are diagnosed. Eliding them is a permitted
  whole-graph optimization but not required. Keeping them in the layout
  is what keeps the layout a pure function of the upstream cone.
- Layouts are derived data. The document stores only user-visible
  structure, no attribute data is serialized, and representation changes
  never require a document migration.

Fields are `Copy`, and larger payloads go behind a pointer-sized field.
Runtime-shaped data (CSV columns, arbitrary JSON) is a single dynamic
attribute holding a map in a fixed-size slot. It is the intended slow
path and puts no constraints on the fast one. Layouts are always static.

A name's type is unique by construction. For declared markers the census
admits one marker per name, checked when the registry is built. For
user-supplied names the binding forms at graph compile time, carrying the
marker's declared value type, and two names colliding at different types
is a graph compile error that names both nodes. We do not attempt
coercion.

## Levels and residency

Levels are numbered from the innermost out. This keeps layout keys
stable when a structure node pushes a level (nothing renumbers) and
matches how indices are already numbered. The binding rules are:

- A read or write binds to the top level of the wire at the node's chain
  position.
- A structure node pushes a level and then writes its per-copy
  attributes into the former top row, and the new top row starts empty.
- A node that reads the element (concrete type or bound) is pinned to
  level 0. An element-agnostic node binds to whatever the top currently
  is, which is also what allows a pure attribute node to run at a level
  where no element is materialized at all.

An attribute at level j ignores indices deeper than j by definition, so
the level a value's storage actually varies with (its residency) lies
somewhere between its binding level and the root. The compiler computes
residency with the same index-invariance analysis used for context
nullification. Constant-everywhere is residency at the root: one slot.
A per-item attribute that only varies per group is bound at level 0 but
resident at level 1, so it gets one slot per group rather than one per
item.

Storage is level-resident and columnar, and the contiguous record is a view.
Per-lane consumers get the view assembled across levels and columns into
their activation frames. Reads across a level boundary use the same
index decomposition the structure nodes already perform, and in batches
that decomposition is hoisted per run.

## Runtime representation

- Every node's per-lane output is an activation frame on a per-thread
  record stack, the shape of an ordinary call stack: an evaluation
  claims its frame at the stack pointer, evaluates its carrier beyond
  it, and releases on completion, leaving the returned record readable
  until the next claim. A node with several record sources lays their
  regions side by side, so values held across sibling evaluations
  survive. "Allocating" a result is pointer arithmetic; frames are
  overwritten each lane, transients never touch the arena, and
  publishing into a cache copies out of the stack.
- No global slot assignment exists: a node's wiring state is its own
  frame size, so incremental recompiles and instance reuse cannot
  invalidate storage, and the stack belongs to whichever thread runs
  the evaluation, created lazily in thread-local storage, so worker
  counts never enter wiring. The total stack bound is derived by the
  wiring layer from the same layouts it computes (own frame plus
  carrier need, maxed over value inputs, summed over sources) and
  reserved once per evaluation.
- This imposes one rule: a borrow of a released frame must not survive
  the next claim. Consuming by copy is always fine, and the per-source
  regions above make kernel-held record values safe by construction.
- Batch results are per-field columns, each statically Varying (an
  array) or Uniform (a single value) per the residency analysis. A node
  that does not touch a column forwards the pointer, so bypass costs
  nothing, and uniform columns give constant attributes their one-slot
  cost regardless of lane count. Both execution forms share one layout
  descriptor, and crossing from a batched producer to a per-lane consumer
  costs about 1.5ns per lane through a lane-view adapter.
- Alignment padding only exists in the per-lane view. In a row, a `u8`
  element costs the same as a `u64`, while
  packed columns keep the cost proportional to the element size (2x
  cheaper than rows when cache-resident, around 8x when memory-bound).
  Columns are the storage format, so the proportional cost holds
  wherever data accumulates, and the padding only survives in transient view
  slots, whose number is bounded by graph depth.

## Kernel io lowering

| Signature form | Meaning | Lowering |
| --- | --- | --- |
| first non-context param | primary input | carrier record |
| `_: ()` | no primary input | no carrier edge |
| `element: T` (unbounded, returned first) | explicit passthrough | erased byte carry, where `T` is instantiated with a zero-sized token, so the routing is checked by the type system and costs nothing |
| `element: Concrete` / bound | element read | field read at offset 0, monomorphized per implementations list, binds level 0 |
| `x: Attr<A>` | attribute read | offset read, or the default constant |
| `Attr<A>` in the return tuple | attribute write | offset write into the output record |
| `keys: List<K>` | whole-extent input | wired edge, evaluated over its extent into a view |
| plain parameters | wired value inputs | ordinary wired edges |
| `impl Node<Context<'_>, Output = Concrete>` | lazy value input | the value flows, attributes do not |
| `impl Node<Context<'_>, Output = T>` (unbounded) | source of an opaque record family | routing, see below |
| `-> List<W>` with `level_extent =` | per-lane level production | structural skeleton emitted by the macro |
| `-> List<W>` without | store form | whole-level body, node owns storage |

`level_extent =` names a parameter, or a function over the node's values
(author code never receives the node struct). The compiler derives both
the extent formula and the matching index decomposition from this one
declaration, which is what keeps them consistent. `emit(...)` is an
optional tail marker for the per-lane form whose parentheses double as
the tuple's, so multi-write lanes pay no extra nesting.

The rule behind all the lazy forms: kernels control whether, when, and
at which index their inputs are evaluated, but never how the records
move. Attributes travel inside record values or
through generated machinery, so kernel-controlled evaluation cannot
misalign them, and domain declarations stay with the extent system.

## Structure shapes

A structure node pairs an extent composition with an index
decomposition, and both come from one declaration:

- Multiplicative (Repeat, map/enter): the extent is the new level's
  count times the carrier's. A flat index splits by division into the
  copy index (pushed as a level) and the content index. Batches split
  into maximal per-copy runs.
- Additive (Merge): the extent is the sum of the inputs'. A flat index
  range-splits into a segment and a local index, so per lane, merge is a
  selector whose condition is the index, and the selector machinery
  below is reused as-is. Item rows union with per-segment default fill.
  Each input's top row is pushed down one level onto that input's items
  via entries in the translation plan (a level remap computed at wiring;
  no values are needed at compile time), composing by the declared
  combine rule with inner-wins as the fallback. The merged top row
  starts empty. An explicit Wrap node is how the user nests instead. An
  input with unbounded (Free) extent contributes exactly one item, so
  merge is an extent-forcing boundary, which is the scalar base case.
  Batched merge forwards per-segment sub-ranges to its inputs, so
  column uniformity survives concatenation per segment, and default
  materialization is only paid on the per-lane and store paths.

## Opaque record values

An unbounded generic names a family of opaque record values. Its
sources are the lazy inputs whose `Output` is the generic; the element
passthrough is the same mechanism with the carrier as the family's only
source. Wiring computes the union of the sources' layouts and a
translation plan per source (field moves plus default fills). The
kernel-facing handles wrap the edges the same way the error dialect
wraps status plumbing: evaluating a source evaluates its edge at the
unchanged context and yields a value carrying the resulting record,
either through the plan into that source's own buffer, or, when the
source's layout already equals the union, by forwarding the record
pointer untouched. The forwarding case compiles to a conditional move
plus a tail call; the +4.7ns per lane of a two-branch switch is the
condition and ordinary branch misprediction, and a translating source
costs +6.5ns per lane at eight attributes.

The kernel routes these values as ordinary Rust values. It can evaluate
any source any number of times, hold several results at once (per-source
buffers keep them valid side by side), pass them through helper
functions, and return any of them. The returned value's record is the
node's output, so provenance is carried by the value itself: element and
attributes travel together, and returning a result obtained before some
later evaluation is well-defined. A value is live until its own source
is evaluated again, which overwrites that source's buffer; a kernel that
needs two results of one input side by side declares the input twice.
The values are opaque and unforgeable, and inspecting one requires
bounds on the generic, which is element access and monomorphization as
usual.

This is the general form of selection: switch, fallback, N-way
multiplexers, and per-lane data-driven choice among inputs are all plain
kernels over the same mechanism, and none of them needs anything from
the macro beyond the family lowering. Whole-list switching vs. per-item
zip is just the residency of the condition: an invariant condition
collapses through nullification, a varying one selects per lane.

The one-source shape also covers the registry's infrastructure rows.
Monitor, context modification, memoize, and the lend and clone adapters
are all `T -> T` passthroughs with a side effect. Over the record family
each is a single generic node: the record forwards, and the side effect
is orthogonal to the type (a reflective snapshot through the layout
descriptor, a derived context, or a persistence copy sized by the
layout). Persisting a non-Copy element needs a clone and drop function
per element type, registered once beside the type itself rather than
once per infrastructure node, so the per-type surface is types plus
nodes rather than types times nodes, and compiler-inserted
infrastructure splices one generic proto node without naming value
types. The genuine conversion rows (the Into and Convert matrix)
remain, because those do real per-type work.

A kernel that modifies the index on the context evaluates an input at a
lane other than its own, which makes index-computable reorders plain
kernels:

```rs
/// Reverses the order of the input list.
#[node_macro::node(category("General"))]
fn reverse<T>(
	ctx: impl Ctx + DeriveCtx + ModifyIndex,
	_: (),
	content: impl Node<Context<'_>, Output = T>,
) -> T {
	content.eval(&ctx.with_index(content.extent(&ctx)? - 1 - ctx.innermost_index()))
}
```

Shift, slice, and read-item-at-index are the same shape. Sort and
shuffle still compute a whole-extent permutation once per sweep, which a
pure per-lane kernel cannot hold, so they keep the remap-returning
kernel. Applying a remap has a spec: per lane, evaluate the input at the
permuted index. The generated batch kernel is the law-bound override of
that spec, materializing the input's columns once
and gathering each index-varying column through the wiring-resolved
table with index-invariant columns skipped (about 1.4ns per varying
column per lane; the comparison work of the sort itself does not depend
on the representation). For a bijective permutation the per-lane spec
already costs the same number of upstream evaluations as direct
consumption, so the batch form buys cache locality and run coherence
rather than correctness.

## Compiler passes

Everything happens at graph compile time. The census is assembled from
the marker declarations (names, types, defaults, combine rules). Each
wire's layout is constructed from its upstream write set, and residency
comes from the index-invariance analysis. Offsets are resolved into node
state, the stack bound is folded from the layouts, and union and
translation plans are built at selectors and merges. A per-name
dependency analysis feeds the cache keys. The diagnostics produced along
the way are unknown or misspelled names (checked against the census,
with nearest-match suggestions), custom-name collisions, reads that some
evaluation path cannot satisfy, and layout conflicts. The runtime checks
nothing. Debug assertions guard the generated code against itself at
wiring boundaries, following the existing precedent for TypeId checks.

## Soundness

Attributes only move through generated machinery. Kernels receive
dereferenced values and opaque handles, and the translation and carry
plans behind them are emitted from wiring-resolved layouts. Layout
identity is captured by stable node ids, so an instance that survives a
recompile can never meet a changed layout. Because layouts are functions
of wires rather than of anything a kernel controls, safe kernel code can
make semantic mistakes (evaluating an input it did not need to) but
cannot misalign an offset. Kernels see contexts only as an opaque
`impl Ctx + ...` they cannot construct, and lifetimes keep them from
stashing handles in node state. The one remaining discipline lives
inside generated code (a result buffer must not be borrowed across a
sibling evaluation it could alias), and debug assertions guard it at
wiring boundaries, following the existing precedent for TypeId checks.

# Drawbacks

- The node macro absorbs real complexity: io classification, layout
  bookkeeping, the structural skeletons, and the record-family lowering are all
  generated code. That is the point (authors stay simple, the privileged
  surface stays auditable), but macro diagnostics will need work to stay
  better than raw trait-solver errors.
- Changing a document's attribute set changes layouts, which recompiles
  the affected cone and reconstructs its instances. This is the same
  cost class as editing node parameters today, but a runtime-map design
  would absorb attribute renames without recompiling.
- Until an elision pass exists, attributes that are written but never
  read occupy slots and copies.
- Transient per-lane views pad small elements up to the record
  alignment, and only the columnar storage is footprint-proportional.
- Two execution forms (per-lane views and columnar batches) are more
  machinery than one representation. They share a single layout
  descriptor, and the measured seam between them is about 1.5ns per
  lane, but the machinery still has to exist.

# Rationale and alternatives

- Keep runtime maps (the current implementation): roughly 500ns per item
  on the reference chain and ~57ns marginal per attribute, an allocation
  per value, and no compile-time name checking. Interning the keys
  improves the constant (about 1.9ns per access vs. 0.43 for a resolved
  offset) but keeps a per-access search and rules out the structural
  optimizations that need static layouts: bypass, uniform columns, slot
  coalescing.
- Attributes as separate graph edges, one channel per attribute: bypass
  and per-channel caching become graph structure. We prototyped and
  measured this. Without caching at fan-outs, every channel re-evaluates
  the shared upstream work (2-4x slower on realistic chains), and the
  cache that fixes it stores a multi-channel result, which is a record,
  so the fixed version converges on this design while keeping the extra
  edges, dispatch, and graph inflation. The two structural insights of
  the channel model survive here as the column structure of batch
  results.
- Typed attribute tuples in the wire type: layouts become
  document-dependent types, which the registry's precompiled constructor
  rows cannot cover, and row polymorphism leaks into type resolution.
  Keeping layouts as side metadata means attribute sets never gate
  convergence (merge unions them, defaults answer switch mismatches) and
  the resolver is untouched.
- The numbers cited throughout come from a reference prototype with
  type-erased node edges (the indirect calls were verified in the
  disassembly), thin LTO, 64k-lane workloads, and best-of-nine timing.
  Chain results use ten nodes and eight f64 attributes.

# Prior art

Attributes were specified in issue #3779 and first implemented by the
Item and List wire types work, which remains the behavioral reference
for this design: name-specific defaults, merge with default fill, and
the Data panel's presentation of items all carry over, and the wire
rank display and Data panel belong to the editor and are unaffected
here. One behavior is refined rather than kept: flat merge previously
had to drop one input's top-level attributes, which the push-down rule
now preserves. The present representation (string-keyed storage inside
`List<T>`, with per-carrier implementations rows) is what the Motivation
section measures. This RFC keeps its observable behavior while replacing
the storage and registration strategy underneath.

Outside Graphite, the nearest prior art is row polymorphism in records
(Rémy; PureScript and Elm) for the layout unions, ECS archetype storage
for resolved column handles, and the uniform vs. varying distinction
from shading languages for residency.

# Unresolved questions

- Where and how the combine rule is declared on the attribute marker.
  Merge push-down and flatten both consume it, and inner-wins is the
  implemented fallback.
- The macro spelling of the additive structure shape. Merge is
  currently a hand-written reference lowering, and it has no domain
  logic of its own, so it is not clear what a kernel for it would even
  contain.
- The graph UX of the map/enter construct, and whether attribute-typed
  parameters can ever be ordinary exposed inputs.
- Naming: `Attribute` trait vs. `Attr` wrapper, and whether the
  authoring `List` sharing the wire type's name helps or confuses.
- Whether evaluating at a lane outside the input's extent is clamped,
  wrapped, or a debug assertion.
- `List<List<W>>` outputs, i.e. one node pushing two levels.
- How chatty the editor boundary becomes per frame, given that tools
  consume materialized views today.

# Future possibilities

- Scope variables: varargs with graph-compile-time-known names, the
  context-side mirror of this design. The same census and marker
  machinery, reads resolved to a hop count into a stack-allocated chain
  (0.43ns through two hops in our measurements), pushes that are free of
  allocation (0.56ns), and injection handles that make a missing or
  doubled push unrepresentable. This shrinks the context to a hot core
  and replaces coarse context features with per-name dependencies in
  cache keys.
- Write elision for never-read attributes, once the whole-graph analysis
  pass exists.
- Mask-run decomposition in the selector's batch kernel: dense
  sub-ranges for uniform condition runs, and optionally
  compute-both-and-select speculation, which purity makes legal.
- GPU consumption: uniform vs. varying columns map directly onto
  constant buffers vs. vertex attributes.
- A user-routable remap value (shuffle, manual orderings, an apply-remap
  node) built on the sort machinery.
