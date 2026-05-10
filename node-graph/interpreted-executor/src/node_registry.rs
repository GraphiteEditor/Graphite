use dyn_any::StaticType;
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::DocumentNode;
use graph_craft::document::value::RenderOutput;
use graph_craft::proto::{NodeConstructor, TypeErasedBox};
use graphene_std::any::DynAnyNode;
use graphene_std::application_io::ImageTexture;
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::gradient::GradientStops;
use graphene_std::list::{AttributeDyn, AttributeValueDyn, Item, List, ListDyn};
#[cfg(target_family = "wasm")]
use graphene_std::platform_application_io::canvas_utils::CanvasHandle;
#[cfg(feature = "gpu")]
use graphene_std::raster::GPU;
use graphene_std::raster::color::Color;
use graphene_std::raster::*;
use graphene_std::raster::{CPU, Raster};
use graphene_std::render_node::RenderIntermediate;
use graphene_std::transform::Footprint;
use graphene_std::uuid::NodeId;
use graphene_std::vector::Vector;
use graphene_std::{Artboard, Context, Graphic, NodeIO, NodeIOTypes, ProtoNodeIdentifier, concrete, fn_type_fut, future};
use node_registry_macros::{async_node, convert_node, into_node};
use std::collections::HashMap;
#[cfg(feature = "gpu")]
use wgpu_executor::WgpuExecutor;

