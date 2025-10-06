use dyn_any::StaticType;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::document::DocumentNode;
use graph_craft::document::value::RenderOutput;
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_core::raster::color::Color;
use graphene_core::raster::*;
#[cfg(feature = "gpu")]
use graphene_core::raster_types::GPU;
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::{Artboard, concrete};
use graphene_core::{Cow, ProtoNodeIdentifier};
use graphene_core::{NodeIO, NodeIOTypes};
use graphene_core::{fn_type_fut, future};
use graphene_std::Context;
use graphene_std::Graphic;
#[cfg(feature = "gpu")]
use graphene_std::any::DowncastBothNode;
use graphene_std::any::DynAnyNode;
use graphene_std::application_io::{ImageTexture, SurfaceFrame};
use graphene_std::brush::brush_cache::BrushCache;
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::gradient::GradientStops;
use graphene_std::render_node::RenderIntermediate;
use graphene_std::table::Table;
use graphene_std::transform::Footprint;
use graphene_std::uuid::NodeId;
use graphene_std::vector::Vector;
use graphene_std::wasm_application_io::WasmEditorApi;
#[cfg(feature = "gpu")]
use graphene_std::wasm_application_io::WasmSurfaceHandle;
use node_registry_macros::{async_node, convert_node, into_node};
use once_cell::sync::Lazy;
use std::collections::HashMap;
#[cfg(feature = "gpu")]
use std::sync::Arc;
#[cfg(feature = "gpu")]
use wgpu_executor::WgpuExecutor;
use wgpu_executor::{WgpuSurface, WindowHandle};

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let mut node_types: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		// ==========
		// INTO NODES
		// ==========
		into_node!(from: Table<Graphic>, to: Table<Graphic>),
		into_node!(from: Table<Vector>, to: Table<Vector>),
		into_node!(from: Table<Raster<CPU>>, to: Table<Raster<CPU>>),
		#[cfg(feature = "gpu")]
		into_node!(from: Table<Raster<GPU>>, to: Table<Raster<GPU>>),
		into_node!(from: Table<Vector>, to: Table<Graphic>),
		into_node!(from: Table<Raster<CPU>>, to: Table<Graphic>),
		#[cfg(feature = "gpu")]
		into_node!(from: Table<Raster<GPU>>, to: Table<Graphic>),
		// into_node!(from: Table<Raster<CPU>>, to: Table<Raster<SRGBA8>>),
		#[cfg(feature = "gpu")]
		into_node!(from: &WasmEditorApi, to: &WgpuExecutor),
		convert_node!(from: String, to: String),
		convert_node!(from: bool, to: String),
		convert_node!(from: DVec2, to: String),
		convert_node!(from: IVec2, to: String),
		convert_node!(from: DAffine2, to: String),
		#[cfg(feature = "gpu")]
		convert_node!(from: Table<Raster<CPU>>, to: Table<Raster<CPU>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: Table<Raster<CPU>>, to: Table<Raster<GPU>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: Table<Raster<GPU>>, to: Table<Raster<GPU>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: Table<Raster<GPU>>, to: Table<Raster<CPU>>, converter: &WgpuExecutor),
		// =============
		// MONITOR NODES
		// =============
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ()]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<Artboard>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<Vector>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<Raster<GPU>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<Color>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Table<GradientStops>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => IVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => DVec2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => DAffine2]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => bool]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => u32]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => u64]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => ImageTexture]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_path_bool::BooleanOperation]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Fill]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::StrokeCap]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::StrokeJoin]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::PaintOrder]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::StrokeAlign]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Stroke]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Gradient]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => GradientStops]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<graphene_core::uuid::NodeId>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Box<graphene_core::vector::VectorModification>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Option<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<DVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => [f64; 4]]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<NodeId>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Graphic]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::text::Font]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Vec<BrushStroke>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => BrushCache]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => DocumentNode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::curve::Curve]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::transform::Footprint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::blending::BlendMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::LuminanceCalculation]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::extract_xy::XY]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RedGreenBlue]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RedGreenBlueAlpha]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::animation::RealTimeMode]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::NoiseType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::FractalType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::CellularDistanceFunction]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::CellularReturnType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::DomainWarpType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::RelativeAbsolute]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_std::raster::adjustments::SelectiveColorChoice]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::GridType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::ArcType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::MergeByDistanceAlgorithm]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::FillType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::style::GradientType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => graphene_core::text::TextAlign]),
		// Context nullification
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => &WasmEditorApi, Context => graphene_std::ContextFeatures]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Arc<WasmSurfaceHandle>, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => RenderIntermediate, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => WgpuSurface, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Option<WgpuSurface>, Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => WindowHandle, Context => graphene_std::ContextFeatures]),
		// ==========
		// MEMO NODES
		// ==========
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => ()]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => bool]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<Artboard>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<Graphic>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<Vector>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<Raster<CPU>>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Image<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<GradientStops>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => GradientStops]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<DVec2>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<NodeId>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<f64>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<f32>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Arc<WasmSurfaceHandle>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => WindowHandle]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Option<WgpuSurface>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => SurfaceFrame]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => f64]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => f32]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => u32]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => u64]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => DVec2]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => String]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => DAffine2]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Footprint]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => RenderOutput]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => &WasmEditorApi]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => WgpuSurface]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Table<Raster<GPU>>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Option<f64>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Color]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Option<Color>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => [f64; 4]]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Graphic]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => glam::f32::Vec2]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => glam::f32::Affine2]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Stroke]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Gradient]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::text::Font]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Vec<BrushStroke>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => BrushCache]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => DocumentNode]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::ContextFeatures]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::curve::Curve]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::transform::Footprint]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => Box<graphene_core::vector::VectorModification>]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::Fill]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::blending::BlendMode]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::LuminanceCalculation]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::extract_xy::XY]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::RedGreenBlue]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::RedGreenBlueAlpha]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::animation::RealTimeMode]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::NoiseType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::FractalType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::CellularDistanceFunction]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::CellularReturnType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::DomainWarpType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::RelativeAbsolute]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_std::raster::SelectiveColorChoice]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::GridType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::ArcType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::MergeByDistanceAlgorithm]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::PointSpacingType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::StrokeCap]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::StrokeJoin]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::StrokeAlign]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::PaintOrder]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::FillType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::style::GradientType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::transform::ReferencePoint]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::vector::misc::CentroidType]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_path_bool::BooleanOperation]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => graphene_core::text::TextAlign]),
		async_node!(graphene_core::memo::MemoNode<_, _>, input: Context, fn_params: [Context => RenderIntermediate]),
		// =================
		// IMPURE MEMO NODES
		// =================
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<Artboard>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<Graphic>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<Vector>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<Raster<CPU>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<Raster<GPU>>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<Color>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Table<GradientStops>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => WgpuSurface]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => Option<WgpuSurface>]),
		async_node!(graphene_core::memo::ImpureMemoNode<_, _, _>, input: Context, fn_params: [Context => ImageTexture]),
		// =======================
		// CREATE GPU SURFACE NODE
		// =======================
		#[cfg(feature = "gpu")]
		(
			ProtoNodeIdentifier::new(stringify!(wgpu_executor::CreateGpuSurfaceNode<_>)),
			|args| {
				Box::pin(async move {
					let editor_api: DowncastBothNode<Context, &WasmEditorApi> = DowncastBothNode::new(args[0].clone());
					let node = <wgpu_executor::CreateGpuSurfaceNode<_>>::new(editor_api);
					let any: DynAnyNode<Context, _, _> = DynAnyNode::new(node);
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
	// =============
	// CONVERT NODES
	// =============
	node_types.extend(
		[
			convert_node!(from: f32, to: numbers),
			convert_node!(from: f64, to: numbers),
			convert_node!(from: i8, to: numbers),
			convert_node!(from: u8, to: numbers),
			convert_node!(from: u16, to: numbers),
			convert_node!(from: i16, to: numbers),
			convert_node!(from: i32, to: numbers),
			convert_node!(from: u32, to: numbers),
			convert_node!(from: i64, to: numbers),
			convert_node!(from: u64, to: numbers),
			convert_node!(from: i128, to: numbers),
			convert_node!(from: u128, to: numbers),
			convert_node!(from: isize, to: numbers),
			convert_node!(from: usize, to: numbers),
		]
		.into_iter()
		.flatten(),
	);

	let mut map: HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();

	for (id, entry) in graphene_core::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for (constructor, types) in entry.iter() {
			map.entry(id.clone()).or_default().insert(types.clone(), *constructor);
		}
	}

	for (id, c, types) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.name.replace('\n', " ");

		// Remove struct generics for all nodes except for the IntoNode and ConvertNode
		if !(new_name.contains("IntoNode") || new_name.contains("ConvertNode")) {
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
	macro_rules! convert_node {
		(from: $from:ty, to: numbers) => {{
			let x: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				convert_node!(from: $from, to: f32),
				convert_node!(from: $from, to: f64),
				convert_node!(from: $from, to: i8),
				convert_node!(from: $from, to: u8),
				convert_node!(from: $from, to: u16),
				convert_node!(from: $from, to: i16),
				convert_node!(from: $from, to: i32),
				convert_node!(from: $from, to: u32),
				convert_node!(from: $from, to: i64),
				convert_node!(from: $from, to: u64),
				convert_node!(from: $from, to: i128),
				convert_node!(from: $from, to: u128),
				convert_node!(from: $from, to: isize),
				convert_node!(from: $from, to: usize),
				convert_node!(from: $from, to: String),
			];
			x
		}};
		(from: $from:ty, to: $to:ty) => {
			convert_node!(from: $from, to: $to, converter: ())
		};
		(from: $from:ty, to: $to:ty, converter: $convert:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				|mut args| {
					Box::pin(async move {
						let mut args = args.drain(..);
						let node = graphene_core::ops::ConvertNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.next().expect("Convert node did not get first argument")),
							graphene_std::any::downcast_node::<Context, $convert>(args.next().expect("Convert node did not get converter argument")),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>))
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_core::ops::ConvertNode::new(
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $from> + Send>>>::new(),
						graphene_std::any::PanicNode::<Context, core::pin::Pin<Box<dyn core::future::Future<Output = $convert> + Send>>>::new(),
						graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>))
					);
					let params = vec![fn_type_fut!(Context, $from), fn_type_fut!(Context, $convert)];
					let node_io = NodeIO::<'_, Context>::to_async_node_io(&node, params);
					node_io
				},
			)
		};
	}

	pub(crate) use async_node;
	pub(crate) use convert_node;
	pub(crate) use into_node;
}
