use super::*;
use core_types::Context;
use core_types::descriptor;
use core_types::list::{Item, List};
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::Vector;

#[test]
fn push_node_sync() {
	let mut tree = BorrowTree::default();
	let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32).into()), vec![]);
	let context = TypingContext::default();
	let future = tree.push_node(NodeId(0), val_1_protonode, &context);
	futures::executor::block_on(future).unwrap();
	let _node = tree.get(NodeId(0)).unwrap();
	let result = futures::executor::block_on(tree.eval(NodeId(0), ()));
	assert_eq!(result, Some(2_u32));
}

/// Builds a two-node network feeding the given value into Bounding Box, whose primary input registers both `Item<Vector>` and `List<Vector>` wire variants.
fn bounding_box_network(content: TaggedValue) -> ProtoNetwork {
	let value_node = ProtoNode::value(ConstructionArgs::Value(content.into()), vec![NodeId(0)]);

	let mut bounding_box_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	bounding_box_node.identifier = ProtoNodeIdentifier::new("core_types::vector::BoundingBoxNode");

	ProtoNetwork {
		inputs: vec![],
		output: NodeId(1),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), bounding_box_node)],
	}
}

fn compile_bounding_box_network(content: TaggedValue) -> BorrowTree {
	let network = bounding_box_network(content);
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("The network should resolve against exactly one registered wire variant");
	futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The resolved variant's constructor should instantiate")
}

#[test]
fn item_wire_variant_resolves_and_executes() {
	let tree = compile_bounding_box_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)));

	let context: Context = None;
	let result: Option<Item<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context.clone()));
	assert!(result.is_some(), "The Item wire variant should downcast and execute end-to-end");

	let wrong_type: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context));
	assert!(wrong_type.is_none(), "An Item wire should not downcast as a List");
}

#[test]
fn item_wire_promotes_to_list_connector() {
	let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(Item<f64>)).into()), vec![NodeId(0)]);

	let mut sum_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	sum_node.identifier = ProtoNodeIdentifier::new("math_nodes::SumNode");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(1),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), sum_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("An Item wire should resolve a List connector via promotion");
	assert!(typing_context.promotions(NodeId(1)).is_some(), "The typing pass should record the promotion");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion adapter should instantiate");

	let context: Context = None;
	let result: Option<Item<f64>> = futures::executor::block_on(tree.eval(NodeId(1), context));
	assert!(result.is_some(), "The promoted wire should execute end-to-end");
}

// The layer content path: a rank-0 content wire enters Wrap Graphic's `List` connector by singleton raise, and the
// wrapped `Item<Graphic>` raises again at Extend's `List` connector, so layers accept rank-0 chains without new machinery
#[test]
fn rank_0_content_promotes_through_the_layer_coercion_path() {
	let content_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(Item<Vector>)).into()), vec![NodeId(0)]);

	let mut wrap_graphic_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	wrap_graphic_node.identifier = ProtoNodeIdentifier::new("graphic_nodes::graphic::WrapGraphicNode");

	let base_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(2)]);

	let mut extend_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(2), NodeId(1)]), vec![NodeId(3)]);
	extend_node.identifier = ProtoNodeIdentifier::new("graphic_nodes::graphic::ExtendNode");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(3),
		nodes: vec![(NodeId(0), content_node), (NodeId(1), wrap_graphic_node), (NodeId(2), base_node), (NodeId(3), extend_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A rank-0 content wire should resolve the layer coercion path via promotion");
	assert!(typing_context.promotions(NodeId(1)).is_some(), "The rank-0 content should be raised at Wrap Graphic's List connector");
	assert!(typing_context.promotions(NodeId(3)).is_some(), "The wrapped Item<Graphic> should be raised at Extend's List connector");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion adapters should instantiate");

	let context: Context = None;
	let result: Option<List<graphene_std::Graphic>> = futures::executor::block_on(tree.eval(NodeId(3), context));
	let stack = result.expect("The layer coercion path should execute end-to-end");
	assert_eq!(stack.len(), 1, "The rank-0 content should contribute exactly one graphic to the stack");
}

/// Builds a network feeding the given content plus a promoted bare distance into Offset Points, whose distance connector is ranked `Item<f64>`.
fn offset_points_network(content: TaggedValue) -> ProtoNetwork {
	let content_node = ProtoNode::value(ConstructionArgs::Value(content.into()), vec![NodeId(0)]);
	let distance_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(10.).into()), vec![NodeId(1)]);

	let mut input_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1)]), vec![NodeId(2)]);
	input_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<f64>");

	let mut offset_points_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(2)]), vec![NodeId(3)]);
	offset_points_node.identifier = ProtoNodeIdentifier::new("core_types::vector::OffsetPointsNode");

	ProtoNetwork {
		inputs: vec![],
		output: NodeId(3),
		nodes: vec![(NodeId(0), content_node), (NodeId(1), distance_node), (NodeId(2), input_adapter_node), (NodeId(3), offset_points_node)],
	}
}

