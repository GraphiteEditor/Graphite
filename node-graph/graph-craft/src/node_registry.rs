use std::marker::PhantomData;

use borrow_stack::FixedSizeStack;
use graphene_core::generic::FnNode;
use graphene_core::ops::{AddNode, IdNode};
use graphene_core::raster::color::Color;
use graphene_core::structural::{ConsNode, Then};
use graphene_core::{AsRefNode, Node};
use graphene_std::any::DowncastBothNode;
use graphene_std::any::{Any, DowncastNode, DynAnyNode, IntoTypeErasedNode, TypeErasedNode};
use graphene_std::raster::Image;

use crate::proto::Type;
use crate::proto::{ConstructionArgs, NodeIdentifier, ProtoNode, ProtoNodeInput, Type::Concrete};

use dyn_any::Upcast;

type NodeConstructor = fn(ProtoNode, &FixedSizeStack<TypeErasedNode<'static>>);

//TODO: turn into hasmap
static NODE_REGISTRY: &[(NodeIdentifier<'static>, NodeConstructor)] = &[
	(
		NodeIdentifier {
			name: "graphene_core::ops::IdNode",
			types: &[Concrete("Any<'_>")],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node = pre_node.then(graphene_core::ops::IdNode);
				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::ops::IdNode",
			types: &[Type::Generic],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node = pre_node.then(graphene_core::ops::IdNode);
				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::ops::AddNode",
			types: &[Concrete("u32"), Concrete("u32")],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node: DynAnyNode<AddNode, (u32, u32), _, _> = DynAnyNode::new(graphene_core::ops::AddNode);
				let node = (pre_node).then(node);

				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::ops::AddNode",
			types: &[Concrete("&u32"), Concrete("&u32")],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node: DynAnyNode<AddNode, (&u32, &u32), _, _> = DynAnyNode::new(graphene_core::ops::AddNode);
				let node = (pre_node).then(node);

				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::ops::AddNode",
			types: &[Concrete("&u32"), Concrete("u32")],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node: DynAnyNode<AddNode, (&u32, u32), _, _> = DynAnyNode::new(graphene_core::ops::AddNode);
				let node = (pre_node).then(node);

				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::structural::ConsNode",
			types: &[Concrete("&u32"), Concrete("u32")],
		},
		|proto_node, stack| {
			if let ConstructionArgs::Nodes(cons_node_arg) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					let cons_node_arg = nodes.get(cons_node_arg[0] as usize).unwrap();

					let cons_node = ConsNode::new(DowncastNode::<_, &u32>::new(cons_node_arg));
					let node: DynAnyNode<_, u32, _, _> = DynAnyNode::new(cons_node);
					let node = match proto_node.input {
						ProtoNodeInput::Network => node.into_type_erased(),
						ProtoNodeInput::Node(node_id) => {
							let pre_node = nodes.get(node_id as usize).unwrap();
							(pre_node).then(node).into_type_erased()
						}
						ProtoNodeInput::None => unreachable!(),
					};
					node
				})
			} else {
				unimplemented!()
			}
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::structural::ConsNode",
			types: &[Concrete("u32"), Concrete("u32")],
		},
		|proto_node, stack| {
			if let ConstructionArgs::Nodes(cons_node_arg) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					let cons_node_arg = nodes.get(cons_node_arg[0] as usize).unwrap();

					let cons_node = ConsNode::new(DowncastNode::<_, u32>::new(cons_node_arg));
					let node: DynAnyNode<_, u32, _, _> = DynAnyNode::new(cons_node);
					let node = match proto_node.input {
						ProtoNodeInput::Network => node.into_type_erased(),
						ProtoNodeInput::Node(node_id) => {
							let pre_node = nodes.get(node_id as usize).unwrap();
							(pre_node).then(node).into_type_erased()
						}
						ProtoNodeInput::None => unreachable!(),
					};
					node
				})
			} else {
				unimplemented!()
			}
		},
	),
	// TODO: create macro to impl for all types
	(
		NodeIdentifier {
			name: "graphene_core::structural::ConsNode",
			types: &[Concrete("&u32"), Concrete("&u32")],
		},
		|proto_node, stack| {
			let node_id = proto_node.input.unwrap_node() as usize;
			if let ConstructionArgs::Nodes(cons_node_arg) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					let pre_node = nodes.get(node_id).unwrap();
					let cons_node_arg = nodes.get(cons_node_arg[0] as usize).unwrap();

					let cons_node = ConsNode::new(DowncastNode::<_, &u32>::new(cons_node_arg));
					let node: DynAnyNode<_, &u32, _, _> = DynAnyNode::new(cons_node);
					let node = (pre_node).then(node);
					node.into_type_erased()
				})
			} else {
				unimplemented!()
			}
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::any::DowncastNode",
			types: &[Concrete("&u32")],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();
				let node = pre_node.then(graphene_core::ops::IdNode);
				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::value::ValueNode",
			types: &[Concrete("Any<'_>")],
		},
		|proto_node, stack| {
			stack.push_fn(|_nodes| {
				if let ConstructionArgs::Value(value) = proto_node.construction_args {
					let node = FnNode::new(move |_| value.clone().up_box() as Any<'static>);

					node.into_type_erased()
				} else {
					unreachable!()
				}
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::value::ValueNode",
			types: &[Type::Generic],
		},
		|proto_node, stack| {
			stack.push_fn(|_nodes| {
				if let ConstructionArgs::Value(value) = proto_node.construction_args {
					let node = FnNode::new(move |_| value.clone().up_box() as Any<'static>);
					node.into_type_erased()
				} else {
					unreachable!()
				}
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::raster::GrayscaleNode",
			types: &[],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let node = DynAnyNode::new(graphene_core::raster::GrayscaleNode);

				if let ProtoNodeInput::Node(pre_id) = proto_node.input {
					let pre_node = nodes.get(pre_id as usize).unwrap();
					(pre_node).then(node).into_type_erased()
				} else {
					node.into_type_erased()
				}
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_std::raster::MapImageNode",
			types: &[],
		},
		|proto_node, stack| {
			if let ConstructionArgs::Nodes(operation_node_id) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					let operation_node = nodes.get(operation_node_id[0] as usize).unwrap();
					let operation_node: DowncastBothNode<_, Color, Color> = DowncastBothNode::new(operation_node);
					let map_node = DynAnyNode::new(graphene_std::raster::MapImageNode::new(operation_node));

					if let ProtoNodeInput::Node(node_id) = proto_node.input {
						let pre_node = nodes.get(node_id as usize).unwrap();
						(pre_node).then(map_node).into_type_erased()
					} else {
						map_node.into_type_erased()
					}
				})
			} else {
				unimplemented!()
			}
		},
	),
	(
		NodeIdentifier {
			name: "graphene_std::raster::ImageNode",
			types: &[Concrete("&str")],
		},
		|_proto_node, stack| {
			stack.push_fn(|_nodes| {
				let image = FnNode::new(|s: &str| graphene_std::raster::image_node::<&str>().eval(s).unwrap());
				let node: DynAnyNode<_, &str, _, _> = DynAnyNode::new(image);
				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_std::raster::ExportImageNode",
			types: &[Concrete("&str")],
		},
		|proto_node, stack| {
			stack.push_fn(|nodes| {
				let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();

				let image = FnNode::new(|input: (Image, &str)| graphene_std::raster::export_image_node().eval(input).unwrap());
				let node: DynAnyNode<_, (Image, &str), _, _> = DynAnyNode::new(image);
				let node = (pre_node).then(node);
				node.into_type_erased()
			})
		},
	),
	(
		NodeIdentifier {
			name: "graphene_core::structural::ConsNode",
			types: &[Concrete("Image"), Concrete("&str")],
		},
		|proto_node, stack| {
			let node_id = proto_node.input.unwrap_node() as usize;
			if let ConstructionArgs::Nodes(cons_node_arg) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					let pre_node = nodes.get(node_id).unwrap();
					let cons_node_arg = nodes.get(cons_node_arg[0] as usize).unwrap();

					let cons_node = ConsNode::new(DowncastNode::<_, &str>::new(cons_node_arg));
					let node: DynAnyNode<_, Image, _, _> = DynAnyNode::new(cons_node);
					let node = (pre_node).then(node);
					node.into_type_erased()
				})
			} else {
				unimplemented!()
			}
		},
	),
];

