use glam::{DAffine2, DVec2};
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginateSamplingMethod, ImaginateStatus};
use graphene_core::ops::{CloneNode, IdNode, TypeNode};
use graphene_core::vector::VectorData;
use once_cell::sync::Lazy;
use std::collections::HashMap;

use graphene_core::raster::color::Color;
use graphene_core::raster::*;
use graphene_core::structural::Then;
use graphene_core::value::{ClonedNode, CopiedNode, ForgetNode, ValueNode};
use graphene_core::{Node, NodeIO, NodeIOTypes};
use graphene_std::brush::*;
use graphene_std::raster::*;

use graphene_std::any::{ComposeTypeErased, DowncastBothNode, DowncastBothRefNode, DynAnyInRefNode, DynAnyNode, DynAnyRefNode, IntoTypeErasedNode, TypeErasedPinnedRef};

use graphene_core::{Cow, NodeIdentifier, Type, TypeDescriptor};

use graph_craft::proto::NodeConstructor;

use graphene_core::{concrete, fn_type, generic, value_fn};
use graphene_std::memo::{CacheNode, LetNode};
use graphene_std::raster::BlendImageTupleNode;

use crate::executor::NodeContainer;

use dyn_any::StaticType;

use graphene_core::quantization::QuantizationChannels;

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
				let params = vec![$(value_fn!($type)),*];
				let mut node_io = <$path as NodeIO<'_, $input>>::to_node_io(&node, params);
				node_io.input = concrete!(<$input as StaticType>::Static);
				node_io
			},
		)
		]
	};
}
macro_rules! raster_node {
	($path:ty, params: [$($type:ty),*]) => {{
		// this function could also be inlined but serves as a workaround for
		// [wasm-pack#981](https://github.com/rustwasm/wasm-pack/issues/981).
		// The non-inlining function leads to fewer locals in the resulting
		// wasm binary. This issue currently only applies to debug builds, so
		// we guard inlining to only happen on production builds for
		// optimization purposes.
		#[cfg_attr(debug_assertions, inline(never))]
		#[cfg_attr(not(debug_assertions), inline)]
		fn generate_triples() -> Vec<(NodeIdentifier, NodeConstructor, NodeIOTypes)> {
			vec![
			(
				NodeIdentifier::new(stringify!($path)),
				|args| {
					let node = construct_node!(args, $path, [$($type),*]);
					let any: DynAnyNode<Color, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
					Box::pin(any)
				},
				{
					let params = vec![$(value_fn!($type)),*];
					NodeIOTypes::new(concrete!(Color), concrete!(Color), params)
				},
			),
			(
				NodeIdentifier::new(stringify!($path)),
				|args| {
					let node = construct_node!(args, $path, [$($type),*]);
					let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
					let any: DynAnyNode<Image<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
					Box::pin(any)
				},
				{
					let params = vec![$(value_fn!($type)),*];
					NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), params)
				},
			),
			(
				NodeIdentifier::new(stringify!($path)),
				|args| {
					let node = construct_node!(args, $path, [$($type),*]);
					let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(map_node));
					Box::pin(any)
				},
				{
					let params = vec![$(value_fn!($type)),*];
					NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(ImageFrame<Color>), params)
				},
			)
			]
		}
		generate_triples()
	}};
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
		register_node!(graphene_core::ops::CloneNode<_>, input: &ImageFrame<Color>, params: []),
		register_node!(graphene_core::ops::CloneNode<_>, input: &graphene_core::EditorApi, params: []),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: u32, params: [u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &u32, params: [u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: u32, params: [&u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &u32, params: [&u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: f64, params: [f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &f64, params: [f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: f64, params: [&f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &f64, params: [&f64]),
		register_node!(graphene_core::ops::SomeNode, input: graphene_core::EditorApi, params: []),
		register_node!(graphene_std::raster::DownresNode<_>, input: ImageFrame<Color>, params: []),
		register_node!(graphene_std::raster::MaskImageNode<_, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Color>]),
		register_node!(graphene_std::raster::MaskImageNode<_, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Luma>]),
		register_node!(graphene_std::raster::EmptyImageNode<_, _>, input: DAffine2, params: [Color]),
		register_node!(graphene_std::memo::MonitorNode<_>, input: ImageFrame<Color>, params: []),
		register_node!(graphene_std::memo::MonitorNode<_>, input: graphene_core::GraphicGroup, params: []),
		#[cfg(feature = "gpu")]
		register_node!(graphene_std::executor::MapGpuSingleImageNode<_>, input: Image<Color>, params: [String]),
		vec![(
			NodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>"),
			|args| {
				let node = ComposeTypeErased::new(args[0], args[1]);
				node.into_type_erased()
			},
			NodeIOTypes::new(
				generic!(T),
				generic!(U),
				vec![Type::Fn(Box::new(generic!(T)), Box::new(generic!(V))), Type::Fn(Box::new(generic!(V)), Box::new(generic!(U)))],
			),
		)],
		//register_node!(graphene_std::brush::ReduceNode<_, _>, input: core::slice::Iter<ImageFrame<Color>>, params: [ImageFrame<Color>, &ValueNode<BlendImageTupleNode<ValueNode<BlendNode<ClonedNode<BlendMode>, ClonedNode<f64>>>>>]),
		//register_node!(graphene_std::brush::ReduceNode<_, _>, input: core::slice::Iter<ImageFrame<Color>>, params: [AxisAlignedBbox, &MergeBoundingBoxNode]),
		register_node!(graphene_std::brush::IntoIterNode<_>, input: &Vec<DVec2>, params: []),
		vec![(
			NodeIdentifier::new("graphene_std::brush::BrushNode"),
			|args| {
				use graphene_core::structural::*;
				use graphene_core::value::*;
				use graphene_std::brush::*;

				let image: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0]);
				let bounds: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[1]);
				let trace: DowncastBothNode<(), Vec<DVec2>> = DowncastBothNode::new(args[2]);
				let diameter: DowncastBothNode<(), f64> = DowncastBothNode::new(args[3]);
				let hardness: DowncastBothNode<(), f64> = DowncastBothNode::new(args[4]);
				let flow: DowncastBothNode<(), f64> = DowncastBothNode::new(args[5]);
				let color: DowncastBothNode<(), Color> = DowncastBothNode::new(args[6]);

				let stamp = BrushStampGeneratorNode::new(color, CopiedNode::new(hardness.eval(())), CopiedNode::new(flow.eval(())));
				let stamp = stamp.eval(diameter.eval(()));

				let frames = TranslateNode::new(CopiedNode::new(stamp));
				let frames = MapNode::new(ValueNode::new(frames));
				let frames = frames.eval(trace.eval(()).into_iter()).collect::<Vec<_>>();

				let background_bounds = ReduceNode::new(ClonedNode::new(None), ValueNode::new(MergeBoundingBoxNode::new()));
				let background_bounds = background_bounds.eval(frames.clone().into_iter());
				let background_bounds = MergeBoundingBoxNode::new().eval((background_bounds, image.eval(())));
				let mut background_bounds = CopiedNode::new(background_bounds.unwrap().to_transform());

				let bounds_transform = bounds.eval(()).transform;
				if bounds_transform != DAffine2::ZERO {
					background_bounds = CopiedNode::new(bounds_transform);
				}

				let background_image = background_bounds.then(EmptyImageNode::new(CopiedNode::new(Color::TRANSPARENT)));
				let blend_node = graphene_core::raster::BlendNode::new(CopiedNode::new(BlendMode::Normal), CopiedNode::new(100.));

				let background = ExtendImageNode::new(background_image);
				let background_image = image.then(background);

				let final_image = ReduceNode::new(background_image, ValueNode::new(BlendImageTupleNode::new(ValueNode::new(blend_node))));
				let final_image = ClonedNode::new(frames.into_iter()).then(final_image);

				let any: DynAnyNode<(), _, _> = graphene_std::any::DynAnyNode::new(ValueNode::new(final_image));
				Box::pin(any)
			},
			NodeIOTypes::new(
				concrete!(()),
				concrete!(ImageFrame<Color>),
				vec![
					value_fn!(ImageFrame<Color>),
					value_fn!(ImageFrame<Color>),
					value_fn!(Vec<DVec2>),
					value_fn!(f64),
					value_fn!(f64),
					value_fn!(f64),
					value_fn!(Color),
				],
			),
		)],
		vec![(
			NodeIdentifier::new("graphene_std::brush::ReduceNode<_, _>"),
			|args| {
				let acc: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0]);
				let image = acc.eval(());
				let blend_node = graphene_core::raster::BlendNode::new(ClonedNode::new(BlendMode::Normal), ClonedNode::new(1.0));
				let _ = &blend_node as &dyn for<'i> Node<'i, (Color, Color), Output = Color>;
				let node = ReduceNode::new(ClonedNode::new(image), ValueNode::new(BlendImageTupleNode::new(ValueNode::new(blend_node))));
				//let _ = &node as &dyn for<'i> Node<'i, core::slice::Iter<ImageFrame<Color>>, Output = ImageFrame<Color>>;
				let any: DynAnyNode<Box<dyn Iterator<Item = ImageFrame<Color>> + Sync + Send>, _, _> = graphene_std::any::DynAnyNode::new(ValueNode::new(node));
				Box::pin(any)
			},
			NodeIOTypes::new(
				concrete!(Box<dyn Iterator<Item = &ImageFrame<Color>> + Sync + Send>),
				concrete!(ImageFrame<Color>),
				vec![value_fn!(ImageFrame<Color>)],
			),
		)],
		// Filters
		raster_node!(graphene_core::raster::LuminanceNode<_>, params: [LuminanceCalculation]),
		raster_node!(graphene_core::raster::LevelsNode<_, _, _, _, _>, params: [f64, f64, f64, f64, f64]),
		register_node!(graphene_std::image_segmentation::ImageSegmentationNode<_>, input: ImageFrame<Color>, params: [ImageFrame<Color>]),
		register_node!(graphene_core::raster::IndexNode<_>, input: Vec<ImageFrame<Color>>, params: [u32]),
		vec![
			(
				NodeIdentifier::new("graphene_core::raster::BlendNode<_, _, _, _>"),
				|args| {
					let image: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0]);
					let blend_mode: DowncastBothNode<(), BlendMode> = DowncastBothNode::new(args[1]);
					let opacity: DowncastBothNode<(), f64> = DowncastBothNode::new(args[2]);
					let blend_node = graphene_core::raster::BlendNode::new(CopiedNode::new(blend_mode.eval(())), CopiedNode::new(opacity.eval(())));
					let node = graphene_std::raster::BlendImageNode::new(image, ValueNode::new(blend_node));
					let _ = &node as &dyn for<'i> Node<'i, ImageFrame<Color>, Output = ImageFrame<Color>>;
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
					any.into_type_erased()
				},
				NodeIOTypes::new(
					concrete!(ImageFrame<Color>),
					concrete!(ImageFrame<Color>),
					vec![value_fn!(ImageFrame<Color>), value_fn!(BlendMode), value_fn!(f64)],
				),
			),
			(
				NodeIdentifier::new("graphene_core::raster::EraseNode<_, _>"),
				|args| {
					let image: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0]);
					let opacity: DowncastBothNode<(), f64> = DowncastBothNode::new(args[1]);
					let blend_node = graphene_std::brush::EraseNode::new(ClonedNode::new(opacity.eval(())));
					let node = graphene_std::raster::BlendImageNode::new(image, ValueNode::new(blend_node));
					let _ = &node as &dyn for<'i> Node<'i, ImageFrame<Color>, Output = ImageFrame<Color>>;
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(ImageFrame<Color>), vec![value_fn!(ImageFrame<Color>), value_fn!(f64)]),
			),
		],
		raster_node!(graphene_core::raster::GrayscaleNode<_, _, _, _, _, _, _>, params: [Color, f64, f64, f64, f64, f64, f64]),
		raster_node!(graphene_core::raster::HueSaturationNode<_, _, _>, params: [f64, f64, f64]),
		raster_node!(graphene_core::raster::InvertRGBNode, params: []),
		raster_node!(graphene_core::raster::ThresholdNode<_, _, _>, params: [f64, f64, LuminanceCalculation]),
		raster_node!(graphene_core::raster::VibranceNode<_>, params: [f64]),
		raster_node!(
			graphene_core::raster::ChannelMixerNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>,
			params: [bool, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64]
		),
		raster_node!(
			graphene_core::raster::SelectiveColorNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>,
			params: [RelativeAbsolute, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64]
		),
		vec![(
			NodeIdentifier::new("graphene_core::raster::BrightnessContrastNode<_, _, _>"),
			|args| {
				use graphene_core::raster::brightness_contrast::*;

				let brightness: DowncastBothNode<(), f64> = DowncastBothNode::new(args[0]);
				let brightness = ClonedNode::new(brightness.eval(()) as f32);
				let contrast: DowncastBothNode<(), f64> = DowncastBothNode::new(args[1]);
				let contrast = ClonedNode::new(contrast.eval(()) as f32);
				let use_legacy: DowncastBothNode<(), bool> = DowncastBothNode::new(args[2]);

				if use_legacy.eval(()) {
					let generate_brightness_contrast_legacy_mapper_node = GenerateBrightnessContrastLegacyMapperNode::new(brightness, contrast);
					let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_brightness_contrast_legacy_mapper_node.eval(())));
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(ValueNode::new(map_image_frame_node));
					Box::pin(any)
				} else {
					let generate_brightness_contrast_mapper_node = GenerateBrightnessContrastMapperNode::<f32, _, _>::new(brightness, contrast);
					let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_brightness_contrast_mapper_node.eval(())));
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(ValueNode::new(map_image_frame_node));
					Box::pin(any)
				}
			},
			NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(ImageFrame<Color>), vec![value_fn!(f64), value_fn!(f64), value_fn!(bool)]),
		)],
		vec![
			(
				NodeIdentifier::new("graphene_core::raster::CurvesNode<_>"),
				|args| {
					use graphene_core::raster::{curve::Curve, GenerateCurvesNode};

					let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0]);
					let curve = ClonedNode::new(curve.eval(()));

					let generate_curves_node = GenerateCurvesNode::<f32, _>::new(curve);
					let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_curves_node.eval(())));
					let any: DynAnyNode<ImageFrame<Luma>, _, _> = graphene_std::any::DynAnyNode::new(ValueNode::new(map_image_frame_node));
					Box::pin(any)
				},
				NodeIOTypes::new(concrete!(ImageFrame<Luma>), concrete!(ImageFrame<Luma>), vec![value_fn!(graphene_core::raster::curve::Curve)]),
			),
			// TODO: Use channel split and merge for this instead of using LuminanceMut for the whole color.
			(
				NodeIdentifier::new("graphene_core::raster::CurvesNode<_>"),
				|args| {
					use graphene_core::raster::{curve::Curve, GenerateCurvesNode};

					let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0]);
					let curve = ClonedNode::new(curve.eval(()));

					let generate_curves_node = GenerateCurvesNode::<f32, _>::new(curve);
					let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_curves_node.eval(())));
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(ValueNode::new(map_image_frame_node));
					Box::pin(any)
				},
				NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(ImageFrame<Color>), vec![value_fn!(graphene_core::raster::curve::Curve)]),
			),
		],
		raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::PosterizeNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::ExposureNode<_, _, _>, params: [f64, f64, f64]),
		vec![
			(
				NodeIdentifier::new("graphene_std::memo::LetNode<_>"),
				|_| {
					let node: LetNode<ImageFrame<Color>> = graphene_std::memo::LetNode::new();
					let any = graphene_std::any::DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Option<ImageFrame<Color>>), concrete!(&ImageFrame<Color>), vec![]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::LetNode<_>"),
				|_| {
					let node: LetNode<graphene_core::EditorApi> = graphene_std::memo::LetNode::new();
					let any = graphene_std::any::DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Option<graphene_core::EditorApi>), concrete!(&graphene_core::EditorApi), vec![]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::EndLetNode<_>"),
				|args| {
					let input: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0]);
					let node = graphene_std::memo::EndLetNode::new(input);
					let any: DynAnyInRefNode<graphene_core::EditorApi, _, _> = graphene_std::any::DynAnyInRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(generic!(T), concrete!(graphene_core::EditorApi), vec![value_fn!(ImageFrame<Color>)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::EndLetNode<_>"),
				|args| {
					let input: DowncastBothNode<(), VectorData> = DowncastBothNode::new(args[0]);
					let node = graphene_std::memo::EndLetNode::new(input);
					let any: DynAnyInRefNode<graphene_core::EditorApi, _, _> = graphene_std::any::DynAnyInRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(generic!(T), concrete!(graphene_core::EditorApi), vec![value_fn!(VectorData)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::EndLetNode<_>"),
				|args| {
					let input: DowncastBothNode<(), graphene_core::GraphicGroup> = DowncastBothNode::new(args[0]);
					let node = graphene_std::memo::EndLetNode::new(input);
					let any: DynAnyInRefNode<graphene_core::EditorApi, _, _> = graphene_std::any::DynAnyInRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(generic!(T), concrete!(graphene_core::EditorApi), vec![value_fn!(graphene_core::GraphicGroup)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::EndLetNode<_>"),
				|args| {
					let input: DowncastBothNode<(), graphene_core::Artboard> = DowncastBothNode::new(args[0]);
					let node = graphene_std::memo::EndLetNode::new(input);
					let any: DynAnyInRefNode<graphene_core::EditorApi, _, _> = graphene_std::any::DynAnyInRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(generic!(T), concrete!(graphene_core::EditorApi), vec![value_fn!(graphene_core::Artboard)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::RefNode<_, _>"),
				|args| {
					let map_fn: DowncastBothRefNode<Option<graphene_core::EditorApi>, graphene_core::EditorApi> = DowncastBothRefNode::new(args[0]);
					let node = graphene_std::memo::RefNode::new(map_fn);
					let any = graphene_std::any::DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(()), concrete!(&graphene_core::EditorApi), vec![]),
			),
			(
				NodeIdentifier::new("graphene_core::structural::MapImageNode"),
				|args| {
					let map_fn: DowncastBothNode<Color, Color> = DowncastBothNode::new(args[0]);
					let node = graphene_std::raster::MapImageNode::new(ValueNode::new(map_fn));
					let any: DynAnyNode<Image<Color>, _, _> = graphene_std::any::DynAnyNode::new(graphene_core::value::ValueNode::new(node));
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), vec![]),
			),
			(
				NodeIdentifier::new("graphene_std::raster::ImaginateNode<_>"),
				|args| {
					let cached = graphene_std::any::input_node::<Option<std::sync::Arc<Image<Color>>>>(args[15]);
					let node = graphene_std::raster::ImaginateNode::new(cached);
					let any = DynAnyNode::new(ValueNode::new(node));
					any.into_type_erased()
				},
				NodeIOTypes::new(
					concrete!(ImageFrame<Color>),
					concrete!(ImageFrame<Color>),
					vec![
						value_fn!(f64),
						value_fn!(Option<DVec2>),
						value_fn!(f64),
						value_fn!(ImaginateSamplingMethod),
						value_fn!(f64),
						value_fn!(String),
						value_fn!(String),
						value_fn!(bool),
						value_fn!(f64),
						value_fn!(Option<Vec<u64>>),
						value_fn!(bool),
						value_fn!(f64),
						value_fn!(ImaginateMaskStartingFill),
						value_fn!(bool),
						value_fn!(bool),
						value_fn!(Option<std::sync::Arc<Image<Color>>>),
						value_fn!(f64),
						value_fn!(ImaginateStatus),
					],
				),
			),
			(
				NodeIdentifier::new("graphene_core::raster::BlurNode"),
				|args| {
					let radius = DowncastBothNode::<(), u32>::new(args[0]);
					let sigma = DowncastBothNode::<(), f64>::new(args[1]);
					let image = DowncastBothRefNode::<Image<Color>, Image<Color>>::new(args[2]);
					let empty_image: ValueNode<Image<Color>> = ValueNode::new(Image::empty());
					let empty: TypeNode<_, (), Image<Color>> = TypeNode::new(empty_image.then(CloneNode::new()));
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
					let node: DynAnyNode<&Image<Color>, _, _> = DynAnyNode::new(ValueNode::new(new_image));
					node.into_type_erased()
				},
				NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), vec![value_fn!(u32), value_fn!(f64)]),
			),
			//register_node!(graphene_std::memo::CacheNode<_>, input: Image<Color>, params: []),
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|args| {
					let input: DowncastBothNode<(), Image<Color>> = DowncastBothNode::new(args[0]);
					let node: CacheNode<Image<Color>, _> = graphene_std::memo::CacheNode::new(input);
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(()), concrete!(&Image<Color>), vec![value_fn!(Image<Color>)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|args| {
					let input: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0]);
					let node: CacheNode<ImageFrame<Color>, _> = graphene_std::memo::CacheNode::new(input);
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(()), concrete!(&ImageFrame<Color>), vec![value_fn!(ImageFrame<Color>)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|args| {
					let input: DowncastBothNode<ImageFrame<Color>, ImageFrame<Color>> = DowncastBothNode::new(args[0]);
					let node: CacheNode<ImageFrame<Color>, _> = graphene_std::memo::CacheNode::new(input);
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(&ImageFrame<Color>), vec![fn_type!(ImageFrame<Color>, ImageFrame<Color>)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|args| {
					let input: DowncastBothNode<(), QuantizationChannels> = DowncastBothNode::new(args[0]);
					let node: CacheNode<QuantizationChannels, _> = graphene_std::memo::CacheNode::new(input);
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(()), concrete!(&QuantizationChannels), vec![value_fn!(QuantizationChannels)]),
			),
			(
				NodeIdentifier::new("graphene_std::memo::CacheNode"),
				|args| {
					let input: DowncastBothNode<(), Vec<DVec2>> = DowncastBothNode::new(args[0]);
					let node: CacheNode<Vec<DVec2>, _> = graphene_std::memo::CacheNode::new(input);
					let any = DynAnyRefNode::new(node);
					any.into_type_erased()
				},
				NodeIOTypes::new(concrete!(()), concrete!(&Vec<DVec2>), vec![value_fn!(Vec<DVec2>)]),
			),
		],
		register_node!(graphene_core::structural::ConsNode<_, _>, input: Image<Color>, params: [&str]),
		register_node!(graphene_std::raster::ImageFrameNode<_, _>, input: Image<Color>, params: [DAffine2]),
		#[cfg(feature = "quantization")]
		register_node!(graphene_std::quantization::GenerateQuantizationNode<_, _>, input: ImageFrame<Color>, params: [u32, u32]),
		raster_node!(graphene_core::quantization::QuantizeNode<_>, params: [QuantizationChannels]),
		raster_node!(graphene_core::quantization::DeQuantizeNode<_>, params: [QuantizationChannels]),
		register_node!(graphene_core::ops::CloneNode<_>, input: &QuantizationChannels, params: []),
		register_node!(graphene_core::transform::TransformNode<_, _, _, _, _>, input: VectorData, params: [DVec2, f64, DVec2, DVec2, DVec2]),
		register_node!(graphene_core::transform::TransformNode<_, _, _, _, _>, input: ImageFrame<Color>, params: [DVec2, f64, DVec2, DVec2, DVec2]),
		register_node!(graphene_core::transform::SetTransformNode<_>, input: VectorData, params: [VectorData]),
		register_node!(graphene_core::transform::SetTransformNode<_>, input: ImageFrame<Color>, params: [ImageFrame<Color>]),
		register_node!(graphene_core::transform::SetTransformNode<_>, input: VectorData, params: [DAffine2]),
		register_node!(graphene_core::transform::SetTransformNode<_>, input: ImageFrame<Color>, params: [DAffine2]),
		register_node!(graphene_core::vector::SetFillNode<_, _, _, _, _, _, _>, input: VectorData, params: [graphene_core::vector::style::FillType, Option<graphene_core::Color>, graphene_core::vector::style::GradientType, DVec2, DVec2, DAffine2, Vec<(f64, Option<graphene_core::Color>)>]),
		register_node!(graphene_core::vector::SetStrokeNode<_, _, _, _, _, _, _>, input: VectorData, params: [Option<graphene_core::Color>, f64, Vec<f32>, f64, graphene_core::vector::style::LineCap, graphene_core::vector::style::LineJoin, f64]),
		register_node!(graphene_core::vector::generator_nodes::UnitCircleGenerator, input: (), params: []),
		register_node!(
			graphene_core::vector::generator_nodes::PathGenerator<_>,
			input: Vec<graphene_core::vector::bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>,
			params: [Vec<graphene_core::uuid::ManipulatorGroupId>]
		),
		register_node!(graphene_core::text::TextGenerator<_, _, _>, input: graphene_core::EditorApi, params: [String, graphene_core::text::Font, f64]),
		register_node!(graphene_std::brush::VectorPointsNode, input: VectorData, params: []),
		register_node!(graphene_core::ExtractImageFrame, input: graphene_core::EditorApi, params: []),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: graphene_core::vector::VectorData, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: ImageFrame<Color>, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: graphene_core::GraphicGroup, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: graphene_core::Artboard, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructArtboardNode<_>, input: graphene_core::GraphicGroup, params: [Option<[glam::IVec2; 2]>]),
	];
	let mut map: HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();
	for (id, c, types) in node_types.into_iter().flatten() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let new_name = id.name.replace('\n', " ");
		let nid = NodeIdentifier { name: Cow::Owned(new_name) };
		map.entry(nid).or_default().insert(types.clone(), c);
	}
	map
}

pub static NODE_REGISTRY: Lazy<HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>> = Lazy::new(|| node_registry());

#[cfg(test)]
mod protograph_testing {
	// TODO: adde tests testing the node registry
}