#[test]
fn mixed_rank_connectors_resolve_via_promotion() {
	let network = offset_points_network(TaggedValue::TypeDefault(descriptor!(List<Vector>)));
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context
		.update(&network)
		.expect("A List primary with an Item parameter should resolve the mapped variant via promotion");
	assert!(typing_context.promotions(NodeId(3)).is_some(), "The Item distance should be marked for promotion");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("Construction should wrap the promoted argument");

	let context: Context = None;
	let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(3), context));
	assert!(result.is_some(), "The zipped mapped variant should execute end-to-end");
}

#[test]
fn all_item_connectors_resolve_without_promotion() {
	let network = offset_points_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)));
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("All-Item connectors should resolve the rank-0 variant exactly");
	assert!(typing_context.promotions(NodeId(3)).is_none(), "No promotion should be needed at rank 0");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The rank-0 variant should instantiate");

	let context: Context = None;
	let result: Option<Item<Vector>> = futures::executor::block_on(tree.eval(NodeId(3), context));
	assert!(result.is_some(), "The rank-0 variant should execute and stay rank 0");
}

/// Builds a Transform network: content (node 0) plus four parameter values, each promoted onto Item wires as the preprocessor would.
fn transform_network(content: TaggedValue, rotation: TaggedValue) -> ProtoNetwork {
	let mut nodes = vec![(NodeId(0), ProtoNode::value(ConstructionArgs::Value(content.into()), vec![NodeId(0)]))];

	let parameters = [
		(TaggedValue::DVec2(glam::DVec2::new(5., 0.)), "DVec2"),
		(rotation, "f64"),
		(TaggedValue::DVec2(glam::DVec2::ONE), "DVec2"),
		(TaggedValue::DVec2(glam::DVec2::ZERO), "DVec2"),
	];
	let mut transform_inputs = vec![NodeId(0)];
	let mut next_id = 1;
	for (value, element) in parameters {
		let value_id = NodeId(next_id);
		let input_adapter_id = NodeId(next_id + 1);
		next_id += 2;

		nodes.push((value_id, ProtoNode::value(ConstructionArgs::Value(value.into()), vec![value_id])));
		let mut input_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![value_id]), vec![input_adapter_id]);
		input_adapter_node.identifier = ProtoNodeIdentifier::with_owned_string(format!("graphene_core::ops::InputAdapterNode<{element}>"));
		nodes.push((input_adapter_id, input_adapter_node));
		transform_inputs.push(input_adapter_id);
	}

	let output = NodeId(next_id);
	let mut transform_node = ProtoNode::value(ConstructionArgs::Nodes(transform_inputs), vec![output]);
	transform_node.identifier = graphene_std::transform_nodes::transform::IDENTIFIER;
	nodes.push((output, transform_node));

	ProtoNetwork { inputs: vec![], output, nodes }
}

#[test]
fn transform_composes_onto_item_wire() {
	use glam::{DAffine2, DVec2};

	let network = transform_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)), TaggedValue::F64(0.));
	let output = network.output;
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("Transform should resolve its rank-0 variant");
	assert!(typing_context.promotions(output).is_none(), "All-Item connectors should need no promotion");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("Transform's rank-0 variant should instantiate");

	let context: Context = None;
	let result: Option<Item<Vector>> = futures::executor::block_on(tree.eval(output, context));
	let item = result.expect("A rank-0 chain through Transform should stay rank 0");
	let transform = item.attribute_cloned_or_default::<DAffine2>(core_types::ATTR_TRANSFORM);
	assert_eq!(transform.translation, DVec2::new(5., 0.), "The translation should compose onto the item's transform attribute");
}

