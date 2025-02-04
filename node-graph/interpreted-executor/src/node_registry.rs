use dyn_any::{DynFuture, StaticType};
use graph_craft::document::value::RenderOutput;
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_core::fn_type;
use graphene_core::ops::IdentityNode;
use graphene_core::raster::color::Color;
use graphene_core::raster::image::ImageFrameTable;
use graphene_core::raster::*;
use graphene_core::value::{ClonedNode, ValueNode};
use graphene_core::vector::VectorDataTable;
use graphene_core::{concrete, generic, Artboard, GraphicGroupTable};
use graphene_core::{fn_type_fut, future};
use graphene_core::{Cow, ProtoNodeIdentifier, Type};
use graphene_core::{Node, NodeIO, NodeIOTypes};
use graphene_std::any::{ComposeTypeErased, DowncastBothNode, DynAnyNode, FutureWrapperNode, IntoTypeErasedNode};
use graphene_std::application_io::TextureFrame;
use graphene_std::wasm_application_io::*;
use graphene_std::Context;
use graphene_std::{GraphicElement, GraphicGroup};
#[cfg(feature = "gpu")]
use wgpu_executor::{ShaderInputFrame, WgpuExecutor};
use wgpu_executor::{WgpuSurface, WindowHandle};

use glam::{DVec2, UVec2};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

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
				// TODO: Propagate the future type through the node graph
				// let params = vec![$(Type::Fn(Box::new(concrete!(())), Box::new(Type::Future(Box::new(concrete!($type)))))),*];
				let params = vec![$(fn_type_fut!($arg, $type)),*];
				let mut node_io = NodeIO::<'_, $input>::to_async_node_io(&node, params);
				node_io.call_argument = concrete!(<$input as StaticType>::Static);
				node_io
			},
		)
	};
}

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let node_types: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		// (
		// 	ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode"),
		// 	|_| Box::pin(async move { FutureWrapperNode::new(IdentityNode::new()).into_type_erased() }),
		// 	NodeIOTypes::new(generic!(I), generic!(I), vec![]),
		// ),
		// async_node!(graphene_core::ops::IntoNode<ImageFrameTable<SRGBA8>>, input: ImageFrameTable<Color>, params: []),
		// async_node!(graphene_core::ops::IntoNode<ImageFrameTable<Color>>, input: ImageFrameTable<SRGBA8>, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroupTable>, input: ImageFrameTable<Color>, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroupTable>, input: VectorDataTable, params: []),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::ops::IntoNode<&WgpuExecutor>, input: &WasmEditorApi, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicElement>, input: VectorDataTable, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicElement>, input: ImageFrameTable<Color>, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicElement>, input: GraphicGroupTable, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroupTable>, input: VectorDataTable, params: []),
		async_node!(graphene_core::ops::IntoNode<GraphicGroupTable>, input: ImageFrameTable<Color>, params: []),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ImageFrameTable<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => TextureFrame]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => VectorDataTable]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => GraphicGroupTable]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => GraphicElement]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Artboard]),
		#[cfg(feature = "gpu")]
		(
			ProtoNodeIdentifier::new(stringify!(wgpu_executor::CreateGpuSurfaceNode<_>)),
			|args| {
				Box::pin(async move {
					let editor_api: DowncastBothNode<(), &WasmEditorApi> = DowncastBothNode::new(args[0].clone());
					let node = <wgpu_executor::CreateGpuSurfaceNode<_>>::new(editor_api);
					let any: DynAnyNode<(), _, _> = graphene_std::any::DynAnyNode::new(node);
					Box::new(any) as TypeErasedBox
				})
			},
			{
				let node = <wgpu_executor::CreateGpuSurfaceNode<_>>::new(graphene_std::any::PanicNode::<Context, DynFuture<'static, &WasmEditorApi>>::new());
				let params = vec![fn_type_fut!(Context, &WasmEditorApi)];
				let mut node_io = <wgpu_executor::CreateGpuSurfaceNode<_> as NodeIO<'_, Context>>::to_async_node_io(&node, params);
				node_io.call_argument = concrete!(<Context as StaticType>::Static);
				node_io
			},
		),
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
		// (
		// 	ProtoNodeIdentifier::new("graphene_core::raster::CurvesNode"),
		// 	|args| {
		// 		use graphene_core::raster::{curve::Curve, GenerateCurvesNode};
		// 		let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0].clone());
		// 		Box::pin(async move {
		// 			let curve = ClonedNode::new(curve.eval(()).await);

		// 			let generate_curves_node = GenerateCurvesNode::new(curve, ClonedNode::new(0_f32));
		// 			let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_curves_node.eval(())));
		// 			let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
		// 			let any: DynAnyNode<ImageFrameTable<Luma>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
		// 			any.into_type_erased()
		// 		})
		// 	},
		// 	NodeIOTypes::new(concrete!(ImageFrameTable<Luma>), concrete!(ImageFrameTable<Luma>), vec![fn_type!(graphene_core::raster::curve::Curve)]),
		// ),
		// TODO: Use channel split and merge for this instead of using LuminanceMut for the whole color.
		// (
		// 	ProtoNodeIdentifier::new("graphene_core::raster::CurvesNode"),
		// 	|args| {
		// 		use graphene_core::raster::{curve::Curve, GenerateCurvesNode};
		// 		let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0].clone());
		// 		Box::pin(async move {
		// 			let curve = ValueNode::new(ClonedNode::new(curve.eval(()).await));

		// 			let generate_curves_node = GenerateCurvesNode::new(FutureWrapperNode::new(curve), FutureWrapperNode::new(ClonedNode::new(0_f32)));
		// 			let map_image_frame_node = graphene_std::raster::MapImageNode::new(FutureWrapperNode::new(ValueNode::new(generate_curves_node.eval(()))));
		// 			let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
		// 			let any: DynAnyNode<ImageFrameTable<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
		// 			any.into_type_erased()
		// 		})
		// 	},
		// 	NodeIOTypes::new(
		// 		concrete!(ImageFrameTable<Color>),
		// 		concrete!(ImageFrameTable<Color>),
		// 		vec![fn_type!(graphene_core::raster::curve::Curve)],
		// 	),
		// ),
		// (
		// 	ProtoNodeIdentifier::new("graphene_std::raster::ImaginateNode"),
		// 	|args: Vec<graph_craft::proto::SharedNodeContainer>| {
		// 		Box::pin(async move {
		// 			use graphene_std::raster::ImaginateNode;
		// 			macro_rules! instantiate_imaginate_node {
		// 						($($i:expr,)*) => { ImaginateNode::new($(graphene_std::any::input_node(args[$i].clone()),)* ) };
		// 					}
		// 			let node: ImaginateNode<Color, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _> = instantiate_imaginate_node!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,);
		// 			let any = graphene_std::any::DynAnyNode::new(node);
		// 			any.into_type_erased()
		// 		})
		// 	},
		// 	NodeIOTypes::new(
		// 		concrete!(ImageFrameTable<Color>),
		// 		concrete!(ImageFrameTable<Color>),
		// 		vec![
		// 			fn_type!(&WasmEditorApi),
		// 			fn_type!(ImaginateController),
		// 			fn_type!(f64),
		// 			fn_type!(Option<DVec2>),
		// 			fn_type!(u32),
		// 			fn_type!(ImaginateSamplingMethod),
		// 			fn_type!(f64),
		// 			fn_type!(String),
		// 			fn_type!(String),
		// 			fn_type!(bool),
		// 			fn_type!(f64),
		// 			fn_type!(bool),
		// 			fn_type!(f64),
		// 			fn_type!(ImaginateMaskStartingFill),
		// 			fn_type!(bool),
		// 			fn_type!(bool),
		// 			fn_type!(u64),
		// 		],
		// 	),
		// ),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Image<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => VectorDataTable]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => ImageFrameTable<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<DVec2>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Arc<WasmSurfaceHandle>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => WindowHandle]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => ShaderInputFrame]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WgpuSurface]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Option<wgpu_executor::WgpuSurface>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => wgpu_executor::WindowHandle]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: UVec2, fn_params: [UVec2 => graphene_std::SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => RenderOutput]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => GraphicElement]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => GraphicGroup]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => VectorDataTable]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => ShaderInputFrame]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => WgpuSurface]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Option<WgpuSurface>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => TextureFrame]),
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
