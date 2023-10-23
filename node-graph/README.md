# Creating Nodes In Graphite

## Purpose of Nodes

Graphite is an image editor which is centred around a node based editing workflow, which allows operations to be visually connected in a graph. This is flexible as it allows all operations to be viewed or modified at any time without losing original data. The node system has been designed to be as general as possible with all data types being representable and a broad selection of nodes for a variety of use cases being planned.

## The Document Graph

The graph that is presented to users in the editor is known as the document graph, and is defined in the `NodeNetwork` struct. Each node that has been placed in this graph has the following properties:

```rs
pub struct DocumentNode {
	/// An identifier used to display in the UI and to display the appropriate properties.
	pub name: String,
	/// The inputs to a node, which are either:
	/// - From other nodes within this graph [`NodeInput::Node`],
	/// - A constant value [`NodeInput::Value`],
	/// - A [`NodeInput::Network`] which specifies that this input is from outside the graph, which is resolved in the graph flattening step.
	pub inputs: Vec<NodeInput>,
	/// TODO: what is this?
	pub manual_composition: Option<Type>,
	// A nested document network or a proto-node identifier
	pub implementation: DocumentNodeImplementation,
	/// Metadata about the node including its position in the graph UI.
	pub metadata: DocumentNodeMetadata,
	/// When 2 different protonodes hash to the same value (e.g. 2 value nodes that contain `2u32` or 2 multiply nodes that have the same node ids as input)
	/// the duplicates are removed. See [`crate::proto::ProtoNetwork::generate_stable_node_ids`] for details.
	/// However sometimes this is not desirable, for example in the case of a [`graphene_core::memo::MonitorNode`] that needs to be accessed outside of the graph.
	#[serde(default)]
	pub skip_deduplication: bool,
	/// The path to this node as of when [`NodeNetwork::generate_node_paths`] was called. For example if this node was id 6 inside a node with id 4
	/// and with a [`DocumentNodeImplementation::Network`], the path would be [4,6].
	pub path: Option<Vec<NodeId>>,
}
```
(The actual defenition is currently found at `node-graph/graph-craft/src/document.rs:38`)

Each `DocumentNode` is of a particular type, for example the "Opacity" node type. You can define your own type of document node in `editor/src/messages/portfolio/document/node_graph/node_graph_message_handler/document_node_types.rs`. A sample document node type definition for the opacity node is shown:

```rs
DocumentNodeType {
	name: "Opacity",
	category: "Image Adjustments",
	identifier: NodeImplementation::proto("graphene_core::raster::OpacityNode<_>"),
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

The input names are shown in the graph when an input is exposed (with a dot in the properties panel). The default input is used when a node is first created or when a link is disconnected. An input is comprised from a `TaggedValue` (allowing serialisation of a dynamic type with serde) in addition to an exposed boolean, which defines if the input is shown as a dot in the node graph UI by default. In the opacity node, the "Color" input is shown but the "Factor" input is hidden from the graph by default, allowing for a less cluttered graph.

The properties field is a function that defines a number input, which can be seen by selecting the opacity node in the graph. The code for this property is shown below:

```rs
pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let factor = number_widget(document_node, node_id, 1, "Factor", NumberInput::default().min(0.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: factor }]
}
```

## Graphene (protonode executor)

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

The `eval` function can only take one input. To support more than one input, the node struct can store references to other nodes. This can be seen here, as the `opacity_multiplier` field, which generic and is is constrained to the trait `Node<'i, (), Output = f64>`. This means that it is a node with the input of `()` (no input is required to comput the opacity) and an output of an `f64`.

To compute the value when executing the `OpacityNode`, we need to call `self.opacity_multiplier.eval(())`. This evaluates the node that provides the `opacity_multiplier` input, with the input value of `()` - nothing. This occurs each time the opacity node is run.

To test this:
```rs
#[test]
fn test_opacity_node() {
	let opacity_node = OpacityNode {
		opacity_multiplier: crate::value::CopiedNode(10f64), // set opacity to 10%
	};
	assert_eq!(opacity_node.eval(Color::WHITE), Color::from_rgbaf32_unchecked(1., 1., 1., 0.1));
}
```

The `graphene_core::value::CopiedNode` is a node that, when evaluated, copies `10f32` and returns it.

## Creating a new protonode

Instead of manually implementing the `Node` trait with complex generics, one can use the `node_fn` macro, which can be applied to a function like `image_opacity` with an attribute of the name of the node:

