# Summary

Give every item flowing through the graph a set of named, typed attributes
next to its primary `element` value. Attributes are stored as a packed
record whose layout the compiler computes at graph compile time. Nodes
declare their attribute reads and writes in their signatures, and the
compiler resolves every access to a byte offset during wiring, so there is
no name lookup at runtime. Storage and batch results are per-attribute
columns. The contiguous record only exists as a per-lane view, assembled
into frames claimed from the evaluation's frame space. All of the machinery that could
corrupt a layout is generated code, so getting it wrong is a type error or
a graph compile error rather than undefined behavior.

# Motivation

Attributes currently exist as a string-keyed store of type-erased
columns beside the elements:

```rs
pub struct List<T> {
	element: Vec<T>,
	attributes: Attributes,
}

struct Attributes {
	attributes: Vec<(String, Box<dyn AnyAttribute>)>,
	len: usize,
}
```

So the storage is already columnar: one boxed column per name, each
holding `len` values. What it is not is resolved. Every access searches
the key list by string comparison and downcasts the column, and merging
eagerly pads missing attributes with materialized defaults. Scalar
access boxes per value at the item boundary, where the same store
appears as `ItemAttributeValues(Vec<(String, Box<dyn AnyAttributeValue>)>)`.
On a ten-node chain with eight attributes over 64k items this costs us
around 500ns per item. The design described here measures between 3.5
and 47ns on the same workload, depending on the execution mode, and the
cost is mostly independent of the attribute count.

There is also a cost at compile time and in the node catalog. Because
attributes ride inside `List<T>`, a node that touches a property carries
an implementations list enumerating every carrier type: the opacity and
blend-mode nodes each name seven. Each row monomorphizes, adding a new
carrier type means editing every one of these lists, and the duplicated
instantiations show up in the build size, measured at 6.16 MB and about
4% of frame rate when the rank work landed. An escape hatch exists for
the narrow case of reading an attribute without the element (`ListDyn`,
which erases the carrier), but it does not extend to nodes that write.
Separately, opacity on a group and opacity on each member composite
differently once members overlap, so the two are semantically distinct
and the representation has nowhere to record which one a node meant.

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
attribute! {
	/// How visible the content is.
	pub Opacity("opacity"): f64 = 1.;
	/// The item's label, parked in the arena by the writer.
	pub Label("label"): &str;
}
```

Each entry expands to the marker struct, its `Attribute` impl, and the
census registration. This fixes the name, the value type, and the
name-specific default
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

A write can also be generic over both the name and the value type. The
attribute then arrives on its own wire, as an input whose element is
`()`:

```rs
/// Attaches the attribute to the content.
#[node_macro::node(category("Attributes"))]
fn set_attribute<T, A, Y>(
	_: impl Ctx,
	element: T,
	(_, attr): ((), Attr<Custom<A, Y>>),
) -> (T, Attr<Custom<A, Y>>) {
	(element, attr)
}
```

A unit value component means the edge exists and carries only its
attributes (`_: ()` still means no edge at all). The name enters the
graph at a source node holding the constant text input, whose output
type is filled at graph compile time, where user-supplied names join
the name table anyway; the compiler pairs `A` and `Y` through the wire
types, so the write set is derived from types alone, and the
one-name-one-type check covers the binding, making a declared name
targeted at a different type a graph compile error. A generic read
resolves only when the input wire's type determines the binding
uniquely, and anything else is a validation error. The node is one
compiled instance: `A` and `Y` instantiate with tokens and the value
rides the copy plan as a byte move, parked in the arena when its type
has drop glue, so no implementations list exists. A kernel that
computes on the value uses a bound and monomorphizes per its
implementations list as usual.

## Reading and writing attributes

A node declares its attribute io in its signature. A parameter that
reads attributes destructures its input into the wired value and the
reads taken from that input's wire. The opacity node becomes:

```rs
/// Modifies the opacity of the input by multiplying the existing value by this percentage.
#[node_macro::node(category("Blending"))]
fn opacity<T>(
	_: impl Ctx,
	(element, opacity): (T, Attr<Opacity>),
	/// How visible the content should be, from 100% (fully opaque) to 0% (fully transparent).
	#[default(100.)]
	factor: Percentage,
) -> (T, Attr<Opacity>) {
	(element, Attr(*opacity * factor / 100.))
}
```

An `Attr<A>` inside a parameter tuple is a read from that parameter's
wire (it yields the declared default if nothing upstream wrote the
attribute), an `Attr<A>` in the return tuple is a write, and the same
marker on both sides is a modify. A `RemoveAttr<A>` in the return
tuple is a delete: the name leaves the output layout, downstream reads
yield the default again, and the column leaves the Data panel. A read
binds to the input it is destructured from, so which wire an attribute
comes from is always explicit in the signature, and secondary inputs
declare reads the same way:

```rs
	(factor, out_of_100): (Percentage, Attr<OutOf100>),
