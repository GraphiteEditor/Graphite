// TODO: Eventually remove this document upgrade code
// This file contains lots of hacky code for upgrading old documents to the new format

use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate, OutputConnector};
use crate::messages::prelude::DocumentMessageHandler;
use glam::{DVec2, IVec2};
use graph_craft::document::DocumentNode;
use graph_craft::document::{DocumentNodeImplementation, NodeInput, value::TaggedValue};
use graphene_std::ProtoNodeIdentifier;
use graphene_std::subpath::Subpath;
use graphene_std::table::Table;
use graphene_std::text::{TextAlign, TypesettingConfig};
use graphene_std::uuid::NodeId;
use graphene_std::vector::Vector;
use graphene_std::vector::style::{PaintOrder, StrokeAlign};
use std::collections::HashMap;
use std::f64::consts::PI;

const TEXT_REPLACEMENTS: &[(&str, &str)] = &[
	("graphene_core::vector::vector_nodes::SamplePointsNode", "graphene_core::vector::SamplePolylineNode"),
	("graphene_core::vector::vector_nodes::SubpathSegmentLengthsNode", "graphene_core::vector::SubpathSegmentLengthsNode"),
	("\"manual_composition\":null", "\"manual_composition\":{\"Generic\":\"T\"}"),
];

pub struct NodeReplacement<'a> {
	node: ProtoNodeIdentifier,
	aliases: &'a [&'a str],
}