```rs
#[derive(Debug, Clone, Copy)]
pub struct OpacityNode<O> {
	opacity_multiplier: O,
}

#[node_macro::node_fn(OpacityNode)]
fn image_opacity(color: Color, opacity_multiplier: f64) -> Color {
	let opacity_multiplier = opacity_multiplier as f32 / 100.;
	Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), color.a() * opacity_multiplier)
}
```

## Executing a document `NodeNetwork`

When the document graph is executed, the following steps are occur:
- The `NodeNetwork` is flattened using `NodeNetwork::flatten`. This involves removing any `DocumentNodeImplementation::Network` - which allow for nested document node networks (not currently exposed in the UI). Instead, all of the inner nodes into a single node graph.
- The `NodeNetwork` is converted to a proto-graph, which separates out the primary input from the secondary inputs. The secondary inputs are stored as a list of node ids in the `ConstructionArgs` struct in the `ProtoNode`. Converting a document graph into a proto graph is done with `NodeNetwork::into_proto_networks`.
- The newly created `ProtoNode`s are then converted into the corresponding constructor functions using the mapping defined in `node-graph/interpreted-executor/src/node_registry.rs`. This is done by `BorrowTree::push_node`.
- The constructor functions are run with the `ConstructionArgs` enum. Constructors generally evaluate the result of these secondary inputs e.g. if you have a `Pi` node that is used as the second input to an `Add` node, the `Add` node's constructor will evaluate the `Pi` node. This is visible if you place a log statement in the `Pi` node's implementation.
- The resolved functions are stored in a `BorrowTree`, which allows previous proto-nodes to be referenced as inputs by later nodes. The `BorrowTree` ensures nodes can't be removed while being referenced by other nodes.

The defenition for the constructor of a node that applies the opacity transformation to each pixel of an image:
```rs
(
	// Matches agains the string defined in the document node.
	NodeIdentifier::new("graphene_core::raster::OpacityNode<_>"),
	// This function is run when converting the `ProtoNode` struct into the desired struct.
	|args| {
		Box::pin(async move {
			// Creates an instance of the struct that defines the node.
			let node = construct_node!(args, graphene_core::raster::OpacityNode<_>, [f64]).await;
			// Create a new map image node, that calles the `node` for each pixel
			let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
			// Wraps this in a type erased future `Box<Pin<dyn core::future::Future<Output = T> + 'n>>` - this allows it to work with async
			let map_node = graphene_std::any::FutureWrapperNode::new(map_node);
			// The `DynAnyNode` downcasts its input from a `Box<dyn DynAny>` i.e. dynamically typed, to the desired staticly typed input value. It then runs the wrapped node and converts the result back into a dynamically typed `Box<dyn DynAny>`
			let any: DynAnyNode<Image<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
			// Nodes are stored as type erased, which means they are `Box<dyn NodeIo + Node>`. This allows us to create dynamic graphs, using dynamic dispatch so we do not have to know all node combinations at compile time.
			any.into_type_erased()
		})
	},
	// Defines the input, output and parameters (where each perameter is a function taking in some input and returning another input)
	NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), vec![fn_type!((),f64))]),
),
```

Nodes in the borrow stack take a `Box<dyn DynAny>` as input and output another `Box<dyn DynAny>`, to allow for any type. To use a specific type, we must downcast the values that have been passed in.
However the `OpacityNode` only works on one pixel at a time, so we first insert a `MapImageNode` to call the `OpacityNode` for every pixel in the image.
Finally we call `.into_type_erased()` on the result and that is inserted into the borrow stack.

We also need to add an implementation so that the user can change the opacity of just a single color. To simplify this process for raster nodes, a `raster_node!` macro is available which can simplify the defention of the opacity node to:
```rs
raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
```

There is also the more general `register_node!` for nodes that do not need to run per pixel.
```rs
register_node!(graphene_core::transform::SetTransformNode<_>, input: VectorData, params: [DAffine2]),
```

## Debugging

Debugging inside your node can be done with the `log` macros, for example `info!("The opacity is {opacity_multiplier}");`.

We need a utility to easily view a graph as the various steps are applied. We also need a way to transparently see which constructors are being run, which nodes are being evaluated and in what order.

## Conclusion

Currently defining nodes is a very laborious and error prone process, spanning many files and concepts. It is necessary to simplify this if we want contributors to be able to write their own nodes.

Any contributions you might have would be greatly appreciated. If any parts of this guide are outdated or difficult to understand, please feel free to ask for help in the Graphite Discord. We are very happy to answer any questions :)