```

There is no implicit attribute flow between inputs. The primary
input's attributes pass through to the output, overwritten where the
node writes; a secondary input contributes exactly the reads its tuple
names. An input without reads stays a plain parameter. Parameter
attributes (`#[default]`, `#[implementations]`, doc comments) apply to
the value component; attribute markers are concrete types and never
enter monomorphization.

The first parameter after the context is the primary input, with or
without a read tuple, and an unbounded generic `element: T` in its
value position that is returned in the first tuple position means
"I pass the element through unchanged". The compiler lowers this to a
byte copy (often to nothing, see below), and a single compiled instance
covers every element type, with no trait bounds and no implementations
list. A node that actually computes on the element uses a concrete type
or a bound instead and monomorphizes per its implementations list.
`_: ()` means "no primary input".

## Levels: before vs. after a structure node

This part of the design is not settled, and the first implementation
resolves every attribute at a single level. It is written up here
because the requirement is real and the rest of the model has to leave
room for it; see the open question at the end.

Where a node sits in the chain should decide which nesting level it
affects. Applying the opacity node to a shape and then repeating it
would give every copy its own opacity. Repeating first and then applying
opacity would set one value on the whole group, which composites
differently where copies overlap. The node's code is identical in both
cases. Reads and writes bind to the top level of the wire at the node's
position in the chain, and Repeat pushed a level in one of the two
arrangements. Reaching an inner level from outside is an explicit
map/enter construct, so "set on the parent" and "map over the children"
are visibly different graphs.

## Structure nodes

A node that produces a list declares the new level's extent and writes
per-copy values. The body is one lane of that list. There are two
shapes, and which one applies is decided by whether the copies have to
re-evaluate the content.

The general shape takes the content lazily and evaluates it once per
copy, at that copy's index:

```rs
/// Instances the content a number of times, spaced by the direction vector.
#[node_macro::node(category("Repeat"), extent(repeat_extent))]
fn repeat<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex,
	content: impl Node<Context<'_>, Output = (T, Attr<Transform>)>,
	#[default(1)]
	#[hard(1..)]
	count: u32,
	#[default(100., 100.)]
	direction: DVec2,
) -> Result<IList<(T, Attr<Transform>)>, Interrupt> {
	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);
	if copy >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	let offset = direction * copy as f64;
	let mut frame = IndexLink { index: 0, outer: None };
	let (element, transform) = content.eval(&ctx.push_level(&mut frame, copy, rest))?;
	Ok((element, Attr(DAffine2::from_translation(offset) * *transform)))
}
```

The kernel splits the flat index into its own copy index and the index
below it, then evaluates the content at a context with the copy pushed
as a level. `DeriveCtx` is what admits the push, and only a lazy input
can receive a derived context, so this shape is exactly the one where
the copies are allowed to differ: anything upstream that reads the index
sees the copy it is being evaluated for.

Where the copies do not re-derive the content, the input stays an
ordinary carrier and the kernel only computes the copy's own attributes:

```rs
#[node_macro::node(category("Repeat"), extent(repeat_opacity_extent))]
fn repeat_opacity(ctx: impl Ctx + ExtractIndex, element: f64, count: u32) -> IList<(f64, Attr<Opacity>)> {
	emit(element, Attr(ctx.index() as f64))
}
```