#[test]
fn transform_broadcasts_item_content_across_a_framed_parameter() {
	use glam::DAffine2;

	let network = transform_network(TaggedValue::TypeDefault(descriptor!(Item<Vector>)), TaggedValue::F64Array(vec![0., 90.]));
	let output = network.output;
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context
		.update(&network)
		.expect("A framed rotation should resolve the mapped variant via promotion of the other connectors");
	assert!(typing_context.promotions(output).is_some(), "The Item-typed connectors should be raised into the frame");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The mapped variant should instantiate");

	let context: Context = None;
	let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(output, context));
	let list = result.expect("The broadcast should produce a List");
	assert_eq!(list.len(), 2, "One output item per frame slot");

	let first: DAffine2 = list.attribute_cloned_or_default(core_types::ATTR_TRANSFORM, 0);
	let second: DAffine2 = list.attribute_cloned_or_default(core_types::ATTR_TRANSFORM, 1);
	assert!((first.matrix2.col(0).y - 0.).abs() < 1e-10, "Slot 0 should be unrotated");
	assert!((second.matrix2.col(0).y - 1.).abs() < 1e-10, "Slot 1 should be rotated 90 degrees");
}

#[test]
fn generator_frames_over_a_list_parameter() {
	// A `()` generator (Circle) fed a `List<f64>` radius should frame into one circle per slot
	let primary = ProtoNode::value(ConstructionArgs::Value(TaggedValue::None.into()), vec![NodeId(0)]);

	let radii = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![10., 20., 30.]).into()), vec![NodeId(1)]);
	let mut radius_adapter = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1)]), vec![NodeId(2)]);
	radius_adapter.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<f64>");

	let mut circle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(2)]), vec![NodeId(3)]);
	circle_node.identifier = graphene_std::vector_nodes::circle::IDENTIFIER;

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(3),
		nodes: vec![(NodeId(0), primary), (NodeId(1), radii), (NodeId(2), radius_adapter), (NodeId(3), circle_node)],
	};

	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A List<f64> radius should resolve Circle's mapped generator variant");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The mapped generator variant should instantiate");

	let context: Context = None;
	let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(3), context));
	let list = result.expect("The generator frame should produce a List<Vector>");
	assert_eq!(list.len(), 3, "One circle per radius slot");
}

/// Builds the compiler's cache chain (child, then Memoize, then Context Modification) around a value, as `insert_context_nullification_node` does.
fn nullification_chain_network(value: TaggedValue) -> ProtoNetwork {
	let value_node = ProtoNode::value(ConstructionArgs::Value(value.into()), vec![NodeId(0)]);

	let mut memoize_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	memoize_node.identifier = graphene_core::memo::memoize::IDENTIFIER;

	let features_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::ContextFeatures(Default::default()).into()), vec![NodeId(2)]);

	let mut nullification_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1), NodeId(2)]), vec![NodeId(3)]);
	nullification_node.identifier = graphene_core::context_modification::context_modification::IDENTIFIER;

	ProtoNetwork {
		inputs: vec![],
		output: NodeId(3),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), memoize_node), (NodeId(2), features_node), (NodeId(3), nullification_node)],
	}
}

#[test]
fn the_nullification_chain_resolves_for_ranked_enum_wires() {
	use graphene_std::vector::style::StrokeAlign;

	// The bare form, as a constant enum wire presents to the inserted cache chain
	let network = nullification_chain_network(TaggedValue::StrokeAlign(StrokeAlign::default()));
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A bare StrokeAlign wire should resolve through the compiler's cache chain");

	// The Item form, as a wrapped input adapter's output presents to the chain
	let network = nullification_chain_network(TaggedValue::TypeDefault(descriptor!(Item<StrokeAlign>)));
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("An Item<StrokeAlign> wire should resolve through the compiler's cache chain");
}

