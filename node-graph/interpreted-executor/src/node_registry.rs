use glam::{DAffine2, DVec2};
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginateSamplingMethod, ImaginateStatus};
use graphene_core::ops::{CloneNode, IdNode, TypeNode};
use graphene_core::vector::VectorData;
use once_cell::sync::Lazy;
use std::collections::HashMap;

use graphene_core::raster::color::Color;
use graphene_core::raster::*;
use graphene_core::structural::Then;
use graphene_core::value::{ClonedNode, ForgetNode, ValueNode};
use graphene_core::{Node, NodeIO, NodeIOTypes};

use graphene_std::any::{ComposeTypeErased, DowncastBothNode, DowncastBothRefNode, DynAnyInRefNode, DynAnyNode, DynAnyRefNode, IntoTypeErasedNode, TypeErasedPinnedRef};

use graphene_core::{Cow, NodeIdentifier, Type, TypeDescriptor};

use graph_craft::proto::NodeConstructor;

use graphene_core::{concrete, generic};
use graphene_std::memo::{CacheNode, LetNode};

use crate::executor::NodeContainer;

use dyn_any::StaticType;

#[cfg(feature = "quantization")]
use graphene_core::quantization::{Quantization, QuantizationChannels};

macro_rules! construct_node {
	($args: ident, $path:ty, [$($type:tt),*]) => {{
		let mut args: Vec<TypeErasedPinnedRef<'static>> = $args.clone();
		args.reverse();
		<$path>::new($(graphene_core::value::ClonedNode::new(
			graphene_std::any::input_node::<$type>(args.pop()
				.expect("Not enough arguments provided to construct node")).eval(()))
			),*
		)
	}}
}

macro_rules! register_node {
	($path:ty, input: $input:ty, params: [$($type:ty),*]) => {
		vec![
		(
			NodeIdentifier::new(stringify!($path)),
			|args| {
				let node = construct_node!(args, $path, [$($type),*]);
				let any: DynAnyNode<$input, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
				Box::pin(any)
			},
			{
				let node = IdNode::new().into_type_erased();
				let node = NodeContainer::new(node, vec![]);
				let _node = unsafe { node.erase_lifetime().static_ref() };
				let node = <$path>::new($(
					graphene_std::any::input_node::<$type>(_node)
				),*);
				let params = vec![$((concrete!(()), concrete!($type))),*];
				let mut node_io = <$path as NodeIO<'_, $input>>::to_node_io(&node, params);
				node_io.input = concrete!(<$input as StaticType>::Static);
				node_io
			},
		)
		]
	};
}
macro_rules! raster_node {
	($path:ty, params: [$($type:ty),*]) => {
		vec![
		(
			NodeIdentifier::new(stringify!($path)),
			|args| {
				let node = construct_node!(args, $path, [$($type),*]);
				let any: DynAnyNode<Color, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
				Box::pin(any)
			},
			{
				let params = vec![$((concrete!(()), concrete!($type))),*];
				NodeIOTypes::new(concrete!(Color), concrete!(Color), params)
			},
		),
		(
			NodeIdentifier::new(stringify!($path)),
			|args| {
				let node = construct_node!(args, $path, [$($type),*]);
				let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
				let any: DynAnyNode<Image, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
				Box::pin(any)
			},
			{
				let params = vec![$((concrete!(()), concrete!($type))),*];
				NodeIOTypes::new(concrete!(Image), concrete!(Image), params)
			},
		),
		(
			NodeIdentifier::new(stringify!($path)),
			|args| {
				let node = construct_node!(args, $path, [$($type),*]);
				let map_node = graphene_std::raster::MapImageFrameNode::new(graphene_core::value::ValueNode::new(node));
				let any: DynAnyNode<ImageFrame, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
				Box::pin(any)
			},
			{
				let params = vec![$((concrete!(()), concrete!($type))),*];
				NodeIOTypes::new(concrete!(ImageFrame), concrete!(ImageFrame), params)
			},
		)
		]
	}
}