A carrier is evaluated once for the whole run rather than per lane, so
every copy sees the same element and the variation lives entirely in the
attributes. The `emit(...)` tail marks the one-lane form and doubles as
the tuple constructor, and it is optional.

## Merging

Merging concatenates. The merged attribute set is the union of the
inputs', and an attribute missing on one side is filled with its declared
default for that side's items, so the result is rectangular in every
attribute. A scalar input contributes one item. When lists are combined,
each input's own top-level attributes are pushed down onto that input's
items (composing by the attribute's declared rule where one exists;
otherwise the pushed value fills the items that never wrote the name
and the inner value wins where they did, resolved from the write sets
at graph compile time), and the merged list starts with an empty
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
not. The tuple form composes with laziness: a lazy input declared
`Output = (T, Attr<A>)` yields the element and the declared reads at
each evaluation, so a kernel can branch on another input's attribute
without evaluating the branch it rejects. It is a read declaration
only: the lazy input's fields do not pass through to the output, since
the kernel controls whether and how often the edge is evaluated;
forwarding a lazy input's attributes is routing.

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

```
  byte  0       4       8      12      16      20      24      28   31
        ┌───────────────┐
  f64   │    element    │
        └───────────────┘

        ┌───────────────┬───────────────────────────────┬───────┬─┬───┐
  +3    │    element    │      Attr<&str>  (ptr, len)   │  u32  │b│pad│
        └───────────────┴───────────────────────────────┴───────┴─┴───┘

  Canonical order (descending alignment, then size) leaves no interior
  padding here; 3 bytes of tail round the record up to align 8.
```

A wire's layout is the set of all attributes written in its upstream
cone and not removed since, in a canonical order (descending alignment,
then size, then name and level), computed at graph compile time. Some
consequences:

- Layout identity is captured by stable node ids, because the write set
  is part of the hashed upstream cone. An instance that survives an
  incremental recompile cannot meet a changed layout.
- Reads resolve to `Option<offset>` at wiring, each against the layout
  of the input wire its tuple destructures. Present means a field
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
- Semantically a wire value has every attribute at all times: a read of
  a name nobody wrote yields the declared default, so a written default
  and an absent name are indistinguishable at runtime. Presence
  (membership in the layout) is representation, consulted only by merge
  push-down's fallback and the Data panel, which presents the layout:
  column presence is a pure function of the graph, stable across frames
  and across the branches a selector takes.
- Writes are unconditional: presence never depends on a value, so a
  conditionally relevant attribute is written at its default, and a
  runtime `Option` around a value buys nothing (`None` could only mean
  the default). A name that wants a distinguished unset declares an
  `Option` value type on its marker.

Fields are `Copy`, and larger payloads go behind a pointer-sized field.
Runtime-shaped data (CSV columns, arbitrary JSON) is a single dynamic
attribute holding a map in a fixed-size slot. It is the intended slow
path and puts no constraints on the fast one. Layouts are always static.

A name's type is unique by construction. For declared markers the census
admits one marker per name, checked when the registry is built. For
user-supplied names the binding forms at graph compile time, carrying the
marker's declared value type, and two names colliding at different types
is a graph compile error that names both nodes. Generic-typed writes
join the same table, carrying the name and value type their bindings
resolve to, so the check runs over declared markers, user-supplied
names, and generic instantiations together. We do not attempt
coercion.

## Levels and residency

Layout keys carry a level, and levels are numbered from the innermost
out, which keeps them stable when a structure node pushes a level
(nothing renumbers) and matches how indices are already numbered. Only
level 0 is populated today: the packed-record tier is flat, so the
binding rules below and the residency analysis that follows them are the
intended design rather than the implemented one. The level in the key is
what leaves room for both.

The binding rules are:

- A read binds to the top level of the input wire it is destructured
  from at the node's chain position; a write binds to the top level of
  the output wire.
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

Storage is meant to be level-resident and columnar, with the contiguous
record as a view. Per-lane consumers get the view assembled across
levels and columns into their activation frames. Reads across a level
boundary use the same index decomposition the structure nodes already
perform, and in batches that decomposition is hoisted per run. Neither
the residency analysis nor the multi-level storage exists yet; a single
level with resolved offsets is the base case both are built on.

