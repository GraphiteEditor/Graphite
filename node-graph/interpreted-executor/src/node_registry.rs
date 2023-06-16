use graph_craft::imaginate_input::{ImaginateCache, ImaginateController, ImaginateMaskStartingFill, ImaginateSamplingMethod};
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_core::ops::IdNode;
use graphene_core::quantization::QuantizationChannels;

use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::color::Color;
use graphene_core::structural::Then;
use graphene_core::value::{ClonedNode, CopiedNode, ValueNode};
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::VectorData;
use graphene_core::{application_io::SurfaceHandle, SurfaceFrame, WasmSurfaceHandleFrame};
use graphene_core::{concrete, generic};
use graphene_core::{fn_type, raster::*};
use graphene_core::{Cow, NodeIdentifier, Type, TypeDescriptor};
use graphene_core::{Node, NodeIO, NodeIOTypes};
use graphene_std::any::{ComposeTypeErased, DowncastBothNode, DynAnyNode, FutureWrapperNode, IntoTypeErasedNode};
use graphene_std::wasm_application_io::*;

#[cfg(feature = "gpu")]
use gpu_executor::{GpuExecutor, ShaderInput, ShaderInputFrame};
use graphene_std::raster::*;
use graphene_std::wasm_application_io::WasmEditorApi;
use wgpu_executor::WgpuExecutor;

use dyn_any::StaticType;
use glam::{DAffine2, DVec2};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

macro_rules! construct_node {
	($args: ident, $path:ty, [$($type:ty),*]) => { async move {
		let mut args = $args.clone();
		args.reverse();
		let node = <$path>::new($(
				{
					let node = graphene_std::any::input_node::<$type>(args.pop().expect("Not enough arguments provided to construct node"));
					let value = node.eval(()).await;
					graphene_core::value::ClonedNode::new(value)
				}
			),*
		);
		node

	}}
}

