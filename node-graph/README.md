# Creating Nodes In Graphite

## Purpose of Nodes

Graphite is an image editor which is centered around a node based editing workflow, which allows operations to be visually connected in a graph. This is flexible as it allows all operations to be viewed or modified at any time without losing original data. The node system has been designed to be as general as possible with all data types being representable and a broad selection of nodes for a variety of use cases being planned.

## The Document Graph

The graph that is presented to users in the editor is known as the document graph, and is defined in the `NodeNetwork` struct. Each node that has been placed in this graph has the following properties:

```rs
pub struct DocumentNode {
	pub inputs: Vec<NodeInput>,
	pub call_argument: Type,
	pub implementation: DocumentNodeImplementation,
	pub visible: bool,
	pub skip_deduplication: bool,
	pub context_features: ContextDependencies,
	pub original_location: OriginalLocation,
}
```
(Explanatory comments omitted; the actual definition is currently found in [`node-graph/graph-craft/src/document.rs`](https://github.com/GraphiteEditor/Graphite/blob/master/node-graph/graph-craft/src/document.rs))

Each `DocumentNode` is of a particular type, for example the "Opacity" node type. The blueprint for a type is a `DocumentNodeDefinition`, found in `editor/src/messages/portfolio/document/node_graph/document_node_definitions.rs`:

```rs
pub struct DocumentNodeDefinition {
	pub identifier: &'static str,
	pub node_template: NodeTemplate,
	pub category: &'static str,
	pub description: Cow<'static, str>,
	pub properties: Option<&'static str>,
}
```

Only the definitions of nodes built from a nested network, such as the empty "Custom Node", are written by hand in that file. A node implemented as a single proto-node has its definition generated from the metadata the `node` macro records about its function signature (see [Creating a new node](#creating-a-new-node)), keyed by the proto-node identifier, which for the Opacity node is `graphene_std::blending_nodes::opacity::IDENTIFIER`.

## Properties panel

Each input of a node is a `NodeInput`: either a wire from another node or a constant `TaggedValue` (a dynamically typed value that serializes with serde) paired with an exposed flag, which is whether the input is shown as a connector in the node graph by default. Both come from the function signature. A parameter's type picks the `TaggedValue` variant, `#[default(...)]` sets the value a new node starts with and the value an input falls back to when its wire is disconnected, and `#[expose]` shows a secondary input's connector, while the primary input always has one. In the Opacity node, `content` is the primary input while `opacity` and `fill` appear only in the Properties panel by default, keeping the graph uncluttered.

The Properties panel is generated from the same signature, which can be seen by selecting the Opacity node in the graph. A number parameter becomes a number input whose bounds and slider come from `#[soft]`, `#[hard]`, and `#[range]` (see [Additional Macro Options](#additional-macro-options)), and `#[unit(...)]` sets the `%` suffix here. A `bool` becomes a checkbox and a `ChoiceType` enum becomes a dropdown. When the generated widget isn't right, `#[widget(ParsedWidgetOverride::Hidden)]` hides it and `#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]` names a hand-written widget function in `node_properties.rs`, which is how the Opacity node pairs each percentage with the checkbox that enables it.

## Graphene (proto node executor)

The node crates under `node-graph/nodes/`, such as the Graphene core crate (`gcore/`) and the Graphene standard library (`gstd/`), are where the implementations of nodes are located.

Implementing a node is done by defining a `struct` implementing the `Node` trait. The `Node` trait has a required function named `eval` that takes one generic input. A sample implementation for an opacity node acting on a color is seen below:

```rs
use crate::{Color, Node};

#[derive(Debug, Clone, Copy)]
pub struct OpacityNode<OpacityMultiplierInput> {
	opacity_multiplier: OpacityMultiplierInput,
}

impl<'i, OpacityMultiplierInput: Node<'i, (), Output = f64> + 'i> Node<'i, Color> for OpacityNode<OpacityMultiplierInput> {
	type Output = Color;
	fn eval(&'i self, color: Color) -> Color {
		let opacity_multiplier = self.opacity_multiplier.eval(()) as f32 / 100.;
		Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), color.a() * opacity_multiplier)
	}
}
```

The `eval` function can only take one input. To support more than one input, the node struct can store references to other nodes. This can be seen here, as the `opacity_multiplier` field, which is generic and is constrained to the trait `Node<'i, (), Output = f64>`. This means that it is a node with the input of `()` (no input is required to compute the opacity) and an output of an `f64`.

To compute the value when executing the `OpacityNode`, we need to call `self.opacity_multiplier.eval(())`. This evaluates the node that provides the `opacity_multiplier` input, with the input value of `()`— nothing. This occurs each time the opacity node is run.

To test this:
```rs
#[test]
fn test_opacity_node() {
	let opacity_node = OpacityNode {
		opacity_multiplier: crate::value::CopiedNode(10_f64), // set opacity to 10%
	};
	assert_eq!(opacity_node.eval(Color::WHITE), Color::from_rgbaf32_unchecked(1., 1., 1., 0.1));
}
```

The `graphene_core::value::CopiedNode` is a node that, when evaluated, copies `10_f64` and returns it.

## Creating a new node

Instead of manually implementing the `Node` trait with complex generics, one can use the `node` macro, which is applied to a function like `opacity`. This generates the struct, its `Node` implementation, the node registry entries, the document node definition, and the Properties panel entries. The first parameter is the evaluation context and the second is the primary input, and each parameter rides its wire as an `Item<T>` (one value with its attributes) or a `List<T>` (a whole list). Doc comments on the function and its parameters become the tooltips shown in the editor (omitted here). This is the Opacity node as it exists in `node-graph/nodes/blending/src/lib.rs`:

```rs
#[node_macro::node(category("Blending"))]
fn opacity<T>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)]
	content: Item<T>,
	#[widget(ParsedWidgetOverride::Hidden)]
	#[default(true)]
	has_opacity: Item<bool>,
	#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
	#[widget(ParsedWidgetOverride::Hidden)]
	has_fill: Item<bool>,
	#[widget(ParsedWidgetOverride::Custom = "optional_percentage")]
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	fill: Item<f64>,
) -> Item<T> {
	let mut content = content;
	let (has_opacity, opacity, has_fill, fill) = (*has_opacity.element(), *opacity.element(), *has_fill.element(), *fill.element());

	if has_opacity {
		let multiplied = content.attribute_cloned_or(ATTR_OPACITY, 1.) * (opacity / 100.);
		content.set_attribute(ATTR_OPACITY, multiplied);
	}

	if has_fill {
		let multiplied = content.attribute_cloned_or(ATTR_OPACITY_FILL, 1.) * (fill / 100.);
		content.set_attribute(ATTR_OPACITY_FILL, multiplied);
	}

	content
}
```

## Additional Macro Options

The macro invocation can be extended with additional attributes. The currently supported attributes are (`name`, `path`, `skip_impl`, `category`). When using generics the `#[implementations()]` attribute can be used to automatically populate the node_registry for you. You can also use the `default`, `expose`, `soft`, `hard`, `range`, `unit`, `multiline`, and `progression` attributes to influence how the properties are generated. The `#[unit("...")]` attribute appends a suffix like `%` or ` px` to a number's widget, while `#[multiline]` gives a `String` the text area widget and `#[progression]` splits a number into its whole and fractional parts. The `#[soft(a..b)]` and `#[hard(a..b)]` attributes set the slider's suggested extent and its enforced clamp, respectively (either endpoint may be omitted, e.g. `0..` or `..100`; both endpoints are inclusive, so there is no `..=` form), and `#[range]` renders the input as a draggable slider. Values typed into the input may exceed the soft extent but are clamped to the hard bounds, so `#[soft]` is only meaningful together with `#[range]`.

## Executing a document `NodeNetwork`

When the document graph is executed, the following steps occur:
- The `NodeNetwork` is flattened using `NodeNetwork::flatten`. This involves removing any `DocumentNodeImplementation::Network` - which allow for nested document node networks. Instead, all of the inner nodes are moved into a single node graph.
- The `NodeNetwork` is converted into a proto-graph. Each node's inputs are stored as a list of node IDs in the `ConstructionArgs` struct in the `ProtoNode`. Converting a document graph into a proto graph is done with `NodeNetwork::into_proto_networks`.
- The newly created `ProtoNode`s are then converted into the corresponding constructor functions using the mapping defined in `node-graph/interpreted-executor/src/node_registry.rs`. This is done by `BorrowTree::push_node`.
- The constructor functions are run with the `ConstructionArgs` enum. Constructors generally evaluate the result of these inputs, e.g. if you have a `Pi` node that is used as the second input to an `Add` node, the `Add` node's constructor will evaluate the `Pi` node. This is visible if you place a log statement in the `Pi` node's implementation.
- The resolved functions are stored in a `BorrowTree`, which allows previous proto-nodes to be referenced as inputs by later nodes. The `BorrowTree` ensures nodes can't be removed while being referenced by other nodes.

The registry rows for a node declared with the `node` macro are generated by it, one per `#[implementations(...)]` pairing. The rows written by hand in `node_registry.rs` are for the adapters that convert between wire types and for executor-internal nodes such as the memoize and monitor nodes.

## Debugging

Debugging inside your node can be done with the `log::debug!()` macro, for example `log::debug!("The opacity is {opacity_multiplier}");`.

We need a utility to easily view a graph as the various steps are applied. We also need a way to transparently see which constructors are being run, which nodes are being evaluated, and in what order.

## Conclusion

While we simplify the writing of nodes using the macro by hiding some of the details, creating nodes involves many files and concepts. We will work to continue making this system easier to use.

Any contributions you might have would be greatly appreciated. If any parts of this guide are outdated or difficult to understand, please feel free to ask for help in the Graphite Discord. We are very happy to answer any questions :)