## Runtime representation

- Every node's per-lane output lands in a frame claimed from the
  evaluation's frame space: a grow-only buffer the executor owns and
  lends down, sized at wiring by folding each path's frame need, so
  exhaustion is an accounting failure a debug assertion catches rather
  than a hot-path branch, and frame bytes carry no drop glue. A node's
  caller mints the node's frame claim out of its own free space; the
  node writes through the claim and closes it through `serve`, the
  `Node` trait's one required method, whose `Served` proof is mintable
  only by the claim's closing methods, so a served record is of the
  claimed layout by construction. The claim carries the free space
  beyond the frame, the node's inputs claim past it one after another,
  and a claim's space is free again when the claim dies, on value,
  error, and pending exits alike, with no per-exit ritual. "Allocating"
  a result is cursor arithmetic; a droppable payload parks in the arena
  once and the record carries the reference.
- No global slot assignment exists: a node's wiring state is its own
  frame size, so incremental recompiles and instance reuse cannot
  invalidate storage. The frame space belongs to the evaluation rather
  than to a thread, so worker counts never enter wiring. The reserve is
  the peak of a per-path fold over the graph (a node's need is its own
  frame plus its inputs' frames plus the deepest input's peak), computed
  once at wiring; it exceeds the plain sum of node frames because
  fan-out re-evaluation keeps several copies of a shared node's frame
  live at once.
- Held record values are safe without a guard: a claim keeps its record
  until it dies, so no input is released while a later sibling
  evaluates. Shared subtrees additionally rely on single consumption.
  The compiler's boundary frame memos provide it where the
  nullification pass inserts them; automatic insertion at every fan-out
  is designed but not built, and until it lands it gates the
  stack-return optimization for shared un-memoized subtrees.
- Batch results are a run of lanes behind a resolved offset per field,
  and the target form is per-field columns, each statically Varying (an
  array) or Uniform (a single value) per the residency analysis. A node
  that does not touch a column then forwards the pointer, so bypass
  costs nothing, and uniform columns give constant attributes their
  one-slot cost regardless of lane count. Both execution forms share one
  layout descriptor, and crossing from a batched producer to a per-lane
  consumer costs about 1.5ns per lane through a lane-view adapter. The
  first implementation lays the run out as an array of records and
  resolves each marker to an offset within a lane; moving that to
  struct-of-arrays, and then to flat tables, happens behind the same
  accessors. A batched producer serves lanes in place over the caller's
  scratch through a claimed run of same-layout slots, so a lane serves
  with no staging copy.
- A materialized level carries its lane count beside its layout and its
  storage, which is either an arena-resident run or an owned copy.
  Erased consumers (the Data panel, capture, deep copy) read the count
  and the fields off that handle without reaching into the element type,
  and Varying vs Uniform is meant to stay static in the layout, a
  Uniform column addressing a single value. This is the same picture as
  a batch result, so materializing a level and returning a batch are one
  format. A record value is one pointer wide, and only a record whose
  layout is empty rides inside the value itself; everything else spills
  to claimed frame space.
- Alignment padding is what the column form buys. In a row, a `u8`
  element costs the same as a `u64`, while packed columns keep the cost
  proportional to the element size (2x cheaper than rows when
  cache-resident, around 8x when memory-bound). Once columns are the
  storage format, the proportional cost holds wherever data accumulates
  and the padding survives only in transient view slots, whose number is
  bounded by graph depth. Until then a run pays a row's padding on every
  lane, which is the cost the struct-of-arrays step removes.

## Kernel io lowering

