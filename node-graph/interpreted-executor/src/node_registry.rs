use dyn_any::StaticType;
use graph_craft::document::value::RenderOutput;
use graph_craft::imaginate_input::{ImaginateController, ImaginateMaskStartingFill, ImaginateSamplingMethod};
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_core::fn_type;
use graphene_core::ops::IdentityNode;
use graphene_core::raster::color::Color;
use graphene_core::raster::*;
use graphene_core::structural::Then;
use graphene_core::transform::Footprint;
use graphene_core::value::{ClonedNode, ValueNode};
use graphene_core::vector::VectorData;
use graphene_core::{concrete, generic, Artboard, GraphicGroup};
use graphene_core::{Cow, ProtoNodeIdentifier, Type};
use graphene_core::{Node, NodeIO, NodeIOTypes};
use graphene_std::any::{ComposeTypeErased, DowncastBothNode, DynAnyNode, FutureWrapperNode, IntoTypeErasedNode};
use graphene_std::application_io::TextureFrame;
use graphene_std::raster::*;
use graphene_std::wasm_application_io::*;
use graphene_std::GraphicElement;
#[cfg(feature = "gpu")]
use wgpu_executor::{ShaderInputFrame, WgpuExecutor};
use wgpu_executor::{WgpuSurface, WindowHandle};

use glam::{DAffine2, DVec2, UVec2};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