const NODE_REPLACEMENTS: &[NodeReplacement<'static>] = &[
	// artboard
	NodeReplacement {
		node: graphene_std::artboard::create_artboard::IDENTIFIER,
		aliases: &[
			"graphene_core::ConstructArtboardNode",
			"graphene_core::graphic_element::ToArtboardNode",
			"graphene_core::artboard::ToArtboardNode",
		],
	},
	// graphic
	NodeReplacement {
		node: graphene_std::graphic::to_graphic::IDENTIFIER,
		aliases: &[
			"graphene_core::ToGraphicGroupNode",
			"graphene_core::graphic_element::ToGroupNode",
			"graphene_core::graphic::ToGroupNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::wrap_graphic::IDENTIFIER,
		aliases: &[
			// Converted from "To Element"
			"graphene_core::ToGraphicElementNode",
			"graphene_core::graphic_element::ToElementNode",
			"graphene_core::graphic::ToElementNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::legacy_layer_extend::IDENTIFIER,
		aliases: &[
			"graphene_core::graphic_element::LayerNode",
			"graphene_core::graphic::LayerNode",
			// Converted from "Append Artboard"
			"graphene_core::AddArtboardNode",
			"graphene_core::graphic_element::AppendArtboardNode",
			"graphene_core::graphic::AppendArtboardNode",
			"graphene_core::artboard::AppendArtboardNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::flatten_graphic::IDENTIFIER,
		aliases: &["graphene_core::graphic_element::FlattenGroupNode", "graphene_core::graphic::FlattenGroupNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::flatten_vector::IDENTIFIER,
		aliases: &["graphene_core::graphic_element::FlattenVectorNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::index::IDENTIFIER,
		aliases: &["graphene_core::graphic_element::IndexNode"],
	},
	// math_nodes
	NodeReplacement {
		node: graphene_std::math_nodes::math::IDENTIFIER,
		aliases: &["graphene_core::ops::MathNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::add::IDENTIFIER,
		aliases: &["graphene_core::ops::AddNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::subtract::IDENTIFIER,
		aliases: &["graphene_core::ops::SubtractNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::multiply::IDENTIFIER,
		aliases: &["graphene_core::ops::MultiplyNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::divide::IDENTIFIER,
		aliases: &["graphene_core::ops::DivideNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::modulo::IDENTIFIER,
		aliases: &["graphene_core::ops::ModuloNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::exponent::IDENTIFIER,
		aliases: &["graphene_core::ops::ExponentNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::root::IDENTIFIER,
		aliases: &["graphene_core::ops::RootNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::absolute_value::IDENTIFIER,
		aliases: &["graphene_core::ops::AbsoluteValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logarithm::IDENTIFIER,
		aliases: &["graphene_core::ops::LogarithmNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::sine::IDENTIFIER,
		aliases: &["graphene_core::ops::SineNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::cosine::IDENTIFIER,
		aliases: &["graphene_core::ops::CosineNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::tangent::IDENTIFIER,
		aliases: &["graphene_core::ops::TangentNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::sine_inverse::IDENTIFIER,
		aliases: &["graphene_core::ops::SineInverseNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::cosine_inverse::IDENTIFIER,
		aliases: &["graphene_core::ops::CosineInverseNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::tangent_inverse::IDENTIFIER,
		aliases: &["graphene_core::ops::TangentInverseNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::random::IDENTIFIER,
		aliases: &["graphene_core::ops::RandomNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::to_u_32::IDENTIFIER,
		aliases: &["graphene_core::ops::ToU32Node"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::to_u_64::IDENTIFIER,
		aliases: &["graphene_core::ops::ToU64Node"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::to_f_64::IDENTIFIER,
		aliases: &["graphene_core::ops::ToF64Node"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::round::IDENTIFIER,
		aliases: &["graphene_core::ops::RoundNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::floor::IDENTIFIER,
		aliases: &["graphene_core::ops::FloorNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::ceiling::IDENTIFIER,
		aliases: &["graphene_core::ops::CeilingNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::min::IDENTIFIER,
		aliases: &["graphene_core::ops::MinNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::max::IDENTIFIER,
		aliases: &["graphene_core::ops::MaxNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::clamp::IDENTIFIER,
		aliases: &["graphene_core::ops::ClampNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::equals::IDENTIFIER,
		aliases: &["graphene_core::ops::EqualsNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::not_equals::IDENTIFIER,
		aliases: &["graphene_core::ops::NotEqualsNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::less_than::IDENTIFIER,
		aliases: &["graphene_core::ops::LessThanNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::greater_than::IDENTIFIER,
		aliases: &["graphene_core::ops::GreaterThanNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logical_or::IDENTIFIER,
		aliases: &["graphene_core::ops::LogicalOrNode", "graphene_core::ops::LogicAndNode", "graphene_core::logic::LogicAndNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logical_and::IDENTIFIER,
		aliases: &["graphene_core::ops::LogicalAndNode", "graphene_core::ops::LogicNotNode", "graphene_core::logic::LogicNotNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logical_not::IDENTIFIER,
		aliases: &["graphene_core::ops::LogicalNotNode", "graphene_core::ops::LogicOrNode", "graphene_core::logic::LogicOrNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::bool_value::IDENTIFIER,
		aliases: &["graphene_core::ops::BoolValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::number_value::IDENTIFIER,
		aliases: &["graphene_core::ops::NumberValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::percentage_value::IDENTIFIER,
		aliases: &["graphene_core::ops::PercentageValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::vec_2_value::IDENTIFIER,
		aliases: &[
			"graphene_core::ops::ConstructVector2",
			"graphene_core::ops::Vector2ValueNode",
			"graphene_core::ops::CoordinateValueNode",
			"graphene_math_nodes::CoordinateValueNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::cut_segments::IDENTIFIER,
		aliases: &["graphene_core::vector::SplitSegmentsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::cut_path::IDENTIFIER,
		aliases: &["graphene_core::vector::SplitPathNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::vec_2_to_point::IDENTIFIER,
		aliases: &["graphene_core::vector::PositionToPointNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::color_value::IDENTIFIER,
		aliases: &["graphene_core::ops::ColorValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::gradient_value::IDENTIFIER,
		aliases: &["graphene_core::ops::GradientValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::sample_gradient::IDENTIFIER,
		aliases: &["graphene_core::ops::SampleGradientNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::string_value::IDENTIFIER,
		aliases: &["graphene_core::ops::StringValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::dot_product::IDENTIFIER,
		aliases: &["graphene_core::ops::DotProductNode"],
	},
	// debug
	NodeReplacement {
		node: graphene_std::debug::size_of::IDENTIFIER,
		aliases: &["graphene_core::ops::SizeOfNode"],
	},
	NodeReplacement {
		node: graphene_std::debug::some::IDENTIFIER,
		aliases: &["graphene_core::ops::SomeNode"],
	},
	NodeReplacement {
		node: graphene_std::debug::unwrap_option::IDENTIFIER,
		aliases: &["graphene_core::ops::UnwrapNode", "graphene_core::debug::UnwrapNode"],
	},
	NodeReplacement {
		node: graphene_std::debug::clone::IDENTIFIER,
		aliases: &["graphene_core::ops::CloneNode"],
	},
	// ???
	NodeReplacement {
		node: graphene_std::extract_xy::extract_xy::IDENTIFIER,
		aliases: &["graphene_core::ops::ExtractXyNode"],
	},
	NodeReplacement {
		node: graphene_std::blending_nodes::blend_mode::IDENTIFIER,
		aliases: &["graphene_core::raster::BlendModeNode"],
	},
	NodeReplacement {
		node: graphene_std::blending_nodes::opacity::IDENTIFIER,
		aliases: &["graphene_core::raster::OpacityNode"],
	},
	NodeReplacement {
		node: graphene_std::blending_nodes::blending::IDENTIFIER,
		aliases: &["graphene_core::raster::BlendingNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::auto_tangents::IDENTIFIER,
		aliases: &["graphene_core::vector::GenerateHandlesNode", "graphene_core::vector::RemoveHandlesNode"],
	},
	// graphene_raster_nodes::blending_nodes
	NodeReplacement {
		node: graphene_std::raster_nodes::blending_nodes::blend::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::BlendNode",
			"graphene_core::raster::adjustments::BlendNode",
			"graphene_core::raster::BlendNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::blending_nodes::color_overlay::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::ColorOverlayNode",
			"graphene_core::raster::adjustments::ColorOverlayNode",
			"graphene_raster_nodes::generate_curves::ColorOverlayNode",
		],
	},
	// graphene_raster_nodes::adjustments
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::luminance::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::LuminanceNode", "graphene_core::raster::LuminanceNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::extract_channel::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::ExtractChannelNode", "graphene_core::raster::ExtractChannelNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::make_opaque::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::MakeOpaqueNode", "graphene_core::raster::ExtractOpaqueNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::brightness_contrast::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::BrightnessContrastNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::levels::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::LevelsNode", "graphene_core::raster::LevelsNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::black_and_white::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::BlackAndWhiteNode", "graphene_core::raster::BlackAndWhiteNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::hue_saturation::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::HueSaturationNode", "graphene_core::raster::HueSaturationNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::invert::IDENTIFIER,
		aliases: &[
			"graphene_core::raster::adjustments::InvertNode",
			"graphene_core::raster::InvertNode",
			"graphene_core::raster::InvertRGBNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::threshold::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::ThresholdNode", "graphene_core::raster::ThresholdNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::vibrance::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::VibranceNode", "graphene_core::raster::VibranceNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::channel_mixer::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::ChannelMixerNode", "graphene_core::raster::ChannelMixerNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::selective_color::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::SelectiveColorNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::posterize::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::PosterizeNode", "graphene_core::raster::PosterizeNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::exposure::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::ExposureNode", "graphene_core::raster::ExposureNode"],
	},
	// graphene_raster_nodes::*
	NodeReplacement {
		node: graphene_std::raster_nodes::gradient_map::gradient_map::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::gradient_map::GradientMapNode",
			"graphene_raster_nodes::adjustments::GradientMapNode",
			"graphene_core::raster::adjustments::GradientMapNode",
			"graphene_core::raster::GradientMapNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::generate_curves::generate_curves::IDENTIFIER,
		aliases: &["graphene_core::raster::adjustments::GenerateCurvesNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::dehaze::dehaze::IDENTIFIER,
		aliases: &["graphene_std::dehaze::DehazeNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::filter::blur::IDENTIFIER,
		aliases: &["graphene_std::filter::BlurNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::image_color_palette::image_color_palette::IDENTIFIER,
		aliases: &["graphene_std::image_color_palette::ImageColorPaletteNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::sample_image::IDENTIFIER,
		aliases: &["graphene_std::raster::SampleImageNode", "graphene_std::raster::SampleNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::combine_channels::IDENTIFIER,
		aliases: &["graphene_std::raster::CombineChannelsNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::mask::IDENTIFIER,
		aliases: &["graphene_std::raster::MaskNode", "graphene_std::raster::MaskImageNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::extend_image_to_bounds::IDENTIFIER,
		aliases: &["graphene_std::raster::ExtendImageToBoundsNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::empty_image::IDENTIFIER,
		aliases: &["graphene_std::raster::EmptyImageNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::image_value::IDENTIFIER,
		aliases: &["graphene_std::raster::ImageValueNode", "graphene_std::raster::ImageNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::noise_pattern::IDENTIFIER,
		aliases: &["graphene_std::raster::NoisePatternNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::mandelbrot::IDENTIFIER,
		aliases: &["graphene_std::raster::MandelbrotNode"],
	},
	// text
	NodeReplacement {
		node: graphene_std::text::text::IDENTIFIER,
		aliases: &["graphene_core::text::TextGeneratorNode"],
	},
	// transform
	NodeReplacement {
		node: graphene_std::transform_nodes::replace_transform::IDENTIFIER,
		aliases: &["graphene_core::transform::SetTransformNode", "graphene_core::transform::ReplaceTransformNode"],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::transform::IDENTIFIER,
		aliases: &["graphene_core::transform::TransformNode"],
	},
	// ???
	NodeReplacement {
		node: graphene_std::vector::spline::IDENTIFIER,
		aliases: &["graphene_core::vector::SplinesFromPointsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::ellipse::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::EllipseGenerator"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::line::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::LineGenerator"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::rectangle::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::RectangleGenerator"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::RegularPolygonGenerator"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::star::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::StarGenerator"],
	},
	NodeReplacement {
		node: graphene_std::ops::identity::IDENTIFIER,
		aliases: &[
			"graphene_core::transform::CullNode",
			"graphene_core::transform::BoundlessFootprintNode",
			"graphene_core::transform::FreezeRealTimeNode",
			"graphene_core::transform_nodes::BoundlessFootprintNode",
			"graphene_core::transform_nodes::FreezeRealTimeNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::flatten_path::IDENTIFIER,
		aliases: &["graphene_core::vector::FlattenVectorElementsNode"],
	},
	NodeReplacement {
		node: graphene_std::path_bool::boolean_operation::IDENTIFIER,
		aliases: &["graphene_std::vector::BooleanOperationNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::path_modify::IDENTIFIER,
		aliases: &["graphene_core::vector::vector_data::modification::PathModifyNode"],
	},
	// brush
	NodeReplacement {
		node: graphene_std::brush::brush::brush_stamp_generator::IDENTIFIER,
		aliases: &["graphene_std::brush::BrushStampGeneratorNode"],
	},
	NodeReplacement {
		node: graphene_std::brush::brush::blit::IDENTIFIER,
		aliases: &["graphene_std::brush::BlitNode"],
	},
	NodeReplacement {
		node: graphene_std::brush::brush::brush::IDENTIFIER,
		aliases: &["graphene_std::brush::BrushNode"],
	},
];

const REPLACEMENTS: &[(&str, &str)] = &[];

pub fn document_migration_string_preprocessing(document_serialized_content: String) -> String {
	TEXT_REPLACEMENTS
		.iter()
		.fold(document_serialized_content, |document_serialized_content, (old, new)| document_serialized_content.replace(old, new))
}

pub fn document_migration_reset_node_definition(document_serialized_content: &str) -> bool {
	// Upgrade a document being opened to use fresh copies of all nodes
	if document_serialized_content.contains("node_output_index") {
		return true;
	}

	// Upgrade layer implementation from https://github.com/GraphiteEditor/Graphite/pull/1946 (see also `fn fix_nodes()` in `main.rs` of Graphene CLI)
	if document_serialized_content.contains("graphene_core::ConstructLayerNode") || document_serialized_content.contains("graphene_core::AddArtboardNode") {
		return true;
	}

	false
}

pub fn document_migration_upgrades(document: &mut DocumentMessageHandler, reset_node_definitions_on_open: bool) {
	document.network_interface.migrate_path_modify_node();

	let network = document.network_interface.document_network().clone();

	// Apply string and node replacements to each node
	let mut replacements = HashMap::<&str, ProtoNodeIdentifier>::new();
	Iterator::chain(
		NODE_REPLACEMENTS.iter().flat_map(|NodeReplacement { node, aliases }| aliases.iter().map(|old| (*old, node.clone()))),
		REPLACEMENTS.iter().map(|(old, new)| (*old, ProtoNodeIdentifier::new(new))),
	)
	.for_each(|(old, new)| {
		if replacements.insert(old, new).is_some() {
			panic!("Duplicate old name `{old}`");
		}
	});

	for (node_id, node, network_path) in network.recursive_nodes() {
		if let DocumentNodeImplementation::ProtoNode(protonode_id) = &node.implementation {
			let node_path_without_type_args = protonode_id.name.split('<').next();
			if let Some(new) = node_path_without_type_args.and_then(|node_path| replacements.get(node_path)) {
				let mut default_template = NodeTemplate::default();
				default_template.document_node.implementation = DocumentNodeImplementation::ProtoNode(new.clone());
				document.network_interface.replace_implementation(node_id, &network_path, &mut default_template);
				document.network_interface.set_call_argument(node_id, &network_path, default_template.document_node.call_argument);
			}
		}
	}

	// Apply upgrades to each unmodified node.
	let nodes = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.map(|(node_id, node, path)| (*node_id, node.clone(), path))
		.collect::<Vec<(NodeId, graph_craft::document::DocumentNode, Vec<NodeId>)>>();
	for (node_id, node, network_path) in &nodes {
		migrate_node(node_id, node, network_path, document, reset_node_definitions_on_open);
	}
}

fn migrate_node(node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId], document: &mut DocumentMessageHandler, reset_node_definitions_on_open: bool) -> Option<()> {
	if reset_node_definitions_on_open {
		if let Some(Some(reference)) = document.network_interface.reference(node_id, network_path) {
			let node_definition = resolve_document_node_type(reference)?;
			document.network_interface.replace_implementation(node_id, network_path, &mut node_definition.default_node_template());
		}
	}

	// Upgrade old nodes to use `Context` instead of `()` or `Footprint` as their call argument
	if node.call_argument == graph_craft::concrete!(()) || node.call_argument == graph_craft::concrete!(graphene_std::transform::Footprint) {
		document.network_interface.set_call_argument(node_id, network_path, graph_craft::concrete!(graphene_std::Context));
	}

	// Only nodes that have not been modified and still refer to a definition can be updated
	let reference = document.network_interface.reference(node_id, network_path).cloned().flatten()?;
	let reference = &reference;

	let inputs_count = node.inputs.len();

	// Upgrade Stroke node to reorder parameters and add "Align" and "Paint Order" (#2644)
	if reference == "Stroke" && inputs_count == 8 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let align_input = NodeInput::value(TaggedValue::StrokeAlign(StrokeAlign::Center), false);
		let paint_order_input = NodeInput::value(TaggedValue::PaintOrder(PaintOrder::StrokeAbove), false);

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 3), align_input, network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[5].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 5), old_inputs[6].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 6), old_inputs[7].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 7), paint_order_input, network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 8), old_inputs[3].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 9), old_inputs[4].clone(), network_path);
	}

	// Rename the old "Splines from Points" node to "Spline" and upgrade it to the new "Spline" node
	if reference == "Splines from Points" {
		document.network_interface.set_reference(node_id, network_path, Some("Spline".to_string()));
	}

	// Upgrade the old "Spline" node to the new "Spline" node
	if reference == "Spline" {
		// Retrieve the proto node identifier and verify it is the old "Spline" node, otherwise skip it if this is the new "Spline" node
		let identifier = document
			.network_interface
			.implementation(node_id, network_path)
			.and_then(|implementation| implementation.get_proto_node());
		if identifier.map(|identifier| &identifier.name) != Some(&"graphene_core::vector::generator_nodes::SplineNode".into()) {
			return None;
		}

		// Obtain the document node for the given node ID, extract the vector points, and create a Vector path from the list of points
		let node = document.network_interface.document_node(node_id, network_path)?;
		let Some(TaggedValue::VecDVec2(points)) = node.inputs.get(1).and_then(|tagged_value| tagged_value.as_value()) else {
			log::error!("The old Spline node's input at index 1 is not a TaggedValue::VecDVec2");
			return None;
		};
		let vector = Vector::from_subpath(Subpath::from_anchors_linear(points.to_vec(), false));

		// Retrieve the output connectors linked to the "Spline" node's output connector
		let Some(spline_outputs) = document.network_interface.outward_wires(network_path)?.get(&OutputConnector::node(*node_id, 0)).cloned() else {
			log::error!("Vec of InputConnector Spline node is connected to its output connector 0.");
			return None;
		};

		// Get the node's current position in the graph
		let Some(node_position) = document.network_interface.position(node_id, network_path) else {
			log::error!("Could not get position of spline node.");
			return None;
		};

		// Get the "Path" node definition and fill it in with the Vector path and default vector modification
		let Some(path_node_type) = resolve_document_node_type("Path") else {
			log::error!("Path node does not exist.");
			return None;
		};
		let path_node = path_node_type.node_template_input_override([
			Some(NodeInput::value(TaggedValue::Vector(Table::new_from_element(vector)), true)),
			Some(NodeInput::value(TaggedValue::VectorModification(Default::default()), false)),
		]);

		// Get the "Spline" node definition and wire it up with the "Path" node as input
		let Some(spline_node_type) = resolve_document_node_type("Spline") else {
			log::error!("Spline node does not exist.");
			return None;
		};
		let spline_node = spline_node_type.node_template_input_override([Some(NodeInput::node(NodeId(1), 0))]);

		// Create a new node group with the "Path" and "Spline" nodes and generate new node IDs for them
		let nodes = vec![(NodeId(1), path_node), (NodeId(0), spline_node)];
		let new_ids = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect::<HashMap<_, _>>();
		let new_spline_id = *new_ids.get(&NodeId(0))?;
		let new_path_id = *new_ids.get(&NodeId(1))?;

		// Remove the old "Spline" node from the document
		document.network_interface.delete_nodes(vec![*node_id], false, network_path);

		// Insert the new "Path" and "Spline" nodes into the network interface with generated IDs
		document.network_interface.insert_node_group(nodes.clone(), new_ids, network_path);

		// Reposition the new "Spline" node to match the original "Spline" node's position
		document.network_interface.shift_node(&new_spline_id, node_position, network_path);

		// Reposition the new "Path" node with an offset relative to the original "Spline" node's position
		document.network_interface.shift_node(&new_path_id, node_position + IVec2::new(-7, 0), network_path);

		// Redirect each output connection from the old node to the new "Spline" node's output connector
		for input_connector in spline_outputs {
			document.network_interface.set_input(&input_connector, NodeInput::node(new_spline_id, 0), network_path);
		}
	}

	// Upgrade Text node to include line height and character spacing, which were previously hardcoded to 1, from https://github.com/GraphiteEditor/Graphite/pull/2016
	if reference == "Text" && inputs_count != 11 {
		let mut template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[3].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 4),
			if inputs_count == 6 {
				old_inputs[4].clone()
			} else {
				NodeInput::value(TaggedValue::F64(TypesettingConfig::default().line_height_ratio), false)
			},
			network_path,
		);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 5),
			if inputs_count == 6 {
				old_inputs[5].clone()
			} else {
				NodeInput::value(TaggedValue::F64(TypesettingConfig::default().character_spacing), false)
			},
			network_path,
		);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 6),
			if inputs_count >= 7 {
				old_inputs[6].clone()
			} else {
				NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_width), false)
			},
			network_path,
		);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 7),
			if inputs_count >= 8 {
				old_inputs[7].clone()
			} else {
				NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_height), false)
			},
			network_path,
		);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 8),
			if inputs_count >= 9 {
				old_inputs[8].clone()
			} else {
				NodeInput::value(TaggedValue::F64(TypesettingConfig::default().tilt), false)
			},
			network_path,
		);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 9),
			if inputs_count >= 11 {
				old_inputs[9].clone()
			} else {
				NodeInput::value(TaggedValue::TextAlign(TextAlign::default()), false)
			},
			network_path,
		);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 10),
			if inputs_count >= 11 {
				old_inputs[10].clone()
			} else {
				NodeInput::value(TaggedValue::Bool(false), false)
			},
			network_path,
		);
	}

	// Upgrade Sine, Cosine, and Tangent nodes to include a boolean input for whether the output should be in radians, which was previously the only option but is now not the default
	if (reference == "Sine" || reference == "Cosine" || reference == "Tangent") && inputs_count == 1 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	// Upgrade the 'Tangent on Path' node to include a boolean input for whether the output should be in radians, which was previously the only option but is now not the default
	if (reference == "Tangent on Path") && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[3].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 4), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	// Upgrade the Modulo node to include a boolean input for whether the output should be always positive, which was previously not an option
	if reference == "Modulo" && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);
	}

	// Upgrade the Mirror node to add the `keep_original` boolean input
	if reference == "Mirror" && inputs_count == 3 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 3), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	// Upgrade the Mirror node to add the `reference_point` input and change `offset` from `DVec2` to `f64`
	if reference == "Mirror" && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let Some(&TaggedValue::DVec2(old_offset)) = old_inputs[1].as_value() else { return None };
		let old_offset = if old_offset.x.abs() > old_offset.y.abs() { old_offset.x } else { old_offset.y };

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 1),
			NodeInput::value(TaggedValue::ReferencePoint(graphene_std::transform::ReferencePoint::Center), false),
			network_path,
		);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::F64(old_offset), false), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[3].clone(), network_path);
	}

	// Upgrade artboard name being passed as hidden value input to "Create Artboard"
	if reference == "Artboard" && reset_node_definitions_on_open {
		let label = document.network_interface.display_name(node_id, network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(NodeId(0), 1), NodeInput::value(TaggedValue::String(label), false), &[*node_id]);
	}

	if reference == "Image" && inputs_count == 1 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		// Insert a new empty input for the image
		document.network_interface.add_import(TaggedValue::None, false, 0, "Empty", "", &[*node_id]);
		document.network_interface.set_reference(node_id, network_path, Some("Image".to_string()));
	}

	if reference == "Noise Pattern" && inputs_count == 15 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);
		for (i, input) in old_inputs.iter().enumerate() {
			document.network_interface.set_input(&InputConnector::node(*node_id, i + 1), input.clone(), network_path);
		}
	}

	if reference == "Instance on Points" && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
	}

	if reference == "Morph" && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
		// We have removed the last input, so we don't add index 3
	}

	if reference == "Brush" && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		// We have removed the second input ("bounds"), so we don't add index 1 and we shift the rest of the inputs down by one
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[3].clone(), network_path);
	}

	if reference == "Flatten Vector Elements" {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);

		document.network_interface.replace_reference_name(node_id, network_path, "Flatten Path".to_string());
	}

	if reference == "Remove Handles" {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::F64(0.), false), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);

		document.network_interface.replace_reference_name(node_id, network_path, "Auto-Tangents".to_string());
	}

	if reference == "Generate Handles" {
		let mut node_template = resolve_document_node_type("Auto-Tangents")?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(true), false), network_path);

		document.network_interface.replace_reference_name(node_id, network_path, "Auto-Tangents".to_string());
	}

	if reference == "Merge by Distance" && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 2),
			NodeInput::value(TaggedValue::MergeByDistanceAlgorithm(graphene_std::vector::misc::MergeByDistanceAlgorithm::Topological), false),
			network_path,
		);
	}

	if reference == "Spatial Merge by Distance" {
		let mut node_template = resolve_document_node_type("Merge by Distance")?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node(*node_id, 2),
			NodeInput::value(TaggedValue::MergeByDistanceAlgorithm(graphene_std::vector::misc::MergeByDistanceAlgorithm::Spatial), false),
			network_path,
		);

		document.network_interface.replace_reference_name(node_id, network_path, "Merge by Distance".to_string());
	}

	if reference == "Sample Points" && inputs_count == 5 {
		let mut node_template = resolve_document_node_type("Sample Polyline")?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		let new_spacing_value = NodeInput::value(TaggedValue::PointSpacingType(graphene_std::vector::misc::PointSpacingType::Separation), false);
		let new_quantity_value = NodeInput::value(TaggedValue::U32(100), false);

		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), new_spacing_value, network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 3), new_quantity_value, network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 5), old_inputs[3].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node(*node_id, 6), old_inputs[4].clone(), network_path);

		document.network_interface.replace_reference_name(node_id, network_path, "Sample Polyline".to_string());
	}

	// Make the "Quantity" parameter a u32 instead of f64
	if reference == "Sample Polyline" {
		// Get the inputs, obtain the quantity value, and put the inputs back
		let quantity_value = document
			.network_interface
			.input_from_connector(&InputConnector::Node { node_id: *node_id, input_index: 3 }, network_path)?;

		if let NodeInput::Value { tagged_value, exposed } = quantity_value {
			if let TaggedValue::F64(value) = **tagged_value {
				let new_quantity_value = NodeInput::value(TaggedValue::U32(value as u32), *exposed);
				document.network_interface.set_input(&InputConnector::node(*node_id, 3), new_quantity_value, network_path);
			}
		}
	}

	// Make the "Grid" node, if its input of index 3 is a DVec2 for "angles" instead of a u32 for the "columns" input that now succeeds "angles", move the angle to index 5 (after "columns" and "rows")
	if reference == "Grid" && inputs_count == 6 {
		let node_definition = resolve_document_node_type(reference)?;
		let mut new_node_template = node_definition.default_node_template();

		let mut current_node_template = document.network_interface.create_node_template(node_id, network_path)?;
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut new_node_template)?;
		let index_3_value = old_inputs.get(3).cloned();

		let mut upgraded = false;

		if let Some(NodeInput::Value { tagged_value, exposed: _ }) = index_3_value {
			if matches!(*tagged_value, TaggedValue::DVec2(_)) {
				// Move index 3 to the end
				document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
				document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
				document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
				document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[4].clone(), network_path);
				document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[5].clone(), network_path);
				document.network_interface.set_input(&InputConnector::node(*node_id, 5), old_inputs[3].clone(), network_path);

				upgraded = true;
			}
		}

		if !upgraded {
			let _ = document.network_interface.replace_inputs(node_id, network_path, &mut current_node_template);
		}
	}

	// Add the "Depth" parameter to the "Instance Index" node
	if reference == "Instance Index" && inputs_count == 0 {
		let mut node_template = resolve_document_node_type(reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		document.network_interface.set_display_name(node_id, "Instance Index".to_string(), network_path);

		let mut node_path = network_path.to_vec();
		node_path.push(*node_id);

		document.network_interface.add_import(TaggedValue::None, false, 0, "Primary", "", &node_path);
		document.network_interface.add_import(TaggedValue::U32(0), false, 1, "Loop Level", "TODO", &node_path);
	}

	// Migrate the Transform node to use degrees instead of radians
	if reference == "Transform" && node.inputs.get(6).is_none() {
		let mut node_template = resolve_document_node_type("Transform")?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// Value
		document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), network_path);
		// Translation
		document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), network_path);
		// Rotation
		document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), network_path);
		// Scale
		document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[3].clone(), network_path);
		// Skew
		document.network_interface.set_input(&InputConnector::node(*node_id, 4), old_inputs[4].clone(), network_path);
		// Origin Offset
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 5), NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false), network_path);
		// Scale Appearance
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, 6), NodeInput::value(TaggedValue::Bool(true), false), network_path);

		// Migrate rotation from radians to degrees
		match node.inputs.get(2)? {
			NodeInput::Value { tagged_value, exposed } => {
				// Read the existing Properties panel number value, which used to be in radians
				let TaggedValue::F64(radians) = *tagged_value.clone().into_inner() else { return None };

				// Convert the radians to degrees and set it back as the new input value
				let degrees = NodeInput::value(TaggedValue::F64(radians.to_degrees()), *exposed);
				document.network_interface.set_input(&InputConnector::node(*node_id, 2), degrees, network_path);
			}
			NodeInput::Node { .. } => {
				// Construct a new Multiply node for converting from degrees to radians
				let Some(multiply_node) = resolve_document_node_type("Multiply") else {
					log::error!("Could not get multiply node from definition when upgrading transform");
					return None;
				};
				let mut multiply_template = multiply_node.default_node_template();
				multiply_template.document_node.inputs[1] = NodeInput::value(TaggedValue::F64(180. / PI), false);

				// Decide on the placement position of the new Multiply node
				let multiply_node_id = NodeId::new();
				let Some(transform_position) = document.network_interface.position_from_downstream_node(node_id, network_path) else {
					log::error!("Could not get positon for transform node {node_id}");
					return None;
				};
				let multiply_position = transform_position + IVec2::new(-7, 1);

				// Insert the new Multiply node into the network directly before it's used
				document.network_interface.insert_node(multiply_node_id, multiply_template, network_path);
				document.network_interface.shift_absolute_node_position(&multiply_node_id, multiply_position, network_path);
				document.network_interface.insert_node_between(&multiply_node_id, &InputConnector::node(*node_id, 2), 0, network_path);
			}
			_ => {}
		};

		// Migrate skew from radians to degrees
		if let NodeInput::Value { tagged_value, exposed } = node.inputs.get(4)? {
			// Read the existing Properties panel number value, which used to be in radians
			let TaggedValue::DVec2(old_value) = *tagged_value.clone().into_inner() else { return None };

			// The previous value stored the tangent of the displayed degrees. Now it stores the degrees, so take the arctan of it and convert to degrees.
			let new_value = DVec2::new(old_value.x.atan().to_degrees(), old_value.y.atan().to_degrees());
			let new_input = NodeInput::value(TaggedValue::DVec2(new_value), *exposed);
			document.network_interface.set_input(&InputConnector::node(*node_id, 4), new_input, network_path);
		}
	}

	// Add context features to nodes that don't have them (fine-grained context caching migration)
	if node.context_features == graphene_std::ContextDependencies::default()
		&& let Some(reference) = document.network_interface.reference(node_id, network_path).cloned().flatten()
		&& let Some(node_definition) = resolve_document_node_type(&reference)
	{
		let context_features = node_definition.node_template.document_node.context_features;
		document.network_interface.set_context_features(node_id, network_path, context_features);
	}

	// ==================================
	// PUT ALL MIGRATIONS ABOVE THIS LINE
	// ==================================

	// Ensure layers are positioned as stacks if they are upstream siblings of another layer
	document.network_interface.load_structure();
	let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(document.network_interface.document_metadata()).collect::<Vec<_>>();
	for layer in all_layers {
		let (downstream_node, input_index) = document
			.network_interface
			.outward_wires(&[])
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(layer.to_node(), 0)))
			.and_then(|outward_wires| outward_wires.first())
			.and_then(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())))?;
		// If the downstream node is a layer and the input is the first input and the current layer is not in a stack
		if input_index == 0 && document.network_interface.is_layer(&downstream_node, &[]) && !document.network_interface.is_stack(&layer.to_node(), &[]) {
			// Ensure the layer is horizontally aligned with the downstream layer to prevent changing the layout of old files
			let (Some(layer_position), Some(downstream_position)) = (document.network_interface.position(&layer.to_node(), &[]), document.network_interface.position(&downstream_node, &[])) else {
				log::error!("Could not get position for layer {:?} or downstream node {} when opening file", layer.to_node(), downstream_node);
				return None;
			};
			if layer_position.x == downstream_position.x {
				document.network_interface.set_stack_position_calculated_offset(&layer.to_node(), &downstream_node, &[]);
			}
		}
	}

	Some(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_no_duplicate_node_replacements() {
		let mut hashmap = HashMap::<ProtoNodeIdentifier, u32>::new();
		NODE_REPLACEMENTS.iter().for_each(|node| {
			*hashmap.entry(node.node.clone()).or_default() += 1;
		});
		let duplicates = hashmap.iter().filter(|(_, count)| **count > 1).map(|(node, _)| &node.name).collect::<Vec<_>>();
		if !duplicates.is_empty() {
			panic!("Duplicate entries in `NODE_REPLACEMENTS`: {duplicates:?}");
		}
	}
}
