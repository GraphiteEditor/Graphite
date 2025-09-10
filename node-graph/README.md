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
	pub skip_deduplication: bool,
	pub visible: bool,
	pub original_location: OriginalLocation,
}
```
(Explanatory comments omitted; the actual definition is currently found in [`node-graph/graph-craft/src/document.rs`](https://github.com/GraphiteEditor/Graphite/blob/master/node-graph/graph-craft/src/document.rs))

Each `DocumentNode` is of a particular type, for example the "Opacity" node type. You can define your own type of document node in `editor/src/messages/portfolio/document/node_graph/node_graph_message_handler/document_node_types.rs`. A sample document node type definition for the opacity node is shown:

```rs
DocumentNodeDefinition {
	name: "Opacity",
	category: "Image Adjustments",
	implementation: DocumentNodeImplementation::proto("graphene_core::raster::OpacityNode"),
	inputs: vec![
		DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
		DocumentInputType::value("Factor", TaggedValue::F32(100.), false),
	],
	outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
	properties: node_properties::multiply_opacity,
	..Default::default()
},
```

The identifier here must be the same as that of the proto-node which will be discussed soon (usually the path to the node implementation).

> [!NOTE]
> Nodes defined in `graphene_core` are re-exported by `graphene_std`. However if the strings for the type names do not match exactly then you will encounter an error.

## Properties panel

The input names are shown in the graph when an input is exposed (with a dot in the properties panel). The default input is used when a node is first created or when a link is disconnected. An input is comprised from a `TaggedValue` (allowing serialization of a dynamic type with serde) in addition to an exposed boolean, which defines if the input is shown as a dot in the node graph UI by default. In the opacity node, the "Color" input is shown but the "Factor" input is hidden from the graph by default, allowing for a less cluttered graph.

The properties field is a function that defines a number input, which can be seen by selecting the opacity node in the graph. The code for this property is shown below:

```rs
pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let factor = number_widget(document_node, node_id, 1, "Factor", NumberInput::default().min(0.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: factor }]
}
```

## Graphene (proto node executor)

The graphene crate (found in `gcore/`) and the graphene standard library (found in `gstd/`) is where actual implementation for nodes are located. 

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

The `graphene_core::value::CopiedNode` is a node that, when evaluated, copies `10_f32` and returns it.

## Creating a new node

Instead of manually implementing the `Node` trait with complex generics, one can use the `node` macro, which can be applied to a function like `opacity`. This will generate the struct, implementation, node_registry entry, document node definition and properties panel entries:

```rs
#[node_macro::node(category("Raster: Adjustments"))]
fn opacity(_input: (), #[default(424242)] color: Color, #[soft_min(0.1)] opacity_multiplier: f64) -> Color {
	let opacity_multiplier = opacity_multiplier as f32 / 100.;
	Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), color.a() * opacity_multiplier)
}
```

## Additional Macro Options

The macro invocation can be extended with additional attributes. The currently supported attributes are (`name`, `path`, `skip_impl`, `category`). When using generics the `#[implementations()]` attribute can be used to automatically populate the node_registry for you. You can also use the `default`, `expose`, `min`, `max` and `range_mode` attributes to influence how the properties are generated.

## Executing a document `NodeNetwork`

When the document graph is executed, the following steps occur:
- The `NodeNetwork` is flattened using `NodeNetwork::flatten`. This involves removing any `DocumentNodeImplementation::Network` - which allow for nested document node networks. Instead, all of the inner nodes are moved into a single node graph.
- The `NodeNetwork` is converted into a proto-graph. Each node's inputs are stored as a list of node IDs in the `ConstructionArgs` struct in the `ProtoNode`. Converting a document graph into a proto graph is done with `NodeNetwork::into_proto_networks`.
- The newly created `ProtoNode`s are then converted into the corresponding constructor functions using the mapping defined in `node-graph/interpreted-executor/src/node_registry.rs`. This is done by `BorrowTree::push_node`.
- The constructor functions are run with the `ConstructionArgs` enum. Constructors generally evaluate the result of these inputs, e.g. if you have a `Pi` node that is used as the second input to an `Add` node, the `Add` node's constructor will evaluate the `Pi` node. This is visible if you place a log statement in the `Pi` node's implementation.
- The resolved functions are stored in a `BorrowTree`, which allows previous proto-nodes to be referenced as inputs by later nodes. The `BorrowTree` ensures nodes can't be removed while being referenced by other nodes.

The definition for the constructor of a node that applies the opacity transformation to each pixel of an image:
```rs
(
	// Matches against the string defined in the document node.
	ProtoNodeIdentifier::new("graphene_core::raster::OpacityNode"),
	// This function is run when converting the `ProtoNode` struct into the desired struct.
	|args| {
		Box::pin(async move {
			// Creates an instance of the struct that defines the node.
			let node = construct_node!(args, graphene_core::raster::OpacityNode<_>, [f64]).await;
			// Create a new map image node, that calls the `node` for each pixel.
			let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
			// Wraps this in a type erased future `Box<Pin<dyn core::future::Future<Output = T> + 'n>>` - this allows it to work with async.
			let map_node = graphene_std::any::FutureWrapperNode::new(map_node);
			// The `DynAnyNode` downcasts its input from a `Box<dyn DynAny>` i.e. dynamically typed, to the desired statically typed input value. It then runs the wrapped node and converts the result back into a dynamically typed `Box<dyn DynAny>`.
			let any: DynAnyNode<Image<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
			// Nodes are stored as type erased, which means they are `Box<dyn NodeIo + Node>`. This allows us to create dynamic graphs, using dynamic dispatch so we do not have to know all node combinations at compile time.
			any.into_type_erased()
		})
	},
	// Defines the call argument, return value, and inputs.
	NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), vec![fn_type!((), f64))]),
),
```

Nodes in the borrow stack take a `Box<dyn DynAny>` as input and output another `Box<dyn DynAny>`, to allow for any type. To use a specific type, we must downcast the values that have been passed in.
However the `OpacityNode` only works on one pixel at a time, so we first insert a `MapImageNode` to call the `OpacityNode` for every pixel in the image.
Finally we call `.into_type_erased()` on the result and that is inserted into the borrow stack.

We also need to add an implementation so that the user can change the opacity of just a single color. To simplify this process for raster nodes, a `raster_node!` macro is available which can simplify the definition of the opacity node to:
```rs
raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
```

There is also the more general `register_node!` for nodes that do not need to run per pixel.
```rs
register_node!(graphene_core::transform_nodes::SetTransformNode<_>, input: Vector, params: [DAffine2]),
```

## Debugging

Debugging inside your node can be done with the `log::debug!()` macro, for example `log::debug!("The opacity is {opacity_multiplier}");`.

We need a utility to easily view a graph as the various steps are applied. We also need a way to transparently see which constructors are being run, which nodes are being evaluated, and in what order.

## Conclusion

While we simplify the writing of nodes using the macro by hiding some of the details, creating nodes involves many files and concepts. We will work to continue making this system easier to use.

Any contributions you might have would be greatly appreciated. If any parts of this guide are outdated or difficult to understand, please feel free to ask for help in the Graphite Discord. We are very happy to answer any questions :)