| Signature form | Meaning | Lowering |
| --- | --- | --- |
| first non-context param | primary input | carrier record |
| `_: ()` | no primary input | no carrier edge |
| `element: T` (unbounded, returned first) | explicit passthrough | erased byte carry, where `T` is instantiated with a zero-sized token, so the routing is checked by the type system and costs nothing |
| `element: Concrete` / bound | element read | field read at offset 0, monomorphized per implementations list, binds level 0 |
| `(x, a): (X, Attr<A>)` | input with attribute reads | the value as its ordinary lowering; each `Attr` an offset read into that input's record, or the default constant |
| `(_, a): ((), Attr<A>)` | attribute-only input | wired record edge with unit element; the attribute is the payload |
| `Attr<A>` in the return tuple | attribute write | offset write into the output record |
| `RemoveAttr<A>` in the return tuple | attribute delete | the name leaves the output layout; functionally a write of the default |
| `keys: IList<K>` | whole-extent input | wired edge, evaluated over its extent into a view |
| plain parameters | wired value inputs | ordinary wired edges; attributes on their wires do not flow |
| `impl Node<Context<'_>, Output = Concrete>` | lazy value input | the value flows, attributes do not |
| `impl Node<Context<'_>, Output = (T, Attr<A>, ..)>` | lazy input with attribute reads | each eval yields the element plus the declared reads, offsets resolved against that edge's layout; a read declaration only, and no field pass-through |
| `impl Node<Context<'_>, Output = T>` (unbounded) | source of an opaque record family | routing, see below |
| `-> IList<W>` with a lazy subject and `DeriveCtx` | per-copy level production | the kernel splits the index and evaluates the subject at the pushed level |
| `-> IList<W>` with a carrier subject | per-lane level production | the carrier binds once for the run; the kernel computes each lane's own fields |
| `-> IList<W>` without `extent(fn)` | store form | whole-level body, node owns storage |

`extent(fn)` names a function over the node's inputs and the queried level
(author code never receives the node struct); `extent_raw(fn)` is the
escape hatch for anything that vocabulary cannot say. The declaration
answers what the level's counts are; the kernel performs the matching
index decomposition itself. The two are written separately and have to
agree, which is why the split and push go through shared helpers rather
than open-coded arithmetic. `emit(...)` is an optional tail marker for
the carrier form whose parentheses double as the tuple's, so multi-write
lanes pay no extra nesting.

The rule behind all the lazy forms: kernels control whether, when, and
at which index their inputs are evaluated, but never how the records
move. Attributes travel inside record values or
through generated machinery, so kernel-controlled evaluation cannot
misalign them, and domain declarations stay with the extent system.

## Structure shapes

A structure node pairs an extent composition with an index
decomposition. The extent composition is an ordinary extent override:
the node macro's `extent(fn)` attribute names a function over the node's
inputs and the queried level to `GPoll<Extent>`, with `extent_raw(fn)` as
the escape hatch keeping the raw node, context, and level form, so
multiplicative, additive, and data-dependent extents are one mechanism
rather than a macro taxonomy, and the default stays the meet over the
value inputs.

- Multiplicative (Repeat, map/enter): the extent override answers the
  pushed level with the copy count and forwards inner levels to the
  content. The kernel splits the flat index into the copy index and the
  index below it through the shared split helper, and pushes the copy as
  a level on a derived context before evaluating the content there. The
  extent declaration and the split are written separately and have to
  agree; the helpers are shared so that they can. Batches split into
  maximal per-copy runs.
- Additive (Merge): an ordinary routing kernel whose selector condition
  is the index. The kernel range-splits the flat index into a segment
  and a local index through the shared split helper and evaluates that
  input at the shifted index via the derived-context lowering; the
  extent override sums the inputs' extents through `Extent::sum`. An
  input with unbounded (Free) extent counts as exactly one item in the
  sum, so merge is an extent-forcing boundary, which is the scalar base
  case. Item rows union with per-segment default fill. Each input's top
  row is pushed down one level onto that input's items via entries in
  the translation plan (a level remap computed at wiring; no values are
  needed at compile time), composing by the declared combine rule; the
  fallback is inner wins iff the inner level wrote the name, resolved
  from the write sets at wiring. The merged top row starts empty. An
  explicit Wrap node is how the user nests instead; a marker on the
  merge node enables the push-down plan variant. Batched merge forwards
  per-segment sub-ranges derived from the same split helper, so column
  uniformity survives concatenation per segment, default
  materialization is only paid on the per-lane and store paths, and
  per-lane vs batched agreement is law-bound.

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
	ctx: impl Ctx + ModifyIndex + Copy,
	_: (),
	content: impl Node<Context<'_>, Output = T>,
) -> Result<T, Interrupt> {
	let total = match content.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("reverse over a non-exact extent").into()),
	};
	let mut shifted = *ctx;
	shifted.set_index(total - 1 - ctx.innermost_index());
	content.eval(&shifted)
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
evaluation path cannot satisfy, and layout conflicts. The runtime does
no name lookup. Layout construction and the safe record builders check
field type identity and panic on a mismatch, which is what guards the
generated code against itself at wiring boundaries.

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
stashing handles in node state.

