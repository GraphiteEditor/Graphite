use dyn_any::StaticType;
use glam::{DVec2, UVec2};
use graph_craft::document::value::RenderOutput;
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_core::fn_type;
use graphene_core::raster::color::Color;
use graphene_core::raster::image::ImageFrameTable;
use graphene_core::raster::*;
use graphene_core::value::{ClonedNode, ValueNode};
use graphene_core::vector::VectorDataTable;
use graphene_core::{Artboard, GraphicGroupTable, concrete, generic};
use graphene_core::{Cow, ProtoNodeIdentifier, Type};
use graphene_core::{Node, NodeIO, NodeIOTypes};
use graphene_core::{fn_type_fut, future};
use graphene_std::Context;
use graphene_std::GraphicElement;
use graphene_std::any::{ComposeTypeErased, DowncastBothNode, DynAnyNode, FutureWrapperNode, IntoTypeErasedNode};
use graphene_std::application_io::ImageTexture;
use graphene_std::wasm_application_io::*;
use node_registry_macros::{async_node, into_node};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "gpu")]
use wgpu_executor::{ShaderInputFrame, WgpuExecutor};
use wgpu_executor::{WgpuSurface, WindowHandle};

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let node_types: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		into_node!(from: f64, to: f64),
		into_node!(from: ImageFrameTable<Color>, to: GraphicGroupTable),
		into_node!(from: f64, to: f64),
		into_node!(from: u32, to: f64),
		into_node!(from: u8, to: u32),
		into_node!(from: ImageFrameTable<Color>, to: GraphicGroupTable),
		into_node!(from: VectorDataTable, to: GraphicGroupTable),
		into_node!(from: VectorDataTable, to: GraphicElement),
		into_node!(from: ImageFrameTable<Color>, to: GraphicElement),
		into_node!(from: GraphicGroupTable, to: GraphicElement),
		into_node!(from: VectorDataTable, to: GraphicGroupTable),
		into_node!(from: ImageFrameTable<Color>, to: GraphicGroupTable),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ImageFrameTable<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ImageTexture]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => VectorDataTable]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => GraphicGroupTable]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => GraphicElement]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Artboard]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::RasterFrame]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::instances::Instances<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => glam::IVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => glam::DVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => bool]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => u32]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => u64]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ()]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::BooleanOperation]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Option<graphene_core::Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Fill]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::LineCap]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::LineJoin]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Stroke]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Gradient]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::GradientStops]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<graphene_core::uuid::NodeId>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::Color]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Box<graphene_core::vector::VectorModification>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Image<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => VectorDataTable]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => ImageFrameTable<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => GraphicGroupTable]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<DVec2>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Arc<WasmSurfaceHandle>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => WindowHandle]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Option<wgpu_executor::WgpuSurface>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WindowHandle]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: UVec2, fn_params: [UVec2 => graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => RenderOutput]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => GraphicElement]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => GraphicGroupTable]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => VectorDataTable]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => GraphicGroupTable]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => WgpuSurface]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Option<WgpuSurface>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => ImageTexture]),
		// Filters
		// TODO: Move these filters to the new node macro and put them in `graphene_core::raster::adjustments`, then add them to the document upgrade script which moves many of the adjustment nodes from `graphene_core::raster` to `graphene_core::raster::adjustments`
		(
			ProtoNodeIdentifier::new("graphene_core::raster::BrightnessContrastNode"),
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
						let any: DynAnyNode<ImageFrameTable<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
						any.into_type_erased()
					} else {
						let generate_brightness_contrast_mapper_node = GenerateBrightnessContrastMapperNode::new(brightness, contrast);
						let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_brightness_contrast_mapper_node.eval(())));
						let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
						let any: DynAnyNode<ImageFrameTable<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
						any.into_type_erased()
					}
				})
			},
			NodeIOTypes::new(concrete!(ImageFrameTable<Color>), concrete!(ImageFrameTable<Color>), vec![fn_type!(f64), fn_type!(f64), fn_type!(bool)]),
		),
		(
			ProtoNodeIdentifier::new("graphene_core::structural::ComposeNode"),
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
		),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => ShaderInputFrame]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuSurface]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => ShaderInputFrame]),
		#[cfg(feature = "gpu")]
		into_node!(from: &WasmEditorApi, to: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		(
			ProtoNodeIdentifier::new("graphene_std::executor::MapGpuSingleImageNode"),
			|args| {
				Box::pin(async move {
					let document_node: DowncastBothNode<(), graph_craft::document::DocumentNode> = DowncastBothNode::new(args[0].clone());
					let editor_api: DowncastBothNode<(), &WasmEditorApi> = DowncastBothNode::new(args[1].clone());
					let node = graphene_std::gpu_nodes::MapGpuNode::new(document_node, editor_api);
					let any: DynAnyNode<ImageFrameTable<Color>, _, _> = graphene_std::any::DynAnyNode::new(node);
					any.into_type_erased()
				})
			},
			NodeIOTypes::new(
				concrete!(ImageFrameTable<Color>),
				concrete!(ImageFrameTable<Color>),
				vec![fn_type!(graph_craft::document::DocumentNode), fn_type!(WasmEditorApi)],
			),
		),
		#[cfg(feature = "gpu")]
		(
			ProtoNodeIdentifier::new(stringify!(wgpu_executor::CreateGpuSurfaceNode<_>)),
			|args| {
				Box::pin(async move {
					let editor_api: DowncastBothNode<Context, &WasmEditorApi> = DowncastBothNode::new(args[0].clone());
					let node = <wgpu_executor::CreateGpuSurfaceNode<_>>::new(editor_api);
					let any: DynAnyNode<Context, _, _> = graphene_std::any::DynAnyNode::new(node);
					Box::new(any) as TypeErasedBox
				})
			},
			{
				let node = <wgpu_executor::CreateGpuSurfaceNode<_>>::new(graphene_std::any::PanicNode::<Context, dyn_any::DynFuture<'static, &WasmEditorApi>>::new());
				let params = vec![fn_type_fut!(Context, &WasmEditorApi)];
				let mut node_io = <wgpu_executor::CreateGpuSurfaceNode<_> as NodeIO<'_, Context>>::to_async_node_io(&node, params);
				node_io.call_argument = concrete!(<Context as StaticType>::Static);
				node_io
			},
		),
	];

	let mut map: HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();

	for (id, entry) in graphene_core::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for (constructor, types) in entry.iter() {
			map.entry(id.clone().into()).or_default().insert(types.clone(), *constructor);
		}
	}

	for (id, c, types) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.name.replace('\n', " ");
		// Remove struct generics for all nodes except for the IntoNode
		if !new_name.contains("IntoNode") {
			if let Some((path, _generics)) = new_name.split_once("<") {
				new_name = path.to_string();
			}
		}
		let nid = ProtoNodeIdentifier { name: Cow::Owned(new_name) };
		map.entry(nid).or_default().insert(types.clone(), c);
	}

	map
}