pub fn push_node(proto_node: ProtoNode, stack: &FixedSizeStack<TypeErasedNode<'static>>) {
	if let Some((_id, f)) = NODE_REGISTRY.iter().find(|(id, _)| *id == proto_node.identifier) {
		f(proto_node, stack);
	} else {
		panic!("NodeImplementation: {:?} not found in Registry", proto_node.identifier);
	}
}

#[cfg(test)]
mod protograph_testing {
	use borrow_stack::BorrowStack;

	use super::*;

	/// Lookup a node by th suffix of the name (for testing only)
	fn simple_lookup(suffix: &str) -> &(NodeIdentifier, fn(ProtoNode, &FixedSizeStack<TypeErasedNode<'static>>)) {
		NODE_REGISTRY.iter().find(|node| node.0.name.ends_with(suffix)).unwrap()
	}

	#[test]
	fn add_values() {
		let stack = FixedSizeStack::new(256);
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(2u32)));
		push_node(val_1_protonode, &stack);

		let val_2_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(40u32)));
		push_node(val_2_protonode, &stack);

		let cons_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![1]),
			input: ProtoNodeInput::Node(0),
			identifier: NodeIdentifier::new("graphene_core::structural::ConsNode", &[Concrete("u32"), Concrete("u32")]),
		};
		push_node(cons_protonode, &stack);

		let add_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::Node(2),
			identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[Concrete("u32"), Concrete("u32")]),
		};
		push_node(add_protonode, &stack);

		let result = unsafe { stack.get()[3].eval(Box::new(())) };
		let val = *dyn_any::downcast::<u32>(result).unwrap();
		assert_eq!(val, 42);
	}

	#[test]
	fn greyscale_colour() {
		let stack = FixedSizeStack::new(256);
		let val_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(Color::from_rgb8(10, 20, 30))));
		push_node(val_protonode, &stack);

		let greyscale_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::Node(0),
			identifier: NodeIdentifier::new("graphene_core::raster::GrayscaleNode", &[]),
		};
		push_node(greyscale_protonode, &stack);

		let result = unsafe { stack.get()[1].eval(Box::new(())) };
		let val = *dyn_any::downcast::<Color>(result).unwrap();
		assert_eq!(val, Color::from_rgb8(20, 20, 20));
	}

	#[test]
	fn load_image() {
		let stack = FixedSizeStack::new(256);
		let image_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::None,
			identifier: NodeIdentifier::new("graphene_std::raster::ImageNode", &[Concrete("&str")]),
		};
		push_node(image_protonode, &stack);

		let result = unsafe { stack.get()[0].eval(Box::new("../gstd/test-image-1.png")) };
		let image = *dyn_any::downcast::<Image>(result).unwrap();
		assert_eq!(image.height, 240);
	}

	#[test]
	fn greyscale_map_image() {
		let stack = FixedSizeStack::new(256);
		let image_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::None,
			identifier: NodeIdentifier::new("graphene_std::raster::ImageNode", &[Concrete("&str")]),
		};
		push_node(image_protonode, &stack);

		let greyscale_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::None,
			identifier: NodeIdentifier::new("graphene_core::raster::GrayscaleNode", &[]),
		};
		push_node(greyscale_protonode, &stack);

		let image_map_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![1]),
			input: ProtoNodeInput::Node(0),
			identifier: NodeIdentifier::new("graphene_std::raster::MapImageNode", &[]),
		};
		push_node(image_map_protonode, &stack);

		let result = unsafe { stack.get()[2].eval(Box::new("../gstd/test-image-1.png")) };
		let image = *dyn_any::downcast::<Image>(result).unwrap();
		assert!(!image.data.iter().any(|c| c.r() != c.b() || c.b() != c.g()));
	}
}