That is the property the design is for, and reaching it is work rather
than a consequence. Type identity is stamped on every layout field and
checked where layouts are built and where the safe builders write, which
is the enforcement this rests on. What is not yet closed, and is tracked
as such: a producer whose layout was never installed writes through a
path that assumes the inline width, an owned record replays against a
caller-supplied layout it does not check, and the record stack's own
bounds check is a debug assertion while its buffer can be reset under
live records. Each is reachable from safe code, so the claim above holds
by the discipline of the generated code and not yet by construction. The
direction is to replace the emitted raw operations with a small set of
checked surfaces, so that the remaining unsafe is the erased glue and
the wiring-computed byte plans, where it is irreducible.

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
- Per-lane views pad small elements up to the record alignment, and so
  does a run of records until the columnar storage arrives; only the
  column form is footprint-proportional.
- Two execution forms (per-lane views and batches) are more
  machinery than one representation. They share a single layout
  descriptor, and the measured seam between them is about 1.5ns per
  lane, but the machinery still has to exist.

# Rationale and alternatives

- Keep runtime maps (the current implementation): roughly 500ns per item
  on the reference chain and ~57ns marginal per attribute, an allocation
  per column, and no compile-time name checking. Interning the keys
  improves the constant (about 1.9ns per access vs. 0.43 for a resolved
  offset) but keeps a per-access search and rules out the structural
  optimizations that need static layouts: bypass, uniform columns, and
  packing a level's fields to their resolved offsets.
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
here. One behavior is refined rather than kept: flat merge
has to drop one input's top-level attributes, which the push-down rule
is meant to preserve. The present representation (string-keyed storage inside
`List<T>`, with per-carrier implementations rows) is what the Motivation
section measures. This RFC keeps its observable behavior while replacing
the storage and registration strategy underneath.

Outside Graphite, the nearest prior art is row polymorphism in records
(Rémy; PureScript and Elm) for the layout unions, ECS archetype storage
for resolved column handles, and the uniform vs. varying distinction
from shading languages for residency.

# Unresolved questions

- Leveled attributes, the largest open area. Attributes at more than one
  nesting level are designed but not built: the layout key carries a
  level, and nothing populates a level above 0. Open within it are the
  binding rules as stated (does a read really bind to the top level of
  the wire it is destructured from, and is pinning element-reading nodes
  to level 0 the right rule), how a structure node's per-copy write
  lands in the former top row, whether residency is worth its analysis
  or whether the extent machinery already answers it (an attribute whose
  index function ignores the index is `Free`), and what the storage for
  a level above 0 looks like given that the record tier is flat. The
  UX half of the same question is the map/enter construct: how "set on
  the parent" and "map over the children" read differently in the graph.
  Until this is settled, the Repeat-around-Opacity requirement is
  unmet.
- Where and how the combine rule is declared on the attribute marker.
  Merge push-down and flatten both consume it, and inner-wins is the
  intended fallback.
- The spelling of the push-down marker on the merge node, the one part
  of the additive shape the extent override cannot express.
- Naming: `Attribute` trait vs. `Attr` wrapper. The authoring list type
  is spelled `IList` here, as in the implementation, to keep it clear of
  the legacy wire type's `List`; a rename to `List` is planned once that
  type retires.
- Generic-typed writes: where the default for a generically written
  name comes from (a `Default` bound on the value vs. an input on the
  name source), what `A` instantiates to at the Rust level, whether
  attribute-only wires carry exactly one attribute by construction or
  uniqueness is checked per read, and the graph UX of the name source
  node.
- Whether evaluating at a lane outside the input's extent is clamped,
  wrapped, or a debug assertion.
- `IList<IList<W>>` outputs, i.e. one node pushing two levels.
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