//TODO: turn into hashmap
fn node_registry() -> HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let node_types: Vec<Vec<(NodeIdentifier, NodeConstructor, NodeIOTypes)>> = vec![
		//register_node!(graphene_core::ops::IdNode, input: Any<'_>, params: []),
		vec![(
			NodeIdentifier::new("graphene_core::ops::IdNode"),
			|_| IdNode::new().into_type_erased(),
			NodeIOTypes::new(generic!(I), generic!(I), vec![]),
		)],
		// TODO: create macro to impl for all types
		register_node!(graphene_core::structural::ConsNode<_, _>, input: u32, params: [u32]),
		register_node!(graphene_core::structural::ConsNode<_, _>, input: u32, params: [&u32]),
		register_node!(graphene_core::structural::ConsNode<_, _>, input: &u32, params: [u32]),
		register_node!(graphene_core::structural::ConsNode<_, _>, input: &u32, params: [&u32]),
		register_node!(graphene_core::ops::AddNode, input: (u32, u32), params: []),
		register_node!(graphene_core::ops::AddNode, input: (u32, &u32), params: []),
		register_node!(graphene_core::ops::CloneNode<_>, input: &ImageFrame, params: []),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: u32, params: [u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &u32, params: [u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: u32, params: [&u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &u32, params: [&u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: f64, params: [f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &f64, params: [f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: f64, params: [&f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &f64, params: [&f64]),
		register_node!(graphene_core::ops::SomeNode, input: ImageFrame, params: []),
		#[cfg(feature = "gpu")]
		register_node!(graphene_std::executor::MapGpuSingleImageNode<_>, input: Image, params: [String]),
		vec![(
			NodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>"),
			|args| {
				let node = ComposeTypeErased::new(args[0], args[1]);
				node.into_type_erased()
			},
			NodeIOTypes::new(generic!(T), generic!(U), vec![(generic!(T), generic!(V)), (generic!(V), generic!(U))]),
		)],
		// Filters
		raster_node!(graphene_core::raster::LuminanceNode<_>, params: [LuminanceCalculation]),
		raster_node!(graphene_core::raster::LevelsNode<_, _, _, _, _>, params: [f64, f64, f64, f64, f64]),
		vec![(
			NodeIdentifier::new("graphene_core::raster::BlendNode<_, _, _, _>"),
			|args| {
				use graphene_core::Node;
				let image: DowncastBothNode<(), ImageFrame> = DowncastBothNode::new(args[0]);
				let blend_mode: DowncastBothNode<(), BlendMode> = DowncastBothNode::new(args[1]);
				let opacity: DowncastBothNode<(), f64> = DowncastBothNode::new(args[2]);
				let blend_node = graphene_core::raster::BlendNode::new(ClonedNode::new(blend_mode.eval(())), ClonedNode::new(opacity.eval(())));
				let node = graphene_std::raster::BlendImageNode::new(image, ValueNode::new(blend_node));
				let _ = &node as &dyn for<'i> Node<'i, ImageFrame, Output = ImageFrame>;
				let any: DynAnyNode<ImageFrame, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
				any.into_type_erased()
			},
			NodeIOTypes::new(
				concrete!(ImageFrame),
				concrete!(ImageFrame),
				vec![(concrete!(()), concrete!(ImageFrame)), (concrete!(()), concrete!(BlendMode)), (concrete!(()), concrete!(f64))],
			),
		)],
		raster_node!(graphene_core::raster::GrayscaleNode<_, _, _, _, _, _, _>, params: [Color, f64, f64, f64, f64, f64, f64]),
		raster_node!(graphene_core::raster::HueSaturationNode<_, _, _>, params: [f64, f64, f64]),
		raster_node!(graphene_core::raster::InvertRGBNode, params: []),
		raster_node!(graphene_core::raster::ThresholdNode<_, _>, params: [LuminanceCalculation, f64]),
		raster_node!(graphene_core::raster::VibranceNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::BrightnessContrastNode< _, _>, params: [f64, f64]),
		raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::PosterizeNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::ExposureNode<_, _, _>, params: [f64, f64, f64]),
		vec![
			(
				NodeIdentifier::new("graphene_std::memo::LetNode<_>"),
				|_| {
					let node: LetNode<ImageFrame> = graphene_std::memo::LetNode::new();
					let any = graphene_std::any::DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Option<ImageFrame>), concrete!(&ImageFrame), vec![]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::EndLetNode<_>"),
				|args| {
					let input: DowncastBothNode<(), ImageFrame> = DowncastBothNode::new(args[0]);
					let node = graphene_std::memo::EndLetNode::new(input);
					let any: DynAnyInRefNode<ImageFrame, _, _> = graphene_std::any::DynAnyInRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(generic!(T), concrete!(ImageFrame), vec![(concrete!(()), concrete!(ImageFrame))]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::RefNode<_, _>"),
				|args| {
					let map_fn: DowncastBothRefNode<Option<ImageFrame>, ImageFrame> = DowncastBothRefNode::new(args[0]);
					let node = graphene_std::memo::RefNode::new(map_fn);
					let any = graphene_std::any::DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(()), concrete!(&ImageFrame), vec![]),
			),
			(
				NodeIdentifier::new("graphene_core::structural::MapImageNode"),
				|args| {
					let map_fn: DowncastBothNode<Color, Color> = DowncastBothNode::new(args[0]);
					let node = graphene_std::raster::MapImageNode::new(ValueNode::new(map_fn));
					let any: DynAnyNode<Image, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Image), concrete!(Image), vec![]),
			),
			(
				NodeIdentifier::new("graphene_std::raster::ImaginateNode<_>"),
				|args| {
					let cached = graphene_std::any::input_node::<Option<std::sync::Arc<Image>>>(args[16]);
					let node = graphene_std::raster::ImaginateNode::new(cached);
					let any = DynAnyNode::new(ValueNode::new(node));
					any.into_type_erased()
				},
				NodeIOTypes::new(
					concrete!(ImageFrame),
					concrete!(ImageFrame),
					vec![
						(concrete!(()), concrete!(DAffine2)),
						(concrete!(()), concrete!(f64)),
						(concrete!(()), concrete!(Option<DVec2>)),
						(concrete!(()), concrete!(f64)),
						(concrete!(()), concrete!(ImaginateSamplingMethod)),
						(concrete!(()), concrete!(f64)),
						(concrete!(()), concrete!(String)),
						(concrete!(()), concrete!(String)),
						(concrete!(()), concrete!(bool)),
						(concrete!(()), concrete!(f64)),
						(concrete!(()), concrete!(Option<Vec<u64>>)),
						(concrete!(()), concrete!(bool)),
						(concrete!(()), concrete!(f64)),
						(concrete!(()), concrete!(ImaginateMaskStartingFill)),
						(concrete!(()), concrete!(bool)),
						(concrete!(()), concrete!(bool)),
						(concrete!(()), concrete!(Option<std::sync::Arc<Image>>)),
						(concrete!(()), concrete!(f64)),
						(concrete!(()), concrete!(ImaginateStatus)),
					],
				),
			),
			(
				NodeIdentifier::new("graphene_core::raster::BlurNode"),
				|args| {
					let radius = DowncastBothNode::<(), u32>::new(args[0]);
					let sigma = DowncastBothNode::<(), f64>::new(args[1]);
					let image = DowncastBothRefNode::<Image, Image>::new(args[2]);
					let empty_image: ValueNode<Image> = ValueNode::new(Image::empty());
					let empty: TypeNode<_, (), Image> = TypeNode::new(empty_image.then(CloneNode::new()));
					use graphene_core::Node;
					let radius = ClonedNode::new(radius.eval(()));
					let sigma = ClonedNode::new(sigma.eval(()));

					//let image = &image as &dyn for<'a> Node<'a, (), Output = &'a Image>;
					// dirty hack: we abuse that the cache node will ignore the input if it is evaluated a second time
					let image = empty.then(image).then(ImageRefNode::new());

					let window = WindowNode::new(radius, image.clone());
					let map_gaussian = MapSndNode::new(ValueNode::new(DistanceNode.then(GaussianNode::new(sigma))));
					let map_distances = MapNode::new(ValueNode::new(map_gaussian));
					let gaussian_iter = window.then(map_distances);
					let avg = gaussian_iter.then(WeightedAvgNode::new());
					let avg: TypeNode<_, u32, Color> = TypeNode::new(avg);
					let blur_iter = MapNode::new(ValueNode::new(avg));
					let pixel_iter = image.clone().then(ImageIndexIterNode::new());
					let blur = pixel_iter.then(blur_iter);
					let collect = CollectNode {};
					let vec = blur.then(collect);
					let new_image = MapImageSliceNode::new(vec);
					let dimensions = image.then(ImageDimensionsNode::new());
					let dimensions: TypeNode<_, (), (u32, u32)> = TypeNode::new(dimensions);
					let new_image = dimensions.then(new_image);
					let new_image = ForgetNode::new().then(new_image);
					let node: DynAnyNode<&Image, _, _> = DynAnyNode::new(ValueNode::new(new_image));
					node.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Image), concrete!(Image), vec![(concrete!(()), concrete!(u32)), (concrete!(()), concrete!(f64))]),
			),
			//register_node!(graphene_std::memo::CacheNode<_>, input: Image, params: []),
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|_| {
					let node: CacheNode<Image> = graphene_std::memo::CacheNode::new();
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Image), concrete!(&Image), vec![]),
			),
			#[cfg(feature = "quantization")]
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|_| {
					let node: CacheNode<QuantizationChannels> = graphene_std::memo::CacheNode::new();
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(QuantizationChannels), concrete!(&QuantizationChannels), vec![]),
			),
		],
		register_node!(graphene_core::structural::ConsNode<_, _>, input: Image, params: [&str]),
		register_node!(graphene_std::raster::ImageFrameNode<_>, input: Image, params: [DAffine2]),
		#[cfg(feature = "quantization")]
		register_node!(graphene_std::quantization::GenerateQuantizationNode<_, _>, input: ImageFrame, params: [u32, u32]),
		#[cfg(feature = "quantization")]
		raster_node!(graphene_core::quantization::QuantizeNode<_>, params: [QuantizationChannels]),
		#[cfg(feature = "quantization")]
		raster_node!(graphene_core::quantization::DeQuantizeNode<_>, params: [QuantizationChannels]),
		#[cfg(feature = "quantization")]
		register_node!(graphene_core::ops::CloneNode<_>, input: &QuantizationChannels, params: []),
		register_node!(graphene_core::vector::TransformNode<_, _, _, _>, input: VectorData, params: [DVec2, f64, DVec2, DVec2]),
		register_node!(graphene_core::vector::SetFillNode<_, _, _, _, _, _, _>, input: VectorData, params: [ graphene_core::vector::style::FillType, graphene_core::Color, graphene_core::vector::style::GradientType, DVec2, DVec2, DAffine2, Vec<(f64, Option<graphene_core::Color>)>]),
		register_node!(graphene_core::vector::SetStrokeNode<_, _, _, _, _, _, _>, input: VectorData, params: [graphene_core::Color, f64, Vec<f32>, f64, graphene_core::vector::style::LineCap, graphene_core::vector::style::LineJoin, f64]),
		register_node!(graphene_core::vector::generator_nodes::UnitCircleGenerator, input: (), params: []),
		register_node!(
			graphene_core::vector::generator_nodes::PathGenerator,
			input: graphene_core::vector::bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>,
			params: []
		),
		/*
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
		*/
	];
	let mut map: HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();
	for (id, c, types) in node_types.into_iter().flatten() {
		map.entry(id).or_default().insert(types.clone(), c);
	}
	map
}

pub static NODE_REGISTRY: Lazy<HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>> = Lazy::new(|| node_registry());

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
