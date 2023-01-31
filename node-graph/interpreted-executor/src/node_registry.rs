use std::sync::Arc;

use borrow_stack::FixedSizeStack;
use glam::DVec2;
use graphene_core::generic::FnNode;
use graphene_core::ops::{AddNode, CloneNode, IdNode, TypeNode};
use graphene_core::raster::color::Color;
use graphene_core::raster::{Image, MapFnIterator};
use graphene_core::structural::{ComposeNode, ConsNode, Then};
use graphene_core::value::ValueNode;
use graphene_core::vector::subpath::Subpath;
use graphene_core::Node;
use graphene_std::any::{Any, ComposeTypeErased, DowncastNode, DynAnyNode, IntoTypeErasedNode, TypeErasedPinned};
use graphene_std::any::{DowncastBothNode, TypeErasedPinnedRef};

use graph_craft::proto::Type;
use graph_craft::proto::{ConstructionArgs, NodeIdentifier, ProtoNode, ProtoNodeInput};

type NodeConstructor = for<'a> fn(Vec<TypeErasedPinnedRef<'static>>) -> TypeErasedPinned<'static>;

use graph_craft::{concrete, generic};
use graphene_std::memo::CacheNode;

macro_rules! register_node {
	($path:ty, input: $input:ty, params: [$($type:ty),*]) => {
		( {NodeIdentifier::new(stringify!($path), &[concrete!(stringify!($input)), $(concrete!(stringify!($type))),*])},
		|args| {
			let mut args = args.clone();
			args.reverse();
			let node = <$path>::new($(graphene_std::any::input_node::<$type>(args.pop().expect("not enough arguments provided to construct node"))),*);
			let any: DynAnyNode<$input, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
			Box::pin(any) as TypeErasedPinned
		})
	};
}