macro_rules! construct_node {
	($args: ident, $path:ty, [$($arg:ty => $type:ty),*]) => { async move {
		let mut args = $args.clone();
		args.reverse();
		let node = <$path>::new($(
				{
					let node = graphene_std::any::downcast_node::<$arg, $type>(args.pop().expect("Not enough arguments provided to construct node"));
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
		register_node!($path, input: $input, fn_params: [ $(() => $type),*])
	};
	($path:ty, input: $input:ty, fn_params: [ $($arg:ty => $type:ty),*]) => {
		vec![
		(
			ProtoNodeIdentifier::new(stringify!($path)),
			|args| {
				Box::pin(async move {
				let node = construct_node!(args, $path, [$($arg => $type),*]).await;
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
	// assign a Pin<Box<dyn Future<Output=T>>> type to the node, which is not what we want for now.
	//
	// This `params` variant of the macro wraps the normal `fn_params` variant and is used as a shorthand for writing `T` instead of `() => T`
	($path:ty, input: $input:ty, params: [$($type:ty),*]) => {
		async_node!($path, input: $input, fn_params: [ $(() => $type),*])
	};
	($path:ty, input: $input:ty, fn_params: [$($arg:ty => $type:ty),*]) => {
		vec![
		(
			ProtoNodeIdentifier::new(stringify!($path)),
			|mut args| {
				Box::pin(async move {
					args.reverse();
					let node = <$path>::new($(graphene_std::any::downcast_node::<$arg, $type>(args.pop().expect("Not enough arguments provided to construct node"))),*);
					let any: DynAnyNode<$input, _, _> = graphene_std::any::DynAnyNode::new(node);
					Box::new(any) as TypeErasedBox
				})
			},
			{
				let node = <$path>::new($(
							graphene_std::any::PanicNode::<$arg, core::pin::Pin<Box<dyn core::future::Future<Output = $type> + Send>>>::new()
				),*);
				// TODO: Propagate the future type through the node graph
				// let params = vec![$(Type::Fn(Box::new(concrete!(())), Box::new(Type::Future(Box::new(concrete!($type)))))),*];
				let params = vec![$(fn_type!($arg, $type)),*];
				let mut node_io = NodeIO::<'_, $input>::to_async_node_io(&node, params);
				node_io.input = concrete!(<$input as StaticType>::Static);
				node_io
			},
		)
		]
	};
}

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let node_types: Vec<Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)>> = vec![
		vec![(
			ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode"),
			|_| Box::pin(async move { FutureWrapperNode::new(IdentityNode::new()).into_type_erased() }),
			NodeIOTypes::new(generic!(I), generic!(I), vec![]),
		)],
		async_node!(graphene_core::ops::IntoNode<ImageFrame<SRGBA8>>, input: ImageFrame<Color>, params: []),
		async_node!(graphene_core::ops::IntoNode<ImageFrame<Color>>, input: ImageFrame<SRGBA8>, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroup>, input: ImageFrame<Color>, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroup>, input: VectorData, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroup>, input: GraphicGroup, params: []),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::ops::IntoNode<&WgpuExecutor>, input: &WasmEditorApi, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicElement>, input: VectorData, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicElement>, input: ImageFrame<Color>, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicElement>, input: GraphicGroup, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroup>, input: GraphicGroup, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroup>, input: VectorData, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroup>, input: ImageFrame<Color>, params: []),
		register_node!(graphene_std::raster::MaskImageNode<_, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Color>]),
		register_node!(graphene_std::raster::MaskImageNode<_, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Luma>]),
		register_node!(graphene_std::raster::InsertChannelNode<_, _, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Color>, RedGreenBlue]),
		register_node!(graphene_std::raster::InsertChannelNode<_, _, _, _>, input: ImageFrame<Color>, params: [ImageFrame<Luma>, RedGreenBlue]),
		vec![(
			ProtoNodeIdentifier::new("graphene_std::raster::CombineChannelsNode"),
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
						..Default::default()
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
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Footprint, fn_params: [Footprint => ImageFrame<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: (), params: [ImageFrame<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Footprint, fn_params: [Footprint => VectorData]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: (), fn_params: [() => VectorData]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Footprint, fn_params: [Footprint => graphene_core::GraphicGroup]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: (), fn_params: [() => graphene_core::GraphicGroup]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Footprint, fn_params: [Footprint => graphene_core::GraphicElement]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: (), fn_params: [() => graphene_core::GraphicElement]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Footprint, fn_params: [Footprint => Artboard]),
		#[cfg(feature = "gpu")]
		register_node!(wgpu_executor::CreateGpuSurfaceNode<_>, input: (), params: [&WasmEditorApi]),
		#[cfg(feature = "gpu")]
		vec![(
			ProtoNodeIdentifier::new("graphene_std::executor::MapGpuSingleImageNode<_>"),
			|args| {
				Box::pin(async move {
					let document_node: DowncastBothNode<(), graph_craft::document::DocumentNode> = DowncastBothNode::new(args[0].clone());
					let editor_api: DowncastBothNode<(), &WasmEditorApi> = DowncastBothNode::new(args[1].clone());
					// let document_node = ClonedNode::new(document_node.eval(()));
					let node = graphene_std::gpu_nodes::MapGpuNode::new(document_node, editor_api);
					let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(node);
					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(ImageFrame<Color>),
				concrete!(ImageFrame<Color>),
				vec![fn_type!(graph_craft::document::DocumentNode), fn_type!(WasmEditorApi)],
			),
		)],
		vec![(
			ProtoNodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>"),
			|args| {
				Box::pin(async move {
					let node = ComposeTypeErased::new(args[0].clone(), args[1].clone());
					node.into_type_erased()
				})
			},
			// This is how we can generically define composition of two nodes.
			// See further details in the code definition for the `struct ComposeNode<First, Second, I> { ... }` struct.
			NodeIOTypes::new(
				generic!(T),
				generic!(U),
				vec![Type::Fn(Box::new(generic!(T)), Box::new(generic!(V))), Type::Fn(Box::new(generic!(V)), Box::new(generic!(U)))],
			),
		)],
		// Filters
		vec![(
			ProtoNodeIdentifier::new("graphene_core::raster::BrightnessContrastNode<_, _, _>"),
			|args| {
				Box::pin(async move {
					use graphene_core::raster::brightness_contrast::*;

					let brightness: DowncastBothNode<(), f64> = DowncastBothNode::new(args[0].clone());
					let brightness = ClonedNode::new(brightness.eval(()).await);
					let contrast: DowncastBothNode<(), f64> = DowncastBothNode::new(args[1].clone());
					let contrast = ClonedNode::new(contrast.eval(()).await);
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
		vec![
			(
				ProtoNodeIdentifier::new("graphene_core::raster::CurvesNode<_>"),
				|args| {
					use graphene_core::raster::{curve::Curve, GenerateCurvesNode};
					let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0].clone());
					Box::pin(async move {
						let curve = ClonedNode::new(curve.eval(()).await);

						let generate_curves_node = GenerateCurvesNode::new(curve, ClonedNode::new(0_f32));
						let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_curves_node.eval(())));
						let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
						let any: DynAnyNode<ImageFrame<Luma>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
						any.into_type_erased()
					})
				},
				NodeIOTypes::new(concrete!(ImageFrame<Luma>), concrete!(ImageFrame<Luma>), vec![fn_type!(graphene_core::raster::curve::Curve)]),
			),
			// TODO: Use channel split and merge for this instead of using LuminanceMut for the whole color.
			(
				ProtoNodeIdentifier::new("graphene_core::raster::CurvesNode<_>"),
				|args| {
					use graphene_core::raster::{curve::Curve, GenerateCurvesNode};
					let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0].clone());
					Box::pin(async move {
						let curve = ClonedNode::new(curve.eval(()).await);

						let generate_curves_node = GenerateCurvesNode::new(curve, ClonedNode::new(0_f32));
						let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_curves_node.eval(())));
						let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
						let any: DynAnyNode<ImageFrame<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
						any.into_type_erased()
					})
				},
				NodeIOTypes::new(concrete!(ImageFrame<Color>), concrete!(ImageFrame<Color>), vec![fn_type!(graphene_core::raster::curve::Curve)]),
			),
		],
		vec![(
			ProtoNodeIdentifier::new("graphene_std::raster::ImaginateNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
			|args: Vec<graph_craft::proto::SharedNodeContainer>| {
				Box::pin(async move {
					use graphene_std::raster::ImaginateNode;
					macro_rules! instantiate_imaginate_node {
								($($i:expr,)*) => { ImaginateNode::new($(graphene_std::any::input_node(args[$i].clone()),)* ) };
							}
					let node: ImaginateNode<Color, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _> = instantiate_imaginate_node!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,);
					let any = graphene_std::any::DynAnyNode::new(node);
					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(ImageFrame<Color>),
				concrete!(ImageFrame<Color>),
				vec![
					fn_type!(&WasmEditorApi),
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
					fn_type!(bool),
					fn_type!(f64),
					fn_type!(ImaginateMaskStartingFill),
					fn_type!(bool),
					fn_type!(bool),
					fn_type!(u64),
				],
			),
		)],
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [Image<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [VectorData]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [ImageFrame<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [Vec<DVec2>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [Arc<WasmSurfaceHandle>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [WindowHandle]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [ShaderInputFrame]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [wgpu_executor::WgpuSurface]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [Option<wgpu_executor::WgpuSurface>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [wgpu_executor::WindowHandle]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: (), params: [RenderOutput]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => Image<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => VectorData]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => ImageFrame<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => Vec<DVec2>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => Arc<WasmSurfaceHandle>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => WindowHandle]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => ShaderInputFrame]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => wgpu_executor::WgpuSurface]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => Option<wgpu_executor::WgpuSurface>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => wgpu_executor::WindowHandle]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: UVec2, fn_params: [UVec2 => graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Footprint, fn_params: [Footprint => RenderOutput]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => GraphicElement]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => GraphicGroup]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => VectorData]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => ShaderInputFrame]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => WgpuSurface]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => Option<WgpuSurface>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Footprint, fn_params: [Footprint => TextureFrame]),
		register_node!(graphene_core::structural::ConsNode<_, _>, input: Image<Color>, params: [&str]),
		register_node!(graphene_std::raster::ImageFrameNode<_, _>, input: Image<Color>, params: [DAffine2]),
		register_node!(graphene_std::vector::BooleanOperationNode<_>, input: GraphicGroup, fn_params: [() => graphene_core::vector::misc::BooleanOperation]),
	];
	let mut map: HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();
	for (id, entry) in graphene_core::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for (constructor, types) in entry.iter() {
			map.entry(id.clone().into()).or_default().insert(types.clone(), *constructor);
		}
	}
	for (id, c, types) in node_types.into_iter().flatten() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.name.replace('\n', " ");
		// Remove struct generics
		if let Some((path, _generics)) = new_name.split_once("<") {
			new_name = path.to_string();
		}
		let nid = ProtoNodeIdentifier { name: Cow::Owned(new_name) };
		map.entry(nid).or_default().insert(types.clone(), c);
	}
	map
}

pub static NODE_REGISTRY: Lazy<HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>> = Lazy::new(|| node_registry());

#[cfg(test)]
mod protograph_testing {
	// TODO: add tests testing the node registry
}