#[test]
fn bare_wires_promote_to_item_connectors_at_resolution() {
	use glam::{DAffine2, DVec2};

	let values = [
		TaggedValue::DAffine2(DAffine2::IDENTITY),
		TaggedValue::DVec2(DVec2::new(7., 0.)),
		TaggedValue::F64(0.),
		TaggedValue::DVec2(DVec2::ONE),
		TaggedValue::DVec2(DVec2::ZERO),
	];
	let mut nodes: Vec<_> = values
		.into_iter()
		.enumerate()
		.map(|(index, value)| (NodeId(index as u64), ProtoNode::value(ConstructionArgs::Value(value.into()), vec![NodeId(index as u64)])))
		.collect();
	let mut transform_node = ProtoNode::value(ConstructionArgs::Nodes((0..5).map(NodeId).collect()), vec![NodeId(5)]);
	transform_node.identifier = graphene_std::transform_nodes::transform::IDENTIFIER;
	nodes.push((NodeId(5), transform_node));

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(5),
		nodes,
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("Bare wires should resolve Item connectors via wrap promotion");
	assert_eq!(typing_context.promotions(NodeId(5)).map(Vec::len), Some(5), "All five bare inputs should be wrapped");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The wrap adapters should instantiate");

	let context: Context = None;
	let result: Option<Item<DAffine2>> = futures::executor::block_on(tree.eval(NodeId(5), context));
	let item = result.expect("A bare matrix should flow through Transform as an Item");
	let transform = item.attribute_cloned_or_default::<DAffine2>(core_types::ATTR_TRANSFORM);
	assert_eq!(transform.translation, DVec2::new(7., 0.), "The translation should compose onto the gained transform attribute");
}

#[test]
fn bare_value_promotes_to_item_wire() {
	let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(3.).into()), vec![NodeId(0)]);

	let mut input_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	input_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<f64>");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(1),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), input_adapter_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A bare f64 should resolve the promotion variant");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion constructor should instantiate");

	let context: Context = None;
	let result: Option<Item<f64>> = futures::executor::block_on(tree.eval(NodeId(1), context));
	assert_eq!(result.map(|item| *item.element()), Some(3.), "The bare value should arrive wrapped as an Item");
}

// Path Modify's ranked modification parameter: a bare `Box<VectorModification>` wraps onto the `Item` wire through its input adapter,
// exercising the nested-generic identifier round-trip between the registered `stringify!` name and the preprocessor's simplified name
#[test]
fn bare_modification_promotes_to_item_wire() {
	use graphene_std::vector::VectorModification;

	let modification = TaggedValue::VectorModification(Box::new(VectorModification::default()));
	let value_node = ProtoNode::value(ConstructionArgs::Value(modification.into()), vec![NodeId(0)]);

	let mut input_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	input_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<Box<VectorModification>>");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(1),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), input_adapter_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A bare Box<VectorModification> should resolve the promotion variant");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The promotion constructor should instantiate");

	let context: Context = None;
	let result: Option<Item<Box<VectorModification>>> = futures::executor::block_on(tree.eval(NodeId(1), context));
	assert!(result.is_some(), "The bare modification should arrive wrapped as an Item");
}

// The Write Attribute value slot: an Item wire's element boxes into a type-erased attribute value, and a stored bare value reaches the same row via a wrap promotion
#[test]
fn item_wire_boxes_into_the_attribute_value_connector() {
	use graphene_std::list::AttributeValueDyn;

	let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(3.).into()), vec![NodeId(0)]);
	let mut wrap_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	wrap_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::WrapItemNode<f64>");
	let mut attribute_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1)]), vec![NodeId(2)]);
	attribute_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<AttributeValueDyn>");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(2),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), wrap_node), (NodeId(2), attribute_adapter_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("An Item<f64> wire should resolve the attribute value boxing row");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The boxing constructor should instantiate");

	let context: Context = None;
	let result: Option<Item<AttributeValueDyn>> = futures::executor::block_on(tree.eval(NodeId(2), context));
	let boxed = result.expect("The boxed attribute value should arrive as an Item");
	assert_eq!(
		boxed.element().0.as_any().downcast_ref::<f64>(),
		Some(&3.),
		"The stored value should be the bare element, not the whole Item"
	);

	let value_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64(5.).into()), vec![NodeId(0)]);
	let mut attribute_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	attribute_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<AttributeValueDyn>");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(1),
		nodes: vec![(NodeId(0), value_node), (NodeId(1), attribute_adapter_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A bare value should wrap-promote into the attribute value boxing row");
	assert!(typing_context.promotions(NodeId(1)).is_some(), "The bare value should be raised by a wrap promotion");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The wrap and boxing constructors should instantiate");

	let context: Context = None;
	let result: Option<Item<AttributeValueDyn>> = futures::executor::block_on(tree.eval(NodeId(1), context));
	let boxed = result.expect("The promoted bare value should arrive boxed as an Item");
	assert_eq!(boxed.element().0.as_any().downcast_ref::<f64>(), Some(&5.), "The stored value should be the bare element");
}

#[test]
fn list_wire_variant_resolves_and_executes() {
	let tree = compile_bounding_box_network(TaggedValue::TypeDefault(descriptor!(List<Vector>)));

	let context: Context = None;
	let result: Option<List<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context.clone()));
	assert!(result.is_some(), "The mapped List wire variant should downcast and execute end-to-end");

	let wrong_type: Option<Item<Vector>> = futures::executor::block_on(tree.eval(NodeId(1), context));
	assert!(wrong_type.is_none(), "A List wire should not downcast as an Item");
}

