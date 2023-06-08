# Creating Nodes In Graphite

## Purpose of Nodes

Graphite is an image editor which is centred around a node based editing workflow, which allows operations to be visually connected in a graph. This is flexible as it allows all operations to be viewed or modified at any time without losing original data. The node system has been designed to be as general as possible with all data types being representable and a broad selection of nodes for a variety of use cases being planned.

## The Document Graph

The graph that is presented to users in the editor is known as the document graph. Each node that has been placed in this graph has the following properties:

```rs
pub struct DocumentNode {
	// An identifier used to display in the editor and to display the appropriate properties.
	pub name: String,
	// A NodeInput::Node { node_id, output_index } specifies an input from another node.
	// A NodeInput::Value { tagged_value, exposed } specifies a constant value. An exposed value is visible as a dot in the node graph UI.
	// A NodeInput::Network(Type) specifies a node will get its input from outside the graph, which is resolved later.
	pub inputs: Vec<NodeInput>,
	// A nested document network or a proto-node identifier
	pub implementation: DocumentNodeImplementation,
	// Contains the position of the node and other future properties
	pub metadata: DocumentNodeMetadata,
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
		DocumentInputType::value("Factor", TaggedValue::F64(100.), false),
	],
	outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
	properties: node_properties::multiply_opacity,
},
```

The identifier here must be the same as that of the proto-node which will be discussed soon (usually the path to the node implementation).

The input names are shown in the graph when an input is exposed (with a dot in the properties panel). The default input is used when a node is first created or when a link is disconnected. An input is comprised from a `TaggedValue` (allowing serialisation of a dynamic type with serde) in addition to an exposed boolean, which defines if the input is shown as a dot in the node graph UI by default. In the opacity node, the "Color" input is shown but the "Factor" input is hidden from the graph by default, allowing for a less cluttered graph.

The properties field is a function that defines a number input, which can be seen by selecting the opacity node in the graph. The code for this property is shown below:

```rs
pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let factor = number_widget(document_node, node_id, 1, "Factor", NumberInput::default().min(0.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: factor }]
}
```

## Node Implementation

Defining the actual implementation for a node is done by implementing the `Node` trait. The `Node` trait has a function called `eval` that takes one generic input. A node implementation for the opacity node is seen below:

```rs
#[derive(Debug, Clone, Copy)]
pub struct OpacityNode<O> {
	opacity_multiplier: O,
}

impl<'i, N: Node<'i, (), Output = f64> + 'i> Node<'i, Color> for OpacityNode<N> {
	type Output = Color;
	fn eval<'s: 'i>(&'s self, color: Color) -> Color {
		let opacity_multiplier = self.opacity_multiplier.eval(()) as f32 / 100.;
		Color::from_rgbaf32_unchecked(color.r(), color.g(), color.b(), color.a() * opacity_multiplier)
	}
}

impl<N> OpacityNode<N> {
	pub fn new(node: N) -> Self {
		Self { opacity_multiplier: node }
	}
}
```

The `eval` function can only take one input. To support more than one input, the node struct can contain references to other nodes (it is the references that implement the `Node` trait). If the input is a constant, then it will reference a node that simply evaluates to a constant. If the input is a node, then the relevant proto-node will be referenced. To evaluate the opacity multiplier input, you can pass in `()` (because no input is required to calculate the opacity multiplier) which returns an `f64`. This is because of the generics we have applied: `N: Node<'i, (), Output = f64>` A helper function to create a new node struct is also defined here.

This process can be made more concise using the `node_fn` macro, which can be applied to a function like `image_opacity` with an attribute of the name of the node:

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

## Inserting the Proto-Node

When the document graph is executed, it is first converted to a proto-graph, which has all of the nested node graphs flattened as well as separating out the primary input from the secondary inputs. The secondary inputs are stored as a list of node ids in the construction arguments field of the `ProtoNode`. The newly created `ProtoNode`s are then converted into the corresponding dynamic rust functions using the mapping defined in `node-graph/interpreted-executor/src/node_registry.rs`. The resolved functions are then stored in a `BorrowTree`, which allows previous proto-nodes to be referenced as inputs by later nodes. The `BorrowTree` ensures nodes can't be removed while being referenced by other nodes.

```rs
(
	NodeIdentifier::new("graphene_core::raster::OpacityNode<_>"),
	|args| {
		Box::pin(async move {
			let node = construct_node!(args, graphene_core::raster::OpacityNode<_>, [f64]).await;
			let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
			let map_node = graphene_std::any::FutureWrapperNode::new(map_node);
			let any: DynAnyNode<Image<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
			any.into_type_erased()
		})
	},
	NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), vec![fn_type!(f64))]),
),
raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
```

Nodes in the borrow stack take a `Box<dyn DynAny>` as input and output another `Box<dyn DynAny>`, to allow for any type. To use a specific type, we must downcast the values that have been passed in.
However the `OpacityNode` only works on one pixel at a time, so we first insert a `MapImageNode` to call the `OpacityNode` for every pixel in the image.
Finally we call `.into_type_erased()` on the result and that is inserted into the borrow stack.

However we also need to add an implementation so that the user can change the opacity of just a single color. To simplify this process for raster nodes, a `raster_node!` macro is available which can simplify the defention of the opacity node to:
```rs
raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
```

There is also the more general `register_node!` for nodes that do not need to run per pixel.
```rs
register_node!(graphene_core::transform::SetTransformNode<_>, input: VectorData, params: [DAffine2]),
```

## Debugging

Debugging inside your node can be done with the `log` macros, for example `info!("The opacity is {opacity_multiplier}");`

## Conclusion

Defining some basic nodes to allow for a simple image editing workflow would be invaluable. Currently defining nodes is quite a laborious process however efforts at simplification are being discussed. Any contributions you might have would be greatly appreciated. If any parts of this guide are outdated or difficult to understand, please feel free to ask for help in the Graphite Discord. We are very happy to answer any questions :)