pub static NODE_REGISTRY: Lazy<HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>> = Lazy::new(|| node_registry());

mod node_registry_macros {
	macro_rules! async_node {
		// TODO: we currently need to annotate the type here because the compiler would otherwise (correctly)
		// TODO: assign a Pin<Box<dyn Future<Output=T>>> type to the node, which is not what we want for now.
		//
		// This `params` variant of the macro wraps the normal `fn_params` variant and is used as a shorthand for writing `T` instead of `() => T`
		($path:ty, input: $input:ty, params: [$($type:ty),*]) => {
			async_node!($path, input: $input, fn_params: [ $(() => $type),*])
		};
		($path:ty, input: $input:ty, fn_params: [$($arg:ty => $type:ty),*]) => {
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
					let params = vec![$(fn_type_fut!($arg, $type)),*];
					let mut node_io = NodeIO::<'_, $input>::to_async_node_io(&node, params);
					node_io.call_argument = concrete!(<$input as StaticType>::Static);
					node_io
				},
			)
		};
	}

	macro_rules! into_node {
		(from: $from:ty, to: $to:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::IntoNode<", stringify!($to), ">"]),
				|mut args| {
					Box::pin(async move {
						args.reverse();
						let node = graphene_core::ops::IntoNode::<$to>::new();
						let any: DynAnyNode<$from, _, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::IntoNode::<$to>::new();
					let mut node_io = NodeIO::<'_, $from>::to_async_node_io(&node, vec![]);
					node_io.call_argument = future!(<$from as StaticType>::Static);
					node_io
				},
			)
		};
	}

	pub(crate) use async_node;
	pub(crate) use into_node;
}