#[test]
fn expander_flattens_under_the_frame() {
	// A bare string wrapped onto an Item wire feeds String Split's expander primary; its parameters ride Item wires via promotion
	let string_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::String("a,b".into()).into()), vec![NodeId(0)]);
	let mut wrap_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	wrap_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::WrapItemNode<String>");

	let delimiter_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::String(",".into()).into()), vec![NodeId(2)]);
	let mut delimiter_input_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(2)]), vec![NodeId(3)]);
	delimiter_input_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<String>");

	let escaping_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::Bool(false).into()), vec![NodeId(4)]);
	let mut escaping_input_adapter_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(4)]), vec![NodeId(5)]);
	escaping_input_adapter_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::InputAdapterNode<bool>");

	let output = NodeId(6);
	let mut string_split_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1), NodeId(3), NodeId(5)]), vec![output]);
	string_split_node.identifier = graphene_std::text_nodes::string_split::IDENTIFIER;

	let network = ProtoNetwork {
		inputs: vec![],
		output,
		nodes: vec![
			(NodeId(0), string_node),
			(NodeId(1), wrap_node),
			(NodeId(2), delimiter_node),
			(NodeId(3), delimiter_input_adapter_node),
			(NodeId(4), escaping_node),
			(NodeId(5), escaping_input_adapter_node),
			(output, string_split_node),
		],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context
		.update(&network)
		.expect("All-Item connectors should resolve the expander's direct `Item -> List` variant");
	assert!(typing_context.promotions(output).is_none(), "No promotion should be needed when every connector is already an Item");
	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The expander variant should instantiate");

	let context: Context = None;
	let result: Option<List<String>> = futures::executor::block_on(tree.eval(output, context));
	let list = result.expect("An Item-wired expander should produce a List");
	assert_eq!(list.len(), 2, "Splitting \"a,b\" on the comma should expand into two rows");
	let substrings: Vec<_> = list.iter_element_values().map(|s| s.as_str()).collect();
	assert_eq!(substrings, ["a", "b"], "The rows should hold the split substrings");
}

#[test]
fn whole_list_switches_as_one_bundle() {
	// One bool selecting between two whole `List<Graphic>` stacks: each branch bundles into a rank-0 cell, and the result unbundles back to the flat stack
	let condition_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::Bool(true).into()), vec![NodeId(0)]);
	let if_true_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(1)]);
	let if_false_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(2)]);

	let mut switch_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(1), NodeId(2)]), vec![NodeId(3)]);
	switch_node.identifier = ProtoNodeIdentifier::new("math_nodes::SwitchNode");

	let mut unbundle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(3)]), vec![NodeId(4)]);
	unbundle_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::UnbundleNode<Graphic>");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(4),
		nodes: vec![
			(NodeId(0), condition_node),
			(NodeId(1), if_true_node),
			(NodeId(2), if_false_node),
			(NodeId(3), switch_node),
			(NodeId(4), unbundle_node),
		],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context
		.update(&network)
		.expect("A List<Graphic> branch should resolve the Item<Bundle<Graphic>> row via the bundle wrap");

	let promotions = typing_context.promotions(NodeId(3)).expect("The condition wrap and both branch bundles should be recorded");
	let branch_bundles = promotions
		.iter()
		.filter(|(index, adapter)| *index != 0 && matches!(adapter, graph_craft::proto::Promotion::Bundle(_)))
		.count();
	assert_eq!(branch_bundles, 2, "Both branches should bundle their whole list into one opaque cell");

	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The bundle, wrap, and unbundle adapters should instantiate");
	let context: Context = None;
	let result: Option<List<graphene_std::Graphic>> = futures::executor::block_on(tree.eval(NodeId(4), context));
	assert!(result.is_some(), "The whole stack should round-trip through the bundle switch back to a flat List<Graphic>");
}