// TODO: turn into hashmap
fn node_registry() -> HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> {
	let mut node_types: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
		// ==========
		// INTO NODES
		// ==========
		into_node!(from: Item<List<Graphic>>, to: Item<List<Graphic>>),
		into_node!(from: Item<List<Vector>>, to: Item<List<Vector>>),
		into_node!(from: Item<List<Raster<CPU>>>, to: Item<List<Raster<CPU>>>),
		#[cfg(feature = "gpu")]
		into_node!(from: Item<List<Raster<GPU>>>, to: Item<List<Raster<GPU>>>),
		convert_node!(from: Item<List<Vector>>, to: Item<List<Graphic>>),
		convert_node!(from: Item<List<Raster<CPU>>>, to: Item<List<Graphic>>),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<List<Raster<GPU>>>, to: Item<List<Graphic>>),
		// Type-erased attribute conversions for the `Attach Attribute` node, so it monomorphizes only over the destination `List` type.
		convert_node!(from: Item<List<Artboard>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<Graphic>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<Vector>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<Raster<CPU>>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<Color>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<GradientStops>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<f64>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<bool>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<String>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<DAffine2>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<BlendMode>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<graphene_std::vector::style::GradientType>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<graphene_std::vector::style::GradientSpreadMethod>>, to: Item<AttributeDyn>),
		convert_node!(from: Item<List<Artboard>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<Graphic>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<Vector>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<Raster<CPU>>>, to: Item<ListDyn>),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<List<Raster<GPU>>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<Color>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<GradientStops>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<f64>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<bool>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<String>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<u8>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<NodeId>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<DAffine2>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<BlendMode>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<graphene_std::vector::style::GradientType>>, to: Item<ListDyn>),
		convert_node!(from: Item<List<graphene_std::vector::style::GradientSpreadMethod>>, to: Item<ListDyn>),
		// Type-erased attribute value conversions for the `Write Attribute` node, so it monomorphizes only over the destination `List` type.
		convert_node!(from: Item<f64>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<u32>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<u64>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<bool>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<String>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<DVec2>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<DAffine2>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<Color>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<BlendMode>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<graphene_std::vector::style::GradientType>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<graphene_std::vector::style::GradientSpreadMethod>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<List<String>>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<List<NodeId>>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<List<Color>>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<List<GradientStops>>, to: Item<AttributeValueDyn>),
		convert_node!(from: Item<List<Graphic>>, to: Item<AttributeValueDyn>),
		// into_node!(from: List<Raster<CPU>>, to: List<Raster<SRGBA8>>),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<&PlatformEditorApi>, to: Item<&WgpuExecutor>),
		convert_node!(from: Item<DVec2>, to: Item<DVec2>),
		convert_node!(from: Item<String>, to: Item<String>),
		convert_node!(from: Item<bool>, to: Item<String>),
		convert_node!(from: Item<DVec2>, to: Item<String>),
		convert_node!(from: Item<IVec2>, to: Item<String>),
		convert_node!(from: Item<DAffine2>, to: Item<String>),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<List<Raster<CPU>>>, to: Item<List<Raster<CPU>>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<List<Raster<CPU>>>, to: Item<List<Raster<GPU>>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<List<Raster<GPU>>>, to: Item<List<Raster<GPU>>>, converter: &WgpuExecutor),
		#[cfg(feature = "gpu")]
		convert_node!(from: Item<List<Raster<GPU>>>, to: Item<List<Raster<CPU>>>, converter: &WgpuExecutor),
		// =============
		// MONITOR NODES
		// =============
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<()>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<Artboard>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<Graphic>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<Vector>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<Raster<CPU>>>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<Raster<GPU>>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<Color>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<GradientStops>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Image<Color>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<String>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<IVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DVec2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DAffine2>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<bool>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<f64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<u32>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<u64>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<ImageTexture>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::ReferencePoint>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::BooleanOperation>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::Fill>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::StrokeCap>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::StrokeJoin>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::PaintOrder>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::StrokeAlign>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::Stroke>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::Gradient>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Box<graphene_std::vector::VectorModification>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::CentroidType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::PointSpacingType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Option<f64>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<String>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<NodeId>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<f64>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<u8>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<bool>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<DAffine2>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<BlendMode>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<graphene_std::vector::style::GradientType>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<graphene_std::vector::style::GradientSpreadMethod>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<AttributeDyn>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<AttributeValueDyn>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<ListDyn>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<Graphic>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::text::Font>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<List<BrushStroke>>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<DocumentNode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::Footprint>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::blending::BlendMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::LuminanceCalculation>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::extract_xy::XY>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::text_nodes::StringCapitalization>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::RedGreenBlue>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::RedGreenBlueAlpha>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::animation::RealTimeMode>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::NoiseType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::FractalType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::CellularDistanceFunction>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::CellularReturnType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::DomainWarpType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::RelativeAbsolute>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::adjustments::SelectiveColorChoice>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::GridType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::ArcType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::RowsOrColumns>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::MergeByDistanceAlgorithm>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::ExtrudeJoiningAlgorithm>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::PointSpacingType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::GradientType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::ReferencePoint>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::CentroidType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::text::TextAlign>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::ScaleType>]),
		async_node!(graphene_core::memo::MonitorNode<_, _, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::InterpolationDistribution>]),
		// Context nullification
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<&PlatformEditorApi>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<RenderIntermediate>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<RenderOutput>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<AttributeDyn>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<AttributeValueDyn>, Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<ListDyn>, Context => Item<graphene_std::ContextFeatures>]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::context_modification::ContextModificationNode<_, _>, input: Context, fn_params: [Context => Item<CanvasHandle>, Context => Item<graphene_std::ContextFeatures>]),
		// ==========
		// MEMO NODES
		// ==========
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<()>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<bool>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<Artboard>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<Graphic>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<Vector>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<Raster<CPU>>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<Color>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Image<Color>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<GradientStops>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<String>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<NodeId>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<f64>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<u8>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<bool>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<DAffine2>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<BlendMode>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<graphene_std::vector::style::GradientType>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<graphene_std::vector::style::GradientSpreadMethod>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<AttributeDyn>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<AttributeValueDyn>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<ListDyn>]),
		#[cfg(target_family = "wasm")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<CanvasHandle>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<f64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<f32>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<u32>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<u64>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DVec2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<String>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DAffine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Footprint>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<RenderOutput>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<&PlatformEditorApi>]),
		#[cfg(feature = "gpu")]
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<Raster<GPU>>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Option<f64>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Option<Color>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Graphic>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<glam::f32::Vec2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<glam::f32::Affine2>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::Stroke>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::Gradient>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::text::Font>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<List<BrushStroke>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<DocumentNode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::ContextFeatures>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::Footprint>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<Box<graphene_std::vector::VectorModification>>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::Fill>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::blending::BlendMode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::LuminanceCalculation>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::QRCodeErrorCorrectionLevel>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::extract_xy::XY>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::text_nodes::StringCapitalization>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::RedGreenBlue>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::RedGreenBlueAlpha>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::animation::RealTimeMode>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::NoiseType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::FractalType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::CellularDistanceFunction>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::CellularReturnType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::DomainWarpType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::RelativeAbsolute>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::raster::SelectiveColorChoice>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::GridType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::ArcType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::RowsOrColumns>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::MergeByDistanceAlgorithm>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::ExtrudeJoiningAlgorithm>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::PointSpacingType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::StrokeCap>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::StrokeJoin>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::StrokeAlign>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::PaintOrder>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::style::GradientType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::ReferencePoint>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::CentroidType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::BooleanOperation>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::text::TextAlign>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::transform::ScaleType>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<graphene_std::vector::misc::InterpolationDistribution>]),
		async_node!(graphene_core::memo::MemoizeNode<_, _>, input: Context, fn_params: [Context => Item<RenderIntermediate>]),
	];
	// =============
	// CONVERT NODES
	// =============
	node_types.extend(
		[
			convert_node!(from: Item<f32>, to: numbers),
			convert_node!(from: Item<f64>, to: numbers),
			convert_node!(from: Item<i8>, to: numbers),
			convert_node!(from: Item<u8>, to: numbers),
			convert_node!(from: Item<u16>, to: numbers),
			convert_node!(from: Item<i16>, to: numbers),
			convert_node!(from: Item<i32>, to: numbers),
			convert_node!(from: Item<u32>, to: numbers),
			convert_node!(from: Item<i64>, to: numbers),
			convert_node!(from: Item<u64>, to: numbers),
			convert_node!(from: Item<i128>, to: numbers),
			convert_node!(from: Item<u128>, to: numbers),
			convert_node!(from: Item<isize>, to: numbers),
			convert_node!(from: Item<usize>, to: numbers),
			convert_node!(from: numbers, to: Item<DVec2>),
			convert_node!(from: numbers, to: Item<String>),
		]
		.into_iter()
		.flatten(),
	);

	let mut map: HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>> = HashMap::new();

	for (id, entry) in graphene_std::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for (constructor, types) in entry.iter() {
			map.entry(id.clone()).or_default().insert(types.clone(), *constructor);
		}
	}

	for (id, node_constructor, types) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.as_str().replace('\n', " ");

		// Remove struct generics for all nodes except for the IntoNode and ConvertNode
		if !(new_name.contains("IntoNode") || new_name.contains("ConvertNode"))
			&& let Some((path, _generics)) = new_name.split_once("<")
		{
			new_name = path.to_string();
		}

		map.entry(ProtoNodeIdentifier::with_owned_string(new_name)).or_default().insert(types.clone(), node_constructor);
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
						let node = graphene_std::ops::IntoNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.pop().unwrap()),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>)),
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::IntoNode::new(
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
				convert_node!(from: $from, to: Item<f32>),
				convert_node!(from: $from, to: Item<f64>),
				convert_node!(from: $from, to: Item<i8>),
				convert_node!(from: $from, to: Item<u8>),
				convert_node!(from: $from, to: Item<u16>),
				convert_node!(from: $from, to: Item<i16>),
				convert_node!(from: $from, to: Item<i32>),
				convert_node!(from: $from, to: Item<u32>),
				convert_node!(from: $from, to: Item<i64>),
				convert_node!(from: $from, to: Item<u64>),
				convert_node!(from: $from, to: Item<i128>),
				convert_node!(from: $from, to: Item<u128>),
				convert_node!(from: $from, to: Item<isize>),
				convert_node!(from: $from, to: Item<usize>),
			];
			x
		}};
		(from: numbers, to: $to:ty) => {{
			let x: Vec<(ProtoNodeIdentifier, NodeConstructor, NodeIOTypes)> = vec![
				convert_node!(from: Item<f32>, to: $to),
				convert_node!(from: Item<f64>, to: $to),
				convert_node!(from: Item<i8>, to: $to),
				convert_node!(from: Item<u8>, to: $to),
				convert_node!(from: Item<u16>, to: $to),
				convert_node!(from: Item<i16>, to: $to),
				convert_node!(from: Item<i32>, to: $to),
				convert_node!(from: Item<u32>, to: $to),
				convert_node!(from: Item<i64>, to: $to),
				convert_node!(from: Item<u64>, to: $to),
				convert_node!(from: Item<i128>, to: $to),
				convert_node!(from: Item<u128>, to: $to),
				convert_node!(from: Item<isize>, to: $to),
				convert_node!(from: Item<usize>, to: $to),
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
						let node = graphene_std::ops::ConvertNode::new(
							graphene_std::any::downcast_node::<Context, $from>(args.next().expect("Convert node did not get first argument")),
							graphene_std::any::downcast_node::<Context, $convert>(args.next().expect("Convert node did not get converter argument")),
							graphene_std::any::FutureWrapperNode::new(graphene_std::value::ClonedNode::new(std::marker::PhantomData::<$to>))
						);
						let any: DynAnyNode<Context, $to, _> = graphene_std::any::DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox
					})
				},
				{
					let node = graphene_std::ops::ConvertNode::new(

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