//TODO: turn into hashmap
static NODE_REGISTRY: &[(NodeIdentifier, NodeConstructor)] = &[
	register_node!(graphene_core::ops::IdNode, input: Any<'_>, params: []),
	// TODO: create macro to impl for all types
	register_node!(graphene_core::structural::ConsNode<_, _>, input: u32, params: [u32]),
	register_node!(graphene_core::structural::ConsNode<_, _>, input: u32, params: [&u32]),
	register_node!(graphene_core::structural::ConsNode<_, _>, input: &u32, params: [u32]),
	register_node!(graphene_core::structural::ConsNode<_, _>, input: &u32, params: [&u32]),
	register_node!(graphene_core::ops::AddNode, input: (u32, u32), params: []),
	register_node!(graphene_core::ops::AddNode, input: (&u32, u32), params: []),
	register_node!(graphene_core::ops::AddNode, input: (u32, &u32), params: []),
	register_node!(graphene_core::ops::AddNode, input: (&u32, &u32), params: []),
	register_node!(graphene_core::ops::AddNode, input: (f64, f64), params: []),
	register_node!(graphene_core::ops::AddNode, input: (&f64, f64), params: []),
	register_node!(graphene_core::ops::AddNode, input: (f64, &f64), params: []),
	register_node!(graphene_core::ops::AddNode, input: (&f64, &f64), params: []),
	register_node!(graphene_core::raster::GrayscaleColorNode, input: Color, params: []),
	register_node!(graphene_core::raster::BrightenColorNode<_>, input: Color, params: [f32]),
	register_node!(graphene_core::raster::HueShiftColorNode<_>, input: Color, params: [f32]),
	(NodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>", &[generic!("T"), generic!("U")]), |args| {
		let args = args;
		let node = ComposeTypeErased::new(args[0], args[1]);
		Box::pin(node) as TypeErasedPinned
	}),
	(NodeIdentifier::new("graphene_core::ops::IdNode", &[generic!("T")]), |_| IdNode::new().into_type_erased()),
	/*
	(NodeIdentifier::new("graphene_std::raster::MapImageNode", &[]), |proto_node, stack| {
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
	}),
	(NodeIdentifier::new("graphene_std::raster::GrayscaleNode", &[]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let node = DynAnyNode::new(graphene_std::raster::GrayscaleNode);

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::InvertRGBNode", &[]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let node = DynAnyNode::new(graphene_std::raster::InvertRGBNode);

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::HueSaturationNode", &[concrete!("&TypeErasedNode")]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("HueSaturationNode Node constructed without inputs") };

			let hue: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
			let saturation: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[1] as usize).unwrap());
			let lightness: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[2] as usize).unwrap());
			let node = DynAnyNode::new(graphene_std::raster::HueSaturationNode::new(hue, saturation, lightness));
			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(
		NodeIdentifier::new("graphene_std::raster::BrightnessContrastNode", &[concrete!("&TypeErasedNode")]),
		|proto_node, stack| {
			stack.push_fn(move |nodes| {
				let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("BrightnessContrastNode Node constructed without inputs") };

				let brightness: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
				let contrast: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[1] as usize).unwrap());
				let node = DynAnyNode::new(graphene_std::raster::BrightnessContrastNode::new(brightness, contrast));

				if let ProtoNodeInput::Node(node_id) = proto_node.input {
					let pre_node = nodes.get(node_id as usize).unwrap();
					(pre_node).then(node).into_type_erased()
				} else {
					node.into_type_erased()
				}
			})
		},
	),
	(NodeIdentifier::new("graphene_std::raster::GammaNode", &[concrete!("&TypeErasedNode")]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("GammaNode Node constructed without inputs") };
			let gamma: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
			let node = DynAnyNode::new(graphene_std::raster::GammaNode::new(gamma));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::OpacityNode", &[concrete!("&TypeErasedNode")]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("OpacityNode Node constructed without inputs") };
			let opacity: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
			let node = DynAnyNode::new(graphene_std::raster::OpacityNode::new(opacity));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::PosterizeNode", &[concrete!("&TypeErasedNode")]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("Posterize node constructed without inputs") };
			let value: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
			let node = DynAnyNode::new(graphene_std::raster::PosterizeNode::new(value));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::ExposureNode", &[concrete!("&TypeErasedNode")]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("ExposureNode constructed without inputs") };
			let value: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
			let node = DynAnyNode::new(graphene_std::raster::ExposureNode::new(value));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::ImaginateNode", &[concrete!("&TypeErasedNode")]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("ImaginateNode constructed without inputs") };
			let value: DowncastBothNode<_, (), Option<std::sync::Arc<graphene_core::raster::Image>>> = DowncastBothNode::new(nodes.get(construction_nodes[15] as usize).unwrap());

			let node = DynAnyNode::new(graphene_std::raster::ImaginateNode::new(value));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::ImageNode", &[concrete!("&str")]), |_proto_node, stack| {
		stack.push_fn(|_nodes| {
			let image = FnNode::new(|s: &str| graphene_std::raster::image_node::<&str>().eval(s).unwrap());
			let node: DynAnyNode<_, &str, _, _> = DynAnyNode::new(image);
			node.into_type_erased()
		})
	}),
	(NodeIdentifier::new("graphene_std::raster::ExportImageNode", &[concrete!("&str")]), |proto_node, stack| {
		stack.push_fn(|nodes| {
			let pre_node = nodes.get(proto_node.input.unwrap_node() as usize).unwrap();

			let image = FnNode::new(|input: (Image, &str)| graphene_std::raster::export_image_node().eval(input).unwrap());
			let node: DynAnyNode<_, (Image, &str), _, _> = DynAnyNode::new(image);
			let node = (pre_node).then(node);
			node.into_type_erased()
		})
	}),
	(
		NodeIdentifier::new("graphene_core::structural::ConsNode", &[concrete!("Image"), concrete!("&str")]),
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
	(NodeIdentifier::new("graphene_std::memo::CacheNode", &[concrete!("Image")]), |proto_node, stack| {
		let node_id = proto_node.input.unwrap_node() as usize;
		use graphene_core::raster::*;
		if let ConstructionArgs::Nodes(image_args) = proto_node.construction_args {
			stack.push_fn(move |nodes| {
				let image = nodes.get(node_id).unwrap();
				let node: DynAnyNode<_, Image, Image, &Image> = DynAnyNode::new(CacheNode::new());

				let node = image.then(node);
				node.into_type_erased()
			})
		} else {
			unimplemented!()
		}
	}),
	(NodeIdentifier::new("graphene_core::raster::BlurNode", &[]), |proto_node, stack| {
		let node_id = proto_node.input.unwrap_node() as usize;
		use graphene_core::raster::*;

		static EMPTY_IMAGE: ValueNode<Image> = ValueNode::new(Image::empty());
		if let ConstructionArgs::Nodes(blur_args) = proto_node.construction_args {
			stack.push_fn(move |nodes| {
				let pre_node = nodes.get(node_id).unwrap();
				let radius = nodes.get(blur_args[0] as usize).unwrap();
				let sigma = nodes.get(blur_args[1] as usize).unwrap();
				let radius = DowncastBothNode::<_, (), u32>::new(radius);
				let sigma = DowncastBothNode::<_, (), f64>::new(sigma);
				let image = DowncastBothNode::<_, Image, &Image>::new(pre_node);
				// dirty hack: we abuse that the cache node will ignore the input if it is
				// evaluated a second time
				let empty: TypeNode<_, (), Image> = TypeNode::new((&EMPTY_IMAGE).then(CloneNode));
				let image = empty.then(image).then(ImageRefNode::new());
				let window = WindowNode::new(radius, image);
				let window: TypeNode<_, u32, ImageWindowIterator<'static>> = TypeNode::new(window);
				let map_gaussian = MapSndNode::new(DistanceNode.then(GaussianNode::new(sigma)));
				let map_distances = MapNode::new(map_gaussian);
				let gaussian_iter = window.then(map_distances);
				let avg = gaussian_iter.then(WeightedAvgNode::new());
				let avg: TypeNode<_, u32, Color> = TypeNode::new(avg);
				let blur_iter = MapNode::new(avg);
				let blur = ImageIndexIterNode.then(blur_iter);
				let blur: TypeNode<_, ImageSlice<'_>, MapFnIterator<_, _>> = TypeNode::new(blur);
				let collect = CollectNode {};
				let vec = blur.then(collect);
				let vec: TypeNode<_, ImageSlice<'_>, Vec<Color>> = TypeNode::new(vec);
				let new_image = MapImageSliceNode::new(vec);
				let new_image: TypeNode<_, ImageSlice<'_>, Image> = TypeNode::new(new_image);
				let node: DynAnyNode<_, &Image, Image, Image> = DynAnyNode::new(ImageRefNode.then(new_image));
				let node = ComposeNode::new(pre_node, node);
				node.into_type_erased()
			})
		} else {
			unimplemented!()
		}
	}),
	(NodeIdentifier::new("graphene_std::vector::generator_nodes::UnitCircleGenerator", &[]), |_proto_node, stack| {
		stack.push_fn(|_nodes| DynAnyNode::new(graphene_std::vector::generator_nodes::UnitCircleGenerator).into_type_erased())
	}),
	(NodeIdentifier::new("graphene_std::vector::generator_nodes::UnitSquareGenerator", &[]), |_proto_node, stack| {
		stack.push_fn(|_nodes| DynAnyNode::new(graphene_std::vector::generator_nodes::UnitSquareGenerator).into_type_erased())
	}),
	(NodeIdentifier::new("graphene_std::vector::generator_nodes::BlitSubpath", &[]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("BlitSubpath without subpath input") };
			let value_node = nodes.get(construction_nodes[0] as usize).unwrap();
			let input_node: DowncastBothNode<_, (), Subpath> = DowncastBothNode::new(value_node);
			let node = DynAnyNode::new(graphene_std::vector::generator_nodes::BlitSubpath::new(input_node));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	(NodeIdentifier::new("graphene_std::vector::generator_nodes::TransformSubpathNode", &[]), |proto_node, stack| {
		stack.push_fn(move |nodes| {
			let ConstructionArgs::Nodes(construction_nodes) = proto_node.construction_args else { unreachable!("TransformSubpathNode without subpath input") };
			let translate_node: DowncastBothNode<_, (), DVec2> = DowncastBothNode::new(nodes.get(construction_nodes[0] as usize).unwrap());
			let rotate_node: DowncastBothNode<_, (), f64> = DowncastBothNode::new(nodes.get(construction_nodes[1] as usize).unwrap());
			let scale_node: DowncastBothNode<_, (), DVec2> = DowncastBothNode::new(nodes.get(construction_nodes[2] as usize).unwrap());
			let shear_node: DowncastBothNode<_, (), DVec2> = DowncastBothNode::new(nodes.get(construction_nodes[3] as usize).unwrap());

			let node = DynAnyNode::new(graphene_std::vector::generator_nodes::TransformSubpathNode::new(translate_node, rotate_node, scale_node, shear_node));

			if let ProtoNodeInput::Node(node_id) = proto_node.input {
				let pre_node = nodes.get(node_id as usize).unwrap();
				(pre_node).then(node).into_type_erased()
			} else {
				node.into_type_erased()
			}
		})
	}),
	#[cfg(feature = "gpu")]
	(
		NodeIdentifier::new("graphene_std::executor::MapGpuNode", &[concrete!("&TypeErasedNode"), concrete!("Color"), concrete!("Color")]),
		|proto_node, stack| {
			if let ConstructionArgs::Nodes(operation_node_id) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					info!("Map image Depending upon id {:?}", operation_node_id);
					let operation_node = nodes.get(operation_node_id[0] as usize).unwrap();
					let input_node: DowncastBothNode<_, (), &graph_craft::document::NodeNetwork> = DowncastBothNode::new(operation_node);
					let map_node: graphene_std::executor::MapGpuNode<_, Vec<u32>, u32, u32> = graphene_std::executor::MapGpuNode::new(input_node);
					let map_node = DynAnyNode::new(map_node);

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
	#[cfg(feature = "gpu")]
	(
		NodeIdentifier::new("graphene_std::executor::MapGpuSingleImageNode", &[concrete!("&TypeErasedNode")]),
		|proto_node, stack| {
			if let ConstructionArgs::Nodes(operation_node_id) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					info!("Map image Depending upon id {:?}", operation_node_id);
					let operation_node = nodes.get(operation_node_id[0] as usize).unwrap();
					let input_node: DowncastBothNode<_, (), String> = DowncastBothNode::new(operation_node);
					let map_node = graphene_std::executor::MapGpuSingleImageNode(input_node);
					let map_node = DynAnyNode::new(map_node);

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
	#[cfg(feature = "quantization")]
	(
		NodeIdentifier::new("graphene_std::quantization::GenerateQuantizationNode", &[concrete!("&TypeErasedNode")]),
		|proto_node, stack| {
			if let ConstructionArgs::Nodes(operation_node_id) = proto_node.construction_args {
				stack.push_fn(move |nodes| {
					info!("Quantization Depending upon id {:?}", operation_node_id);
					let samples_node = nodes.get(operation_node_id[0] as usize).unwrap();
					let index_node = nodes.get(operation_node_id[1] as usize).unwrap();
					let samples_node: DowncastBothNode<_, (), u32> = DowncastBothNode::new(samples_node);
					let index_node: DowncastBothNode<_, (), u32> = DowncastBothNode::new(index_node);
					let map_node = graphene_std::quantization::GenerateQuantizationNode::new(samples_node, index_node);
					let map_node = DynAnyNode::new(map_node);

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
	*/
];