macro_rules! register_node {
	($path:ty, input: $input:ty, params: [ $($type:ty),*]) => {
		vec![
		(
			NodeIdentifier::new(stringify!($path)),
			|args| {
				Box::pin(async move {
				let node = construct_node!(args, $path, [$($type),*]).await;
				let node = graphene_std::any::FutureWrapperNode::new(node);
				let any: DynAnyNode<$input, _, _> = graphene_std::any::DynAnyNode::new(node);
				Box::new(any) as TypeErasedBox
				})
			},
			{
				let node = <$path>::new($(
						graphene_std::any::PanicNode::<(), $type>::new()
				),*);
				let params = vec![$(fn_type!((), $type)),*];
				let mut node_io = <$path as NodeIO<'_, $input>>::to_node_io(&node, params);
				node_io.input = concrete!(<$input as StaticType>::Static);
				node_io
			},
		)
		]
	};
}
macro_rules! async_node {
	// TODO: we currently need to annotate the type here because the compiler would otherwise (correctly)
	// assign a Pin<Box<dyn Fututure<Output=T>>> type to the node, which is not what we want for now.
	($path:ty, input: $input:ty, output: $output:ty, params: [ $($type:ty),*]) => {
		vec![
		(
			NodeIdentifier::new(stringify!($path)),
			|mut args| {
				Box::pin(async move {
				args.reverse();
				let node = <$path>::new($(graphene_std::any::input_node::<$type>(args.pop().expect("Not enough arguments provided to construct node"))),*);
				let any: DynAnyNode<$input, _, _> = graphene_std::any::DynAnyNode::new(node);
				Box::new(any) as TypeErasedBox
				})
			},
			{
				let node = <$path>::new($(
							graphene_std::any::PanicNode::<(), core::pin::Pin<Box<dyn core::future::Future<Output = $type>>>>::new()
				),*);
				// TODO: Propagate the future type through the node graph
				//let params = vec![$(Type::Fn(Box::new(concrete!(())), Box::new(Type::Future(Box::new(concrete!($type)))))),*];
				let params = vec![$(Type::Fn(Box::new(concrete!(())), Box::new(concrete!($type)))),*];
				let mut node_io = NodeIO::<'_, $input>::to_node_io(&node, params);
				node_io.input = concrete!(<$input as StaticType>::Static);
				node_io.input = concrete!(<$input as StaticType>::Static);
				node_io.output = concrete!(<$output as StaticType>::Static);
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
					Box::pin(async move {
						let node = construct_node!(args, $path, [$($type),*]).await;
						let node = graphene_std::any::FutureWrapperNode::new(node);
						let any: DynAnyNode<Color, _, _> = graphene_std::any::DynAnyNode::new(node);
						any.into_type_erased()
					})
				},
				{
					let params = vec![$(fn_type!($type)),*];
					NodeIOTypes::new(concrete!(Color), concrete!(Color), params)
				},
			),
			(
				NodeIdentifier::new(stringify!($path)),
				|args| {
					Box::pin(async move {
						let node = construct_node!(args, $path, [$($type),*]).await;
						let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
						let map_node = graphene_std::any::FutureWrapperNode::new(map_node);
						let any: DynAnyNode<Image<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_node);
						any.into_type_erased()
					})
				},
				{
					let params = vec![$(fn_type!($type)),*];
					NodeIOTypes::new(concrete!(Image<Color>), concrete!(Image<Color>), params)
				},
			),
			(
				NodeIdentifier::new(stringify!($path)),
				|args| {
					Box::pin(async move {
						let node = construct_node!(args, $path, [$($type),*]).await;
						let map_node = graphene_std::raster::MapImageNode::new(graphene_core::value::ValueNode::new(node));
						let map_node = graphene_std::any::FutureWrapperNode::new(map_node);
						let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_node);
						any.into_type_erased()
					})
				},
				{
					let params = vec![$(fn_type!($type)),*];
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
			|_| Box::pin(async move { FutureWrapperNode::new(IdNode::new()).into_type_erased() }),
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
		register_node!(graphene_core::ops::CloneNode<_>, input: &WasmEditorApi, params: []),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: u32, params: [u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &u32, params: [u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: u32, params: [&u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &u32, params: [&u32]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: f64, params: [f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &f64, params: [f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: f64, params: [&f64]),
		register_node!(graphene_core::ops::AddParameterNode<_>, input: &f64, params: [&f64]),
		register_node!(graphene_core::ops::SomeNode, input: WasmEditorApi, params: []),
		async_node!(graphene_core::ops::IntoNode<_, ImageFrame<SRGBA8>>, input: ImageFrame<Color>, output: ImageFrame<SRGBA8>, params: []),
		async_node!(graphene_core::ops::IntoNode<_, ImageFrame<Color>>, input: ImageFrame<SRGBA8>, output: ImageFrame<Color>, params: []),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::ops::IntoNode<_, &WgpuExecutor>, input: WasmEditorApi, output: &WgpuExecutor, params: []),
		register_node!(graphene_std::raster::DownresNode<_>, input: ImageFrame<Color>, params: []),
		register_node!(graphene_std::raster::MaskImageNode<_, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Color>]),
		register_node!(graphene_std::raster::MaskImageNode<_, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Luma>]),
		register_node!(graphene_std::raster::InsertChannelNode<_, _, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Color>, RedGreenBlue]),
		register_node!(graphene_std::raster::InsertChannelNode<_, _, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Luma>, RedGreenBlue]),
		vec![(
			NodeIdentifier::new("graphene_std::raster::CombineChannelsNode"),
			|args| {
				Box::pin(async move {
					use graphene_core::raster::*;
					use graphene_core::value::*;

					let channel_r: ImageFrame<Color> = DowncastBothNode::new(args[0].clone()).eval(()).await;
					let channel_g: ImageFrame<Color> = DowncastBothNode::new(args[1].clone()).eval(()).await;
					let channel_b: ImageFrame<Color> = DowncastBothNode::new(args[2].clone()).eval(()).await;
					let channel_a: ImageFrame<Color> = DowncastBothNode::new(args[3].clone()).eval(()).await;

					let insert_r = InsertChannelNode::new(ClonedNode::new(channel_r.clone()), CopiedNode::new(RedGreenBlue::Red));
					let insert_g = InsertChannelNode::new(ClonedNode::new(channel_g.clone()), CopiedNode::new(RedGreenBlue::Green));
					let insert_b = InsertChannelNode::new(ClonedNode::new(channel_b.clone()), CopiedNode::new(RedGreenBlue::Blue));
					let complete_node = insert_r.then(insert_g).then(insert_b);
					let complete_node = complete_node.then(MaskImageNode::new(ClonedNode::new(channel_a.clone())));

					// TODO: Move to FN Node for better performance
					let (mut transform, mut bounds) = (DAffine2::ZERO, glam::UVec2::ZERO);
					for image in [channel_a, channel_r, channel_g, channel_b] {
						if image.image.width() > bounds.x {
							bounds = glam::UVec2::new(image.image.width(), image.image.height());
							transform = image.transform;
						}
					}
					let empty_image = ImageFrame {
						image: Image::new(bounds.x, bounds.y, Color::BLACK),
						transform,
					};
					let final_image = ClonedNode::new(empty_image).then(complete_node);
					let final_image = FutureWrapperNode::new(final_image);

					let any: DynAnyNode<(), _, _> = graphene_std::any::DynAnyNode::new(final_image);
					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(()),
				concrete!(ImageFrame<Color>),
				vec![fn_type!(ImageFrame<Color>), fn_type!(ImageFrame<Color>), fn_type!(ImageFrame<Color>), fn_type!(ImageFrame<Color>)],
			),
		)],
		register_node!(graphene_std::raster::EmptyImageNode<_, _>, input: DAffine2, params: [Color]),
		register_node!(graphene_core::memo::MonitorNode<_>, input: ImageFrame<Color>, params: []),
		register_node!(graphene_core::memo::MonitorNode<_>, input: graphene_core::GraphicGroup, params: []),
		async_node!(graphene_std::wasm_application_io::CreateSurfaceNode, input: WasmEditorApi, output: Arc<SurfaceHandle<<graphene_std::wasm_application_io::WasmApplicationIo as graphene_core::application_io::ApplicationIo>::Surface>>, params: []),
		async_node!(
			graphene_std::wasm_application_io::DrawImageFrameNode<_>,
			input: ImageFrame<SRGBA8>,
			output: WasmSurfaceHandleFrame,
			params: [Arc<WasmSurfaceHandle>]
		),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::UniformNode<_>, input: f32, output: ShaderInput<WgpuExecutor>, params: [&WgpuExecutor]),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::StorageNode<_>, input: Vec<u8>, output: ShaderInput<WgpuExecutor>, params: [&WgpuExecutor]),
		#[cfg(feature = "gpu")]
		async_node!(
			gpu_executor::PushNode<_>,
			input: Vec<ShaderInput<WgpuExecutor>>,
			output: Vec<ShaderInput<WgpuExecutor>>,
			params: [ShaderInput<WgpuExecutor>]
		),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::CreateOutputBufferNode<_, _>, input: usize, output: gpu_executor::ShaderInput<WgpuExecutor>, params: [&WgpuExecutor, Type]),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::CreateComputePassNode<_, _, _>, input: gpu_executor::PipelineLayout<WgpuExecutor>, output: <WgpuExecutor as GpuExecutor>::CommandBuffer, params: [&WgpuExecutor, ShaderInput<WgpuExecutor>, gpu_executor::ComputePassDimensions]),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::CreatePipelineLayoutNode<_, _, _, _>, input: <WgpuExecutor as GpuExecutor>::ShaderHandle, output: gpu_executor::PipelineLayout<WgpuExecutor>, params: [String, gpu_executor::Bindgroup<WgpuExecutor>, Arc<ShaderInput<WgpuExecutor>>]),
		#[cfg(feature = "gpu")]
		async_node!(
			gpu_executor::ExecuteComputePipelineNode<_>,
			input: <WgpuExecutor as GpuExecutor>::CommandBuffer,
			output: (),
			params: [&WgpuExecutor]
		),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::ReadOutputBufferNode<_, _>, input: Arc<ShaderInput<WgpuExecutor>>, output: Vec<u8>, params: [&WgpuExecutor, ()]),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::CreateGpuSurfaceNode, input: WasmEditorApi, output: Arc<SurfaceHandle<<WgpuExecutor as GpuExecutor>::Surface>>, params: []),
		#[cfg(feature = "gpu")]
		async_node!(gpu_executor::RenderTextureNode<_, _>, input: ShaderInputFrame<WgpuExecutor>, output: SurfaceFrame, params: [Arc<SurfaceHandle<<WgpuExecutor as GpuExecutor>::Surface>>, &WgpuExecutor]),
		#[cfg(feature = "gpu")]
		async_node!(
			gpu_executor::UploadTextureNode<_>,
			input: ImageFrame<Color>,
			output: ShaderInputFrame<WgpuExecutor>,
			params: [&WgpuExecutor]
		),
		#[cfg(feature = "gpu")]
		vec![(
			NodeIdentifier::new("graphene_std::executor::MapGpuSingleImageNode<_>"),
			|args| {
				Box::pin(async move {
					let document_node: DowncastBothNode<(), graph_craft::document::DocumentNode> = DowncastBothNode::new(args[0].clone());
					let editor_api: DowncastBothNode<(), WasmEditorApi> = DowncastBothNode::new(args[1].clone());
					//let document_node = ClonedNode::new(document_node.eval(()));
					let node = graphene_std::gpu_nodes::MapGpuNode::new(document_node, editor_api);
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(node);
					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(ImageFrame<Color>),
				concrete!(SurfaceFrame),
				vec![fn_type!(graph_craft::document::DocumentNode), fn_type!(WasmEditorApi)],
			),
		)],
		#[cfg(feature = "gpu")]
		vec![(
			NodeIdentifier::new("graphene_std::executor::BlendGpuImageNode<_, _, _>"),
			|args| {
				Box::pin(async move {
					let background: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0].clone());
					let blend_mode: DowncastBothNode<(), BlendMode> = DowncastBothNode::new(args[1].clone());
					let opacity: DowncastBothNode<(), f32> = DowncastBothNode::new(args[2].clone());
					let node = graphene_std::gpu_nodes::BlendGpuImageNode::new(background, blend_mode, opacity);
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(node);

					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(ImageFrame<Color>),
				concrete!(ImageFrame<Color>),
				vec![fn_type!(ImageFrame<Color>), fn_type!(BlendMode), fn_type!(f32)],
			),
		)],
		vec![(
			NodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>"),
			|args| {
				Box::pin(async move {
					let node = ComposeTypeErased::new(args[0].clone(), args[1].clone());
					node.into_type_erased()
				})
			},
			NodeIOTypes::new(
				generic!(T),
				generic!(U),
				vec![Type::Fn(Box::new(generic!(T)), Box::new(generic!(V))), Type::Fn(Box::new(generic!(V)), Box::new(generic!(U)))],
			),
		)],
		register_node!(graphene_std::brush::IntoIterNode<_>, input: &Vec<BrushStroke>, params: []),
		async_node!(graphene_std::brush::BrushNode<_, _, _>, input: ImageFrame<Color>, output: ImageFrame<Color>, params: [ImageFrame<Color>, Vec<BrushStroke>, BrushCache]),
		// Filters
		raster_node!(graphene_core::raster::LuminanceNode<_>, params: [LuminanceCalculation]),
		raster_node!(graphene_core::raster::ExtractChannelNode<_>, params: [RedGreenBlue]),
		raster_node!(graphene_core::raster::ExtractAlphaNode<>, params: []),
		raster_node!(graphene_core::raster::ExtractOpaqueNode<>, params: []),
		raster_node!(graphene_core::raster::LevelsNode<_, _, _, _, _>, params: [f64, f64, f64, f64, f64]),
		register_node!(graphene_std::image_segmentation::ImageSegmentationNode<_>, input: ImageFrame<Color>, params: [ImageFrame<Color>]),
		register_node!(graphene_core::raster::IndexNode<_>, input: Vec<ImageFrame<Color>>, params: [u32]),
		vec![(
			NodeIdentifier::new("graphene_core::raster::BlendNode<_, _, _, _>"),
			|args| {
				Box::pin(async move {
					let image: DowncastBothNode<(), ImageFrame<Color>> = DowncastBothNode::new(args[0].clone());
					let blend_mode: DowncastBothNode<(), BlendMode> = DowncastBothNode::new(args[1].clone());
					let opacity: DowncastBothNode<(), f64> = DowncastBothNode::new(args[2].clone());
					let blend_node = graphene_core::raster::BlendNode::new(CopiedNode::new(blend_mode.eval(()).await), CopiedNode::new(opacity.eval(()).await));
					let node = graphene_std::raster::BlendImageNode::new(image, blend_node);
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(node);
					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(ImageFrame<Color>),
				concrete!(ImageFrame<Color>),
				vec![fn_type!(ImageFrame<Color>), fn_type!(BlendMode), fn_type!(f64)],
			),
		)],
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
				Box::pin(async move {
					use graphene_core::raster::brightness_contrast::*;

					let brightness: DowncastBothNode<(), f64> = DowncastBothNode::new(args[0].clone());
					let brightness = ClonedNode::new(brightness.eval(()).await as f32);
					let contrast: DowncastBothNode<(), f64> = DowncastBothNode::new(args[1].clone());
					let contrast = ClonedNode::new(contrast.eval(()).await as f32);
					let use_legacy: DowncastBothNode<(), bool> = DowncastBothNode::new(args[2].clone());

					if use_legacy.eval(()).await {
						let generate_brightness_contrast_legacy_mapper_node = GenerateBrightnessContrastLegacyMapperNode::new(brightness, contrast);
						let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_brightness_contrast_legacy_mapper_node.eval(())));
						let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
						let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
						any.into_type_erased()
					} else {
						let generate_brightness_contrast_mapper_node = GenerateBrightnessContrastMapperNode::new(brightness, contrast);
						let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_brightness_contrast_mapper_node.eval(())));
						let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
						let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
						any.into_type_erased()
					}
				})
			},
			NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(ImageFrame<Color>), vec![fn_type!(f64), fn_type!(f64), fn_type!(bool)]),
		)],
		raster_node!(graphene_core::raster::OpacityNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::PosterizeNode<_>, params: [f64]),
		raster_node!(graphene_core::raster::ExposureNode<_, _, _>, params: [f64, f64, f64]),
		register_node!(graphene_core::memo::LetNode<_>, input: Option<ImageFrame<Color>>, params: []),
		register_node!(graphene_core::memo::LetNode<_>, input: Option<WasmEditorApi>, params: []),
		async_node!(graphene_core::memo::EndLetNode<_>, input: WasmEditorApi, output: ImageFrame<Color>, params: [ImageFrame<Color>]),
		async_node!(graphene_core::memo::EndLetNode<_>, input: WasmEditorApi, output: VectorData, params: [VectorData]),
		async_node!(
			graphene_core::memo::EndLetNode<_>,
			input: WasmEditorApi,
			output: graphene_core::GraphicGroup,
			params: [graphene_core::GraphicGroup]
		),
		async_node!(
			graphene_core::memo::EndLetNode<_>,
			input: WasmEditorApi,
			output: graphene_core::Artboard,
			params: [graphene_core::Artboard]
		),
		async_node!(
			graphene_core::memo::EndLetNode<_>,
			input: WasmEditorApi,
			output: WasmSurfaceHandleFrame,
			params: [WasmSurfaceHandleFrame]
		),
		async_node!(graphene_core::memo::EndLetNode<_>, input: WasmEditorApi, output: SurfaceFrame, params: [SurfaceFrame]),
		vec![
			(
				NodeIdentifier::new("graphene_core::memo::RefNode<_, _>"),
				|args| {
					Box::pin(async move {
						let node: DowncastBothNode<Option<WasmEditorApi>, WasmEditorApi> = graphene_std::any::DowncastBothNode::new(args[0].clone());
						let node = <graphene_core::memo::RefNode<_, _>>::new(node);
						let any: DynAnyNode<(), _, _> = graphene_std::any::DynAnyNode::new(node);

						any.into_type_erased()
					})
				},
				NodeIOTypes::new(concrete!(()), concrete!(WasmEditorApi), vec![fn_type!(Option<WasmEditorApi>, WasmEditorApi)]),
			),
			(
				NodeIdentifier::new("graphene_std::raster::ImaginateNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
				|args: Vec<Arc<graph_craft::proto::NodeContainer>>| {
					Box::pin(async move {
						use graphene_std::raster::ImaginateNode;
						let cache: ImaginateCache = graphene_std::any::input_node(args.last().unwrap().clone()).eval(()).await;
						macro_rules! instanciate_imaginate_node {
							($($i:expr,)*) => { ImaginateNode::new($(graphene_std::any::input_node(args[$i].clone()),)* cache.into_inner()) };
						}
						let node: ImaginateNode<Color, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _> = instanciate_imaginate_node!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,);
						let any = graphene_std::any::DynAnyNode::new(node);
						any.into_type_erased()
					})
				},
				NodeIOTypes::new(
					concrete!(ImageFrame<Color>),
					concrete!(ImageFrame<Color>),
					vec![
						fn_type!(WasmEditorApi),
						fn_type!(ImaginateController),
						fn_type!(f64),
						fn_type!(Option<DVec2>),
						fn_type!(u32),
						fn_type!(ImaginateSamplingMethod),
						fn_type!(f64),
						fn_type!(String),
						fn_type!(String),
						fn_type!(bool),
						fn_type!(f64),
						fn_type!(Option<Vec<u64>>),
						fn_type!(bool),
						fn_type!(f64),
						fn_type!(ImaginateMaskStartingFill),
						fn_type!(bool),
						fn_type!(bool),
						fn_type!(ImaginateCache),
					],
				),
			),
		],
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: Image<Color>, params: [Image<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: ImageFrame<Color>, params: [ImageFrame<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: QuantizationChannels, params: [QuantizationChannels]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: Vec<DVec2>, params: [Vec<DVec2>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: Arc<WasmSurfaceHandle>, params: [Arc<WasmSurfaceHandle>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: ShaderInputFrame<WgpuExecutor>, params: [ShaderInputFrame<WgpuExecutor>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: wgpu_executor::WgpuSurface, params: [wgpu_executor::WgpuSurface]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), output: SurfaceFrame, params: [SurfaceFrame]),
		register_node!(graphene_core::structural::ConsNode<_, _>, input: Image<Color>, params: [&str]),
		register_node!(graphene_std::raster::ImageFrameNode<_, _>, input: Image<Color>, params: [DAffine2]),
		#[cfg(feature = "quantization")]
		register_node!(graphene_std::quantization::GenerateQuantizationNode<_, _>, input: ImageFrame<Color>, params: [u32, u32]),
		raster_node!(graphene_core::quantization::QuantizeNode<_>, params: [QuantizationChannels]),
		raster_node!(graphene_core::quantization::DeQuantizeNode<_>, params: [QuantizationChannels]),
		register_node!(graphene_core::ops::CloneNode<_>, input: &QuantizationChannels, params: []),
		register_node!(graphene_core::transform::TransformNode<_, _, _, _, _>, input: VectorData, params: [DVec2, f64, DVec2, DVec2, DVec2]),
		register_node!(graphene_core::transform::TransformNode<_, _, _, _, _>, input: ImageFrame<Color>, params: [DVec2, f64, DVec2, DVec2, DVec2]),
		register_node!(graphene_core::transform::TransformNode<_, _, _, _, _>, input: WasmSurfaceHandleFrame, params: [DVec2, f64, DVec2, DVec2, DVec2]),
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
		register_node!(graphene_core::text::TextGenerator<_, _, _>, input: WasmEditorApi, params: [String, graphene_core::text::Font, f64]),
		register_node!(graphene_std::brush::VectorPointsNode, input: VectorData, params: []),
		register_node!(graphene_core::ExtractImageFrame, input: WasmEditorApi, params: []),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: graphene_core::vector::VectorData, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: ImageFrame<Color>, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: graphene_core::GraphicGroup, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>, input: graphene_core::Artboard, params: [String, BlendMode, f32, bool, bool, bool, graphene_core::GraphicGroup]),
		register_node!(graphene_core::ConstructArtboardNode<_, _, _>, input: graphene_core::GraphicGroup, params: [glam::IVec2, glam::IVec2, Color]),
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
