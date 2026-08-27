use dyn_any::StaticType;
use glam::{DAffine2, DVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::application_io::resource::Resource;
use graph_craft::document::value::RenderOutput;
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_std::animation::RealTimeMode;
use graphene_std::any::DynAnyNode;
use graphene_std::brush::brush_stroke::BrushTrace;
use graphene_std::extract_xy::XY;
use graphene_std::gradient::Gradient;
use graphene_std::list::{AttributeValueDyn, Bundle, Item, List, ListDyn, NodeIdPath};
#[cfg(target_family = "wasm")]
use graphene_std::platform_application_io::canvas_utils::CanvasHandle;
#[cfg(feature = "gpu")]
use graphene_std::raster::GPU;
use graphene_std::raster::color::Color;
use graphene_std::raster::*;
use graphene_std::raster::{CPU, Raster};
use graphene_std::render_node::RenderIntermediate;
use graphene_std::text::{Font, TextAlign};
use graphene_std::text_nodes::StringCapitalization;
use graphene_std::transform::{Footprint, ReferencePoint, ScaleType};
use graphene_std::vector::misc::{
	ArcType, BooleanOperation, BoxCorners, CentroidType, ExtrudeJoiningAlgorithm, GridType, InterpolationDistribution, MergeByDistanceAlgorithm, PointSpacingType, RowsOrColumns, SpiralType,
};
use graphene_std::vector::style::{DashPattern, GradientSpreadMethod, GradientType, PaintOrder, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_std::vector::{QRCodeErrorCorrectionLevel, Vector, VectorModification};
use graphene_std::{Artboard, Context, Graphic, NodeIO, NodeIOTypes, ProtoNodeIdentifier, concrete, fn_type_fut, future};
use node_registry_macros::async_node;
use std::collections::HashMap;

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let mut node_types: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		// =============
		// MONITOR NODES
		// =============
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Vector>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Raster<GPU>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<Gradient>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Vector>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Raster<GPU>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Gradient>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DashPattern>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<BoxCorners>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<String>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<f32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Footprint>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<bool>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<u32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<u64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<String>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<NodeIdPath>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<f32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<u32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<u64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<DVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<bool>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<GradientType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => List<GradientSpreadMethod>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<AttributeValueDyn>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ListDyn]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<BrushTrace>]),
		// Context nullification
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<&PlatformEditorApi>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<RenderIntermediate>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<RenderOutput>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<AttributeValueDyn>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => ListDyn, Context => Item<graphene_std::ContextFeatures>]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<CanvasHandle>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<&PlatformEditorApi>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<&wgpu_executor::WgpuExecutor>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<Option<&wgpu_executor::WgpuExecutor>>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<wgpu_executor::WgpuPipelineCache>, Context => Item<graphene_std::ContextFeatures>]),
		// ==========
		// MEMO NODES
		// ==========
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Artboard>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Graphic>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Vector>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Raster<CPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Color>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Gradient>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<String>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<NodeIdPath>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<DVec2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<bool>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<DAffine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<BlendMode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<GradientType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<GradientSpreadMethod>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Artboard>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Graphic>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Vector>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Raster<GPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Color>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Gradient>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DashPattern>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<BoxCorners>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<String>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<f32>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DAffine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Footprint>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DVec2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<bool>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<u32>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<u64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<BlendMode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<AttributeValueDyn>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => ListDyn]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<CanvasHandle>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<RenderOutput>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<&PlatformEditorApi>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => List<Raster<GPU>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<BrushTrace>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<RenderIntermediate>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<&wgpu_executor::WgpuExecutor>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Option<&wgpu_executor::WgpuExecutor>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<wgpu_executor::WgpuPipelineCache>]),
	];
	// The per-connector input adapter, registered per element type: an `Item` or `List` wire passes through unchanged.
	// The `name` arm registers an `Into`-based whole-wire shift under the given identifier, serving the `ListDyn` erasure rows.
	macro_rules! input_adapter_node {
		(element: $element:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				input_adapter_node!(passthrough: Item<$element>, element: $element),
				input_adapter_node!(passthrough: List<$element>, element: $element),
			];
			entries
		}};
		(passthrough: $ty:ty, element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["input_adapter<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_core::ops::PassthroughNode::new(graphene_std::any::downcast_node::<Context, $ty>(args.pop().unwrap()));
						let any: DynAnyNode<Context, $ty, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::PassthroughNode::new(graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $ty> + Send>>>::new());
					let params = vec![fn_type_fut!(Context, $ty)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
		(name: $name:literal, from: $from:ty, to: $to:ty, element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat![$name, "<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_core::ops::IntoNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::IntoNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
					);
					let params = vec![fn_type_fut!(Context, $from)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A conversion row registered under the same `input_adapter<$element>` identifier so a convertible-but-not-identical
	// ranked wire can feed an `Item<$element>` connector, converting each element via `Into`
	macro_rules! input_adapter_row {
		(from_element: $from:ty, element: $element:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				input_adapter_row!(node: IntoItemNode, from: Item<$from>, to: Item<$element>, element: $element),
				input_adapter_row!(node: IntoListNode, from: List<$from>, to: List<$element>, element: $element),
			];
			entries
		}};
		(node: $node:ident, from: $from:ty, to: $to:ty, element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["input_adapter<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_core::ops::$node::new(
							graphene_std::any::downcast_node::<Context, $from>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$element>)),
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::$node::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$element>)),
					);
					let params = vec![fn_type_fut!(Context, $from)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// The promotion adapter inserted by type resolution when an Item wire feeds a List connector: the singleton raise
	macro_rules! item_to_list_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ItemToListNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_core::ops::ItemToListNode::new(graphene_std::any::downcast_node::<Context, Item<$element>>(args.pop().unwrap()));
						let any: DynAnyNode<Context, List<$element>, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::ItemToListNode::new(graphene_std::any::PanicNode::<
						Context,
						core::pin::Pin<Box<dyn core::future::Future<Output = Item<$element>> + Send>>,
					>::new());
					let params = vec![fn_type_fut!(Context, Item<$element>)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// A whole-List bundle adapter inserted by type resolution when a List wire feeds an Item<Bundle<X>> connector
	macro_rules! bundle_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::BundleNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_core::ops::BundleNode::new(graphene_std::any::downcast_node::<Context, List<$element>>(args.pop().unwrap()));
						let any: DynAnyNode<Context, Item<Bundle<$element>>, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::BundleNode::new(graphene_std::any::PanicNode::<
						Context,
						core::pin::Pin<Box<dyn core::future::Future<Output = List<$element>> + Send>>,
					>::new());
					let params = vec![fn_type_fut!(Context, List<$element>)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// The reverse unbundle adapter inserted by type resolution when an Item<Bundle<X>> wire feeds a List connector
	macro_rules! unbundle_node {
		(element: $element:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::UnbundleNode<", stringify!($element), ">"]),
				|mut args| {
					Box::pin(async move {
						let node = graphene_core::ops::UnbundleNode::new(graphene_std::any::downcast_node::<Context, Item<Bundle<$element>>>(args.pop().unwrap()));
						let any: DynAnyNode<Context, List<$element>, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::UnbundleNode::new(graphene_std::any::PanicNode::<
						Context,
						core::pin::Pin<Box<dyn core::future::Future<Output = Item<Bundle<$element>>> + Send>>,
					>::new());
					let params = vec![fn_type_fut!(Context, Item<Bundle<$element>>)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}
	// ==================
	// RANK ADAPTER NODES
	// ==================
	// Registers the rank adapters (input_adapter, ItemToListNode) for each element type
	macro_rules! rank_adapter_nodes {
		($($element:ty),* $(,)?) => {{
			let mut entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = Vec::new();
			$(
				entries.extend(input_adapter_node!(element: $element));
				entries.push(item_to_list_node!(element: $element));
			)*
			entries
		}};
	}
	// The single list of value/enum element types that ride ranked wires as framable node parameters.
	// Each needs both a rank promotion adapter and a cache chain (memoize/monitor/context),
	// so both registrations below are driven from here by callback, which is what keeps a value type from ending up monitorable but not promotable, or vice versa.
	macro_rules! ranked_value_types {
		($register:ident) => {
			$register!(
				i32,
				i64,
				BlendMode,
				StrokeJoin,
				StrokeAlign,
				StrokeCap,
				PaintOrder,
				GradientType,
				GradientSpreadMethod,
				DashPattern,
				BoxCorners,
				MergeByDistanceAlgorithm,
				ExtrudeJoiningAlgorithm,
				PointSpacingType,
				StringCapitalization,
				LuminanceCalculation,
				RedGreenBlue,
				RedGreenBlueAlpha,
				RelativeAbsolute,
				SelectiveColorChoice,
				BrushTrace,
				XY,
				ScaleType,
				ReferencePoint,
				CentroidType,
				BooleanOperation,
				NoiseType,
				FractalType,
				CellularDistanceFunction,
				CellularReturnType,
				DomainWarpType,
				RealTimeMode,
				GridType,
				ArcType,
				SpiralType,
				TextAlign,
				QRCodeErrorCorrectionLevel,
				Font,
				InterpolationDistribution,
				RowsOrColumns,
				Resource,
			)
		};
	}
	// Primary element types (graphical data and scalars) get promotion adapters here; their monitor/memoize rows are the
	// explicit ones near the top of the registry, so they are deliberately absent from the shared value-type list above
	node_types.extend(rank_adapter_nodes!(
		Vector,
		Raster<CPU>,
		Graphic,
		Color,
		Gradient,
		String,
		f64,
		f32,
		u64,
		u32,
		DVec2,
		DAffine2,
		bool,
		NodeIdPath,
		Footprint,
		Artboard,
	));
	node_types.extend(ranked_value_types!(rank_adapter_nodes));
	// The nested-generic `Box<VectorModification>` registers directly rather than through the shared callback, since the
	// extra macro layer mangles the `stringify!` whitespace in its adapter identifier; its cache chain is likewise direct below
	node_types.extend(rank_adapter_nodes!(Box<VectorModification>));
	#[cfg(feature = "gpu")]
	node_types.extend(rank_adapter_nodes!(Raster<GPU>));
	// Type-erased rows for the `ListDyn` connectors (`Read Attribute`, `List Length`): any `List` wire erases its element type
	macro_rules! list_dyn_rows {
		($($element:ty),* $(,)?) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				$(input_adapter_node!(name: "input_adapter", from: List<$element>, to: ListDyn, element: ListDyn),)*
			];
			entries
		}};
	}
	node_types.push(input_adapter_node!(passthrough: ListDyn, element: ListDyn));
	node_types.extend(list_dyn_rows!(
		Artboard,
		Graphic,
		Vector,
		Raster<CPU>,
		Color,
		Gradient,
		f32,
		f64,
		u32,
		u64,
		bool,
		String,
		DVec2,
		DAffine2,
		BlendMode,
		GradientType,
		GradientSpreadMethod
	));
	#[cfg(feature = "gpu")]
	node_types.extend(list_dyn_rows!(Raster<GPU>));
	// Registers the whole-List bundle adapters for each value type whose entire list may be selected or carried as one opaque cell
	macro_rules! bundle_adapter_nodes {
		($($element:ty),* $(,)?) => {{
			let mut entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = Vec::new();
			$(
				entries.push(bundle_node!(element: $element));
				entries.push(unbundle_node!(element: $element));
			)*
			entries
		}};
	}
	node_types.extend(bundle_adapter_nodes!(
		bool,
		f32,
		f64,
		u32,
		u64,
		DVec2,
		DAffine2,
		Graphic,
		Raster<CPU>,
		Vector,
		String,
		Color,
		Gradient,
		Artboard,
	));
	#[cfg(feature = "gpu")]
	node_types.extend(bundle_adapter_nodes!(Raster<GPU>));
	// The memoize + monitor + context-nullification triple for each value type in the shared list above: the compiler wraps
	// a ranked connector's broadcast sibling in a Memoize + context-nullification pair, and test instrumentation monitors any wire.
	// Node identifiers are passed explicitly since `stringify!` would mangle the path's whitespace through this nested macro.
	macro_rules! cache_chain_nodes {
		(each: $value:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				async_node!(identifier: graphene_core::memo::memoize::IDENTIFIER, graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => $value]),
				async_node!(identifier: graphene_core::memo::monitor::IDENTIFIER, graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => $value]),
				async_node!(identifier: graphene_core::context_modification::context_modification::IDENTIFIER, graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => $value, Context => Item<graphene_std::ContextFeatures>]),
			];
			entries
		}};
		($($element:ty),* $(,)?) => {{
			let mut entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = Vec::new();
			$(
				entries.extend(cache_chain_nodes!(each: Item<$element>));
				entries.extend(cache_chain_nodes!(each: List<$element>));
			)*
			entries
		}};
	}
	node_types.extend(ranked_value_types!(cache_chain_nodes));
	// The direct counterpart to `Box<VectorModification>`'s promotion adapter above (see that note)
	node_types.extend(cache_chain_nodes!(Box<VectorModification>));
	// A position wire may feed a ranked vector connector, each position becoming a single-anchor vector
	node_types.extend(input_adapter_row!(from_element: DVec2, element: Vector));
	// A string wire may feed the ranked `Item<DashPattern>` dash connector by parsing each element into a dash pattern
	node_types.extend(input_adapter_row!(from_element: String, element: DashPattern));
	// A number wire may feed the ranked `Item<DashPattern>` dash connector, each number broadcasting element-wise as a one-length pattern
	node_types.extend(input_adapter_row!(from_element: f64, element: DashPattern));
	// A string wire may feed the ranked `Item<BoxCorners>` connector by parsing each element into a set of corner values
	node_types.extend(input_adapter_row!(from_element: String, element: BoxCorners));
	// A number wire may feed the ranked `Item<BoxCorners>` connector, each number becoming a uniform radius for all four corners
	node_types.extend(input_adapter_row!(from_element: f64, element: BoxCorners));
	// The `Convert`-based counterpart of `input_adapter_row!`, for casts the std `Into` trait cannot express
	macro_rules! convert_adapter_node {
		(from_element: $from:ty, element: $element:ty) => {{
			let entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				input_adapter_row!(node: ConvertItemNode, from: Item<$from>, to: Item<$element>, element: $element),
				input_adapter_row!(node: ConvertListNode, from: List<$from>, to: List<$element>, element: $element),
			];
			entries
		}};
	}
	macro_rules! convert_adapter_wildcard {
		(from: $from:ty, to: [$($to:ty),*]) => {{
			let mut entries: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = Vec::new();
			$(entries.extend(convert_adapter_node!(from_element: $from, element: $to));)*
			entries
		}};
	}
	// Numeric wires cast between numeric element types, splat to fill both axes of a `DVec2` connector, and format into a `String` connector
	node_types.extend(convert_adapter_wildcard!(from: f64, to: [f32, u32, u64, i32, i64, DVec2, String]));
	node_types.extend(convert_adapter_wildcard!(from: f32, to: [f64, u32, u64, i32, i64, DVec2, String]));
	node_types.extend(convert_adapter_wildcard!(from: u32, to: [f64, f32, u64, i32, i64, DVec2, String]));
	node_types.extend(convert_adapter_wildcard!(from: u64, to: [f64, f32, u32, i32, i64, DVec2, String]));
	node_types.extend(convert_adapter_wildcard!(from: i32, to: [f64, f32, u32, u64, i64, DVec2, String]));
	node_types.extend(convert_adapter_wildcard!(from: i64, to: [f64, f32, u32, u64, i32, DVec2, String]));
	// Bool, position, and transform wires may feed a ranked `String` connector by formatting each element as text
	node_types.extend(convert_adapter_node!(from_element: bool, element: String));
	node_types.extend(convert_adapter_node!(from_element: DVec2, element: String));
	node_types.extend(convert_adapter_node!(from_element: DAffine2, element: String));
	// The sanctioned attribute value conversions: an Item wire's elements box per cell, while a List wire boxes whole as one value
	macro_rules! attribute_value_node {
		(Item<$element:ty>) => {
			input_adapter_row!(node: ItemToAttributeValueNode, from: Item<$element>, to: Item<AttributeValueDyn>, element: AttributeValueDyn)
		};
		(List<$element:ty>) => {
			input_adapter_row!(node: ListToAttributeValueNode, from: List<$element>, to: Item<AttributeValueDyn>, element: AttributeValueDyn)
		};
	}
	let attribute_value_rows: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		attribute_value_node!(Item<f64>),
		attribute_value_node!(Item<u32>),
		attribute_value_node!(Item<u64>),
		attribute_value_node!(Item<bool>),
		attribute_value_node!(Item<String>),
		attribute_value_node!(Item<DVec2>),
		attribute_value_node!(Item<DAffine2>),
		attribute_value_node!(Item<Color>),
		attribute_value_node!(Item<BlendMode>),
		attribute_value_node!(Item<GradientType>),
		attribute_value_node!(Item<GradientSpreadMethod>),
		attribute_value_node!(Item<NodeIdPath>),
		attribute_value_node!(List<String>),
		attribute_value_node!(List<Color>),
		attribute_value_node!(List<Gradient>),
		attribute_value_node!(List<Vector>),
		attribute_value_node!(List<Raster<CPU>>),
		#[cfg(feature = "gpu")]
		attribute_value_node!(List<Raster<GPU>>),
		attribute_value_node!(List<Graphic>),
	];
	node_types.extend(attribute_value_rows);
	// Whole-List Transform companion rows: a rank-1 content wire composes the matrix onto every item
	macro_rules! transform_list_node {
		(element: $element:ty) => {
			async_node!(
				identifier: ProtoNodeIdentifier::new("transform_nodes::transform_nodes::TransformNode"),
				graphene_std::transform_nodes::TransformListNode<_, _, _, _, _>,
				input: Context,
				fn_params: [Context => List<$element>, Context => Item<DVec2>, Context => Item<f64>, Context => Item<DVec2>, Context => Item<DVec2>]
			)
		};
	}
	let transform_list_rows: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		transform_list_node!(element: Graphic),
		transform_list_node!(element: String),
		transform_list_node!(element: Vector),
		transform_list_node!(element: Raster<CPU>),
		#[cfg(feature = "gpu")]
		transform_list_node!(element: Raster<GPU>),
		transform_list_node!(element: Color),
		transform_list_node!(element: Gradient),
	];
	node_types.extend(transform_list_rows);
	let mut map: HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();

	// Rank normalization at this merge is the single convergence point for structurally-built and name-encoded `List` types,
	// covering the sources which cannot construct them structurally (reflected return values and opaque macro captures)
	for (id, entry) in graphene_std::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for (constructor, types) in entry.iter() {
			map.entry(id.clone()).or_default().insert(types.clone().normalize_rank(), *constructor);
		}
	}

	for (id, node_constructor, types) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.as_str().replace('\n', " ");

		// Remove struct generics for all nodes except the adapter identifiers, whose element suffix distinguishes their rows
		let element_suffixed_adapter = new_name.starts_with("input_adapter<")
			|| new_name.starts_with("graphene_core::ops::ItemToListNode<")
			|| new_name.starts_with("graphene_core::ops::BundleNode<")
			|| new_name.starts_with("graphene_core::ops::UnbundleNode<");
		if !element_suffixed_adapter && let Some((path, _generics)) = new_name.split_once("<") {
			new_name = path.to_string();
		}

		map.entry(ProtoNodeIdentifier::with_owned_string(new_name))
			.or_default()
			.insert(types.clone().normalize_rank(), node_constructor);
	}

	map
}

// TODO: Replace with `core::cell::LazyCell` (<https://doc.rust-lang.org/core/cell/struct.LazyCell.html>) or similar
pub static NODE_REGISTRY: once_cell::sync::Lazy<HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>> = once_cell::sync::Lazy::new(|| node_registry());

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
			async_node!(identifier: ProtoNodeIdentifier::new(stringify!($path)), $path, input: $input, fn_params: [$($arg => $type),*])
		};
		(identifier: $identifier:expr, $path:ty, input: $input:ty, fn_params: [$($arg:ty => $type:ty),*]) => {
			(
				$identifier,
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

	pub(crate) use async_node;
}