pub fn constrcut_node<'a>(ident: NodeIdentifier, construction_args: Vec<TypeErasedPinnedRef<'static>>) -> TypeErasedPinned<'a> {
	if let Some((_id, f)) = NODE_REGISTRY.iter().find(|(id, _)| *id == ident) {
		f(construction_args)
	} else {
		let other_types = NODE_REGISTRY.iter().map(|(id, _)| id).filter(|id| id.name.as_ref() == ident.name.as_ref()).collect::<Vec<_>>();
		panic!("NodeImplementation: {:?} not found in Registry. Types for which the node is implemented:\n {:#?}", ident, other_types);
	}
}
/*
#[cfg(test)]
mod protograph_testing {
	use borrow_stack::BorrowStack;

	use super::*;

	#[test]
	fn add_values() {
		let stack = FixedSizeStack::new(256);
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(2u32)));
		constrcut_node(val_1_protonode, &stack);

		let val_2_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(40u32)));
		constrcut_node(val_2_protonode, &stack);

		let cons_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![1]),
			input: ProtoNodeInput::Node(0),
			identifier: NodeIdentifier::new("graphene_core::structural::ConsNode", &[concrete!("u32"), concrete!("u32")]),
		};
		constrcut_node(cons_protonode, &stack);

		let add_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::Node(2),
			identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[concrete!("u32"), concrete!("u32")]),
		};
		constrcut_node(add_protonode, &stack);

		let result = unsafe { stack.get()[3].eval(Box::new(())) };
		let val = *dyn_any::downcast::<u32>(result).unwrap();
		assert_eq!(val, 42);
	}

	#[test]
	fn grayscale_color() {
		let stack = FixedSizeStack::new(256);
		let val_protonode = ProtoNode::value(ConstructionArgs::Value(Box::new(Color::from_rgb8(10, 20, 30))));
		constrcut_node(val_protonode, &stack);

		let grayscale_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::Node(0),
			identifier: NodeIdentifier::new("graphene_core::raster::GrayscaleColorNode", &[]),
		};
		constrcut_node(grayscale_protonode, &stack);

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
			identifier: NodeIdentifier::new("graphene_std::raster::ImageNode", &[concrete!("&str")]),
		};
		constrcut_node(image_protonode, &stack);

		let result = unsafe { stack.get()[0].eval(Box::new("../gstd/test-image-1.png")) };
		let image = *dyn_any::downcast::<Image>(result).unwrap();
		assert_eq!(image.height, 240);
	}

	#[test]
	fn grayscale_map_image() {
		let stack = FixedSizeStack::new(256);
		let image_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::None,
			identifier: NodeIdentifier::new("graphene_std::raster::ImageNode", &[concrete!("&str")]),
		};
		constrcut_node(image_protonode, &stack);

		let grayscale_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![]),
			input: ProtoNodeInput::None,
			identifier: NodeIdentifier::new("graphene_core::raster::GrayscaleColorNode", &[]),
		};
		constrcut_node(grayscale_protonode, &stack);

		let image_map_protonode = ProtoNode {
			construction_args: ConstructionArgs::Nodes(vec![1]),
			input: ProtoNodeInput::Node(0),
			identifier: NodeIdentifier::new("graphene_std::raster::MapImageNode", &[]),
		};
		constrcut_node(image_map_protonode, &stack);

		let result = unsafe { stack.get()[2].eval(Box::new("../gstd/test-image-1.png")) };
		let image = *dyn_any::downcast::<Image>(result).unwrap();
		assert!(!image.data.iter().any(|c| c.r() != c.b() || c.b() != c.g()));
	}
}
*/