#[test]
fn a_bundle_unbundles_into_a_list_connector() {
	// A bundled wire (sourced here from a BundleNode, as a Switch branch produces one) feeding Extend's whole-`List` base connector
	let stack_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(0)]);

	let mut bundle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0)]), vec![NodeId(1)]);
	bundle_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::BundleNode<Graphic>");

	let new_layers_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::TypeDefault(descriptor!(List<graphene_std::Graphic>)).into()), vec![NodeId(2)]);

	let mut extend_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(1), NodeId(2)]), vec![NodeId(3)]);
	extend_node.identifier = ProtoNodeIdentifier::new("graphic_nodes::graphic::ExtendNode");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(3),
		nodes: vec![(NodeId(0), stack_node), (NodeId(1), bundle_node), (NodeId(2), new_layers_node), (NodeId(3), extend_node)],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context.update(&network).expect("A bundled wire should feed Extend's List<Graphic> connector via the unbundle");

	let promotions = typing_context.promotions(NodeId(3)).expect("Extend's bundled base should be marked for unbundling");
	assert!(
		promotions.iter().any(|(index, adapter)| *index == 0 && matches!(adapter, graph_craft::proto::Promotion::Unbundle(_))),
		"The base connector should unbundle the whole list"
	);

	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The unbundle adapter should instantiate");
	let context: Context = None;
	let result: Option<List<graphene_std::Graphic>> = futures::executor::block_on(tree.eval(NodeId(3), context));
	assert!(result.is_some(), "The unbundled stack should flow into Extend as a List<Graphic>");
}

#[test]
fn a_whole_list_of_scalars_switches_as_one_bundle() {
	// A single bool selecting between two whole `List<f64>` values, covering a primitive element type and confirming the selected list survives intact
	let condition_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::Bool(true).into()), vec![NodeId(0)]);
	let if_true_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![1., 2.]).into()), vec![NodeId(1)]);
	let if_false_node = ProtoNode::value(ConstructionArgs::Value(TaggedValue::F64Array(vec![3., 4., 5.]).into()), vec![NodeId(2)]);

	let mut switch_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(0), NodeId(1), NodeId(2)]), vec![NodeId(3)]);
	switch_node.identifier = ProtoNodeIdentifier::new("math_nodes::SwitchNode");

	let mut unbundle_node = ProtoNode::value(ConstructionArgs::Nodes(vec![NodeId(3)]), vec![NodeId(4)]);
	unbundle_node.identifier = ProtoNodeIdentifier::new("graphene_core::ops::UnbundleNode<f64>");

	let network = ProtoNetwork {
		inputs: vec![],
		output: NodeId(4),
		nodes: vec![
			(NodeId(0), condition_node),
			(NodeId(1), if_true_node),
			(NodeId(2), if_false_node),
			(NodeId(3), switch_node),
			(NodeId(4), unbundle_node),
		],
	};
	let mut typing_context = TypingContext::new(&crate::node_registry::NODE_REGISTRY);
	typing_context
		.update(&network)
		.expect("A List<f64> branch should resolve the Item<Bundle<f64>> row via the bundle wrap");

	let promotions = typing_context.promotions(NodeId(3)).expect("The condition wrap and both branch bundles should be recorded");
	let branch_bundles = promotions
		.iter()
		.filter(|(index, adapter)| *index != 0 && matches!(adapter, graph_craft::proto::Promotion::Bundle(_)))
		.count();
	assert_eq!(branch_bundles, 2, "Both scalar-list branches should bundle into one opaque cell");

	let tree = futures::executor::block_on(BorrowTree::new(network, &typing_context)).expect("The bundle, wrap, and unbundle adapters should instantiate");
	let context: Context = None;
	let result: Option<List<f64>> = futures::executor::block_on(tree.eval(NodeId(4), context));
	let list = result.expect("The whole scalar list should round-trip through the bundle switch");
	assert_eq!(list.len(), 2, "The true branch's whole list should be selected and preserved intact");
}
