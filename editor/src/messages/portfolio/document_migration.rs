// TODO: Eventually remove this document upgrade code
// This file contains lots of hacky code for upgrading old documents to the new format

use crate::messages::portfolio::document::graph_operation::utility_types::set_stroke_paint_order;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_document_node_type, resolve_network_node_type, resolve_proto_node_type};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate, NodeTemplateImplementation, OutputConnector};
use crate::messages::prelude::DocumentMessageHandler;
use glam::{DVec2, IVec2};
use graph_craft::application_io::resource::{DataSource, Resource, ResourceHash, ResourceId};
use graph_craft::document::DocumentNode;
use graph_craft::document::{DocumentNodeImplementation, NodeInput, value::TaggedValue};
use graph_craft::{Type, concrete, item, list};
use graphene_std::Color;
use graphene_std::ParameterRef;
use graphene_std::ProtoNodeIdentifier;
use graphene_std::extract_xy::XY;
use graphene_std::raster::RedGreenBlueAlpha;
use graphene_std::text::{TextAlign, TypesettingConfig};
use graphene_std::transform::ScaleType;
use graphene_std::uuid::NodeId;
use graphene_std::vector::graphic_types;
use graphene_std::vector::style::{GradientRamp, GradientSpread, PaintOrder, StrokeAlign};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::ops::Range;

const TEXT_REPLACEMENTS: &[(&str, &str)] = &[
	("graphene_core::vector::vector_nodes::SamplePointsNode", "graphene_core::vector::SamplePolylineNode"),
	("graphene_core::vector::vector_nodes::SubpathSegmentLengthsNode", "graphene_core::vector::SubpathSegmentLengthsNode"),
	("\"manual_composition\":null", "\"manual_composition\":{\"Generic\":\"T\"}"),
	(
		"core::option::Option<alloc::sync::Arc<graphene_core::context::OwnedContextImpl>>",
		"core::option::Option<alloc::sync::Arc<core_types::context::OwnedContextImpl>>",
	),
	("graphene_core::transform::Footprint", "graphene_core::transform::Footprint"),
	("\"OptionalF64\":", "\"Number\":"),
	("\"path_bool_nodes::BooleanOperation\"", "\"vector_types::vector::misc::BooleanOperation\""),
	("\"core_types::table::Table<", "\"core_types::list::List<"),
	// The `GradientStops` type was renamed to `Gradient`; stale stored output names are cleared so the display falls back to the live type name
	("\"output_names\":[\"GradientStops\"]", "\"output_names\":[\"\"]"),
	("vector_types::gradient::GradientStops", "vector_types::gradient::Gradient"),
];

pub struct NodeReplacement<'a> {
	node: ProtoNodeIdentifier,
	aliases: &'a [&'a str],
}

/// Every name the Merge layer network's two type-coercion nodes have gone by, which is every alias of the node they both converged on.
fn into_group_aliases() -> impl Iterator<Item = &'static &'static str> {
	NODE_REPLACEMENTS
		.iter()
		.filter(|replacement| replacement.node == graphene_std::graphic::into_group::IDENTIFIER)
		.flat_map(|replacement| replacement.aliases)
}

/// Clears a migrated node's stored display name if it was only the replaced node's default name, so it shows its new node's name instead.
fn reset_default_display_name(document: &mut DocumentMessageHandler, node_id: &NodeId, network_path: &[NodeId], old_default_name: &str) {
	if document.network_interface.display_name(node_id, network_path) == old_default_name {
		document.network_interface.set_display_name(node_id, String::new(), network_path);
	}
}

const NODE_REPLACEMENTS: &[NodeReplacement<'static>] = &[
	// ================================
	// blending
	// ================================
	// The legacy combined "Blending" node was split into separate Blend Mode, Opacity (now also covers fill), and Clip nodes.
	// Old Blending references are remapped here to `blend_mode::IDENTIFIER` so the per-node migration in `migrate_node` can
	// detect a 5-input Blend Mode node and rewrite it as a chain (skipping any sub-node whose value is at the default and
	// not exposed/wired up).
	NodeReplacement {
		node: graphene_std::blending_nodes::blend_mode::IDENTIFIER,
		aliases: &[
			"graphene_core::raster::BlendModeNode",
			"graphene_core::blending_nodes::BlendModeNode",
			"graphene_core::raster::BlendingNode",
			"graphene_core::blending_nodes::BlendingNode",
			"blending_nodes::BlendingNode",
		],
	},
	NodeReplacement {
		node: graphene_std::blending_nodes::opacity::IDENTIFIER,
		aliases: &["graphene_core::raster::OpacityNode", "graphene_core::blending_nodes::OpacityNode"],
	},
	// ================================
	// gcore
	// ================================
	NodeReplacement {
		node: graphene_std::animation::animation_time::IDENTIFIER,
		aliases: &["graphene_core::animation::AnimationTimeNode"],
	},
	NodeReplacement {
		node: graphene_std::ops::passthrough::IDENTIFIER,
		aliases: &[
			"graphene_core::ops::IdentityNode",
			"graphene_core::transform::CullNode",
			"graphene_core::transform::BoundlessFootprintNode",
			"graphene_core::transform::FreezeRealTimeNode",
			"graphene_core::transform_nodes::BoundlessFootprintNode",
			"graphene_core::transform_nodes::FreezeRealTimeNode",
			"graphene_core::vector::SubpathSegmentLengthsNode",
			"core_types::vector::SubpathSegmentLengthsNode",
			// The deleted debug Option trio degrades to a passthrough of its single input
			"graphene_core::ops::SizeOfNode",
			"graphene_core::debug::SizeOfNode",
			"graphene_core::ops::SomeNode",
			"graphene_core::debug::SomeNode",
			"graphene_core::ops::UnwrapNode",
			"graphene_core::debug::UnwrapNode",
			"graphene_core::debug::UnwrapOptionNode",
			// The deleted Sample Image degrades to a passthrough of its image
			"graphene_raster_nodes::std_nodes::SampleImageNode",
			"graphene_std::raster::SampleImageNode",
			"graphene_std::raster::SampleNode",
			"raster_nodes::std_nodes::SampleImageNode",
		],
	},
	NodeReplacement {
		node: graphene_std::memo::monitor::IDENTIFIER,
		aliases: &["graphene_core::memo::MonitorNode"],
	},
	NodeReplacement {
		node: graphene_std::memo::memoize::IDENTIFIER,
		aliases: &["graphene_core::memo::MemoNode", "graphene_core::memo::ImpureMemoNode"],
	},
	NodeReplacement {
		node: graphene_std::animation::real_time::IDENTIFIER,
		aliases: &["graphene_core::animation::RealTimeNode"],
	},
	NodeReplacement {
		node: graphene_std::animation::step_animation_time::IDENTIFIER,
		aliases: &["graphene_core::animation::QuantizeAnimationTimeNode"],
	},
	NodeReplacement {
		node: graphene_std::animation::step_real_time::IDENTIFIER,
		aliases: &["graphene_core::animation::QuantizeRealTimeNode"],
	},
	NodeReplacement {
		node: graphene_std::debug::debug_log::IDENTIFIER,
		aliases: &["graphene_core::debug::LogToConsoleNode"],
	},
	// ================================
	// graphic
	// ================================
	NodeReplacement {
		node: graphene_std::artboard::create_artboard::IDENTIFIER,
		aliases: &[
			"graphene_core::artboard::CreateArtboardNode",
			"graphene_core::ConstructArtboardNode",
			"graphene_core::graphic_element::ToArtboardNode",
			"graphene_core::artboard::ToArtboardNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::extend::IDENTIFIER,
		aliases: &["graphene_core::graphic::graphic::ExtendNode", "graphene_core::graphic::ExtendNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::flatten_graphic::IDENTIFIER,
		aliases: &[
			"graphene_core::graphic::FlattenGraphicNode",
			"graphene_core::graphic_element::FlattenGroupNode",
			"graphene_core::graphic_types::FlattenGroupNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::flatten_vector::IDENTIFIER,
		aliases: &["graphene_core::graphic::FlattenVectorNode", "graphene_core::graphic_element::FlattenVectorNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::item_at_index::IDENTIFIER,
		aliases: &[
			"graphene_core::graphic_element::IndexNode",
			"graphene_core::graphic::IndexNode",
			"graphene_core::graphic::IndexElementsNode",
			"graphic_nodes::graphic::IndexElementsNode",
			"graphic_nodes::graphic::ExtractElementNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::read_attribute_gradient_form::IDENTIFIER,
		aliases: &["graphic_nodes::graphic::ReadAttributeGradientTypeNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::read_attribute_gradient_spread::IDENTIFIER,
		aliases: &["graphic_nodes::graphic::ReadAttributeSpreadMethodNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::remove_at_index::IDENTIFIER,
		aliases: &["graphic_nodes::graphic::OmitElementNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::indices::IDENTIFIER,
		aliases: &["graphic_nodes::graphic::ListIndicesNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::slice::IDENTIFIER,
		aliases: &["graphic_nodes::graphic::ListSliceNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::legacy_layer_extend::IDENTIFIER,
		aliases: &[
			"graphene_core::graphic_element::LayerNode",
			"graphene_core::graphic_types::LayerNode",
			// Converted from "Append Artboard"
			"graphene_core::AddArtboardNode",
			"graphene_core::graphic_element::AppendArtboardNode",
			"graphene_core::graphic_types::AppendArtboardNode",
			"graphene_core::artboard::AppendArtboardNode",
			"graphene_core::graphic::LegacyLayerExtendNode",
		],
	},
	NodeReplacement {
		node: graphene_std::graphic::into_group::IDENTIFIER,
		aliases: &[
			// Converted from "To Element", then "Wrap Graphic"
			"graphene_core::ToGraphicElementNode",
			"graphene_core::graphic_element::ToElementNode",
			"graphene_core::graphic_types::ToElementNode",
			"graphene_core::graphic::WrapGraphicNode",
			"graphic_nodes::graphic::WrapGraphicNode",
			// Converted from "To Graphic", whose grouping of non-graphical content this node now carries alone
			"graphene_core::ToGraphicGroupNode",
			"graphene_core::graphic_element::ToGroupNode",
			"graphene_core::graphic_types::ToGroupNode",
			"graphene_core::graphic::ToGraphicNode",
			"graphic_nodes::graphic::ToGraphicNode",
		],
	},
	// ================================
	// gstd
	// ================================
	NodeReplacement {
		node: graphene_std::platform_application_io::bytes_to_image::IDENTIFIER,
		aliases: &["graphene_std::platform_application_io::DecodeImageNode"],
	},
	// ================================
	// math
	// ================================
	NodeReplacement {
		node: graphene_std::math_nodes::absolute_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::AbsoluteValueNode", "graphene_core::ops::AbsoluteValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::add::IDENTIFIER,
		aliases: &["graphene_math_nodes::AddNode", "graphene_core::ops::AddNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::bool_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::BoolValueNode", "graphene_core::ops::BoolValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::ceiling::IDENTIFIER,
		aliases: &["graphene_math_nodes::CeilingNode", "graphene_core::ops::CeilingNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::clamp::IDENTIFIER,
		aliases: &["graphene_math_nodes::ClampNode", "graphene_core::ops::ClampNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::color_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::ColorValueNode", "graphene_core::ops::ColorValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::cosine::IDENTIFIER,
		aliases: &["graphene_math_nodes::CosineNode", "graphene_core::ops::CosineNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::cosine_inverse::IDENTIFIER,
		aliases: &["graphene_math_nodes::CosineInverseNode", "graphene_core::ops::CosineInverseNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::divide::IDENTIFIER,
		aliases: &["graphene_math_nodes::DivideNode", "graphene_core::ops::DivideNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::dot_product::IDENTIFIER,
		aliases: &["graphene_math_nodes::DotProductNode", "graphene_core::ops::DotProductNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::equals::IDENTIFIER,
		aliases: &["graphene_math_nodes::EqualsNode", "graphene_core::ops::EqualsNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::evaluate_gradient::IDENTIFIER,
		aliases: &["math_nodes::SampleGradientNode", "graphene_math_nodes::SampleGradientNode", "graphene_core::ops::SampleGradientNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::exponent::IDENTIFIER,
		aliases: &["graphene_math_nodes::ExponentNode", "graphene_core::ops::ExponentNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::floor::IDENTIFIER,
		aliases: &["graphene_math_nodes::FloorNode", "graphene_core::ops::FloorNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::footprint_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::FootprintValueNode", "graphene_core::ops::FootprintValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::gradient_form::IDENTIFIER,
		aliases: &["math_nodes::GradientTypeNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::gradient_spread::IDENTIFIER,
		aliases: &["math_nodes::SpreadMethodNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::gradient_value::IDENTIFIER,
		aliases: &[
			"graphene_math_nodes::GradientValueNode",
			"graphene_core::ops::GradientValueNode",
			"graphene_math_nodes::GradientTableValueNode",
			"graphene_core::ops::GradientTableValueNode",
			"math_nodes::GradientTableValueNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::greater_than::IDENTIFIER,
		aliases: &["graphene_math_nodes::GreaterThanNode", "graphene_core::ops::GreaterThanNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::greatest_common_divisor::IDENTIFIER,
		aliases: &[
			"graphene_math_nodes::GreatestCommonDivisor",
			"graphene_core::ops::GreatestCommonDivisor",
			"graphene_math_nodes::GreatestCommonDivisorNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::least_common_multiple::IDENTIFIER,
		aliases: &[
			"graphene_math_nodes::LeastCommonMultiple",
			"graphene_core::ops::LeastCommonMultiple",
			"graphene_math_nodes::LeastCommonMultipleNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::magnitude::IDENTIFIER,
		aliases: &["math_nodes::LengthNode", "graphene_math_nodes::LengthNode", "graphene_core::ops::LenghtNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::less_than::IDENTIFIER,
		aliases: &["graphene_math_nodes::LessThanNode", "graphene_core::ops::LessThanNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logarithm::IDENTIFIER,
		aliases: &["graphene_math_nodes::LogarithmNode", "graphene_core::ops::LogarithmNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logical_and::IDENTIFIER,
		aliases: &[
			"graphene_core::ops::LogicalAndNode",
			"graphene_core::ops::LogicNotNode",
			"graphene_core::logic::LogicNotNode",
			"graphene_math_nodes::LogicalAndNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logical_not::IDENTIFIER,
		aliases: &[
			"graphene_core::ops::LogicalNotNode",
			"graphene_core::ops::LogicOrNode",
			"graphene_core::logic::LogicOrNode",
			"graphene_math_nodes::LogicalNotNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::logical_or::IDENTIFIER,
		aliases: &[
			"graphene_core::ops::LogicalOrNode",
			"graphene_core::ops::LogicAndNode",
			"graphene_core::logic::LogicAndNode",
			"graphene_math_nodes::LogicalOrNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::max::IDENTIFIER,
		aliases: &["graphene_math_nodes::MaxNode", "graphene_core::ops::MaxNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::min::IDENTIFIER,
		aliases: &["graphene_math_nodes::MinNode", "graphene_core::ops::MinNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::modulo::IDENTIFIER,
		aliases: &["graphene_math_nodes::ModuloNode", "graphene_core::ops::ModuloNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::multiply::IDENTIFIER,
		aliases: &["graphene_math_nodes::MultiplyNode", "graphene_core::ops::MultiplyNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::normalize::IDENTIFIER,
		aliases: &["graphene_math_nodes::NormalizeNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::not_equals::IDENTIFIER,
		aliases: &["graphene_math_nodes::NotEqualsNode", "graphene_core::ops::NotEqualsNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::number_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::NumberValueNode", "graphene_core::ops::NumberValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::percentage_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::PercentageValueNode", "graphene_core::ops::PercentageValueNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::random::IDENTIFIER,
		aliases: &["graphene_math_nodes::RandomNode", "graphene_core::ops::RandomNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::remap::IDENTIFIER,
		aliases: &["graphene_math_nodes::RemapNode", "graphene_core::ops::RemapNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::root::IDENTIFIER,
		aliases: &["graphene_math_nodes::RootNode", "graphene_core::ops::RootNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::round::IDENTIFIER,
		aliases: &["graphene_math_nodes::RoundNode", "graphene_core::ops::RoundNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::sine::IDENTIFIER,
		aliases: &["graphene_math_nodes::SineNode", "graphene_core::ops::SineNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::sine_inverse::IDENTIFIER,
		aliases: &["graphene_math_nodes::SineInverseNode", "graphene_core::ops::SineInverseNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::string_to_color::IDENTIFIER,
		aliases: &["math_nodes::HexToColorNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::subtract::IDENTIFIER,
		aliases: &["graphene_math_nodes::SubtractNode", "graphene_core::ops::SubtractNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::tangent::IDENTIFIER,
		aliases: &["graphene_math_nodes::TangentNode", "graphene_core::ops::TangentNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::tangent_inverse::IDENTIFIER,
		aliases: &["graphene_math_nodes::TangentInverseNode", "graphene_core::ops::TangentInverseNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::as_number::IDENTIFIER,
		aliases: &["graphene_math_nodes::ToF64Node", "graphene_core::ops::ToF64Node", "math_nodes::ToF64Node", "math_nodes::AsF64Node"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::as_integer::IDENTIFIER,
		aliases: &[
			"graphene_math_nodes::ToU32Node",
			"graphene_core::ops::ToU32Node",
			"math_nodes::ToU32Node",
			"math_nodes::AsU32Node",
			"graphene_math_nodes::ToU64Node",
			"graphene_core::ops::ToU64Node",
			"math_nodes::ToU64Node",
			"math_nodes::AsU64Node",
		],
	},
	// The old 'Vec2 Value' node took separate X and Y inputs, a role now filled by 'Numbers to Vec2', while the new 'Vec2 Value' node takes a single vec2 input.
	// Old references (including these older aliases) are remapped here to `vec_2_value::IDENTIFIER` so the per-node migration in `migrate_node` can detect the leftover 3-input shape and convert it into a 'Numbers to Vec2' node.
	NodeReplacement {
		node: graphene_std::math_nodes::vec_2_value::IDENTIFIER,
		aliases: &[
			"graphene_math_nodes::Vec2ValueNode",
			"graphene_core::ops::ConstructVector2",
			"graphene_core::ops::Vector2ValueNode",
			"graphene_core::ops::CoordinateValueNode",
			"graphene_math_nodes::CoordinateValueNode",
		],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::numbers_to_vec_2::IDENTIFIER,
		aliases: &["math_nodes::CombineVec2Node"],
	},
	// ================================
	// path bool
	// ================================
	NodeReplacement {
		node: graphene_std::path_bool_nodes::boolean_operation::IDENTIFIER,
		aliases: &["graphene_path_bool::BooleanOperationNode", "graphene_std::vector::BooleanOperationNode"],
	},
	// ================================
	// raster
	// ================================
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::black_and_white::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::BlackAndWhiteNode",
			"graphene_core::raster::adjustments::BlackAndWhiteNode",
			"graphene_core::raster::BlackAndWhiteNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::blending_nodes::mix_colors::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::BlendNode",
			"raster_nodes::adjustments::BlendNode",
			"graphene_core::raster::adjustments::BlendNode",
			"graphene_core::raster::BlendNode",
			"graphene_raster_nodes::blending_nodes::BlendNode",
			"raster_nodes::blending_nodes::BlendNode",
			"raster_nodes::blending_nodes::MixNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::filter::blur::IDENTIFIER,
		aliases: &["graphene_raster_nodes::filter::BlurNode", "graphene_std::filter::BlurNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::brightness_contrast::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::BrightnessContrastNode",
			"graphene_core::raster::adjustments::BrightnessContrastNode",
			"graphene_raster_nodes::adjustments::brightness_contrast_classic",
			"graphene_raster_nodes::adjustments::BrightnessContrastClassicNode",
			"raster_nodes::adjustments::BrightnessContrastClassicNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::channel_mixer::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::ChannelMixerNode",
			"graphene_core::raster::adjustments::ChannelMixerNode",
			"graphene_core::raster::ChannelMixerNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::blending_nodes::color_overlay::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::ColorOverlayNode",
			"graphene_raster_nodes::generate_curves::ColorOverlayNode",
			"raster_nodes::adjustments::ColorOverlayNode",
			"graphene_core::raster::adjustments::ColorOverlayNode",
			"raster_nodes::generate_curves::ColorOverlayNode",
			"graphene_raster_nodes::blending_nodes::ColorOverlayNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::channels_to_image::IDENTIFIER,
		aliases: &[
			"raster_nodes::std_nodes::CombineChannelsNode",
			"graphene_raster_nodes::std_nodes::CombineChannelsNode",
			"graphene_std::raster::CombineChannelsNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::dehaze::dehaze::IDENTIFIER,
		aliases: &["graphene_raster_nodes::dehaze::DehazeNode", "graphene_std::dehaze::DehazeNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::exposure::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::ExposureNode",
			"graphene_core::raster::adjustments::ExposureNode",
			"graphene_core::raster::ExposureNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::extend_image_to_bounds::IDENTIFIER,
		aliases: &["graphene_raster_nodes::std_nodes::ExtendImageToBoundsNode", "graphene_std::raster::ExtendImageToBoundsNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::gamma_correction::IDENTIFIER,
		aliases: &["graphene_raster_nodes::adjustments::GammaCorrectionNode", "graphene_core::raster::adjustments::GammaCorrectionNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::gradient_map::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::gradient_map::GradientMapNode",
			"graphene_raster_nodes::adjustments::GradientMapNode",
			"raster_nodes::gradient_map::GradientMapNode",
			"raster_nodes::adjustments::GradientMapNode",
			"graphene_core::raster::adjustments::GradientMapNode",
			"graphene_core::raster::GradientMapNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::hue_saturation::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::HueSaturationNode",
			"graphene_core::raster::adjustments::HueSaturationNode",
			"graphene_core::raster::HueSaturationNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::image_color_palette::image_color_palette::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::image_color_palette::ImageColorPaletteNode",
			"graphene_std::image_color_palette::ImageColorPaletteNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::image::IDENTIFIER,
		aliases: &[
			"raster_nodes::std_nodes::ImageValueNode",
			"graphene_raster_nodes::std_nodes::ImageValueNode",
			"graphene_std::raster::ImageValueNode",
			"graphene_std::raster::ImageNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::invert::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::InvertNode",
			"graphene_core::raster::adjustments::InvertNode",
			"graphene_core::raster::InvertNode",
			"graphene_core::raster::InvertRGBNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::levels::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::LevelsNode",
			"graphene_core::raster::adjustments::LevelsNode",
			"graphene_core::raster::LevelsNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::desaturate::IDENTIFIER,
		aliases: &[
			"raster_nodes::adjustments::LuminanceNode",
			"graphene_raster_nodes::adjustments::LuminanceNode",
			"graphene_core::raster::adjustments::LuminanceNode",
			"graphene_core::raster::LuminanceNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::make_opaque::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::MakeOpaqueNode",
			"graphene_core::raster::adjustments::MakeOpaqueNode",
			"graphene_core::raster::ExtractOpaqueNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::mandelbrot::IDENTIFIER,
		aliases: &["graphene_raster_nodes::std_nodes::MandelbrotNode", "graphene_std::raster::MandelbrotNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::mask::IDENTIFIER,
		aliases: &["graphene_raster_nodes::std_nodes::MaskNode", "graphene_std::raster::MaskNode", "graphene_std::raster::MaskImageNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::posterize::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::PosterizeNode",
			"graphene_core::raster::adjustments::PosterizeNode",
			"graphene_core::raster::PosterizeNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::std_nodes::noise_pattern::IDENTIFIER,
		aliases: &["graphene_raster_nodes::std_nodes::NoisePatternNode", "graphene_std::raster::NoisePatternNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::selective_color::IDENTIFIER,
		aliases: &["graphene_raster_nodes::adjustments::SelectiveColorNode", "graphene_core::raster::adjustments::SelectiveColorNode"],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::threshold::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::ThresholdNode",
			"graphene_core::raster::adjustments::ThresholdNode",
			"graphene_core::raster::ThresholdNode",
		],
	},
	NodeReplacement {
		node: graphene_std::raster_nodes::adjustments::vibrance::IDENTIFIER,
		aliases: &[
			"graphene_raster_nodes::adjustments::VibranceNode",
			"graphene_core::raster::adjustments::VibranceNode",
			"graphene_core::raster::VibranceNode",
		],
	},
	// ================================
	// text
	// ================================
	NodeReplacement {
		node: ProtoNodeIdentifier::new("graphene_std::text::TextNode"),
		aliases: &["graphene_core::text::text::TextNode", "graphene_core::text::TextGeneratorNode", "graphene_core::text::TextNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_value::IDENTIFIER,
		aliases: &["graphene_math_nodes::StringValueNode", "graphene_core::ops::StringValueNode", "math_nodes::StringValueNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_concatenate::IDENTIFIER,
		aliases: &["graphene_core::logic::StringConcatenateNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_length::IDENTIFIER,
		aliases: &["graphene_core::logic::StringLengthNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_replace::IDENTIFIER,
		aliases: &["graphene_core::logic::StringReplaceNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_slice::IDENTIFIER,
		aliases: &["graphene_core::logic::StringSliceNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_split::IDENTIFIER,
		aliases: &["graphene_core::logic::StringSplitNode"],
	},
	NodeReplacement {
		node: graphene_std::math_nodes::switch::IDENTIFIER,
		aliases: &["graphene_core::logic::SwitchNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::as_string::IDENTIFIER,
		aliases: &["graphene_core::logic::ToStringNode", "text_nodes::ToStringNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::json::query_json::IDENTIFIER,
		aliases: &["graphene_core::logic::JsonGetNode", "graphene_std::text_nodes::JsonGetNode"],
	},
	NodeReplacement {
		node: graphene_std::text_nodes::string_map::IDENTIFIER,
		aliases: &["text_nodes::MapStringNode"],
	},
	// ================================
	// transform
	// ================================
	NodeReplacement {
		node: graphene_std::transform_nodes::decompose_rotation::IDENTIFIER,
		aliases: &[
			"graphene_core::transform_nodes::RotationScaleNode",
			"graphene_core::transform::RotationScaleNode",
			"graphene_core::transform_nodes::DecomposeRotationNode",
		],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::decompose_scale::IDENTIFIER,
		aliases: &["graphene_core::transform_nodes::DecomposeScaleNode", "graphene_core::transform::DecomposeScaleNode"],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::decompose_translation::IDENTIFIER,
		aliases: &["graphene_core::transform_nodes::DecomposeTranslationNode", "graphene_core::transform::DecomposeTranslationNode"],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::extract_transform::IDENTIFIER,
		aliases: &[
			"graphene_core::transform_nodes::ExtractTransformNode",
			"graphene_core::transform::ExtractTransformNode",
			"graphene_core::vector::ExtractTransformNode",
		],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::invert_transform::IDENTIFIER,
		aliases: &["graphene_core::transform_nodes::InvertTransformNode", "graphene_core::transform::InvertTransformNode"],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::replace_transform::IDENTIFIER,
		aliases: &[
			"graphene_core::transform_nodes::ReplaceTransformNode",
			"graphene_core::transform::SetTransformNode",
			"graphene_core::transform::ReplaceTransformNode",
		],
	},
	NodeReplacement {
		node: graphene_std::transform_nodes::transform::IDENTIFIER,
		aliases: &["graphene_core::transform_nodes::TransformNode", "graphene_core::transform::TransformNode"],
	},
	// ================================
	// vector
	// ================================
	NodeReplacement {
		node: graphene_std::vector::bake_transform::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::ApplyTransformNode",
			"graphene_core::vector::vector_modification::ApplyTransformNode",
			"vector_nodes::vector_modification_nodes::ApplyTransformNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::area::IDENTIFIER,
		aliases: &["graphene_core::vector::AreaNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::assign_colors::IDENTIFIER,
		aliases: &["graphene_core::vector::AssignColorsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::smooth_anchors::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::vector_nodes::AutoTangentsNode",
			"graphene_core::vector::AutoTangentsNode",
			"core_types::vector::AutoTangentsNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::bevel::IDENTIFIER,
		aliases: &["graphene_core::vector::BevelNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::bounding_box::IDENTIFIER,
		aliases: &["graphene_core::vector::BoundingBoxNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::box_warp::IDENTIFIER,
		aliases: &["graphene_core::vector::BoxWarpNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::centroid::IDENTIFIER,
		aliases: &["graphene_core::vector::CentroidNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::close_paths::IDENTIFIER,
		aliases: &["graphene_core::vector::ClosePathNode", "core_types::vector::ClosePathNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::count::IDENTIFIER,
		aliases: &["graphene_core::vector::CountElementsNode", "graphene_core::vector::ListLengthNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::cut_path::IDENTIFIER,
		aliases: &["graphene_core::vector::vector_nodes::SplitPathNode", "graphene_core::vector::SplitPathNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::unweld::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::vector_nodes::SplitSegmentsNode",
			"graphene_core::vector::SplitSegmentsNode",
			"graphene_core::vector::CutSegmentsNode",
			"core_types::vector::CutSegmentsNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::dimensions::IDENTIFIER,
		aliases: &["graphene_core::vector::DimensionsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector_nodes::fill::IDENTIFIER,
		aliases: &["graphene_core::vector::vector_nodes::FillNode", "graphene_core::vector::FillNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::combine_paths::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::vector_nodes::FlattenPathNode",
			"graphene_core::vector::FlattenVectorElementsNode",
			"graphene_core::vector::FlattenPathNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::arc::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::ArcNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::circle::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::CircleNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::ellipse::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::EllipseNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::grid::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::GridNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::line::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::LineNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::rectangle::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::RectangleNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::RegularPolygonNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::spiral::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::SpiralNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::generator_nodes::star::IDENTIFIER,
		aliases: &["graphene_core::vector::generator_nodes::StarNode"],
	},
	NodeReplacement {
		node: graphene_std::context::read_index::IDENTIFIER,
		aliases: &["graphene_core::vector::InstanceIndexNode", "core_types::vector::InstanceIndexNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::map::IDENTIFIER,
		aliases: &["graphene_core::vector::InstanceMapNode"],
	},
	NodeReplacement {
		node: graphene_std::context::read_position::IDENTIFIER,
		aliases: &["graphene_core::vector::InstancePositionNode", "core_types::vector::InstancePositionNode"],
	},
	NodeReplacement {
		node: graphene_std::context::read_vector::IDENTIFIER,
		aliases: &["graphene_core::vector::InstanceVectorNode"],
	},
	NodeReplacement {
		node: graphene_std::repeat::repeat::IDENTIFIER,
		aliases: &["graphene_core::vector::InstanceRepeatNode", "core_types::vector::InstanceRepeatNode"],
	},
	NodeReplacement {
		node: graphene_std::repeat::repeat_array::IDENTIFIER,
		aliases: &["graphene_core::vector::RepeatNode", "core_types::vector::RepeatNode"],
	},
	NodeReplacement {
		node: graphene_std::repeat::repeat_radial::IDENTIFIER,
		aliases: &["graphene_core::vector::CircularRepeatNode", "core_types::vector::CircularRepeatNode"],
	},
	NodeReplacement {
		node: graphene_std::repeat::repeat_on_points::IDENTIFIER,
		aliases: &["graphene_core::vector::InstanceOnPointsNode", "core_types::vector::InstanceOnPointsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::copy_to_points::IDENTIFIER,
		aliases: &["graphene_core::vector::CopyToPointsNode", "core_types::vector::CopyToPointsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::jitter_points::IDENTIFIER,
		aliases: &["graphene_core::vector::JitterPointsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::weld::IDENTIFIER,
		aliases: &["graphene_core::vector::MergeByDistanceNode", "core_types::vector::MergeByDistanceNode"],
	},
	NodeReplacement {
		node: graphene_std::graphic::mirror::IDENTIFIER,
		aliases: &["graphene_core::vector::MirrorNode", "core_types::vector::MirrorNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::morph::IDENTIFIER,
		aliases: &["graphene_core::vector::MorphNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::offset_path::IDENTIFIER,
		aliases: &["graphene_core::vector::OffsetPathNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::path_length::IDENTIFIER,
		aliases: &["graphene_core::vector::PathLengthNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::path_modify::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::vector_modification_nodes::PathModifyNode",
			"graphene_core::vector::vector_data::modification::PathModifyNode",
			"graphene_core::vector::vector_modification::PathModifyNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::point_inside::IDENTIFIER,
		aliases: &["graphene_core::vector::PointInsideNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::points_to_polyline::IDENTIFIER,
		aliases: &["graphene_core::vector::PointsToPolylineNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::scatter_points::IDENTIFIER,
		aliases: &["graphene_core::vector::PoissonDiskPointsNode", "core_types::vector::PoissonDiskPointsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::round_corners::IDENTIFIER,
		aliases: &["graphene_core::vector::RoundCornersNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::sample_polyline::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::SamplePolylineNode",
			"graphene_core::vector::SamplePointsNode",
			"graphene_core::vector::vector_nodes::SamplePointsNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::separate_paths::IDENTIFIER,
		aliases: &["graphene_core::vector::SeparateSubpathsNode", "core_types::vector::SeparateSubpathsNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::solidify_stroke::IDENTIFIER,
		aliases: &["graphene_core::vector::SolidifyStrokeNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::spline::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::vector_nodes::SplineNode",
			"graphene_core::vector::SplinesFromPointsNode",
			"graphene_core::vector::SplineNode",
		],
	},
	NodeReplacement {
		node: graphene_std::vector::stroke::IDENTIFIER,
		aliases: &["graphene_core::vector::StrokeNode"],
	},
	NodeReplacement {
		node: graphene_std::vector::as_vector::IDENTIFIER,
		aliases: &[
			"graphene_core::vector::vector_nodes::PositionToPointNode",
			"graphene_core::vector::PositionToPointNode",
			"graphene_core::vector::Vec2ToPointNode",
			"core_types::vector::Vec2ToPointNode",
		],
	},
];

const REPLACEMENTS: &[(&str, &str)] = &[];

pub fn document_migration_string_preprocessing(document_serialized_content: String) -> String {
	let document_serialized_content = replace_optional_f64_null(&document_serialized_content);

	TEXT_REPLACEMENTS
		.iter()
		.fold(document_serialized_content, |document_serialized_content, (old, new)| document_serialized_content.replace(old, new))
}

/// Rebuilds the old 13-input "Text" node template from the current `text` template plus the trailing `separate_glyphs` input it dropped,
/// so the staged input-count migrations can still upgrade old text nodes before the split.
fn legacy_text_node_template() -> Option<NodeTemplate> {
	let mut template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::text::text::IDENTIFIER))?.default_node_template();
	template.implementation = NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode"));
	template.inputs.push(NodeInput::value(TaggedValue::Bool(false), false));
	template.input_metadata.push(Default::default());
	Some(template)
}

fn replace_optional_f64_null(input: &str) -> String {
	let mut result = String::new();
	let mut last_end = 0;
	let key = "\"OptionalF64\":";

	for (start, _) in input.match_indices(key) {
		let search_start = start + key.len();
		if search_start >= input.len() {
			continue;
		}

		let mut after_key_start = search_start;
		for (i, c) in input[search_start..].char_indices() {
			if !c.is_whitespace() {
				after_key_start = search_start + i;
				break;
			}
			// If we reach the end and it's all whitespace, update after_key_start
			if search_start + i + c.len_utf8() == input.len() {
				after_key_start = input.len();
			}
		}

		if input[after_key_start..].starts_with("null") {
			result.push_str(&input[last_end..start]);
			result.push_str(key);
			result.push_str("0.0");
			last_end = after_key_start + "null".len();
		}
	}

	result.push_str(&input[last_end..]);
	result
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

	// The `source_node_id` proto node was removed in favor of `path_of_subgraph` + `write_attribute`.
	// Documents that still reference it inside their Merge or Artboard layer networks need those layer definitions
	// reset to the current default so the new internal plumbing replaces the obsolete node.
	if document_serialized_content.contains("graphic_nodes::graphic::SourceNodeIdNode")
		|| document_serialized_content.contains("graphene_core::graphic::graphic::SourceNodeIdNode")
		|| document_serialized_content.contains("graphene_core::graphic::SourceNodeIdNode")
	{
		return true;
	}

	// Every Merge layer network is built from the two nodes that became "As Graphic" and "Into Group", so their definitions
	// are reset to pick up the current plumbing instead of the alias migration meant for standalone copies of those nodes.
	if into_group_aliases().any(|alias| document_serialized_content.contains(alias)) {
		return true;
	}

	false
}

pub fn document_migration_replace_resources_referenced_by_hash(document_serialized_content: String) -> (String, HashMap<ResourceHash, ResourceId>) {
	fn collect_resources_referenced_by_hash(s: &str) -> HashMap<ResourceHash, Vec<Range<usize>>> {
		let mut out: HashMap<ResourceHash, Vec<Range<usize>>> = HashMap::new();
		let mut offset = 0;

		while let Some(i) = s[offset..].find("\"Resource\":") {
			let abs = offset + i + 11; // pos after `"Resource":`
			offset = abs;

			if let Some(j) = s[offset..].find('}') {
				let chunk_start = offset;
				let chunk = &s[chunk_start..chunk_start + j];

				let quotes: Vec<_> = chunk.match_indices('"').collect();
				if quotes.len() == 2 {
					let q0 = chunk_start + quotes[0].0;
					let q1 = chunk_start + quotes[1].0;
					let hash = &s[q0 + 1..q1];

					if hash.len() == 64
						&& hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
						&& let Ok(hash) = hash.parse::<ResourceHash>()
					{
						out.entry(hash).or_default().push(q0..q1 + 1);
					}
				}

				offset = chunk_start + j + 1;
			} else {
				break;
			}
		}

		out
	}

	let resources_by_hash = collect_resources_referenced_by_hash(&document_serialized_content);

	// Require each referenced hash to also appear as an embedded resource map key (more than ranges.len() occurrences).
	if resources_by_hash.is_empty()
		|| !resources_by_hash
			.iter()
			.all(|(hash, ranges)| document_serialized_content.matches(&hash.to_string()).count() > ranges.len())
	{
		return (document_serialized_content, HashMap::new());
	}

	// Assign a new ResourceId for each hash
	let mut hash_to_id: HashMap<ResourceHash, ResourceId> = HashMap::new();
	for hash in resources_by_hash.keys() {
		#[allow(clippy::unwrap_or_default)]
		hash_to_id.entry(*hash).or_insert_with(ResourceId::new);
	}

	// Each range is 66 bytes (64 hex + 2 quotes) and a ResourceId serializes to at most 20 ASCII digits, so the ID always fits.
	// Overwrite each hash in place with its ID and pad the leftover bytes with spaces, which JSON deserialization discards.
	let mut bytes = document_serialized_content.into_bytes();
	for (hash, ranges) in &resources_by_hash {
		let id_str = format!("{}", hash_to_id[hash]);
		let id_bytes = id_str.as_bytes();
		for range in ranges {
			bytes[range.start..range.start + id_bytes.len()].copy_from_slice(id_bytes);
			bytes[range.start + id_bytes.len()..range.end].fill(b' ');
		}
	}

	let out = String::from_utf8(bytes).expect("in-place hash-to-ID rewrite produced invalid UTF-8");

	(out, hash_to_id)
}

/// Whether the node is a subgraph that feeds its first input through a Monitor into a Transform proto node and forwards the other four inputs unchanged, making it equivalent to a bare Transform node.
fn is_monitored_transform_subgraph(node: &DocumentNode) -> bool {
	let DocumentNodeImplementation::Network(network) = &node.implementation else { return false };
	if node.inputs.len() < 5 || network.nodes.len() != 2 {
		return false;
	}

	let find_proto_node = |identifier: &ProtoNodeIdentifier| {
		network.nodes.iter().find(|(_, inner_node)| match &inner_node.implementation {
			DocumentNodeImplementation::ProtoNode(inner_identifier) => inner_identifier.as_str().split('<').next() == Some(identifier.as_str()),
			_ => false,
		})
	};
	let Some((monitor_id, monitor)) = find_proto_node(&graphene_std::memo::monitor::IDENTIFIER) else {
		return false;
	};
	let Some((transform_id, transform)) = find_proto_node(&graphene_std::transform_nodes::transform::IDENTIFIER) else {
		return false;
	};

	let is_import_at = |input: &NodeInput, expected_index: usize| matches!(input, NodeInput::Import { import_index, .. } if *import_index == expected_index);

	let monitor_reads_first_import = matches!(monitor.inputs.as_slice(), [input] if is_import_at(input, 0));
	let transform_reads_monitor_and_other_imports = transform.inputs.len() == 5
		&& matches!(&transform.inputs[0], NodeInput::Node { node_id, output_index: 0 } if node_id == monitor_id)
		&& transform.inputs.iter().enumerate().skip(1).all(|(index, input)| is_import_at(input, index));
	let exports_transform = matches!(network.exports.as_slice(), [NodeInput::Node { node_id, output_index: 0 }] if node_id == transform_id);

	monitor_reads_first_import && transform_reads_monitor_and_other_imports && exports_transform
}

/// The CPU node a removed GPU twin was generated from, as named in its era, like `raster_nodes::adjustments::InvertNode` for `raster_nodes::adjustments::invert_shader_node::InvertShaderNodeNode`.
fn cpu_node_of_gpu_twin(identifier: &str) -> Option<String> {
	let identifier = identifier.split('<').next()?;
	let (module_path, struct_name) = identifier.rsplit_once("::")?;
	let (parent_module_path, twin_module) = module_path.rsplit_once("::")?;
	let cpu_struct_name = struct_name.strip_suffix("ShaderNodeNode")?;

	twin_module.ends_with("_shader_node").then(|| format!("{parent_module_path}::{cpu_struct_name}Node"))
}

pub fn document_migration_upgrades(document: &mut DocumentMessageHandler, reset_node_definitions_on_open: bool) {
	document.network_interface.migrate_path_modify_node();
	document.network_interface.migrate_rewired_previews();
	document.network_interface.normalize_stored_types();

	// The removed GPU twins become their CPU nodes minus the trailing executor input, so the renames below carry them forward like their era's CPU nodes
	let gpu_twins = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			let DocumentNodeImplementation::ProtoNode(identifier) = &node.implementation else { return None };
			let kept_input_count = node.inputs.len().saturating_sub(1);
			Some((*node_id, path, cpu_node_of_gpu_twin(identifier.as_str())?, node.inputs[..kept_input_count].to_vec()))
		})
		.collect::<Vec<_>>();
	for (node_id, network_path, cpu_identifier, inputs) in gpu_twins {
		let input_metadata = document
			.network_interface
			.node_metadata(&node_id, &network_path)
			.map(|metadata| metadata.persistent_metadata.input_metadata.iter().take(inputs.len()).cloned().collect())
			.unwrap_or_default();

		let mut node_template = NodeTemplate {
			implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::with_owned_string(cpu_identifier)),
			inputs,
			input_metadata,
			..Default::default()
		};
		document.network_interface.replace_implementation(&node_id, &network_path, &mut node_template);
		document.network_interface.replace_inputs(&node_id, &network_path, &mut node_template);
	}

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
			let node_path_without_type_args = protonode_id.as_str().split('<').next();
			if let Some(new) = node_path_without_type_args.and_then(|node_path| replacements.get(node_path)) {
				let mut default_template = NodeTemplate {
					implementation: NodeTemplateImplementation::ProtoNode(new.clone()),
					..Default::default()
				};
				document.network_interface.replace_implementation(node_id, &network_path, &mut default_template);
				document.network_interface.set_call_argument(node_id, &network_path, default_template.call_argument);

				// 'Mix Colors' takes the under color first, like a 'Merge' takes its stack, where 'Mix' and 'Blend' before it took the over color first
				if *new == graphene_std::raster_nodes::blending_nodes::mix_colors::IDENTIFIER
					&& let [over, under, ..] = node.inputs.as_slice()
				{
					document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), under.clone(), &network_path);
					document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), over.clone(), &network_path);
				}
			}
		}
	}

	// The "Transform" wrapper network was replaced with the `transform` proto node directly. Convert old `Network("Transform")` instances to the proto node, forwarding the 5 user-facing inputs (Value, Translation, Rotation, Scale, Skew) and dropping the legacy migration sentinels (Origin Offset, Scale Appearance) at indices 5 and 6 if present.
	// Some old documents store it as a Custom Node subgraph with no reference, where a Monitor feeds the Transform proto node. Those are converted the same way.
	// Pre-pass for the same reason as the Brush migration above: replacing the outer Transform's network impl orphans its child paths.
	let transform_layers: Vec<(NodeId, Vec<NodeId>, usize)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			let reference = document.network_interface.reference(node_id, &path);
			let is_transform_wrapper = reference == Some(DefinitionIdentifier::Network("Transform".into())) || (reference.is_none() && is_monitored_transform_subgraph(node));

			is_transform_wrapper.then_some((*node_id, path, node.inputs.len()))
		})
		.collect();
	for (node_id, network_path, old_inputs_count) in &transform_layers {
		let new_reference = DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER);
		let Some(definition) = resolve_document_node_type(&new_reference) else { continue };
		let mut node_template = definition.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let Some(old_inputs) = document.network_interface.replace_inputs(node_id, network_path, &mut node_template) else {
			continue;
		};
		// Forward the first 5 inputs (Value, Translation, Rotation, Scale, Skew); drop indices 5 and 6 if present.
		for (index, input) in old_inputs.iter().take(5).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}

		// Pre-2024 documents stored Transform with 6 inputs and used radians for Rotation and `tan(radians)` for Skew. Detect that legacy
		// shape (no input at index 6) and convert the units to degrees so the values match what the new Inspector widgets expect.
		if *old_inputs_count == 6 {
			match old_inputs.get(2) {
				Some(NodeInput::Value { tagged_value, exposed }) => {
					if let TaggedValue::Number(radians) = *tagged_value.clone().into_inner() {
						let degrees = NodeInput::value(TaggedValue::Number(radians.to_degrees()), *exposed);
						document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), degrees, network_path);
					}
				}
				Some(NodeInput::Node { .. }) => {
					// Wired upstream: splice in a Multiply node by 180/π that converts radians to degrees so the upstream value
					// (which represented radians in the legacy format) reaches the now-degrees Rotation input correctly.
					if let Some(multiply_node) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::multiply::IDENTIFIER)) {
						let mut multiply_template = multiply_node.default_node_template();
						multiply_template.inputs[1] = NodeInput::value(TaggedValue::Number(180. / PI), false);
						let multiply_node_id = NodeId::new();
						if let Some(transform_position) = document.network_interface.position(node_id, network_path) {
							let multiply_position = transform_position + IVec2::new(-7, 1);
							document.network_interface.insert_node(multiply_node_id, multiply_template, network_path);
							document.network_interface.shift_absolute_node_position(&multiply_node_id, multiply_position, network_path);
							document
								.network_interface
								.insert_node_between(&multiply_node_id, &InputConnector::node_at_index(*node_id, 2), 0, network_path);
						}
					}
				}
				_ => {}
			}

			if let Some(NodeInput::Value { tagged_value, exposed }) = old_inputs.get(4)
				&& let TaggedValue::DVec2(old_value) = *tagged_value.clone().into_inner()
			{
				// The previous skew value stored `tan(radians)`, now it stores degrees directly.
				let new_value = DVec2::new(old_value.x.atan().to_degrees(), old_value.y.atan().to_degrees());
				let new_input = NodeInput::value(TaggedValue::DVec2(new_value), *exposed);
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), new_input, network_path);
			}
		}
	}

	// The "Image" wrapper network was replaced with the `image` proto node directly. Convert old `Network("Image")` instances to the proto node, forwarding the embedded image data so the `image` pass in `migrate_node` below turns it into a resource.
	// Pre-pass for the same reason as the Brush and Transform migrations above: replacing the outer Image's network impl orphans its child paths.
	let image_layers: Vec<(NodeId, Vec<NodeId>)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, _, path)| (document.network_interface.reference(node_id, &path) == Some(DefinitionIdentifier::Network("Image".into()))).then_some((*node_id, path)))
		.collect();
	for (node_id, network_path) in &image_layers {
		let new_reference = DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER);
		let Some(definition) = resolve_document_node_type(&new_reference) else { continue };
		let mut node_template = definition.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let Some(old_inputs) = document.network_interface.replace_inputs(node_id, network_path, &mut node_template) else {
			continue;
		};

		// Forward the embedded image data into input 0, where the `migrate_node` `image` pass finds it and converts it to a resource.
		if let Some(image_input) = old_inputs.into_iter().find(|input| matches!(input.as_value(), Some(TaggedValue::ImageData(_)))) {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), image_input, network_path);
		}
	}

	// The Merge and Artboard networks no longer pass their content through a Monitor node, since thumbnails are found from the layers themselves.
	// Pre-pass for the same reason as the Brush, Transform, and Image migrations above: rebuilding the network orphans its child paths.
	let monitored_layers: Vec<(NodeId, Vec<NodeId>, DefinitionIdentifier)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			let DocumentNodeImplementation::Network(network) = &node.implementation else { return None };
			let reference = document.network_interface.reference(node_id, &path)?;
			let is_merge_or_artboard = [DefinitionIdentifier::Network("Merge".into()), DefinitionIdentifier::Network("Artboard".into())].contains(&reference);

			let monitor = DocumentNodeImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER);
			let has_monitor = network.nodes.values().any(|inner_node| inner_node.implementation == monitor);

			(is_merge_or_artboard && has_monitor).then_some((*node_id, path, reference))
		})
		.collect();
	for (node_id, network_path, reference) in &monitored_layers {
		let Some(definition) = resolve_document_node_type(reference) else { continue };
		document.network_interface.replace_implementation(node_id, network_path, &mut definition.default_node_template());
	}

	// The old "Math" node evaluated an expression over "A" and "B". A lexable static expression not reading a wired `B` becomes "Math f(x)"
	// with `A` as `x` and a constant `B` inlined. Anything else becomes "Extend" feeding "Math f(…)", which reads the pair as `a` and `b`.
	let math_nodes: Vec<(NodeId, Vec<NodeId>, Vec<NodeInput>)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			let DocumentNodeImplementation::ProtoNode(protonode_id) = &node.implementation else { return None };
			let name = protonode_id.as_str().split('<').next().unwrap_or_default();
			let is_old_math = matches!(name, "math_nodes::MathNode" | "graphene_math_nodes::MathNode" | "graphene_core::ops::MathNode");
			(is_old_math && node.inputs.len() >= 3).then(|| (*node_id, path, node.inputs.clone()))
		})
		.collect();
	for (node_id, network_path, old_inputs) in &math_nodes {
		let (operand_a, expression, operand_b) = (&old_inputs[0], &old_inputs[1], &old_inputs[2]);

		// Lex a static expression once, learning both whether it lexes at all and whether it references `B`
		let static_expression = match expression.as_value() {
			Some(TaggedValue::String(expression)) => Some(expression.clone()),
			_ => None,
		};
		let mut references_b = false;
		let lexable = static_expression
			.as_deref()
			.and_then(|source| {
				math_parser::lexer::rename_identifiers(source, |name| {
					references_b |= name.eq_ignore_ascii_case("b");
					None
				})
			})
			.is_some();

		// The rewritten "Math f(x)" expression, or `None` when only the Extend + "Math f(…)" form can preserve the node's meaning
		let inline_b_constant = match operand_b.as_value() {
			Some(TaggedValue::Number(constant)) => Some(*constant),
			_ => None,
		};
		let fx_expression = match &static_expression {
			Some(source) if lexable && !references_b => math_parser::lexer::rename_identifiers(source, |name| name.eq_ignore_ascii_case("a").then(|| "x".to_string())),
			Some(source) if lexable && let Some(constant) = inline_b_constant => math_parser::lexer::rename_identifiers(source, |name| {
				if name.eq_ignore_ascii_case("a") {
					Some("x".to_string())
				} else if name.eq_ignore_ascii_case("b") {
					Some(format!("({constant:?})"))
				} else {
					None
				}
			}),
			_ => None,
		};

		if let Some(fx_expression) = fx_expression {
			// "Math f(x)": forward the old `A` input and the rewritten expression; the old `B` input is dropped
			let Some(definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::math_fx::IDENTIFIER)) else {
				continue;
			};
			let mut node_template = definition.default_node_template();
			document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
			if document.network_interface.replace_inputs(node_id, network_path, &mut node_template).is_none() {
				continue;
			}

			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), operand_a.clone(), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::String(fx_expression), false), network_path);
		} else {
			// "Extend" joins the two operands into the list "Math f(…)" reads; an unwired constant operand becomes a one-item list value
			let as_list_input = |input: &NodeInput| match input.as_value() {
				Some(TaggedValue::Number(value)) => NodeInput::value(TaggedValue::Numbers(vec![*value]), input.is_exposed()),
				_ => input.clone(),
			};
			let Some(extend_definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::graphic::extend::IDENTIFIER)) else {
				continue;
			};
			let extend_template = extend_definition.default_node_template();

			let Some(definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::math_f::IDENTIFIER)) else {
				continue;
			};
			let mut node_template = definition.default_node_template();
			document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
			if document.network_interface.replace_inputs(node_id, network_path, &mut node_template).is_none() {
				continue;
			}

			// Wire inputs cannot ride along on an inserted template, so the Extend node's operands are set after insertion
			let extend_node_id = NodeId::new();
			let math_position = document.network_interface.position(node_id, network_path);
			document.network_interface.insert_node(extend_node_id, extend_template, network_path);
			if let Some(math_position) = math_position {
				document
					.network_interface
					.shift_absolute_node_position(&extend_node_id, math_position + IVec2::new(-7, 1), network_path);
			}
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(extend_node_id, 0), as_list_input(operand_a), network_path);
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(extend_node_id, 1), as_list_input(operand_b), network_path);

			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, 0), NodeInput::node(extend_node_id, 0), network_path);

			// A static expression is rewritten to the exact-lowercase positional spellings, escaping the old node's
			// constants `e` and `i` behind the `\` prefix that always reaches the builtin; a wired or unlexable one forwards untouched
			let positional_expression = match (&static_expression, lexable) {
				(Some(source), true) => math_parser::lexer::rename_identifiers(source, |name| {
					if name.eq_ignore_ascii_case("a") || name.eq_ignore_ascii_case("b") {
						return Some(name.to_ascii_lowercase());
					}
					matches!(name, "e" | "i").then(|| format!("\\{name}"))
				})
				.map(|rewritten| NodeInput::value(TaggedValue::String(rewritten), false)),
				_ => None,
			};
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, 1), positional_expression.unwrap_or_else(|| expression.clone()), network_path);
		}
	}

	// The "Split Vec2", "Split Vector2", and "Split Channels" wrapper networks became the multi-output `vec2_to_numbers` and `image_to_channels` proto nodes.
	// A wrapper saved before these gained a hidden primary output had each field one output lower, so its wires shift up by one.
	// Pre-pass for the same reason as the Brush, Transform, and Image migrations above: replacing the outer network impl orphans its child paths.
	let split_wrapper_replacements = [
		("Split Vec2", graphene_std::math_nodes::vec_2_to_numbers::IDENTIFIER),
		("Split Vector2", graphene_std::math_nodes::vec_2_to_numbers::IDENTIFIER),
		("Split Channels", graphene_std::raster_nodes::adjustments::image_to_channels::IDENTIFIER),
	];
	for (old_reference, new_identifier) in split_wrapper_replacements {
		let split_nodes: Vec<(NodeId, Vec<NodeId>)> = document
			.network_interface
			.document_network()
			.recursive_nodes()
			.filter_map(|(node_id, _, path)| (document.network_interface.reference(node_id, &path) == Some(DefinitionIdentifier::Network(old_reference.into()))).then_some((*node_id, path)))
			.collect();
		for (node_id, network_path) in &split_nodes {
			let old_output_count = document.network_interface.number_of_outputs(node_id, network_path);
			let output_shift = usize::from(!document.network_interface.hidden_primary_output(node_id, network_path));

			// Pre-load `outward_wires` so the chain-break check inside `set_input` resolves the original upstream→node wire from cache
			// rather than triggering a fresh rebuild from the (already-mutated) post-`replace_inputs` state, which would orphan wires.
			let downstream_by_output: Vec<Vec<InputConnector>> = match document.network_interface.outward_wires(network_path) {
				Some(outward_wires) if output_shift > 0 => (0..old_output_count)
					.map(|output_index| outward_wires.get(&OutputConnector::node(*node_id, output_index)).cloned().unwrap_or_default())
					.collect(),
				_ => Vec::new(),
			};

			let new_reference = DefinitionIdentifier::ProtoNode(new_identifier.clone());
			let Some(definition) = resolve_document_node_type(&new_reference) else { continue };
			let mut node_template = definition.default_node_template();
			let output_names = definition.node_template.output_names.clone();

			document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
			let Some(old_inputs) = document.network_interface.replace_inputs(node_id, network_path, &mut node_template) else {
				continue;
			};
			document.network_interface.set_output_names(node_id, output_names, network_path);
			reset_default_display_name(document, node_id, network_path, old_reference);

			if let Some(input) = old_inputs.first() {
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), input.clone(), network_path);
			}

			for (old_output_index, downstream) in downstream_by_output.iter().enumerate() {
				for input_connector in downstream {
					document
						.network_interface
						.set_input(input_connector, NodeInput::node(*node_id, old_output_index + output_shift), network_path);
				}
			}
		}
	}

	// The standalone "Extract XY" and "Extract Channel" nodes were removed in favor of the multi-output "Vec2 to Numbers" and "Image to Channels" nodes.
	// Convert each to its replacement, moving its downstream wires to the field output for the axis or channel it extracted.
	const EXTRACT_XY: [&str; 2] = ["graphene_core::extract_xy::ExtractXyNode", "graphene_core::ops::ExtractXyNode"];
	const EXTRACT_CHANNEL: [&str; 4] = [
		"raster_nodes::adjustments::ExtractChannelNode",
		"graphene_raster_nodes::adjustments::ExtractChannelNode",
		"graphene_core::raster::adjustments::ExtractChannelNode",
		"graphene_core::raster::ExtractChannelNode",
	];
	let extract_nodes: Vec<(NodeId, Vec<NodeId>, ProtoNodeIdentifier, &str)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			let DocumentNodeImplementation::ProtoNode(protonode_id) = &node.implementation else { return None };
			let name = protonode_id.as_str().split('<').next().unwrap_or_default();
			if EXTRACT_XY.contains(&name) {
				Some((*node_id, path, graphene_std::math_nodes::vec_2_to_numbers::IDENTIFIER, "Extract XY"))
			} else if EXTRACT_CHANNEL.contains(&name) {
				Some((*node_id, path, graphene_std::raster_nodes::adjustments::image_to_channels::IDENTIFIER, "Extract Channel"))
			} else {
				None
			}
		})
		.collect();
	for (node_id, network_path, new_identifier, old_default_name) in &extract_nodes {
		// Capture the old output's downstream connections before mutating, so they can be moved to the extracted field's output
		let downstream_from_output = document
			.network_interface
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0)))
			.cloned()
			.unwrap_or_default();

		let Some(definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(new_identifier.clone())) else {
			continue;
		};
		let mut node_template = definition.default_node_template();
		let output_names = definition.node_template.output_names.clone();

		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let Some(old_inputs) = document.network_interface.replace_inputs(node_id, network_path, &mut node_template) else {
			continue;
		};
		document.network_interface.set_output_names(node_id, output_names, network_path);
		reset_default_display_name(document, node_id, network_path, old_default_name);

		if let Some(input) = old_inputs.first() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), input.clone(), network_path);
		}

		// The field outputs follow the hidden primary output, in X, Y or red, green, blue, alpha order
		let output_index = match old_inputs.get(1).and_then(|input| input.as_value()) {
			Some(TaggedValue::XY(XY::X) | TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Red)) => 1,
			Some(TaggedValue::XY(XY::Y) | TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Green)) => 2,
			Some(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Blue)) => 3,
			Some(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Alpha)) => 4,
			_ => {
				log::warn!("Old \"{old_default_name}\" node {node_id} has no fixed axis or channel to migrate, so its wires use the first field output");
				1
			}
		};
		for input_connector in &downstream_from_output {
			document.network_interface.set_input(input_connector, NodeInput::node(*node_id, output_index), network_path);
		}
	}

	// "Position on Path" and "Tangent on Path" were merged into the multi-output "Evaluate Path" node, with position at output 0 and tangent direction at output 1.
	// Tangent instances get an "Inverse Tangent" node inserted to turn that direction back into the angle their downstream wires expect.
	const POSITION_ON_PATH: &str = "graphene_core::vector::PositionOnPathNode";
	const TANGENT_ON_PATH: &str = "graphene_core::vector::TangentOnPathNode";
	let evaluate_path_nodes: Vec<(NodeId, Vec<NodeId>, bool)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			let DocumentNodeImplementation::ProtoNode(identifier) = &node.implementation else { return None };
			match identifier.as_str() {
				POSITION_ON_PATH => Some((*node_id, path, false)),
				TANGENT_ON_PATH => Some((*node_id, path, true)),
				_ => None,
			}
		})
		.collect();
	for (node_id, network_path, is_tangent) in &evaluate_path_nodes {
		// Capture the old output's downstream connections before mutating, so a tangent node's wires can be moved onto the inserted angle conversion
		let downstream_from_output = document
			.network_interface
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0)))
			.cloned()
			.unwrap_or_default();

		let new_reference = DefinitionIdentifier::ProtoNode(graphene_std::vector::evaluate_path::IDENTIFIER);
		let Some(definition) = resolve_document_node_type(&new_reference) else { continue };
		let mut node_template = definition.default_node_template();
		let output_names = definition.node_template.output_names.clone();

		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let Some(old_inputs) = document.network_interface.replace_inputs(node_id, network_path, &mut node_template) else {
			continue;
		};
		// The old single-output nodes have no output names, so set them to match the new multi-output node's "Position", "Tangent", and "Normal" ports
		document.network_interface.set_output_names(node_id, output_names, network_path);
		reset_default_display_name(document, node_id, network_path, if *is_tangent { "Tangent on Path" } else { "Position on Path" });

		// Forward the shared inputs: content, progression, reverse, and parameterized distance
		for (index, input) in old_inputs.iter().take(4).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}

		if *is_tangent && !downstream_from_output.is_empty() {
			let Some(inverse_tangent_definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::tangent_inverse::IDENTIFIER)) else {
				log::error!("Could not resolve the Inverse Tangent node while migrating Tangent on Path node {node_id}");
				continue;
			};

			// Feed the tangent direction into the inserted "Inverse Tangent" node, carrying over the old radians input (true when absent, since radians was once the only option)
			let inverse_tangent_id = NodeId::new();
			document
				.network_interface
				.insert_node(inverse_tangent_id, inverse_tangent_definition.default_node_template(), network_path);
			let radians = old_inputs.get(4).cloned().unwrap_or_else(|| NodeInput::value(TaggedValue::Bool(true), false));
			document.network_interface.set_input(&InputConnector::node_at_index(inverse_tangent_id, 1), radians, network_path);
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(inverse_tangent_id, 0), NodeInput::node(*node_id, 1), network_path);

			if let Some(position) = document.network_interface.position(node_id, network_path) {
				document.network_interface.shift_absolute_node_position(&inverse_tangent_id, position + IVec2::new(7, 0), network_path);
			}

			// Move the tangent node's downstream connections from its old single output to the inserted angle conversion
			for input_connector in &downstream_from_output {
				document.network_interface.set_input(input_connector, NodeInput::node(inverse_tangent_id, 0), network_path);
			}
		}
	}

	// Record which old text nodes are chain-positioned now, before `migrate_node`'s staged input-count migrations run, since those set
	// the upstream chain to absolute; the split below re-chains exactly the nodes that were originally part of a layer chain.
	let text_nodes_in_chain: std::collections::HashSet<NodeId> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, _, path)| {
			(document.network_interface.reference(node_id, &path) == Some(DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode")))
				&& document.network_interface.is_chain(node_id, &path))
			.then_some(*node_id)
		})
		.collect();

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
	create_layer_stacks(document);
	follow_segments_turned_on_load(document);

	// The old geometry-producing "Text" node was split into the current "Text" (`String[]`) -> converter pair, which reuses the same proto
	// identifier. Runs after `migrate_node` normalizes old text nodes to the legacy 13-input layout, distinguished from the current 12-input
	// node by the trailing `separate_glyphs` input (index 12): forward inputs 0..=11 onto the new node and splice the matching converter after it.
	let old_text_nodes: Vec<(NodeId, Vec<NodeId>)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.filter_map(|(node_id, node, path)| {
			// `separate_glyphs` is a `Bool` value or a wire feeding one; only a different value type there means a newer input, not the old node
			let has_legacy_separate_glyphs = node.inputs.len() == 13 && node.inputs.get(12).is_some_and(|input| matches!(input.as_value(), None | Some(TaggedValue::Bool(_))));
			(has_legacy_separate_glyphs && document.network_interface.reference(node_id, &path) == Some(DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode"))))
				.then_some((*node_id, path))
		})
		.collect();
	for (node_id, network_path) in &old_text_nodes {
		// Convert the old node in place to the current `text` node (12 inputs), capturing its old inputs.
		let Some(text_definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::text::text::IDENTIFIER)) else {
			continue;
		};
		let mut text_template = text_definition.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut text_template);
		let Some(old_inputs) = document.network_interface.replace_inputs(node_id, network_path, &mut text_template) else {
			continue;
		};
		// The current `text` node reorders the legacy inputs (Letter Tilt moved up to sit right after Letter Spacing), so map each new
		// input index to the legacy 13-input index it sources from. Legacy order:
		// [primary, text, font, size, line_height, letter_spacing, has_max_width, max_width, has_max_height, max_height, letter_tilt, align, separate_glyphs].
		const LEGACY_INPUT_FOR_NEW: [usize; 12] = [0, 1, 2, 3, 4, 5, 10, 6, 7, 8, 9, 11];
		for (new_index, &legacy_index) in LEGACY_INPUT_FOR_NEW.iter().enumerate() {
			if let Some(input) = old_inputs.get(legacy_index) {
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, new_index), input.clone(), network_path);
			}
		}
		// A `true` toggle at index 12 chose per-glyph geometry, which is now the dedicated "Text to Vector Glyphs" node
		let separate_glyphs = matches!(old_inputs.get(12).and_then(|input| input.as_value()), Some(TaggedValue::Bool(true)));

		// Collect the inputs reading the old text node's output before any rewiring so the new node can be spliced onto those wires.
		let downstream_consumers: Vec<InputConnector> = document
			.network_interface
			.outward_wires(network_path)
			.and_then(|wires| wires.get(&OutputConnector::node(*node_id, 0)))
			.cloned()
			.unwrap_or_default();

		let text_was_in_chain = text_nodes_in_chain.contains(node_id);

		// Insert the converter that turns the `text` `String[]` output back into vector geometry: "Text to Vector Glyphs" for the per-glyph case, otherwise "Text to Vector".
		let converter_identifier = if separate_glyphs {
			graphene_std::text::text_to_vector_glyphs::IDENTIFIER
		} else {
			graphene_std::text::text_to_vector::IDENTIFIER
		};
		let Some(converter_definition) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(converter_identifier)) else {
			continue;
		};
		let converter_id = NodeId::new();
		document.network_interface.insert_node(converter_id, converter_definition.default_node_template(), network_path);

		// Splice the converter onto the wire(s) leaving `text` (`insert_node_between` is the pure wire-splice the editor uses for dropping a node on a wire).
		if let Some((first_consumer, remaining_consumers)) = downstream_consumers.split_first() {
			document.network_interface.insert_node_between(&converter_id, first_consumer, 0, network_path);
			for consumer in remaining_consumers {
				document.network_interface.set_input(consumer, NodeInput::node(converter_id, 0), network_path);
			}
		} else {
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(converter_id, 0), NodeInput::node(*node_id, 0), network_path);
		}

		// If `text` was in a layer chain, re-chain the converter and its upstream so both lay out by distance from the layer (the splice
		// broke the chain, like `move_node_to_chain_start`). Otherwise `text` is absolute, so place the converter beside it instead of
		// leaving it at the origin.
		if text_was_in_chain {
			document.network_interface.force_set_upstream_to_chain(&converter_id, network_path);
		} else if let Some(text_position) = document.network_interface.position(node_id, network_path) {
			document.network_interface.shift_absolute_node_position(&converter_id, text_position + IVec2::new(7, 0), network_path);
		}
	}

	// Parameters that historically accepted fractional numbers are now integers (the polygon and star "Sides", the index,
	// shift amount, and slice bound parameters of the list, string, and regex nodes, the "Path is Closed" index, and the
	// "Blend" count), so a stored F64 truncates as those slots' own casts and Floor node did, preserving what documents drew
	let narrowed_integer_inputs: &[(DefinitionIdentifier, &[usize])] = &[
		(DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::regular_polygon::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::star::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::vector::index_points::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::graphic::item_at_index::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::graphic::remove_at_index::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::graphic::shift::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::graphic::indices::IDENTIFIER), &[1]),
		(DefinitionIdentifier::ProtoNode(graphene_std::graphic::slice::IDENTIFIER), &[1, 2]),
		(DefinitionIdentifier::ProtoNode(graphene_std::text_nodes::string_slice::IDENTIFIER), &[1, 2]),
		(DefinitionIdentifier::ProtoNode(graphene_std::text_nodes::regex::regex_find::IDENTIFIER), &[2]),
		(DefinitionIdentifier::Network("Regex Find".into()), &[2]),
		(DefinitionIdentifier::ProtoNode(graphene_std::vector::path_is_closed::IDENTIFIER), &[1]),
		(DefinitionIdentifier::Network("Blend".into()), &[1]),
	];
	let fractional_value_inputs: Vec<(NodeId, Vec<NodeId>, usize, i64, bool)> = document
		.network_interface
		.document_network()
		.recursive_nodes()
		.flat_map(|(node_id, node, path)| {
			let Some(reference) = document.network_interface.reference(node_id, &path) else {
				return Vec::new();
			};
			let Some((_, indices)) = narrowed_integer_inputs.iter().find(|(identifier, _)| reference == *identifier) else {
				return Vec::new();
			};

			indices
				.iter()
				.filter_map(|&index| {
					let NodeInput::Value { tagged_value, exposed } = node.inputs.get(index)? else { return None };
					let TaggedValue::Number(value) = &**tagged_value else { return None };
					Some((*node_id, path.clone(), index, *value as i64, *exposed))
				})
				.collect::<Vec<_>>()
		})
		.collect();
	for (node_id, network_path, index, integer, exposed) in fractional_value_inputs {
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(node_id, index), NodeInput::value(TaggedValue::Integer(integer), exposed), &network_path);
	}

	// Bringing an old document up to date is not a set of edits on it, it is how the document arrives.
	// The upgrades run through the ordinary mutators, so the store records them; dropping what they
	// recorded leaves the opened document as the baseline rather than as a pile of changes to itself.
	document.network_interface.discard_deltas();
	document.require_whole_document_stage();
}

/// Converts a legacy stroke dash input (a `List<f64>`, single `f64`, or comma/space separated `String`) to the `DashPattern` value type.
fn migrate_dash_input(input: &NodeInput) -> Option<NodeInput> {
	let NodeInput::Value { tagged_value, exposed } = input else { return None };
	let lengths = match &*tagged_value.clone().into_inner() {
		TaggedValue::Numbers(lengths) => lengths.clone(),
		TaggedValue::Number(length) => vec![*length],
		TaggedValue::String(text) => graphene_std::core_types::misc::parse_f64_list(text),
		_ => return None,
	};
	Some(NodeInput::value(TaggedValue::DashPattern(lengths), *exposed))
}

/// Converts a legacy rectangle corner radius input (a single `f64` or a `List<f64>` of up to four values) to the `BoxCorners` value type.
fn migrate_corner_radius_input(input: &NodeInput) -> Option<NodeInput> {
	let NodeInput::Value { tagged_value, exposed } = input else { return None };
	let values = match &*tagged_value.clone().into_inner() {
		TaggedValue::Numbers(values) => values.clone(),
		TaggedValue::Number(value) => vec![*value],
		_ => return None,
	};
	Some(NodeInput::value(TaggedValue::BoxCorners(values), *exposed))
}

/// Rewrites a gradient ramp value input to carry the given gradient spread, which used to live in the Fill node's retired `_spread_method` input.
fn fold_gradient_spread_into_ramp_input(input: &NodeInput, gradient_spread: GradientSpread) -> NodeInput {
	match input.as_value() {
		Some(TaggedValue::GradientRamp(ramp)) => NodeInput::value(TaggedValue::GradientRamp(GradientRamp { gradient_spread, ..ramp.clone() }), input.is_exposed()),
		_ => input.clone(),
	}
}

fn migrate_node(node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId], document: &mut DocumentMessageHandler, reset_node_definitions_on_open: bool) -> Option<()> {
	// Must run before the reset block below: a node referencing a removed catalog entry would otherwise abort
	// `migrate_node` via the `?` on `resolve_document_node_type`, preventing subsequent migration blocks from running.
	migrate_removed_catalog_definitions(node_id, node, network_path, document);

	if reset_node_definitions_on_open && let Some(reference) = document.network_interface.reference(node_id, network_path) {
		let node_definition = resolve_document_node_type(&reference)?;
		document.network_interface.replace_implementation(node_id, network_path, &mut node_definition.default_node_template());
	}

	// Rebuild stale Merge/Artboard subgraphs that still use the removed LegacyLayerExtendNode internally
	if let DocumentNodeImplementation::Network(inner) = &node.implementation
		&& inner
			.nodes
			.values()
			.any(|n| matches!(&n.implementation, DocumentNodeImplementation::ProtoNode(id) if id.as_str().contains("LegacyLayerExtend") || id.as_str().contains("legacy_layer_extend")))
		&& let Some(reference) = document.network_interface.reference(node_id, network_path)
		&& let Some(node_definition) = resolve_document_node_type(&reference)
	{
		document.network_interface.replace_implementation(node_id, network_path, &mut node_definition.default_node_template());
	}

	// Rebuild the Blend subgraph that still floors its Count import, since the now-integer Count enters through As Number instead
	if let DocumentNodeImplementation::Network(inner) = &node.implementation
		&& document.network_interface.reference(node_id, network_path) == Some(DefinitionIdentifier::Network("Blend".into()))
		&& inner.nodes.values().any(|n| {
			matches!(&n.implementation, DocumentNodeImplementation::ProtoNode(identifier) if *identifier == graphene_std::math_nodes::floor::IDENTIFIER)
				&& n.inputs.iter().any(|input| matches!(input, NodeInput::Import { import_index: 1, .. }))
		}) && let Some(node_definition) = resolve_network_node_type("Blend")
	{
		document.network_interface.replace_implementation(node_id, network_path, &mut node_definition.default_node_template());
	}

	// Rebuild the Regex Find subgraph that still imports its Match Index as a decimal, since that index is now an integer
	if let DocumentNodeImplementation::Network(inner) = &node.implementation
		&& document.network_interface.reference(node_id, network_path) == Some(DefinitionIdentifier::Network("Regex Find".into()))
		&& inner.nodes.values().any(|n| {
			n.inputs
				.iter()
				.any(|input| matches!(input, NodeInput::Import { import_type, import_index: 2 } if *import_type == item!(f64)))
		}) && let Some(node_definition) = resolve_network_node_type("Regex Find")
	{
		document.network_interface.replace_implementation(node_id, network_path, &mut node_definition.default_node_template());
	}

	// Upgrade old nodes to use `Context` instead of `()` or `Footprint` as their call argument
	if node.call_argument == graph_craft::concrete!(()) || node.call_argument == graph_craft::concrete!(graphene_std::transform::Footprint) {
		document.network_interface.set_call_argument(node_id, network_path, graph_craft::concrete!(graphene_std::Context));
	}

	// Only nodes that have not been modified and still refer to a definition can be updated
	let reference = document.network_interface.reference(node_id, network_path)?;

	// Show the Artboard's Content input in the Inspector, which the definition once hid
	if reference == DefinitionIdentifier::Network("Artboard".into())
		&& document
			.network_interface
			.persistent_input_metadata(node_id, 1, network_path)
			.and_then(|metadata| metadata.widget_override.as_deref())
			== Some("hidden")
	{
		document.network_interface.set_input_override(node_id, 1, None, network_path);
	}

	// Legacy `noise_properties_*` widget overrides
	for input_index in 0..node.inputs.len() {
		let renamed_override = document
			.network_interface
			.persistent_input_metadata(node_id, input_index, network_path)
			.and_then(|metadata| metadata.widget_override.as_deref())
			.and_then(|widget_override| widget_override.strip_prefix("noise_properties_"))
			.map(|suffix| format!("noise_inspector_{suffix}"));

		if let Some(renamed_override) = renamed_override {
			document.network_interface.set_input_override(node_id, input_index, Some(renamed_override), network_path);
		}
	}

	let mut inputs_count = node.inputs.len();

	// The expression nodes gained an output-type input, a type witness the Inspector edits. A node from before it
	// takes the definition's default choice, Number, which is the output type every such node had.
	let expression_node_identifiers = [graphene_std::math_nodes::math_fx::IDENTIFIER, graphene_std::math_nodes::math_f::IDENTIFIER];
	if expression_node_identifiers.iter().any(|identifier| reference == DefinitionIdentifier::ProtoNode(identifier.clone())) && inputs_count == 2 {
		let definition = resolve_document_node_type(&reference)?;
		let mut template = definition.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut template)?;
		for (index, input) in old_inputs.into_iter().enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}
		inputs_count = 3;
	}

	// Split the legacy combined "Blending" node into a chain of separate Blend Mode, Opacity (now also covers fill), and Clip nodes.
	// `NODE_REPLACEMENTS` rewrites the old `Blending` proto identifier (and its older aliases) to `blend_mode::IDENTIFIER`, so a leftover
	// 5-input shape on a Blend Mode node identifies an old Blending node that still needs structural splitting. Sub-nodes whose values
	// were at the default and weren't exposed/wired up are skipped, and the existing node ID is reused for the first kept sub-node so
	// any references elsewhere in the document survive. If everything would be skipped, a no-op default Blend Mode node is left behind
	// to preserve the chain structure (the user can delete it manually).
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::blending_nodes::blend_mode::IDENTIFIER) && inputs_count == 5 {
		use graphene_std::blending::BlendMode;

		// Snapshot the old inputs before any rewriting
		let old_inputs = node.inputs.clone();
		let content_input = old_inputs[0].clone();
		let blend_mode_input = old_inputs[1].clone();
		let opacity_input = old_inputs[2].clone();
		let fill_input = old_inputs[3].clone();
		let clip_input = old_inputs[4].clone();

		// A sub-node is kept if its input is exposed/wired (so the user can drive it from the graph) OR if its value differs from the default
		let keep_blend_mode = blend_mode_input.is_exposed() || !matches!(blend_mode_input.as_value(), Some(TaggedValue::BlendMode(BlendMode::Normal)));
		let keep_opacity = opacity_input.is_exposed() || !matches!(opacity_input.as_value(), Some(TaggedValue::Number(v)) if (*v - 100.).abs() < f64::EPSILON);
		let keep_fill = fill_input.is_exposed() || !matches!(fill_input.as_value(), Some(TaggedValue::Number(v)) if (*v - 100.).abs() < f64::EPSILON);
		let keep_clip = clip_input.is_exposed() || !matches!(clip_input.as_value(), Some(TaggedValue::Bool(false)));

		// Find the downstream connection so we can chain new nodes between this node and downstream
		let downstream = document
			.network_interface
			.outward_wires(network_path)
			.and_then(|wires| wires.get(&OutputConnector::node(*node_id, 0)))
			.and_then(|connections| connections.first().cloned());

		// The position of the existing node; subsequent chain nodes are placed to the right
		let base_position = document.network_interface.position(node_id, network_path).unwrap_or_default();

		// Decide which sub-node the existing ID becomes (the first one in the chain order: Blend Mode, Opacity, Clip)
		#[derive(Clone, Copy, PartialEq)]
		enum SubNode {
			BlendMode,
			Opacity,
			Clip,
		}
		let first_kind = if keep_blend_mode {
			SubNode::BlendMode
		} else if keep_opacity || keep_fill {
			SubNode::Opacity
		} else if keep_clip {
			SubNode::Clip
		} else {
			// Everything was at default and not exposed: leave a no-op Blend Mode node so the chain structure is preserved
			SubNode::BlendMode
		};

		let identifier_for = |kind: SubNode| match kind {
			SubNode::BlendMode => graphene_std::blending_nodes::blend_mode::IDENTIFIER,
			SubNode::Opacity => graphene_std::blending_nodes::opacity::IDENTIFIER,
			SubNode::Clip => graphene_std::blending_nodes::clipping_mask::IDENTIFIER,
		};

		// Replace the existing node's implementation with the first kept sub-node's template
		let first_reference = DefinitionIdentifier::ProtoNode(identifier_for(first_kind));
		let mut first_template = resolve_document_node_type(&first_reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut first_template);
		let _ = document.network_interface.replace_inputs(node_id, network_path, &mut first_template);

		// Wire the existing node's inputs based on its new role (input 0 is always `content`)
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), content_input, network_path);
		match first_kind {
			SubNode::BlendMode => {
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 1), blend_mode_input.clone(), network_path);
			}
			SubNode::Opacity => {
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Bool(keep_opacity), false), network_path);
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), opacity_input.clone(), network_path);
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 3), NodeInput::value(TaggedValue::Bool(keep_fill), false), network_path);
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), fill_input.clone(), network_path);
			}
			SubNode::Clip => {
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), clip_input.clone(), network_path);
			}
		}

		// Build the list of remaining sub-nodes to insert (after the first one), in chain order. Blend Mode is never
		// reinserted because if it was kept it would already be the `first_kind`.
		let mut remaining: Vec<SubNode> = Vec::new();
		if first_kind != SubNode::Opacity && (keep_opacity || keep_fill) {
			remaining.push(SubNode::Opacity);
		}
		if first_kind != SubNode::Clip && keep_clip {
			remaining.push(SubNode::Clip);
		}

		// Insert each remaining sub-node into the chain between the previous node and the original downstream
		let mut chain_x_offset = 7;
		for sub in remaining {
			let identifier = identifier_for(sub);
			let new_id = NodeId::new();
			let definition = match resolve_document_node_type(&DefinitionIdentifier::ProtoNode(identifier.clone())) {
				Some(definition) => definition,
				None => {
					log::error!("Could not resolve `{identifier:?}` while migrating Blending node");
					continue;
				}
			};
			let template = definition.default_node_template();
			document.network_interface.insert_node(new_id, template, network_path);
			document
				.network_interface
				.shift_absolute_node_position(&new_id, base_position + IVec2::new(chain_x_offset, 0), network_path);
			chain_x_offset += 7;

			// Splice this node in just before the original downstream input (so each iteration appends to the chain end)
			if let Some(downstream_input) = &downstream {
				document.network_interface.insert_node_between(&new_id, downstream_input, 0, network_path);
			}

			// Wire the parameter inputs for the inserted node
			match sub {
				SubNode::BlendMode => {
					document.network_interface.set_input(&InputConnector::node_at_index(new_id, 1), blend_mode_input.clone(), network_path);
				}
				SubNode::Opacity => {
					document
						.network_interface
						.set_input(&InputConnector::node_at_index(new_id, 1), NodeInput::value(TaggedValue::Bool(keep_opacity), false), network_path);
					document.network_interface.set_input(&InputConnector::node_at_index(new_id, 2), opacity_input.clone(), network_path);
					document
						.network_interface
						.set_input(&InputConnector::node_at_index(new_id, 3), NodeInput::value(TaggedValue::Bool(keep_fill), false), network_path);
					document.network_interface.set_input(&InputConnector::node_at_index(new_id, 4), fill_input.clone(), network_path);
				}
				SubNode::Clip => {
					document.network_interface.set_input(&InputConnector::node_at_index(new_id, 1), clip_input.clone(), network_path);
				}
			}
		}

		inputs_count = match first_kind {
			SubNode::BlendMode => 2,
			SubNode::Opacity => 5,
			SubNode::Clip => 2,
		};
	}

	// Upgrade old single-purpose Opacity node (content, opacity) to the new shape that also covers fill:
	// (content, has_opacity, opacity, has_fill, fill). The original opacity input is preserved with `has_opacity` enabled,
	// while `has_fill` is left disabled and the fill value defaults to 100% (no change to fill behavior).
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::blending_nodes::opacity::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Bool(true), false), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[1].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 3), NodeInput::value(TaggedValue::Bool(false), false), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 4), NodeInput::value(TaggedValue::Number(100.), false), network_path);

		inputs_count = 5;
	}

	// Upgrade the legacy 4-input Fill node (content, fill: Fill, _backup_color, _backup_gradient: Gradient) to the value-model
	// 7-input shape (content, fill: generic paint list, _backup_color, _backup_gradient, _gradient_form, _has_transform, _transform).
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::fill::IDENTIFIER) && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// Content: no change
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);

		// Fill: a literal Fill value is decomposed, and a wired input (`List<Gradient> / List<Color>`) is kept as-is
		match old_inputs[1].as_value() {
			Some(TaggedValue::LegacyFill(old_fill)) => {
				let exposed = old_inputs[1].is_exposed();
				let fill_value = match old_fill {
					graphic_types::migrations::legacy::LegacyFill::None => TaggedValue::no_paint(),
					graphic_types::migrations::legacy::LegacyFill::Solid(color) => TaggedValue::Color(*color),
					graphic_types::migrations::legacy::LegacyFill::Gradient(gradient) => TaggedValue::GradientRamp(GradientRamp {
						gradient_spread: gradient.spread_method,
						..gradient.stops.clone()
					}),
				};
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(fill_value, exposed), network_path);

				// Gradient metadata (4, 5, 6): applies only to a literal gradient, solids/none keep the template defaults
				if let graphic_types::migrations::legacy::LegacyFill::Gradient(gradient) = old_fill {
					document.network_interface.set_input(
						&InputConnector::node_at_index(*node_id, 4),
						NodeInput::value(TaggedValue::GradientForm(gradient.gradient_type), false),
						network_path,
					);

					if gradient.absolute {
						let transform = gradient.transform * gradient.to_transform();
						document
							.network_interface
							.set_input(&InputConnector::node_at_index(*node_id, 5), NodeInput::value(TaggedValue::Bool(true), false), network_path);
						document
							.network_interface
							.set_input(&InputConnector::node_at_index(*node_id, 6), NodeInput::value(TaggedValue::DAffine2(transform), false), network_path);
					} else {
						// Baking a legacy bounding-box-relative gradient is deferred until the measurement pre-pass can supply the paint
						// target's bounds, so the template's unbaked `_has_transform = false` stands until the bake lands
						document.pending_gradient_bbox_bake.push((network_path.to_vec(), *node_id, gradient.clone()));
					}
				}
			}
			// Wired/exposed fill keeps the connection.
			// The generic paint connector accepts the existing `List<Color>`/`List<Gradient>` paint sources directly.
			_ => {
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
			}
		}

		// Color backup: no change
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[2].clone(), network_path);

		// Gradient backup: extract stops
		if let Some(TaggedValue::LegacyGradient(g)) = old_inputs[3].as_value() {
			document.network_interface.set_input(
				&InputConnector::node_at_index(*node_id, 3),
				NodeInput::value(
					TaggedValue::GradientRamp(GradientRamp {
						gradient_spread: g.spread_method,
						..g.stops.clone()
					}),
					false,
				),
				network_path,
			);

			// A solid/no-fill node leaves the gradient metadata inputs unused, so seed them from the backup gradient for a later Solid -> Gradient toggle to restore
			if matches!(
				old_inputs[1].as_value(),
				Some(TaggedValue::LegacyFill(
					graphic_types::migrations::legacy::LegacyFill::None | graphic_types::migrations::legacy::LegacyFill::Solid(_)
				))
			) {
				document.network_interface.set_input(
					&InputConnector::node_at_index(*node_id, 4),
					NodeInput::value(TaggedValue::GradientForm(g.gradient_type), false),
					network_path,
				);

				if g.absolute {
					let transform = g.transform * g.to_transform();
					document
						.network_interface
						.set_input(&InputConnector::node_at_index(*node_id, 5), NodeInput::value(TaggedValue::Bool(true), false), network_path);
					document
						.network_interface
						.set_input(&InputConnector::node_at_index(*node_id, 6), NodeInput::value(TaggedValue::DAffine2(transform), false), network_path);
				} else {
					document.pending_gradient_bbox_bake.push((network_path.to_vec(), *node_id, g.clone()));
				}
			}
		}

		inputs_count = 7;
	}

	// Fill split its `Option<DAffine2>` placement into a `_has_transform` bool immediately before the `_transform` matrix. The modern
	// shape is also 7 inputs, so this era is identified by its `_spread_method` input at 5 or its optional transform at 6.
	let is_pre_transform_split_fill = inputs_count == 7
		&& (matches!(node.inputs.get(5).and_then(|input| input.as_value()), Some(TaggedValue::GradientSpread(_)))
			|| matches!(node.inputs.get(6).and_then(|input| input.as_value()), Some(TaggedValue::LegacyOptionalDAffine2(_))));
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER) && is_pre_transform_split_fill {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let gradient_spread = match old_inputs.get(5).and_then(|input| input.as_value()) {
			Some(&TaggedValue::GradientSpread(value)) => value,
			_ => GradientSpread::default(),
		};

		for (index, input) in old_inputs.iter().enumerate().take(5) {
			let input = if index == 1 || index == 3 {
				fold_gradient_spread_into_ramp_input(input, gradient_spread)
			} else {
				input.clone()
			};
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}

		match old_inputs.get(6).and_then(|input| input.as_value()) {
			Some(TaggedValue::LegacyOptionalDAffine2(value)) => {
				let has_transform = value.is_some();
				let transform = value.unwrap_or(glam::DAffine2::IDENTITY);
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 5), NodeInput::value(TaggedValue::Bool(has_transform), false), network_path);
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 6), NodeInput::value(TaggedValue::DAffine2(transform), false), network_path);
			}
			// A wired (or otherwise non-value) transform keeps its connection and is treated as present
			_ => {
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, 5), NodeInput::value(TaggedValue::Bool(true), false), network_path);
				let transform_input = old_inputs.get(6).cloned().unwrap_or_else(|| NodeInput::value(TaggedValue::DAffine2(glam::DAffine2::IDENTITY), false));
				document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 6), transform_input, network_path);
			}
		}

		inputs_count = 7;
	}

	// The Fill node's `_spread_method` input moved into the `GradientRamp` value's own `gradient_spread` field
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER) && inputs_count == 8 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let gradient_spread = match old_inputs.get(5).and_then(|input| input.as_value()) {
			Some(&TaggedValue::GradientSpread(value)) => value,
			_ => GradientSpread::default(),
		};

		for (index, input) in old_inputs.iter().enumerate().take(5) {
			let input = if index == 1 || index == 3 {
				fold_gradient_spread_into_ramp_input(input, gradient_spread)
			} else {
				input.clone()
			};
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 5), old_inputs[6].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 6), old_inputs[7].clone(), network_path);

		inputs_count = 7;
	}

	// Upgrade Stroke node to reorder parameters and add "Align" (#2644)
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER) && inputs_count == 8 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let align_input = NodeInput::value(TaggedValue::StrokeAlign(StrokeAlign::Center), false);

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 3), align_input, network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), old_inputs[5].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 5), old_inputs[6].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 6), old_inputs[7].clone(), network_path);
		let dash_input = migrate_dash_input(&old_inputs[3]).unwrap_or_else(|| old_inputs[3].clone());
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 7), dash_input, network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 8), old_inputs[4].clone(), network_path);

		inputs_count = 9;
	}

	// The Stroke node's "Paint Order" input was retired in favor of the relative order of the Fill and Stroke
	// nodes in the chain, so the stored value becomes a topology rewrite that reorders the two nodes.
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER) && inputs_count == 10 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// A wired paint order input cannot be evaluated statically, so it degrades to the default and leaves its source disconnected
		let paint_order = match old_inputs.get(7).and_then(|input| input.as_value()) {
			Some(&TaggedValue::PaintOrder(value)) => value,
			_ => PaintOrder::StrokeAbove,
		};

		for (index, input) in old_inputs.iter().enumerate().take(7) {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		let dash_input = migrate_dash_input(&old_inputs[8]).unwrap_or_else(|| old_inputs[8].clone());
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 7), dash_input, network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 8), old_inputs[9].clone(), network_path);
		inputs_count = 9;

		set_stroke_paint_order(&mut document.network_interface, network_path, *node_id, paint_order);
	}

	// A legacy "no color" on a plain color connector (`TaggedValue::no_paint()` restored by the deserializer) becomes a color,
	// since only paint connectors keep the no-paint choice
	{
		let migrate_color_input = |input: &NodeInput, fallback: Color| -> Option<NodeInput> {
			let NodeInput::Value { tagged_value, exposed } = input else { return None };
			if !tagged_value.is_no_paint() {
				return None;
			}
			Some(NodeInput::value(TaggedValue::Color(fallback), *exposed))
		};

		let conversions: &[(ParameterRef, Color)] = &[
			(graphene_std::vector::fill::BackupColorInput.into(), Color::BLACK),
			(graphene_std::artboard::create_artboard::BackgroundInput.into(), Color::WHITE),
			(graphene_std::math_nodes::color_value::ColorInput.into(), Color::TRANSPARENT),
			(graphene_std::raster_nodes::adjustments::black_and_white::TintInput.into(), Color::BLACK),
			(graphene_std::raster_nodes::blending_nodes::color_overlay::ColorInput.into(), Color::BLACK),
		];
		for (parameter, fallback) in conversions {
			if reference != DefinitionIdentifier::ProtoNode(parameter.node_identifier.clone()) {
				continue;
			}
			let Some(input) = node.inputs.get(parameter.input_index) else { continue };
			if let Some(migrated) = migrate_color_input(input, *fallback) {
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, parameter.input_index), migrated, network_path);
			}
		}
	}

	// The stroke dash sequence became the `DashPattern` value type; convert any already-shaped stroke that still stores a legacy dash input
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER)
		&& let Some(dash_input) = node.input(graphene_std::vector::stroke::DashPatternInput)
		&& let Some(migrated) = migrate_dash_input(dash_input)
	{
		document
			.network_interface
			.set_input(&InputConnector::node(*node_id, graphene_std::vector::stroke::DashPatternInput), migrated, network_path);
	}

	// The stored no-paint sentinel was the `List<Graphic>` type default before the paint connectors ranked down to `Item<Graphic>`.
	// This must run before the stale-List-default cleanup below, which would otherwise adopt the definition's default paint.
	{
		let legacy_no_paint = TaggedValue::TypeDefault(list!(graphene_std::Graphic));
		let paint_parameters: &[ParameterRef] = &[graphene_std::vector::fill::PaintInput.into(), graphene_std::vector::stroke::PaintInput.into()];
		for parameter in paint_parameters {
			if reference != DefinitionIdentifier::ProtoNode(parameter.node_identifier.clone()) {
				continue;
			}
			let Some(NodeInput::Value { tagged_value, exposed }) = node.inputs.get(parameter.input_index) else {
				continue;
			};
			if **tagged_value == legacy_no_paint {
				document.network_interface.set_input(
					&InputConnector::node_at_index(*node_id, parameter.input_index),
					NodeInput::value(TaggedValue::no_paint(), *exposed),
					network_path,
				);
			}
		}
	}

	// The corner radius became the `BoxCorners` value type; convert any already-shaped rectangle that still stores a legacy corner input
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
		&& let Some(corner_input) = node.input(graphene_std::vector::generator_nodes::rectangle::CornerRadiusInput)
		&& let Some(migrated) = migrate_corner_radius_input(corner_input)
	{
		document.network_interface.set_input(
			&InputConnector::node(*node_id, graphene_std::vector::generator_nodes::rectangle::CornerRadiusInput),
			migrated,
			network_path,
		);
	}

	// The rectangle's corner radius became the `BoxCorners` value type and its hidden individual-radii toggle moved after the
	// user-visible inputs. A legacy rectangle stores that toggle (a plain `bool`) at index 3, where the new shape stores the corner
	// radius, so a `bool` value there identifies the old input order: [width, height, individual, corner_radius, clamped].
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
		&& let Some(toggle_input) = node.inputs.get(3)
		&& matches!(toggle_input.as_value(), Some(TaggedValue::Bool(_)))
	{
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let corner_radius = migrate_corner_radius_input(&old_inputs[4]).unwrap_or_else(|| old_inputs[4].clone());

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 3), corner_radius, network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), old_inputs[5].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 5), old_inputs[3].clone(), network_path);
	}

	// The Text to Vector node's runtime `separate_glyphs` toggle became the dedicated "Text to Vector Glyphs" node, leaving Text to Vector as a plain
	// string-to-compound-path converter. A 2-input Text to Vector is the old toggled shape: a `true` toggle routes to Text to Vector Glyphs, otherwise
	// the node stays Text to Vector; either way the toggle input is dropped and the string wire is preserved.
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::text::text_to_vector::IDENTIFIER) && inputs_count == 2 {
		let separate_glyphs = matches!(node.inputs.get(1).and_then(|input| input.as_value()), Some(TaggedValue::Bool(true)));
		let target = if separate_glyphs {
			graphene_std::text::text_to_vector_glyphs::IDENTIFIER
		} else {
			graphene_std::text::text_to_vector::IDENTIFIER
		};

		let mut node_template = resolve_proto_node_type(target)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		if let Some(string_input) = node.inputs.first() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), string_input.clone(), network_path);
		}
	}

	// Every Text node era before alignment only appended inputs, so each is a prefix of the 11-input layout.
	// Alignment (#2920) is the exception: it landed at index 9 and pushed Per-Glyph Instances out to 10.
	if reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode")) && (4..=10).contains(&inputs_count) {
		let mut template: NodeTemplate = legacy_text_node_template()?;
		document.network_interface.replace_implementation(node_id, network_path, &mut template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut template)?;

		// Line height and character spacing were hardcoded to 1 in the era before they became inputs
		let hardcoded_to_one = || NodeInput::value(TaggedValue::Number(1.), false);
		// Zero is how an absent `Option<f64>` maximum reads to the split below
		let unset_maximum = || NodeInput::value(TaggedValue::Number(0.), false);

		let upgraded_inputs = [
			old_inputs[0].clone(),
			old_inputs[1].clone(),
			old_inputs[2].clone(),
			old_inputs[3].clone(),
			old_inputs.get(4).cloned().unwrap_or_else(hardcoded_to_one),
			old_inputs.get(5).cloned().unwrap_or_else(hardcoded_to_one),
			old_inputs.get(6).cloned().unwrap_or_else(unset_maximum),
			old_inputs.get(7).cloned().unwrap_or_else(unset_maximum),
			old_inputs
				.get(8)
				.cloned()
				.unwrap_or_else(|| NodeInput::value(TaggedValue::Number(TypesettingConfig::default().letter_tilt), false)),
			NodeInput::value(TaggedValue::TextAlign(TextAlign::default()), false),
			old_inputs.get(9).cloned().unwrap_or_else(|| NodeInput::value(TaggedValue::Bool(false), false)),
		];
		for (index, input) in upgraded_inputs.into_iter().enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}

		inputs_count = 11
	}

	// Insert bool parameters for `has_max_width` and `has_max_height`:
	// https://github.com/GraphiteEditor/Graphite/pull/3643
	if reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode")) && inputs_count == 11 {
		let mut template: NodeTemplate = legacy_text_node_template()?;
		document.network_interface.replace_implementation(node_id, network_path, &mut template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut template)?;

		// Copy over old inputs
		#[allow(clippy::needless_range_loop)]
		for i in 0..=5 {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, i), old_inputs[i].clone(), network_path);
		}

		// The old `Option<f64>` maximum becomes a bool plus a value, with zero standing in for the absent option.
		// A wired maximum has no value to read, so it keeps its connection and counts as present.
		let split_maximum = |input: &NodeInput| match input.as_value() {
			Some(&TaggedValue::Number(maximum)) => (maximum != 0., NodeInput::value(TaggedValue::Number(if maximum == 0. { 100. } else { maximum }), false)),
			_ => (true, input.clone()),
		};

		// Max Width
		let (has_max_width, max_width) = split_maximum(&old_inputs[6]);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 6), NodeInput::value(TaggedValue::Bool(has_max_width), false), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 7), max_width, network_path);

		// Max Height
		let (has_max_height, max_height) = split_maximum(&old_inputs[7]);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 8), NodeInput::value(TaggedValue::Bool(has_max_height), false), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 9), max_height, network_path);

		// Copy over old inputs
		#[allow(clippy::needless_range_loop)]
		for i in 10..=12 {
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, i), old_inputs[i - 2].clone(), network_path);
		}

		inputs_count = 13;
	}

	// Upgrade Sine, Cosine, and Tangent nodes to include a boolean input for whether the output should be in radians, which was previously the only option but is now not the default
	if inputs_count == 1
		&& (reference == DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::sine::IDENTIFIER)
			|| reference == DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::cosine::IDENTIFIER)
			|| reference == DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::tangent::IDENTIFIER))
	{
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	// Upgrade the Modulo node to include a boolean input for whether the output should be always positive, which was previously not an option
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::modulo::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);
	}

	// Convert the old 'Vec2 Value' node, identified by its leftover 3-input shape with separate X and Y inputs,
	// into the 'Numbers to Vec2' node which now fills that role (the new 'Vec2 Value' node instead takes a single vec2 input)
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::vec_2_value::IDENTIFIER) && inputs_count == 3 {
		let numbers_to_vec2_reference = DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::numbers_to_vec_2::IDENTIFIER);
		let mut node_template = resolve_document_node_type(&numbers_to_vec2_reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[2].clone(), network_path);
	}

	// Upgrade the Mirror node to add the `keep_original` boolean input
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::graphic::mirror::IDENTIFIER) && inputs_count == 3 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[2].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 3), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	// Upgrade the Mirror node to add the `reference_point` input and change `offset` from `DVec2` to `f64`
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::graphic::mirror::IDENTIFIER) && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let Some(&TaggedValue::DVec2(old_offset)) = old_inputs[1].as_value() else { return None };
		let old_offset = if old_offset.x.abs() > old_offset.y.abs() { old_offset.x } else { old_offset.y };

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node_at_index(*node_id, 1),
			NodeInput::value(TaggedValue::ReferencePoint(graphene_std::transform::ReferencePoint::Center), false),
			network_path,
		);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Number(old_offset), false), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 3), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), old_inputs[3].clone(), network_path);
	}

	// Upgrade `image` nodes that stored image data as `Image<Color>`` to `image` nodes that store a reference to the image data as a resource.
	// Encodes the image data as PNG and stores it in the document's embedded resources and rewires the node to reference that resource.
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER) {
		let image = node.inputs.iter().find_map(|input| match input.as_value()? {
			TaggedValue::ImageData(image) => Some(image.clone()),
			_ => None,
		});

		if let Some(mut image) = image {
			// Legacy embedded pixel data is premultiplied, so restore straight alpha before encoding it
			image.data.iter_mut().for_each(|pixel| *pixel = pixel.to_unassociated_alpha());
			let hash = document.resources.embedded.store(Resource::new(image.to_png()));

			let resource_id = ResourceId::new();
			document.resources.registry.push_source_back(&resource_id, DataSource::Embedded);
			document.resources.registry.resolve(&resource_id, hash);

			let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
			document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
			let _ = document.network_interface.replace_inputs(node_id, network_path, &mut node_template);
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Resource(resource_id), false), network_path);
		}
	}

	// Move the Image node's resource, whether a stored value or a wire, from the primary input to the first secondary input
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER)
		&& inputs_count == 1
		&& !matches!(node.inputs.first().and_then(|input| input.as_value()), Some(TaggedValue::ImageData(_)))
	{
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[0].clone(), network_path);
	}

	// Convert text nodes from the old `editor-api` scope + `Font` input to a single font `Resource` input.
	// The chosen typeface is recorded as a `DataSource::Font` in the document's resource registry and loaded on open.
	if reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode")) && inputs_count == 13 && matches!(node.inputs.first(), Some(NodeInput::Scope(_))) {
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);

		if let Some(TaggedValue::Font(font)) = node.inputs.get(2).and_then(|input| input.as_value()) {
			let resource_id = ResourceId::new();
			document.resources.registry.push_source_back(&resource_id, DataSource::Embedded);
			document.resources.registry.push_source_back(
				&resource_id,
				DataSource::Font {
					family: font.font_family.clone(),
					style: Some(font.font_style.clone()),
				},
			);
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Resource(resource_id), false), network_path);
		}
	}

	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::std_nodes::noise_pattern::IDENTIFIER) && inputs_count == 15 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);
		for (i, input) in old_inputs.iter().enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, i + 1), input.clone(), network_path);
		}
	}

	// The Threshold node's luminance calculation input was retired
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::adjustments::threshold::IDENTIFIER) && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		for (i, input) in old_inputs.iter().enumerate().take(3) {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, i), input.clone(), network_path);
		}
		inputs_count = 3;
	}

	// Vibrance gained a Saturation input after its Vibrance input, whose default of 0 leaves old documents unchanged
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster::vibrance::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(2).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		inputs_count = 3;
	}

	// The removed "Brightness/Contrast Classic" node had no Use Classic input, so its three inputs become the unified node with the toggle on
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster::brightness_contrast::IDENTIFIER) && inputs_count == 3 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(3).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 3), NodeInput::value(TaggedValue::Bool(true), false), network_path);
		inputs_count = 4;
	}

	// Brightness/Contrast gained the classic algorithm's pivot, whose default of 127 matches what PSD adjustment layers store
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster::brightness_contrast::IDENTIFIER) && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(4).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		inputs_count = 5;
	}

	// Levels' Midtones became the gamma value it encoded, and each channel gained its own record after the composite one
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster::levels::IDENTIFIER) && inputs_count == 6 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		let output_level = |index: usize, default: f64| match old_inputs.get(index).and_then(|input| input.as_value()) {
			Some(TaggedValue::Number(percent)) => percent / 100.,
			_ => default,
		};
		let (output_minimums, output_maximums) = (output_level(4, 0.), output_level(5, 1.));
		for (index, input) in old_inputs.iter().take(6).enumerate() {
			let input = match (index, input.as_value()) {
				(2, Some(TaggedValue::Number(percent))) => {
					// The old node's midtones-to-gamma mapping, from https://stackoverflow.com/questions/39510072/algorithm-for-adjustment-of-image-levels
					let midtones = output_minimums + (output_maximums - output_minimums) * percent / 100.;
					let gamma = if midtones < 0.5 { 1. + 9. * (1. - midtones * 2.) } else { ((1. - midtones) * 2.).max(0.01) };
					NodeInput::value(TaggedValue::Number(gamma.clamp(0.01, 9.99)), input.is_exposed())
				}
				_ => input.clone(),
			};
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}
		inputs_count = 27;
	}

	// Hue/Saturation gained colorize and the six hue ranges after its three master sliders
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster::hue_saturation::IDENTIFIER) && inputs_count == 4 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(4).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		inputs_count = 51;
	}

	// Black & White gained a Use Tint toggle ahead of its tint color; a non-black tint used to be the only way to tint
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::raster::black_and_white::IDENTIFIER) && inputs_count == 8 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		for (index, input) in old_inputs.iter().enumerate().skip(1).take(7) {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index + 1), input.clone(), network_path);
		}
		let use_tint = !matches!(old_inputs[1].as_value(), Some(TaggedValue::Color(color)) if *color == Color::BLACK);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Bool(use_tint), false), network_path);
		inputs_count = 9;
	}

	if reference == DefinitionIdentifier::ProtoNode(graphene_std::repeat::repeat_on_points::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
	}

	// Sample Image nodes, now passthroughs, drop their old title
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER) {
		reset_default_display_name(document, node_id, network_path, "Sample Image");
	}

	// Serialize became As String, whose input converts a wired value to text, while a stored value keeps the JSON text Serialize made of it
	let serialize_identifiers = ["text_nodes::SerializeNode", "graphene_core::logic::SerializeNode"];
	if serialize_identifiers
		.iter()
		.any(|&identifier| reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new(identifier)))
	{
		let mut node_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::text_nodes::as_string::IDENTIFIER))?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		let value_input = match old_inputs.first() {
			Some(NodeInput::Value { tagged_value, exposed }) => {
				let json = match &**tagged_value {
					TaggedValue::String(value) => serde_json::to_string(value),
					TaggedValue::Bool(value) => serde_json::to_string(value),
					TaggedValue::Number(value) => serde_json::to_string(value),
					TaggedValue::Integer(value) => serde_json::to_string(value),
					TaggedValue::DVec2(value) => serde_json::to_string(value),
					TaggedValue::DAffine2(value) => serde_json::to_string(value),
					_ => Ok(String::new()),
				};
				NodeInput::value(TaggedValue::String(json.unwrap_or_default()), *exposed)
			}
			Some(wired) => wired.clone(),
			None => return None,
		};
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), value_input, network_path);
		reset_default_display_name(document, node_id, network_path, "Serialize");
	}

	// Empty Image has no equivalent, so it becomes an Image node with no file, which still feeds an image to the nodes after it
	let empty_image_identifiers = [
		"raster_nodes::std_nodes::EmptyImageNode",
		"graphene_raster_nodes::std_nodes::EmptyImageNode",
		"graphene_std::raster::EmptyImageNode",
	];
	if empty_image_identifiers
		.iter()
		.any(|&identifier| reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new(identifier)))
	{
		let mut node_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER))?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		reset_default_display_name(document, node_id, network_path, "Empty Image");
	}

	if reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::RemoveHandlesNode")) {
		let mut node_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::vector::smooth_anchors::IDENTIFIER))?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Number(0.), false), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);
	}

	if reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::GenerateHandlesNode")) {
		let mut node_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::vector::smooth_anchors::IDENTIFIER))?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::weld::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node_at_index(*node_id, 2),
			NodeInput::value(TaggedValue::WeldMethod(graphene_std::vector::misc::WeldMethod::Connected), false),
			network_path,
		);
		reset_default_display_name(document, node_id, network_path, "Merge by Distance");
	}

	// A Merge by Distance node saved with its "Algorithm" input takes Weld's input names and descriptions, keeping its inputs
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::weld::IDENTIFIER)
		&& inputs_count == 3
		&& document
			.network_interface
			.persistent_input_metadata(node_id, 2, network_path)
			.is_some_and(|metadata| metadata.input_name == "Algorithm")
	{
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		node_template.inputs = node.inputs.clone();
		document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		reset_default_display_name(document, node_id, network_path, "Merge by Distance");
	}

	if reference == DefinitionIdentifier::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::SpatialMergeByDistanceNode")) {
		let mut node_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::vector::weld::IDENTIFIER))?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node_at_index(*node_id, 2),
			NodeInput::value(TaggedValue::WeldMethod(graphene_std::vector::misc::WeldMethod::All), false),
			network_path,
		);
		reset_default_display_name(document, node_id, network_path, "Spatial Merge by Distance");
	}

	// Cut Segments became Unweld, keeping its content and taking the All method that splits every anchor as it did
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::unweld::IDENTIFIER) && inputs_count == 1 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		node_template.inputs = vec![
			node.inputs[0].clone(),
			NodeInput::value(TaggedValue::UnweldMethod(graphene_std::vector::misc::UnweldMethod::All), false),
		];
		document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		reset_default_display_name(document, node_id, network_path, "Cut Segments");
		reset_default_display_name(document, node_id, network_path, "Split Segments");
	}

	// Separate Subpaths and Close Path became Separate Paths and Close Paths, whose inputs take their current names and descriptions
	let pluralized = [
		(graphene_std::vector::separate_paths::IDENTIFIER, "Separate Subpaths"),
		(graphene_std::vector::close_paths::IDENTIFIER, "Close Path"),
	];
	if let Some((_, old_default_name)) = pluralized.iter().find(|(identifier, _)| reference == DefinitionIdentifier::ProtoNode(identifier.clone())) {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let stored = document
			.network_interface
			.persistent_input_metadata(node_id, 0, network_path)
			.map(|metadata| (metadata.input_name.clone(), metadata.input_description.clone()));
		let current = node_template
			.input_metadata
			.first()
			.map(|metadata| (metadata.persistent_metadata.input_name.clone(), metadata.persistent_metadata.input_description.clone()));
		if stored != current {
			node_template.inputs = node.inputs.clone();
			document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		}

		reset_default_display_name(document, node_id, network_path, old_default_name);
	}

	// Nodes renamed with unchanged inputs take their new names in place of their old default titles
	let renamed = [
		(graphene_std::animation::step_animation_time::IDENTIFIER, "Quantize Animation Time"),
		(graphene_std::animation::step_real_time::IDENTIFIER, "Quantize Real Time"),
		(graphene_std::debug::debug_log::IDENTIFIER, "Log to Console"),
		(graphene_std::vector::smooth_anchors::IDENTIFIER, "Auto-Tangents"),
		(graphene_std::platform_application_io::bytes_to_image::IDENTIFIER, "Decode Image"),
		(graphene_std::graphic::indices::IDENTIFIER, "List Indices"),
		(graphene_std::vector::count::IDENTIFIER, "List Length"),
		(graphene_std::graphic::slice::IDENTIFIER, "List Slice"),
		(graphene_std::text_nodes::string_map::IDENTIFIER, "Map String"),
	];
	if let Some((_, old_default_name)) = renamed.iter().find(|(identifier, _)| reference == DefinitionIdentifier::ProtoNode(identifier.clone())) {
		reset_default_display_name(document, node_id, network_path, old_default_name);
	}

	if reference == DefinitionIdentifier::Network("Sample Points".into()) && inputs_count == 5 {
		let mut node_template = resolve_network_node_type("Sample Polyline")?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);

		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		let new_spacing_value = NodeInput::value(TaggedValue::PointSpacingType(graphene_std::vector::misc::PointSpacingType::Separation), false);
		let new_quantity_value = NodeInput::value(TaggedValue::Integer(100), false);

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), new_spacing_value, network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 3), new_quantity_value, network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), old_inputs[2].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 5), old_inputs[3].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 6), old_inputs[4].clone(), network_path);
	}

	// Make the "Quantity" parameter a u32 instead of f64
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::sample_polyline::IDENTIFIER) {
		// Get the inputs, obtain the quantity value, and put the inputs back
		let quantity_value = document
			.network_interface
			.input_from_connector(&InputConnector::Node { node_id: *node_id, input_index: 3 }, network_path)?;

		if let NodeInput::Value { tagged_value, exposed } = quantity_value
			&& let TaggedValue::Number(value) = **tagged_value
		{
			let new_quantity_value = NodeInput::value(TaggedValue::Integer(value as i64), *exposed);
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 3), new_quantity_value, network_path);
		}
	}

	// Upgrade the "Grid" node's six- and seven-input layouts, replacing the seventh "connect_cells" toggle with "topology" and "staggered"
	// after "grid_type". Legacy documents also placed "angles" before "columns" and "rows" instead of after, so we reorder those.
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::grid::IDENTIFIER) && (inputs_count == 6 || inputs_count == 7) {
		let mut new_node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut new_node_template)?;

		// The two six-input layouts differ only in where the DVec2 "angles" and the u32 "columns"/"rows" sit:
		//   Legacy: [primary, grid_type, spacing, angles (DVec2), columns (int), rows (int)]
		//   Modern: [primary, grid_type, spacing, columns (int), rows (int), angles (DVec2)]
		// So a DVec2 "angles" at index 3, or anything but angles at index 5, marks the legacy order. Checking both slots classifies
		// correctly even when one of them is a wired or imported connection rather than a literal value.
		let index_3_is_angles = matches!(old_inputs.get(3), Some(NodeInput::Value { tagged_value, .. }) if matches!(**tagged_value, TaggedValue::DVec2(_)));
		let index_5_is_rows = matches!(old_inputs.get(5), Some(NodeInput::Value { tagged_value, .. }) if !matches!(**tagged_value, TaggedValue::DVec2(_)));
		let legacy_angles_layout = index_3_is_angles || index_5_is_rows;

		// Carry over "primary" and "grid_type", then "columns", "rows", "spacing", and "angles" to after "staggered"
		let old_indices = if legacy_angles_layout { [4, 5, 2, 3] } else { [3, 4, 2, 5] };
		for (new_index, old_index) in [(0, 0), (1, 1)].into_iter().chain((4..).zip(old_indices)) {
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, new_index), old_inputs[old_index].clone(), network_path);
		}

		// Grids drew connected edges unless "connect_cells" was turned off, which drew separate cells
		let disconnected_cells = matches!(old_inputs.get(6), Some(NodeInput::Value { tagged_value, .. }) if matches!(**tagged_value, TaggedValue::Bool(false)));
		let topology = if disconnected_cells {
			graphene_std::vector::misc::GridTopology::Cells
		} else {
			graphene_std::vector::misc::GridTopology::Edges
		};
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::GridTopology(topology), false), network_path);
	}

	// Add the "Loop Level" parameter to the "Read Index" node
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::context::read_index::IDENTIFIER) && inputs_count == 0 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		document.network_interface.set_display_name(node_id, "Read Index".to_string(), network_path);

		let mut node_path = network_path.to_vec();
		node_path.push(*node_id);

		let loop_level_description = node_template
			.input_metadata
			.get(1)
			.map(|metadata| metadata.persistent_metadata.input_description.clone())
			.unwrap_or_default();
		document.network_interface.add_import(TaggedValue::None, false, 0, "Primary", "", &node_path);
		document
			.network_interface
			.add_import(TaggedValue::Integer(0), false, 1, "Loop Level", &loop_level_description, &node_path);
	}

	// Drop the placeholder primary input the "Read Vector" node used to carry, since it reads its value from the context
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::context::read_vector::IDENTIFIER) && inputs_count > 0 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
	}

	// The "Dot Product" node gained a "Normalize" toggle, which older nodes predate by always taking the raw dot product
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::dot_product::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		for (index, input) in old_inputs.iter().take(2).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), network_path);
	}

	// The "Query JSON" node succeeded "JSON Get", whose object lookups always returned their strings unquoted
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::text_nodes::json::query_json::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		for (index, input) in old_inputs.iter().take(2).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 2), NodeInput::value(TaggedValue::Bool(true), false), network_path);
	}

	// Upgrade the "Animation" node to add the "Rate" input
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::animation::animation_time::IDENTIFIER) && inputs_count < 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let _ = document.network_interface.replace_inputs(node_id, network_path, &mut node_template);

		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Number(1.), false), network_path);
	}

	// Upgrade the "Read Position" node to add the "Loop Level" input
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::context::read_position::IDENTIFIER) && inputs_count < 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let _ = document.network_interface.replace_inputs(node_id, network_path, &mut node_template);

		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 0), NodeInput::value(TaggedValue::None, false), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::Integer(0), false), network_path);
	}

	// Migrate from the old source/target v1 "Morph" node to the new `List<Vector>`-based v2 "Morph" node.
	// This doesn't produce exactly equivalent results in cases involving input `List<Vector>` values with multiple items.
	// The old version would zip the source and target items, interpolating each pair together.
	// The migrated version will instead deeply flatten both merged `List`s and morph sequentially between all source vectors and all target vector elements.
	// This migration assumes most usages didn't involve multiple parallel vector elements, and instead morphed from a single source to a single target vector element.
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::morph::IDENTIFIER) && (inputs_count == 3 || inputs_count == 4) {
		// 3 inputs - old signature (#3405):
		// async fn morph(_: impl Ctx, source: List<Vector>, #[expose] target: List<Vector>, #[default(0.5)] time: Fraction) -> List<Vector> { ... }
		//
		// 4 inputs - even older signature (commit 80b8df8d4298b6669f124b929ce61bfabfc44e41):
		// async fn morph(_: impl Ctx, source: List<Vector>, #[expose] target: List<Vector>, #[default(0.5)] time: Fraction, start_index: u32) -> List<Vector> { ... }
		//
		// v2 signature:
		// async fn morph<I: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Vector>)] content: I, progression: Progression) -> List<Vector> { ... }

		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// Create a new Merge node
		let Some(merge_node_type) = resolve_network_node_type("Merge") else {
			log::error!("Could not get merge node from definition when upgrading morph");
			return None;
		};
		let merge_template = merge_node_type.default_node_template();
		let merge_node_id = NodeId::new();

		// Decide on the placement position of the new Merge node
		let Some(morph_position) = document.network_interface.position(node_id, network_path) else {
			log::error!("Could not get position for morph node {node_id}");
			return None;
		};
		let merge_position = morph_position + IVec2::new(-7, 0);

		// Insert the new Merge node into the network
		document.network_interface.insert_node(merge_node_id, merge_template, network_path);
		document.network_interface.set_to_node_or_layer(&merge_node_id, network_path, false);
		document.network_interface.shift_absolute_node_position(&merge_node_id, merge_position, network_path);

		// Connect the old 'source' and 'target' inputs to the new Merge node
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(merge_node_id, 0), old_inputs[0].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(merge_node_id, 1), old_inputs[1].clone(), network_path);

		// Connect the new Merge node to the 'content' input of the Morph node
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 0), NodeInput::node(merge_node_id, 0), network_path);
		// Connect the old 'progression' input to the new 'progression' input of the Morph node
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[2].clone(), network_path);

		inputs_count = 2;
	}

	// Migrate from the v2 "Morph" node (2 inputs: content, progression) to the v3 "Morph" node (5 inputs: content, progression, reverse, distribution, path).
	// The old progression used integer part for pair selection (range 0..N-1 where N is the number of content objects).
	// The new progression uses fractional 0..1 for euclidean traversal through all objects.
	// We insert Count → Subtract 1 → Divide to remap: new_progression = old_progression / (N - 1).
	// For the common 2-object case (N=2), this divides by 1 which is a no-op, preserving identical behavior.
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::morph::IDENTIFIER) && inputs_count == 2 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// Reconnect content (input 0) and leave path (input 4) as default
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);

		let Some(morph_position) = document.network_interface.position(node_id, network_path) else {
			log::error!("Could not get position for morph node {node_id}");
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
			return None;
		};

		// Create Count node: counts content `List` items → N
		let Some(count_def) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::vector::count::IDENTIFIER)) else {
			log::error!("Could not get count node from definition when upgrading morph");
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
			return None;
		};
		let count_template = count_def.default_node_template();
		let count_id = NodeId::new();

		// Create Subtract node: N → N-1
		let Some(subtract_def) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::subtract::IDENTIFIER)) else {
			log::error!("Could not get subtract node from definition when upgrading morph");
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
			return None;
		};
		let mut subtract_template = subtract_def.default_node_template();
		subtract_template.inputs[1] = NodeInput::value(TaggedValue::Number(1.), false);
		let subtract_id = NodeId::new();

		// Create Divide node: old_progression / (N-1) → new progression
		let Some(divide_def) = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::divide::IDENTIFIER)) else {
			log::error!("Could not get divide node from definition when upgrading morph");
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
			return None;
		};
		let divide_template = divide_def.default_node_template();
		let divide_id = NodeId::new();

		// Insert and position nodes
		document.network_interface.insert_node(count_id, count_template, network_path);
		document.network_interface.shift_absolute_node_position(&count_id, morph_position + IVec2::new(-21, 2), network_path);

		document.network_interface.insert_node(subtract_id, subtract_template, network_path);
		document.network_interface.shift_absolute_node_position(&subtract_id, morph_position + IVec2::new(-14, 2), network_path);

		document.network_interface.insert_node(divide_id, divide_template, network_path);
		document.network_interface.shift_absolute_node_position(&divide_id, morph_position + IVec2::new(-7, 1), network_path);

		// Wire: content source → Count input 0
		document.network_interface.set_input(&InputConnector::node_at_index(count_id, 0), old_inputs[0].clone(), network_path);

		// Wire: Count output → Subtract input 0 (minuend)
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(subtract_id, 0), NodeInput::node(count_id, 0), network_path);

		// Wire: old progression → Divide input 0 (numerator)
		document.network_interface.set_input(&InputConnector::node_at_index(divide_id, 0), old_inputs[1].clone(), network_path);

		// Wire: Subtract output → Divide input 1 (denominator)
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(divide_id, 1), NodeInput::node(subtract_id, 0), network_path);

		// Wire: Divide output → Morph progression input
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::node(divide_id, 0), network_path);
	}

	// Migrate old Arrow node from (start, end, shaft_width, head_width, head_length) to (arrow_to, shaft_width, head_width, head_length) with a Transform node for positioning
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER) && inputs_count == 6 {
		// Read old start and end values
		let start = match node.inputs.get(1)? {
			NodeInput::Value { tagged_value, .. } => {
				if let TaggedValue::DVec2(v) = *tagged_value.clone().into_inner() {
					v
				} else {
					DVec2::ZERO
				}
			}
			_ => DVec2::ZERO,
		};
		let end = match node.inputs.get(2)? {
			NodeInput::Value { tagged_value, .. } => {
				if let TaggedValue::DVec2(v) = *tagged_value.clone().into_inner() {
					v
				} else {
					DVec2::new(100., 0.)
				}
			}
			_ => DVec2::new(100., 0.),
		};

		// Replace inputs with the new node definition (primary + arrow_to + shaft_width + head_width + head_length)
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// Preserve primary input connection
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		// Set arrow_to = end - start
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::DVec2(end - start), false), network_path);
		// Preserve shaft_width, head_width, head_length (shifted from indices 3,4,5 to 2,3,4)
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[3].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 3), old_inputs[4].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 4), old_inputs[5].clone(), network_path);

		// Find downstream connection to insert Transform node
		let downstream = document
			.network_interface
			.outward_wires(network_path)
			.and_then(|wires| wires.get(&OutputConnector::node(*node_id, 0)))
			.and_then(|connections| connections.first().cloned());

		if let Some(downstream_input) = downstream {
			// Create a Transform node with translation = start
			let Some(transform_node_type) = resolve_proto_node_type(graphene_std::transform_nodes::transform::IDENTIFIER) else {
				log::error!("Transform node definition not found during Arrow migration");
				return None;
			};
			let mut transform_template = transform_node_type.default_node_template();
			transform_template.inputs[1] = NodeInput::value(TaggedValue::DVec2(start), false);

			let transform_id = NodeId::new();

			// Position the Transform node to the right of the Arrow node
			let arrow_position = document.network_interface.position(node_id, network_path).unwrap_or_default();
			document.network_interface.insert_node(transform_id, transform_template, network_path);
			document.network_interface.shift_absolute_node_position(&transform_id, arrow_position + IVec2::new(7, 0), network_path);
			document.network_interface.insert_node_between(&transform_id, &downstream_input, 0, network_path);
		}
	}

	// Migrate old Line node from (start, end) to (line_to) with a Transform node for positioning
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::line::IDENTIFIER) && inputs_count == 3 {
		// Read old start and end values
		let start = match node.inputs.get(1)? {
			NodeInput::Value { tagged_value, .. } => {
				if let TaggedValue::DVec2(v) = *tagged_value.clone().into_inner() {
					v
				} else {
					DVec2::ZERO
				}
			}
			_ => DVec2::ZERO,
		};
		let end = match node.inputs.get(2)? {
			NodeInput::Value { tagged_value, .. } => {
				if let TaggedValue::DVec2(v) = *tagged_value.clone().into_inner() {
					v
				} else {
					DVec2::new(100., 100.)
				}
			}
			_ => DVec2::new(100., 100.),
		};

		// Replace inputs with the new node definition (primary + line_to)
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// Preserve primary input connection
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		// Set line_to = end - start
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 1), NodeInput::value(TaggedValue::DVec2(end - start), false), network_path);

		// Find downstream connection to insert Transform node
		let downstream = document
			.network_interface
			.outward_wires(network_path)
			.and_then(|wires| wires.get(&OutputConnector::node(*node_id, 0)))
			.and_then(|connections| connections.first().cloned());

		if let Some(downstream_input) = downstream {
			// Create a Transform node with translation = start
			let Some(transform_node_type) = resolve_proto_node_type(graphene_std::transform_nodes::transform::IDENTIFIER) else {
				log::error!("Transform node definition not found during Line migration");
				return None;
			};
			let mut transform_template = transform_node_type.default_node_template();
			transform_template.inputs[1] = NodeInput::value(TaggedValue::DVec2(start), false);

			let transform_id = NodeId::new();

			// Position the Transform node to the right of the Line node
			let line_position = document.network_interface.position(node_id, network_path).unwrap_or_default();
			document.network_interface.insert_node(transform_id, transform_template, network_path);
			document.network_interface.shift_absolute_node_position(&transform_id, line_position + IVec2::new(7, 0), network_path);
			document.network_interface.insert_node_between(&transform_id, &downstream_input, 0, network_path);
		}
	}

	// Add the "Scale Type" parameter to the "Decompose Scale" node
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::decompose_scale::IDENTIFIER) && inputs_count == 1 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(
			&InputConnector::node_at_index(*node_id, 1),
			NodeInput::value(TaggedValue::ScaleType(ScaleType::Magnitude), false),
			network_path,
		);
	}

	// Add the "Along Normals" parameter to the "Jitter Points" node
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::vector::jitter_points::IDENTIFIER) && inputs_count == 3 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[1].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 2), old_inputs[2].clone(), network_path);
		document
			.network_interface
			.set_input(&InputConnector::node_at_index(*node_id, 3), NodeInput::value(TaggedValue::Bool(false), false), network_path);
	}

	// SVG-import legacy Path nodes baked their geometry at non-exposed input 0; move it to input 1 (the modern slot for VectorModification).
	// For exposed input 0, any baked value is unused at runtime, so just reset it.
	if reference == DefinitionIdentifier::Network("Path".into()) {
		let input_0 = node.inputs.first()?;
		if let NodeInput::Value { tagged_value, exposed } = input_0
			&& let TaggedValue::VectorModification(modification) = &**tagged_value
		{
			let modification = modification.clone();
			let was_exposed = *exposed;

			document.network_interface.set_input(
				&InputConnector::node_at_index(*node_id, 0),
				NodeInput::type_default(item!(graphene_std::vector::Vector), true),
				network_path,
			);

			if !was_exposed {
				document.network_interface.set_input(
					&InputConnector::node_at_index(*node_id, 1),
					NodeInput::value(TaggedValue::VectorModification(modification), false),
					network_path,
				);
			}
		}
	}

	// `load_resource` no longer takes the `editor-api`
	if reference == DefinitionIdentifier::ProtoNode(graphene_std::platform_application_io::load_resource::IDENTIFIER) && inputs_count == 3 {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), old_inputs[0].clone(), network_path);
		document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 1), old_inputs[2].clone(), network_path);
	}

	// Load Image's URL is empty until set, it used to be `graphite:null`
	if reference == DefinitionIdentifier::Network("Load Image".into())
		&& let Some(url) = node.inputs.get(1)
		&& matches!(url.as_value(), Some(TaggedValue::String(url)) if url == "graphite:null")
	{
		document.network_interface.set_input(
			&InputConnector::node_at_index(*node_id, 1),
			NodeInput::value(TaggedValue::String(String::new()), url.is_exposed()),
			network_path,
		);
	}

	// `wgpu-executor` scope was removed, change to the auto injected scope node `graphene_std::platform_application_io::wgpu_executor`
	for (i, input) in node.inputs.iter().enumerate() {
		if let NodeInput::Scope(name) = input
			&& *name == "wgpu-executor"
		{
			document.network_interface.set_input(
				&InputConnector::node_at_index(*node_id, i),
				NodeInput::Scope("graphene_std::platform_application_io::WgpuExecutorNode".into()),
				network_path,
			);
		}
	}

	let raster_kernel_pipeline = NodeInput::scope(graphene_std::raster_nodes::raster_kernel::raster_kernel_pipeline::IDENTIFIER.as_str().to_string());
	let raster_kernel_nodes = {
		use graphene_std::raster_nodes::adjustments::*;
		[
			desaturate::IDENTIFIER,
			gamma_correction::IDENTIFIER,
			make_opaque::IDENTIFIER,
			brightness_contrast::IDENTIFIER,
			levels::IDENTIFIER,
			curves::IDENTIFIER,
			black_and_white::IDENTIFIER,
			hue_saturation::IDENTIFIER,
			invert::IDENTIFIER,
			threshold::IDENTIFIER,
			gradient_map::IDENTIFIER,
			vibrance::IDENTIFIER,
			channel_mixer::IDENTIFIER,
			selective_color::IDENTIFIER,
			posterize::IDENTIFIER,
			exposure::IDENTIFIER,
			color_balance::IDENTIFIER,
			photo_filter::IDENTIFIER,
			color_lookup::IDENTIFIER,
			image_to_channels::IDENTIFIER,
			graphene_std::raster_nodes::blending_nodes::color_overlay::IDENTIFIER,
			graphene_std::raster_nodes::std_nodes::channels_to_image::IDENTIFIER,
			graphene_std::raster_nodes::std_nodes::mask::IDENTIFIER,
			graphene_std::raster_nodes::std_nodes::extend_image_to_bounds::IDENTIFIER,
			graphene_std::raster_nodes::std_nodes::noise_pattern::IDENTIFIER,
			graphene_std::raster_nodes::std_nodes::mandelbrot::IDENTIFIER,
			graphene_std::raster_nodes::filter::median_filter::IDENTIFIER,
			graphene_std::raster_nodes::filter::blur::IDENTIFIER,
			graphene_std::raster_nodes::image_color_palette::image_color_palette::IDENTIFIER,
			graphene_std::raster_nodes::dehaze::dehaze::IDENTIFIER,
			graphene_std::raster_nodes::std_nodes::image::IDENTIFIER,
			graphene_std::platform_application_io::bytes_to_image::IDENTIFIER,
			#[cfg(target_family = "wasm")]
			graphene_std::platform_application_io::rasterize::IDENTIFIER,
		]
	};
	if raster_kernel_nodes.iter().any(|identifier| reference == DefinitionIdentifier::ProtoNode(identifier.clone())) && !node.inputs.contains(&raster_kernel_pipeline) {
		let mut node_template = resolve_document_node_type(&reference)?.default_node_template();
		let value_inputs = node_template.inputs.iter().position(|input| *input == raster_kernel_pipeline)?;
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		for (index, input) in old_inputs.into_iter().take(value_inputs).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}
	}

	// A value input stored as a List-form TypeDefault adopts the definition's current default when the connector's declared default has since changed (e.g. the connector was ranked down to Item).
	// The red-slash no-paint choice shares that stored form but is a deliberate value, not a stale disconnect default, so it is exempt.
	if let Some(definition) = resolve_document_node_type(&reference) {
		let definition_inputs = definition.node_template.inputs.clone();
		for (index, definition_input) in definition_inputs.iter().enumerate() {
			if !matches!(definition_input, NodeInput::Value { .. }) {
				continue;
			}

			let stale_list_default = document
				.network_interface
				.input_from_connector(&InputConnector::node_at_index(*node_id, index), network_path)
				.is_some_and(|stored_input| match stored_input {
					NodeInput::Value { tagged_value, .. } => match &**tagged_value {
						TaggedValue::TypeDefault(stored_type) if matches!(stored_type, Type::List(_)) && !tagged_value.is_no_paint() => {
							!matches!(definition_input, NodeInput::Value { tagged_value, .. } if matches!(&**tagged_value, TaggedValue::TypeDefault(definition_type) if definition_type == stored_type))
						}
						_ => false,
					},
					_ => false,
				});

			if stale_list_default {
				document
					.network_interface
					.set_input(&InputConnector::node_at_index(*node_id, index), definition_input.clone(), network_path);
			}
		}
	}

	// A `Resource` input disconnected before `Resource` became an item-ranked type default stored the bare `TypeDefault(Resource)`, which no longer unwraps to an `Item<Resource>` default
	if let Some(current_node) = document.network_interface.document_node(node_id, network_path) {
		let bare_resource_inputs: Vec<(usize, bool)> = current_node
			.inputs
			.iter()
			.enumerate()
			.filter_map(|(index, input)| match input {
				NodeInput::Value { tagged_value, exposed } if matches!(&**tagged_value, TaggedValue::TypeDefault(stored_type) if *stored_type == concrete!(Resource)) => Some((index, *exposed)),
				_ => None,
			})
			.collect();
		for (index, exposed) in bare_resource_inputs {
			document
				.network_interface
				.set_input(&InputConnector::node_at_index(*node_id, index), NodeInput::type_default(item!(Resource), exposed), network_path);
		}
	}

	Some(())
}

// TODO: Eventually remove this document upgrade code
/// Older stored edits are turned around where they chain into paths on load. Edits naming those segments by side (a handle,
/// an endpoint, or a G1 pair) in the Path nodes downstream of the one that turned them are turned with them, so each stays on
/// the side it was made on. Segment IDs are only unique within one shape, so nodes not downstream are left alone.
fn follow_segments_turned_on_load(document: &mut DocumentMessageHandler) {
	for (network_path, node_id, index, input) in modifications_following_turned_segments(document.network_interface.document_network()) {
		document.network_interface.set_input(&InputConnector::node_at_index(node_id, index), input, &network_path);
	}
}

// TODO: Eventually remove this document upgrade code
/// The stored edits to rewrite after loading, each with its network path, node, and input index: those that turned segments,
/// with their record taken, and those downstream of them, turned to follow. Costs O(nodes + wires) per node that turned any.
fn modifications_following_turned_segments(network: &graph_craft::document::NodeNetwork) -> Vec<(Vec<NodeId>, NodeId, usize, NodeInput)> {
	type Location = (Vec<NodeId>, NodeId);

	// The nodes reading each node's data, wired straight to the node producing it through nested networks' imports and exports
	let mut readers: HashMap<Location, Vec<Location>> = HashMap::new();
	let mut turned_by: Vec<(Location, HashSet<graphene_std::vector::SegmentId>)> = Vec::new();
	for (node_id, node, network_path) in network.recursive_nodes() {
		let here = (network_path.clone(), *node_id);

		for input in &node.inputs {
			if let NodeInput::Value { tagged_value, .. } = input
				&& let TaggedValue::VectorModification(modification) = &**tagged_value
				&& !modification.segments_turned_on_load().is_empty()
			{
				turned_by.push((here.clone(), modification.segments_turned_on_load().clone()));
			}
		}

		// A network node's inputs only pass through to the nodes inside it, which are wired as readers themselves
		if matches!(node.implementation, DocumentNodeImplementation::Network(_)) {
			continue;
		}
		for input in &node.inputs {
			if let Some(producer) = producing_node(network, &network_path, input, 0) {
				readers.entry(producer).or_default().push(here.clone());
			}
		}
	}
	if turned_by.is_empty() {
		return Vec::new();
	}

	// Each node's turned segments reach the nodes reading its output, then their readers, and so on
	let mut following: HashMap<Location, HashSet<graphene_std::vector::SegmentId>> = HashMap::new();
	for (origin, turned) in &turned_by {
		let (origin_path, origin_id) = origin;
		let Some(origin_node) = network.nested_network(origin_path).and_then(|nested| nested.nodes.get(origin_id)) else {
			continue;
		};

		// Where the output comes from: the node itself, or the nodes a network node exports
		let sources: Vec<Location> = match network.implementation_network(&origin_node.implementation) {
			Some(nested) => (0..nested.exports.len())
				.filter_map(|output| producing_node(network, origin_path, &NodeInput::node(*origin_id, output), 0))
				.collect(),
			None => vec![origin.clone()],
		};

		let mut reached = HashSet::new();
		let mut pending: Vec<Location> = sources.iter().flat_map(|source| readers.get(source).into_iter().flatten().cloned()).collect();
		while let Some(location) = pending.pop() {
			if reached.insert(location.clone()) {
				pending.extend(readers.get(&location).into_iter().flatten().cloned());
			}
		}

		// An edit follows when the node holding it, or a node inside it, reads the turned data
		for (path, node_id) in reached {
			let enclosing = (0..path.len()).map(|depth| (path[..depth].to_vec(), path[depth]));
			for holder in enclosing.chain([(path.clone(), node_id)]).filter(|holder| holder != origin) {
				following.entry(holder).or_default().extend(turned.iter().copied());
			}
		}
	}

	let mut rewritten = Vec::new();
	for (node_id, node, network_path) in network.recursive_nodes() {
		let location = (network_path.clone(), *node_id);
		let follows = following.get(&location);

		for (index, input) in node.inputs.iter().enumerate() {
			let NodeInput::Value { tagged_value, exposed } = input else { continue };
			let TaggedValue::VectorModification(modification) = &**tagged_value else { continue };
			let had_record = !modification.segments_turned_on_load().is_empty();
			if !had_record && follows.is_none() {
				continue;
			}

			let mut modification = modification.clone();
			modification.take_segments_turned_on_load();
			let changed = follows.is_some_and(|turned| modification.follow_segments_turned_elsewhere(turned));

			if had_record || changed {
				let input = NodeInput::value(TaggedValue::VectorModification(modification), *exposed);
				rewritten.push((network_path.clone(), *node_id, index, input));
			}
		}
	}

	rewritten
}

// TODO: Eventually remove this document upgrade code
/// The node, not itself a network, producing the data an input reads, looking through the imports and exports of nested networks.
fn producing_node(network: &graph_craft::document::NodeNetwork, network_path: &[NodeId], input: &NodeInput, depth: usize) -> Option<(Vec<NodeId>, NodeId)> {
	// Nesting is shallow in practice, so this only guards against a malformed document wiring a network back into itself
	if depth > 64 {
		return None;
	}

	match input {
		NodeInput::Node { node_id, output_index } => {
			let node = network.nested_network(network_path)?.nodes.get(node_id)?;
			match network.implementation_network(&node.implementation) {
				Some(nested) => producing_node(network, &[network_path, &[*node_id]].concat(), nested.exports.get(*output_index)?, depth + 1),
				None => Some((network_path.to_vec(), *node_id)),
			}
		}
		NodeInput::Import { import_index, .. } => {
			let (parent, parent_path) = network_path.split_last()?;
			let parent_node = network.nested_network(parent_path)?.nodes.get(parent)?;
			producing_node(network, parent_path, parent_node.inputs.get(*import_index)?, depth + 1)
		}
		_ => None,
	}
}

/// Ensure layers are positioned as stacks if they are upstream siblings of another layer
fn create_layer_stacks(document: &mut DocumentMessageHandler) {
	document.network_interface.load_structure();
	let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(document.network_interface.document_metadata()).collect::<Vec<_>>();
	for layer in all_layers {
		let Some((downstream_node, input_index)) = document
			.network_interface
			.outward_wires(&[])
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(layer.to_node(), 0)))
			.and_then(|outward_wires| outward_wires.first())
			.and_then(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())))
		else {
			continue;
		};
		// If the downstream node is a layer and the input is the first input and the current layer is not in a stack
		if input_index == 0 && document.network_interface.is_layer(&downstream_node, &[]) && !document.network_interface.is_stack(&layer.to_node(), &[]) {
			// Ensure the layer is horizontally aligned with the downstream layer to prevent changing the layout of old files
			let (Some(layer_position), Some(downstream_position)) = (document.network_interface.position(&layer.to_node(), &[]), document.network_interface.position(&downstream_node, &[])) else {
				log::error!("Could not get position for layer {:?} or downstream node {} when opening file", layer.to_node(), downstream_node);
				continue;
			};
			if layer_position.x == downstream_position.x {
				document.network_interface.set_stack_position_calculated_offset(&layer.to_node(), &downstream_node, &[]);
			}
		}
	}
}

/// Migrates document nodes whose catalog definitions have been removed.
///
/// This is called from `migrate_node` BEFORE its standard reset/migration logic, since that logic aborts when it can't
/// resolve the node's `reference` against the current catalog. Each block here detects a specific decommissioned
/// definition by its old reference name, swaps it to a still-supported implementation, and preserves the user's inputs.
/// After this runs, the node's reference resolves cleanly so the rest of `migrate_node` proceeds normally.
fn migrate_removed_catalog_definitions(node_id: &NodeId, node: &DocumentNode, network_path: &[NodeId], document: &mut DocumentMessageHandler) -> Option<()> {
	// Collapse the legacy "Sample Points" and "Sample Polyline" wrapper networks into the standalone `sample_polyline`
	// proto node, which now computes per-bezpath segment lengths inline instead of through the wrapper's helper nodes.
	// The oldest documents lose their stored reference on load, so the wrapper is recognized by the nodes it encloses.
	let wrapper_inputs = match &node.implementation {
		DocumentNodeImplementation::Network(inner) => {
			let helpers = [graphene_std::ops::passthrough::IDENTIFIER, graphene_std::memo::memoize::IDENTIFIER];
			let mut sample_nodes = 0;
			let only_helpers = inner.nodes.values().all(|inner_node| match &inner_node.implementation {
				DocumentNodeImplementation::ProtoNode(identifier) if *identifier == graphene_std::vector::sample_polyline::IDENTIFIER => {
					sample_nodes += 1;
					true
				}
				DocumentNodeImplementation::ProtoNode(identifier) => helpers.contains(identifier),
				_ => false,
			});
			(only_helpers && sample_nodes == 1).then_some(node.inputs.len())
		}
		_ => None,
	};
	if let Some(wrapper_inputs) = wrapper_inputs
		&& (wrapper_inputs == 5 || wrapper_inputs == 7)
	{
		let mut node_template = resolve_proto_node_type(graphene_std::vector::sample_polyline::IDENTIFIER)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;

		// The 5-input era predates the separation/quantity choice, so its lone spacing distance becomes the separation
		let upgraded_inputs: Vec<(usize, NodeInput)> = match wrapper_inputs {
			5 => [0, 2, 4, 5, 6].into_iter().zip(old_inputs.iter().cloned()).collect(),
			_ => old_inputs.iter().take(7).cloned().enumerate().collect(),
		};
		for (index, input) in upgraded_inputs {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input, network_path);
		}
	}

	// Collapse the legacy "Scatter Points" wrapper network into the standalone `scatter_points` proto node.
	// The wrapper's trailing `Memoize` node is now produced automatically by the `memoize` attribute on the
	// proto node, so the wrapper itself is redundant. The 3 user-facing inputs are positionally identical
	// between the old wrapper and the new proto node.
	if let Some(DefinitionIdentifier::Network(name)) = document.network_interface.reference(node_id, network_path)
		&& name == "Scatter Points"
		&& node.inputs.len() == 3
	{
		let mut node_template = resolve_proto_node_type(graphene_std::vector::scatter_points::IDENTIFIER)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(3).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
	}

	// Collapse the legacy "Boolean Operation" wrapper network into the standalone `boolean_operation` proto node.
	// The wrapper's trailing `Memoize` node is now produced automatically by the `memoize` attribute on the
	// proto node, so the wrapper itself is redundant. The 2 user-facing inputs are positionally identical
	// between the old wrapper and the new proto node.
	if let Some(DefinitionIdentifier::Network(name)) = document.network_interface.reference(node_id, network_path)
		&& name == "Boolean Operation"
		&& node.inputs.len() == 2
	{
		let mut node_template = resolve_proto_node_type(graphene_std::path_bool_nodes::boolean_operation::IDENTIFIER)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(2).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
	}

	// Collapse the legacy "Rasterize" wrapper network, which drew into a browser canvas, into the standalone Vello `rasterize` proto node
	#[cfg(feature = "gpu")]
	if let Some(DefinitionIdentifier::Network(name)) = document.network_interface.reference(node_id, network_path)
		&& name == "Rasterize"
		&& node.inputs.len() == 2
	{
		let mut node_template = resolve_proto_node_type(graphene_std::platform_application_io::rasterize::IDENTIFIER)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		for (index, input) in old_inputs.iter().take(2).enumerate() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, index), input.clone(), network_path);
		}
	}

	// The removed Attach Attribute node degrades to a passthrough of its
	// content: its eager whole-list source input cannot be mechanically rewired as Write Attribute's lazy per-item value producer.
	if let Some(DefinitionIdentifier::ProtoNode(identifier)) = document.network_interface.reference(node_id, network_path)
		&& identifier.as_str().ends_with("::AttachAttributeNode")
	{
		let mut node_template = resolve_proto_node_type(graphene_std::ops::passthrough::IDENTIFIER)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		if let Some(content) = old_inputs.first() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), content.clone(), network_path);
		}
	}

	// The removed Upload Texture node, in its proto node or older wrapper network form, degrades to a passthrough of its content
	let is_upload_texture = match document.network_interface.reference(node_id, network_path) {
		Some(DefinitionIdentifier::Network(name)) => name == "Upload Texture",
		Some(DefinitionIdentifier::ProtoNode(identifier)) => identifier.as_str().ends_with("::UploadTextureNode"),
		None => false,
	};
	if is_upload_texture {
		let mut node_template = resolve_proto_node_type(graphene_std::ops::passthrough::IDENTIFIER)?.default_node_template();
		document.network_interface.replace_implementation(node_id, network_path, &mut node_template);
		let old_inputs = document.network_interface.replace_inputs(node_id, network_path, &mut node_template)?;
		if let Some(content) = old_inputs.first() {
			document.network_interface.set_input(&InputConnector::node_at_index(*node_id, 0), content.clone(), network_path);
		}
	}

	Some(())
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Runs the upgrades with the root wire cache loaded, the state a migration meets after its first rewire, and asserts the interface they leave behind is consistent.
	fn migrate(document: &mut DocumentMessageHandler) {
		migrate_with_context(document, "the migration");
	}

	fn migrate_with_context(document: &mut DocumentMessageHandler, context: &str) {
		assert!(document.network_interface.outward_wires(&[]).is_some());
		document_migration_upgrades(document, false);

		let violations = document.network_interface.validate_invariants();
		assert!(violations.is_empty(), "{context} left the interface inconsistent:\n{}", violations.join("\n"));
	}

	// Real documents drive every migration at once, on wiring no synthetic fixture reproduces
	#[test]
	fn demo_artwork_migrates_to_a_consistent_interface() {
		use crate::messages::portfolio::document::storage_tests::test_support::load_demo;

		let demo_artwork = [
			"changing-seasons.graphite",
			"isometric-fountain.graphite",
			"marbled-mandelbrot.graphite",
			"painted-dreams.graphite",
			"parametric-dunescape.graphite",
			"procedural-string-lights.graphite",
			"red-dress.graphite",
			"valley-of-spires.graphite",
		];
		for file_name in demo_artwork {
			let mut document = load_demo(file_name);
			migrate_with_context(&mut document, &format!("migrating {file_name}"));
		}
	}

	// The retired "Paint Order" input is migrated by rebuilding the Stroke node's inputs and then reordering it with its Fill node
	#[tokio::test]
	async fn opening_a_stroke_with_a_retired_paint_order_keeps_the_layer_chain() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::messages::tool::common_functionality::graph_modification_utils::get_stroke_paint_order;
		use crate::test_utils::capture_logged_errors;
		use crate::test_utils::test_prelude::*;
		use graph_craft::application_io::resource::HashMapResourceStorage;

		// Each node of the layer's chain from the layer outward, with its offset from the layer if it is chain-positioned
		let chain_layout = |document: &DocumentMessageHandler, layer: NodeId| {
			let interface = &document.network_interface;
			let layer_position = interface.position(&layer, &[]).unwrap_or_default();
			interface
				.layer_chain_nodes(layer, &[])
				.into_iter()
				.map(|node_id| {
					let offset = interface.is_chain(&node_id, &[]).then(|| interface.position(&node_id, &[]).unwrap_or_default() - layer_position);
					(interface.display_name(&node_id, &[]), offset)
				})
				.collect::<Vec<_>>()
		};

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document_mut();
		let layer = document.metadata().all_layers().next().unwrap().to_node();
		let original = chain_layout(document, layer);

		// Save the shape's Stroke node as an older editor did, with "Paint Order" set to draw the stroke below the fill
		let stroke_reference = DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER);
		let stroke_node_id = document
			.network_interface
			.layer_chain_nodes(layer, &[])
			.into_iter()
			.find(|node_id| document.network_interface.reference(node_id, &[]).as_ref() == Some(&stroke_reference))
			.expect("a fresh shape should have a Stroke node in its chain");
		let mut inputs = document.network_interface.document_node(&stroke_node_id, &[]).unwrap().inputs.clone();
		inputs.insert(7, NodeInput::value(TaggedValue::PaintOrder(PaintOrder::StrokeBelow), false));
		let mut legacy_stroke = NodeTemplate { inputs, ..Default::default() };
		legacy_stroke.normalize_input_metadata();
		document.network_interface.replace_inputs(&stroke_node_id, &[], &mut legacy_stroke);
		let serialized = document.serialize_document();

		let (opened, errors) = capture_logged_errors(|| load_legacy_document(serialized, &HashMapResourceStorage::new(), false));
		let opened = opened.expect("the document should open");
		assert!(errors.is_empty(), "opening the document logged errors: {errors:?}");

		// The Fill and Stroke nodes trade places, and the chain keeps every slot
		assert_eq!(get_stroke_paint_order(LayerNodeIdentifier::new_unchecked(layer), &opened.network_interface), PaintOrder::StrokeBelow);
		let swap_name = |name: &str| match name {
			"Fill" => "Stroke".to_string(),
			"Stroke" => "Fill".to_string(),
			other => other.to_string(),
		};
		let expected = original.iter().map(|(name, offset)| (swap_name(name), *offset)).collect::<Vec<_>>();
		assert_eq!(chain_layout(&opened, layer), expected, "the migration should leave the layer's chain intact");
	}

	#[cfg(feature = "gpu")]
	#[tokio::test]
	async fn opening_a_canvas_rasterize_network_upgrades_it_to_the_vello_node() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkTemplate;
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::test_utils::test_prelude::*;
		use graph_craft::application_io::resource::HashMapResourceStorage;
		use graphene_std::platform_application_io::rasterize;
		use graphene_std::transform::Footprint;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let node_id = editor.create_node_by_name_at(DefinitionIdentifier::ProtoNode(rasterize::IDENTIFIER), 0, 0).await;
		let document = editor.active_document_mut();

		let footprint = Footprint {
			resolution: glam::UVec2::new(64, 32),
			..Default::default()
		};
		let footprint_input = NodeInput::value(TaggedValue::Footprint(footprint), false);
		document.network_interface.set_input(&InputConnector::node_at_index(node_id, 1), footprint_input.clone(), &[]);

		// Save the node as the network that drew into a browser canvas
		let canvas_network = NodeNetworkTemplate {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [(
				NodeId(0),
				NodeTemplate {
					implementation: NodeTemplateImplementation::ProtoNode(rasterize::IDENTIFIER),
					inputs: vec![NodeInput::import(list!(graphic_types::Graphic), 0), NodeInput::import(item!(Footprint), 1)],
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};
		let mut legacy = NodeTemplate {
			implementation: NodeTemplateImplementation::Network(canvas_network),
			..Default::default()
		};
		document.network_interface.replace_implementation(&node_id, &[], &mut legacy);
		document.network_interface.set_reference(&node_id, &[], Some("Rasterize".into()));
		let serialized = document.serialize_document();

		let opened = load_legacy_document(serialized, &HashMapResourceStorage::new(), false).expect("the document should open");
		let interface = &opened.network_interface;
		assert_eq!(interface.reference(&node_id, &[]), Some(DefinitionIdentifier::ProtoNode(rasterize::IDENTIFIER)));
		assert_eq!(
			interface.input_from_connector(&InputConnector::node_at_index(node_id, 1), &[]),
			Some(&footprint_input),
			"the footprint should carry over"
		);
	}

	#[cfg(feature = "gpu")]
	#[tokio::test]
	async fn a_rasterize_node_on_the_export_compiles() {
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let rasterize = editor
			.create_node_by_name_at(DefinitionIdentifier::ProtoNode(graphene_std::platform_application_io::rasterize::IDENTIFIER), 0, 0)
			.await;
		editor
			.active_document_mut()
			.network_interface
			.create_wire(&OutputConnector::primary_output(rasterize), &InputConnector::Export(0), &[]);

		editor.handle_message(NodeGraphMessage::RunDocumentGraph).await;
	}

	#[test]
	fn null_floats_load_as_zero() {
		use crate::messages::portfolio::document::utility_types::misc::PTZ;
		use graph_craft::document::value::deserialize_replacing_null_floats_with_zero;

		#[derive(serde::Deserialize)]
		struct Example {
			position: DVec2,
			scale: f64,
			opacity: Option<f64>,
		}

		let content = "{\n\t\"position\": [1.5, null],\n\t\"scale\": null,\n\t\"opacity\": null\n}";
		let example = deserialize_replacing_null_floats_with_zero::<Example>(content).expect("a `null` in place of a float should load");
		assert_eq!(example.position, DVec2::new(1.5, 0.));
		assert_eq!(example.scale, 0.);
		assert_eq!(example.opacity, None);

		// A `null` standing in for something other than a float is still an error
		let content = r#"{"position": null, "scale": 1, "opacity": 1}"#;
		assert!(deserialize_replacing_null_floats_with_zero::<Example>(content).is_err());

		// Node input values are parsed from an intermediate JSON value, which carries no error positions
		let input = serde_json::from_str::<NodeInput>(r#"{"Value":{"tagged_value":{"DVec2":[null,2.5]},"exposed":false}}"#).expect("a `null` in a node input value should load");
		assert_eq!(input.as_value(), Some(&TaggedValue::DVec2(DVec2::new(0., 2.5))));

		// A zoom of 0 would make the view's transform singular
		let ptz = deserialize_replacing_null_floats_with_zero::<PTZ>(r#"{"pan":[0.0,0.0],"tilt":0.0,"zoom":null,"flip":false}"#).expect("a `null` zoom should load");
		assert_eq!(ptz.zoom(), crate::consts::VIEWPORT_ZOOM_SCALE_MIN);
	}

	// The removed-definition blocks above abort silently via `?` if their swap target ever leaves the catalog
	#[test]
	fn removed_definition_swap_targets_resolve() {
		assert!(resolve_proto_node_type(graphene_std::ops::passthrough::IDENTIFIER).is_some());
	}

	#[test]
	fn gpu_twins_name_the_cpu_nodes_they_came_from() {
		assert_eq!(
			cpu_node_of_gpu_twin("raster_nodes::adjustments::desaturate_shader_node::DesaturateShaderNodeNode").as_deref(),
			Some(graphene_std::raster_nodes::adjustments::desaturate::IDENTIFIER.as_str())
		);
		assert_eq!(
			cpu_node_of_gpu_twin("graphene_raster_nodes::blending_nodes::color_overlay_shader_node::ColorOverlayShaderNodeNode").as_deref(),
			Some("graphene_raster_nodes::blending_nodes::ColorOverlayNode")
		);
		assert_eq!(cpu_node_of_gpu_twin(graphene_std::raster_nodes::adjustments::invert::IDENTIFIER.as_str()), None);
	}

	// Upload Texture opens as a passthrough, and GPU twins open as the CPU nodes they were generated from, keeping their wires but not their executor input
	#[test]
	fn gpu_twins_and_upload_texture_open_as_cpu_nodes() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use graph_craft::application_io::resource::HashMapResourceStorage;
		use graphene_std::raster_nodes::adjustments::desaturate;

		let executor = || NodeInput::scope("graphene_std::platform_application_io::WgpuExecutorNode");
		let legacy_node = |identifier: &'static str, inputs: Vec<NodeInput>| NodeTemplate {
			implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
			input_metadata: inputs.iter().map(|_| ("Value", "").into()).collect(),
			inputs,
			..Default::default()
		};

		let desaturate_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(desaturate::IDENTIFIER))
			.expect("the Desaturate definition should exist")
			.default_node_template();
		let content = NodeInput::value(TaggedValue::Color(Color::RED), false);
		let method = desaturate_template.inputs[1].clone();
		let brightness = NodeInput::value(TaggedValue::Number(25.), false);
		let contrast = NodeInput::value(TaggedValue::Number(-10.), false);

		// Uploaded content feeds a Desaturate GPU twin, which feeds a Brightness/Contrast Classic GPU twin saved under the `raster_nodes` crate name
		let saved_nodes = [
			legacy_node("graphene_std::platform_application_io::UploadTextureNode", vec![content.clone(), executor()]),
			legacy_node(
				"raster_nodes::adjustments::desaturate_shader_node::DesaturateShaderNodeNode",
				vec![NodeInput::node(NodeId(0), 0), method.clone(), executor()],
			),
			legacy_node(
				"raster_nodes::adjustments::brightness_contrast_classic_shader_node::BrightnessContrastClassicShaderNodeNode",
				vec![NodeInput::node(NodeId(1), 0), brightness.clone(), contrast.clone(), executor()],
			),
		];
		let nodes = saved_nodes
			.into_iter()
			.enumerate()
			.map(|(index, node_template)| (NodeId(index as u64), node_template))
			.collect::<Vec<_>>();
		let unchanged_ids = nodes.iter().map(|(node_id, _)| (*node_id, *node_id)).collect();
		let mut document = DocumentMessageHandler::default();
		document.network_interface.insert_node_group(nodes, unchanged_ids, &[]);

		let opened = load_legacy_document(document.serialize_document(), &HashMapResourceStorage::new(), false).expect("the document should open");
		let node = |index: u64| opened.network_interface.document_node(&NodeId(index), &[]).expect("the node should still exist");

		let upload_texture = node(0);
		assert_eq!(upload_texture.implementation, DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER));
		assert_eq!(upload_texture.inputs, [content]);

		let desaturate_node = node(1);
		assert_eq!(desaturate_node.implementation, DocumentNodeImplementation::ProtoNode(desaturate::IDENTIFIER));
		let pipeline = NodeInput::scope(graphene_std::raster_nodes::raster_kernel::raster_kernel_pipeline::IDENTIFIER.as_str().to_string());
		assert_eq!(desaturate_node.inputs, [NodeInput::node(NodeId(0), 0), method, pipeline]);

		let brightness_contrast = node(2);
		assert_eq!(
			brightness_contrast.implementation,
			DocumentNodeImplementation::ProtoNode(graphene_std::raster::brightness_contrast::IDENTIFIER)
		);
		assert_eq!(brightness_contrast.inputs[..3], [NodeInput::node(NodeId(1), 0), brightness, contrast]);
		assert_eq!(
			brightness_contrast.inputs[3],
			NodeInput::value(TaggedValue::Bool(true), false),
			"the Classic twin should turn on the Use Classic toggle"
		);
	}

	// Migrating a Merge network's coercion nodes by alias would leave a reducer in the primary slot, so every alias must reset instead
	#[test]
	fn every_into_group_alias_resets_the_merge_definition() {
		let aliases = into_group_aliases().collect::<Vec<_>>();
		assert!(!aliases.is_empty(), "the reset is driven by these aliases, so losing them all would disable it unnoticed");

		for alias in aliases {
			assert!(
				document_migration_reset_node_definition(&format!(r#""implementation":{{"ProtoNode":"{alias}"}}"#)),
				"a document referencing `{alias}` should reset its layer definitions"
			);
		}
	}

	// The Text node produced geometry until it became a string source, so every shape it ever had must reach the
	// current one and gain the converter that turns its string back into geometry
	#[test]
	fn every_legacy_text_shape_gains_its_geometry_converter() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
		use graphene_std::NodeParameter;
		use graphene_std::text::Font;

		// Each era only appended to the one before it, so the shorter shapes are prefixes of this longest pre-alignment one
		let legacy_inputs = [
			NodeInput::scope("editor-api"),
			NodeInput::value(TaggedValue::String("Lorem".into()), false),
			NodeInput::value(TaggedValue::Font(Font::new("Lato".to_string(), "Regular (400)".to_string())), false),
			NodeInput::value(TaggedValue::Number(48.), false),
			NodeInput::value(TaggedValue::Number(1.5), false),
			NodeInput::value(TaggedValue::Number(2.), false),
			NodeInput::value(TaggedValue::Number(0.), false),
			NodeInput::value(TaggedValue::Number(0.), false),
			NodeInput::value(TaggedValue::Number(10.), false),
			NodeInput::value(TaggedValue::Bool(false), false),
		];

		for shape in [4, 6, 8, 9, 10] {
			let (text_id, consumer_id) = (NodeId(1), NodeId(2));
			let mut document = DocumentMessageHandler::default();
			document.network_interface.insert_node(
				text_id,
				NodeTemplate {
					implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::text::TextNode")),
					inputs: legacy_inputs[..shape].to_vec(),
					..Default::default()
				},
				&[],
			);
			document.network_interface.insert_node(
				consumer_id,
				NodeTemplate {
					inputs: vec![NodeInput::value(TaggedValue::None, false)],
					..Default::default()
				},
				&[],
			);
			document.network_interface.set_input(&InputConnector::node_at_index(consumer_id, 0), NodeInput::node(text_id, 0), &[]);

			migrate(&mut document);

			let network = document.network_interface.document_network();
			let text_node = network.nodes.get(&text_id).expect("the upgraded text node should keep its ID");
			assert_eq!(text_node.inputs.len(), 12, "a {shape}-input text node should reach the current shape");

			// The converter is a new node, so it is found by identity rather than by ID
			let converter = network
				.nodes
				.iter()
				.find(|(_, node)| matches!(&node.implementation, DocumentNodeImplementation::ProtoNode(identifier) if *identifier == graphene_std::text::text_to_vector::IDENTIFIER))
				.map(|(converter_id, _)| *converter_id)
				.unwrap_or_else(|| panic!("a {shape}-input text node should gain a string converter"));
			assert_eq!(
				network.nodes[&consumer_id].inputs.first(),
				Some(&NodeInput::node(converter, 0)),
				"the converter should be spliced onto the wire leaving a {shape}-input text node"
			);

			let input_value = |index: usize| text_node.inputs.get(index).and_then(|input| input.as_value()).cloned();
			assert_eq!(input_value(graphene_std::text::text::SizeInput::INDEX), Some(TaggedValue::Number(48.)), "shape {shape} lost its size");
			if shape >= 6 {
				assert_eq!(
					input_value(graphene_std::text::text::LineHeightInput::INDEX),
					Some(TaggedValue::Number(1.5)),
					"shape {shape} lost its line height"
				);
				assert_eq!(
					input_value(graphene_std::text::text::LetterSpacingInput::INDEX),
					Some(TaggedValue::Number(2.)),
					"shape {shape} lost its letter spacing"
				);
			}
			if shape >= 9 {
				assert_eq!(
					input_value(graphene_std::text::text::LetterTiltInput::INDEX),
					Some(TaggedValue::Number(10.)),
					"shape {shape} lost its letter tilt"
				);
			}
		}
	}

	// An expression node saved before the output-type witness input had only its value and expression inputs
	#[test]
	fn expression_nodes_gain_the_output_type_witness() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;

		let node_id = NodeId(1);
		let mut document = DocumentMessageHandler::default();
		document.network_interface.insert_node(
			node_id,
			NodeTemplate {
				implementation: NodeTemplateImplementation::ProtoNode(graphene_std::math_nodes::math_fx::IDENTIFIER),
				inputs: vec![NodeInput::value(TaggedValue::Number(4.), false), NodeInput::value(TaggedValue::String("x^2".into()), false)],
				..Default::default()
			},
			&[],
		);
		migrate(&mut document);

		// The old inputs stay in place and the witness defaults to Number, the output type every such node had
		let node = &document.network_interface.document_network().nodes[&node_id];
		let value_at = |index: usize| node.inputs.get(index).and_then(|input| input.as_value()).cloned();
		assert_eq!(node.inputs.len(), 3);
		assert_eq!(value_at(0), Some(TaggedValue::Number(4.)));
		assert_eq!(value_at(1), Some(TaggedValue::String("x^2".into())));
		assert_eq!(value_at(2), Some(TaggedValue::Number(0.)));
	}

	// The same upgrade on a node whose value comes from upstream, where restoring the inputs has to preserve the wire
	#[test]
	fn expression_nodes_gaining_the_witness_keep_their_upstream_wire() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;

		let (source_id, node_id) = (NodeId(1), NodeId(2));
		let mut document = DocumentMessageHandler::default();
		document.network_interface.insert_node(
			source_id,
			NodeTemplate {
				inputs: vec![NodeInput::value(TaggedValue::None, false)],
				..Default::default()
			},
			&[],
		);
		document.network_interface.insert_node(
			node_id,
			NodeTemplate {
				implementation: NodeTemplateImplementation::ProtoNode(graphene_std::math_nodes::math_fx::IDENTIFIER),
				inputs: vec![NodeInput::value(TaggedValue::Number(4.), false), NodeInput::value(TaggedValue::String("x^2".into()), false)],
				..Default::default()
			},
			&[],
		);
		document.network_interface.set_input(&InputConnector::node_at_index(node_id, 0), NodeInput::node(source_id, 0), &[]);

		migrate(&mut document);

		let node = &document.network_interface.document_network().nodes[&node_id];
		assert_eq!(node.inputs.len(), 3);
		assert!(
			matches!(node.inputs.first(), Some(NodeInput::Node { node_id: upstream, .. }) if *upstream == source_id),
			"the upstream wire was dropped by the upgrade, leaving {:?}",
			node.inputs.first()
		);

		// The wire has to survive in the outward direction too, since a stale cache is what orphans it
		let outward_wires = document.network_interface.outward_wires(&[]).and_then(|wires| wires.get(&OutputConnector::node(source_id, 0)).cloned());
		assert_eq!(outward_wires.as_deref(), Some([InputConnector::node_at_index(node_id, 0)].as_slice()));
	}

	// The old Math node's expression decides its replacement: a static string that never reads `B` becomes "Math f(x)",
	// while a wired `B` or an uninspectable expression becomes "Extend" feeding "Math f(…)"
	#[test]
	fn old_math_nodes_split_into_the_expression_node_pair() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;

		let math_id = NodeId(1);
		let (source_a_id, source_b_id, source_expression_id) = (NodeId(2), NodeId(3), NodeId(4));

		// Builds a document holding one old Math node with the given inputs, plus three source nodes to wire from
		let build_document = |identifier: &'static str, inputs: Vec<NodeInput>| {
			let mut document = DocumentMessageHandler::default();
			for source_id in [source_a_id, source_b_id, source_expression_id] {
				document.network_interface.insert_node(
					source_id,
					NodeTemplate {
						inputs: vec![NodeInput::value(TaggedValue::None, false)],
						..Default::default()
					},
					&[],
				);
			}
			document.network_interface.insert_node(
				math_id,
				NodeTemplate {
					implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
					inputs: vec![NodeInput::value(TaggedValue::None, false); 3],
					..Default::default()
				},
				&[],
			);
			for (index, input) in inputs.into_iter().enumerate() {
				document.network_interface.set_input(&InputConnector::node_at_index(math_id, index), input, &[]);
			}
			document
		};

		let implementation_of = |document: &DocumentMessageHandler, node_id: NodeId| match &document.network_interface.document_network().nodes[&node_id].implementation {
			DocumentNodeImplementation::ProtoNode(identifier) => identifier.clone(),
			other => panic!("expected a proto node implementation, got {other:?}"),
		};
		let find_extend = |document: &DocumentMessageHandler| {
			document
				.network_interface
				.document_network()
				.nodes
				.iter()
				.find(|(_, node)| matches!(&node.implementation, DocumentNodeImplementation::ProtoNode(identifier) if *identifier == graphene_std::graphic::extend::IDENTIFIER))
				.map(|(extend_id, _)| *extend_id)
				.expect("an Extend node should be spliced in")
		};

		// A static expression that never reads `B` becomes "Math f(x)" with `A` renamed to `x` and the `B` input dropped
		{
			let inputs = vec![
				NodeInput::node(source_a_id, 0),
				NodeInput::value(TaggedValue::String("2 - 0.2A".into()), false),
				NodeInput::value(TaggedValue::Number(0.), false),
			];
			let mut document = build_document("math_nodes::MathNode", inputs);
			migrate(&mut document);

			assert_eq!(implementation_of(&document, math_id), graphene_std::math_nodes::math_fx::IDENTIFIER);
			let node = &document.network_interface.document_network().nodes[&math_id];
			assert_eq!(
				node.inputs.len(),
				3,
				"the old `B` input should be dropped, leaving the value, the expression, and the output-type witness"
			);
			assert_eq!(node.inputs.first(), Some(&NodeInput::node(source_a_id, 0)));
			assert_eq!(node.inputs.get(1).and_then(|input| input.as_value()).cloned(), Some(TaggedValue::String("2 - 0.2x".into())));
			assert_eq!(node.inputs.get(2).and_then(|input| input.as_value()).cloned(), Some(TaggedValue::Number(0.)));
		}

		// A constant `B` inlines into the rewritten string as a parenthesized literal; an alias spelling of the old identifier also matches
		{
			let inputs = vec![
				NodeInput::node(source_a_id, 0),
				NodeInput::value(TaggedValue::String("sqrt(A + B) - B^2".into()), false),
				NodeInput::value(TaggedValue::Number(3.), false),
			];
			let mut document = build_document("graphene_core::ops::MathNode", inputs);
			migrate(&mut document);

			assert_eq!(implementation_of(&document, math_id), graphene_std::math_nodes::math_fx::IDENTIFIER);
			let node = &document.network_interface.document_network().nodes[&math_id];
			assert_eq!(
				node.inputs.get(1).and_then(|input| input.as_value()).cloned(),
				Some(TaggedValue::String("sqrt(x + (3.0)) - (3.0)^2".into()))
			);
		}

		// A wired `B` keeps both wires by joining them through a spliced Extend node, with the expression rewritten to the
		// lowercase positional spellings and the old constants escaped behind the `\` prefix that still reaches them
		{
			let inputs = vec![
				NodeInput::node(source_a_id, 0),
				NodeInput::value(TaggedValue::String("A * B + e".into()), false),
				NodeInput::node(source_b_id, 0),
			];
			let mut document = build_document("math_nodes::MathNode", inputs);
			migrate(&mut document);

			assert_eq!(implementation_of(&document, math_id), graphene_std::math_nodes::math_f::IDENTIFIER);
			let extend_id = find_extend(&document);
			let network = document.network_interface.document_network();
			assert_eq!(network.nodes[&math_id].inputs.first(), Some(&NodeInput::node(extend_id, 0)));
			assert_eq!(
				network.nodes[&math_id].inputs.get(1).and_then(|input| input.as_value()).cloned(),
				Some(TaggedValue::String("a * b + \\e".into()))
			);
			assert_eq!(network.nodes[&extend_id].inputs.first(), Some(&NodeInput::node(source_a_id, 0)));
			assert_eq!(network.nodes[&extend_id].inputs.get(1), Some(&NodeInput::node(source_b_id, 0)));
		}

		// A wired expression cannot be inspected, so it splices too, wrapping unwired constant operands as one-item list values
		{
			let inputs = vec![
				NodeInput::value(TaggedValue::Number(2.), true),
				NodeInput::node(source_expression_id, 0),
				NodeInput::value(TaggedValue::Number(5.), false),
			];
			let mut document = build_document("math_nodes::MathNode", inputs);
			migrate(&mut document);

			assert_eq!(implementation_of(&document, math_id), graphene_std::math_nodes::math_f::IDENTIFIER);
			let extend_id = find_extend(&document);
			let network = document.network_interface.document_network();
			assert_eq!(network.nodes[&math_id].inputs.get(1), Some(&NodeInput::node(source_expression_id, 0)));
			assert_eq!(network.nodes[&extend_id].inputs.first(), Some(&NodeInput::value(TaggedValue::Numbers(vec![2.]), true)));
			assert_eq!(network.nodes[&extend_id].inputs.get(1), Some(&NodeInput::value(TaggedValue::Numbers(vec![5.]), false)));
		}
	}

	// The Blend network's Count is now an integer, so an old instance rounds its stored F64 count and swaps the Floor its
	// subgraph used to whole-number the count for the As Number that brings the integer into the network's decimal math
	#[test]
	fn blend_count_narrows_to_i64_and_its_subgraph_is_rebuilt() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplateImplementation;

		let node_id = NodeId(1);
		let mut document = DocumentMessageHandler::default();
		let mut node_template = resolve_network_node_type("Blend").expect("Blend should exist").default_node_template();
		node_template.inputs[1] = NodeInput::value(TaggedValue::Number(6.9), false);
		let NodeTemplateImplementation::Network(network_template) = &mut node_template.implementation else {
			panic!("Blend should be a network node")
		};
		let count_conversion = network_template.nodes.get_mut(&NodeId(3)).expect("Blend should have a count conversion node");
		count_conversion.implementation = NodeTemplateImplementation::ProtoNode(graphene_std::math_nodes::floor::IDENTIFIER);
		count_conversion.inputs = vec![NodeInput::import(item!(f64), 1)];
		document.network_interface.insert_node(node_id, node_template, &[]);

		migrate(&mut document);

		let node = &document.network_interface.document_network().nodes[&node_id];
		assert_eq!(
			node.inputs.get(1).and_then(|input| input.as_value()).cloned(),
			Some(TaggedValue::Integer(6)),
			"the count truncates as the retired Floor did, so the document keeps drawing six"
		);
		let DocumentNodeImplementation::Network(inner) = &node.implementation else {
			panic!("Blend should stay a network node")
		};
		assert_eq!(
			inner.nodes[&NodeId(3)].implementation,
			DocumentNodeImplementation::ProtoNode(graphene_std::math_nodes::as_number::IDENTIFIER)
		);
	}

	// The rebuild above is keyed on the retired Floor that took the Count import, since the subgraph keeps another Floor
	// for its subpath index and a current Blend would otherwise lose any edit made inside it on every load
	#[test]
	fn current_blend_subgraph_survives_migration() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplateImplementation;

		let node_id = NodeId(1);
		let mut document = DocumentMessageHandler::default();
		let mut node_template = resolve_network_node_type("Blend").expect("Blend should exist").default_node_template();
		let NodeTemplateImplementation::Network(network_template) = &mut node_template.implementation else {
			panic!("Blend should be a network node")
		};
		let edited_subtract = network_template.nodes.get_mut(&NodeId(5)).expect("Blend should have an open-path denominator node");
		edited_subtract.inputs[1] = NodeInput::value(TaggedValue::Number(42.), false);
		document.network_interface.insert_node(node_id, node_template, &[]);

		migrate(&mut document);

		let DocumentNodeImplementation::Network(inner) = &document.network_interface.document_network().nodes[&node_id].implementation else {
			panic!("Blend should stay a network node")
		};
		assert_eq!(inner.nodes[&NodeId(5)].inputs.get(1).and_then(|input| input.as_value()).cloned(), Some(TaggedValue::Number(42.)));
	}

	// Merge and Artboard networks once passed their content through a Monitor node for thumbnails, so their rebuilt networks drop it
	#[test]
	fn merge_and_artboard_networks_drop_their_monitor_node() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplateImplementation;

		for name in ["Merge", "Artboard"] {
			let node_id = NodeId(1);
			let mut document = DocumentMessageHandler::default();
			let mut node_template = resolve_network_node_type(name).expect("The definition should exist").default_node_template();
			let NodeTemplateImplementation::Network(network_template) = &mut node_template.implementation else {
				panic!("{name} should be a network node")
			};

			// Splice a Monitor node into the content entering the exported Extend node, where the old network had it
			let Some(NodeInput::Node { node_id: extend_id, .. }) = network_template.exports.first().cloned() else {
				panic!("{name} should export a node")
			};
			let monitor_id = NodeId(100);
			let extend = network_template.nodes.get_mut(&extend_id).expect("The exported node should exist");
			let content = std::mem::replace(&mut extend.inputs[1], NodeInput::node(monitor_id, 0));
			let monitor_template = NodeTemplate {
				inputs: vec![content],
				implementation: NodeTemplateImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER),
				..Default::default()
			};
			network_template.nodes.insert(monitor_id, monitor_template);
			document.network_interface.insert_node(node_id, node_template, &[]);

			migrate(&mut document);

			let DocumentNodeImplementation::Network(network) = &document.network_interface.document_network().nodes[&node_id].implementation else {
				panic!("{name} should stay a network node")
			};
			let monitor = DocumentNodeImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER);
			assert!(
				network.nodes.values().all(|inner_node| inner_node.implementation != monitor),
				"{name} should no longer have a Monitor node"
			);
		}
	}

	// Regex Find's Match Index is now an integer, so an old instance rounds its stored F64 index and rebuilds the subgraph
	// that fed the decimal index to its two item-at-index nodes
	#[test]
	fn regex_find_match_index_narrows_to_i64_and_its_subgraph_is_rebuilt() {
		use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplateImplementation;

		let node_id = NodeId(1);
		let mut document = DocumentMessageHandler::default();
		let mut node_template = resolve_network_node_type("Regex Find").expect("Regex Find should exist").default_node_template();
		node_template.inputs[2] = NodeInput::value(TaggedValue::Number(1.6), false);
		let NodeTemplateImplementation::Network(network_template) = &mut node_template.implementation else {
			panic!("Regex Find should be a network node")
		};
		network_template.nodes.get_mut(&NodeId(0)).expect("Regex Find should have a find node").inputs[2] = NodeInput::import(item!(f64), 2);
		network_template.nodes.get_mut(&NodeId(1)).expect("Regex Find should have an item-at-index node").inputs[1] = NodeInput::value(TaggedValue::Number(0.), false);
		document.network_interface.insert_node(node_id, node_template, &[]);

		migrate(&mut document);

		let node = &document.network_interface.document_network().nodes[&node_id];
		assert_eq!(
			node.inputs.get(2).and_then(|input| input.as_value()).cloned(),
			Some(TaggedValue::Integer(1)),
			"the match index truncates as the retired cast did"
		);
		let DocumentNodeImplementation::Network(inner) = &node.implementation else {
			panic!("Regex Find should stay a network node")
		};
		assert_eq!(inner.nodes[&NodeId(0)].inputs.get(2), Some(&NodeInput::import(item!(i64), 2)));
		assert_eq!(inner.nodes[&NodeId(1)].inputs.get(1).and_then(|input| input.as_value()).cloned(), Some(TaggedValue::Integer(0)));
	}

	#[test]
	fn every_legacy_image_shape_stores_its_resource_as_secondary_input() {
		use graphene_std::raster::Image;

		let resource_id = ResourceId::new();
		let upstream_id = NodeId(2);
		let legacy_inputs = [
			NodeInput::value(TaggedValue::ImageData(Image::new(2, 2, Color::WHITE)), false),
			NodeInput::value(TaggedValue::Resource(resource_id), false),
			NodeInput::node(upstream_id, 0),
		];

		for legacy_input in legacy_inputs {
			let image_id = NodeId(1);
			let mut document = DocumentMessageHandler::default();
			let image_template = NodeTemplate {
				implementation: NodeTemplateImplementation::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER),
				inputs: vec![NodeInput::value(TaggedValue::None, false)],
				..Default::default()
			};
			document.network_interface.insert_node(upstream_id, NodeTemplate::default(), &[]);
			document.network_interface.insert_node(image_id, image_template, &[]);
			document.network_interface.set_input(&InputConnector::node_at_index(image_id, 0), legacy_input.clone(), &[]);

			migrate(&mut document);

			let image_node = &document.network_interface.document_network().nodes[&image_id];
			assert_eq!(image_node.inputs.len(), 3, "the image node should gain its placeholder primary input and pipeline scope input");
			let migrated = &image_node.inputs[1];
			match legacy_input.as_value() {
				None => assert_eq!(migrated.as_node(), Some(upstream_id), "the wire should sit at input 1"),
				Some(legacy_value) => {
					let Some(TaggedValue::Resource(stored)) = migrated.as_value() else {
						panic!("the file should sit at input 1")
					};
					match legacy_value {
						TaggedValue::Resource(legacy) => assert_eq!(stored, legacy, "the stored resource should be kept"),
						_ => assert!(document.resources.registry.contains(stored), "the embedded pixels should be stored as a new resource"),
					}
				}
			}
		}
	}

	#[test]
	fn bare_resource_type_defaults_become_item_defaults() {
		let node_id = NodeId(1);
		let mut document = DocumentMessageHandler::default();
		let image_template = NodeTemplate {
			implementation: NodeTemplateImplementation::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER),
			inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::value(TaggedValue::TypeDefault(concrete!(Resource)), true)],
			..Default::default()
		};
		document.network_interface.insert_node(node_id, image_template, &[]);

		migrate(&mut document);

		let migrated = &document.network_interface.document_network().nodes[&node_id].inputs[1];
		assert_eq!(migrated.as_value(), Some(&TaggedValue::TypeDefault(item!(Resource))), "the bare default should become the item default");
		assert!(matches!(migrated, NodeInput::Value { exposed: true, .. }), "the exposed flag should be kept");
	}

	#[test]
	fn test_no_duplicate_node_replacements() {
		let mut hashmap = HashMap::<ProtoNodeIdentifier, u32>::new();
		NODE_REPLACEMENTS.iter().for_each(|node| {
			*hashmap.entry(node.node.clone()).or_default() += 1;
		});
		let duplicates = hashmap.iter().filter(|(_, count)| **count > 1).map(|(node, _)| node.as_str()).collect::<Vec<_>>();
		if !duplicates.is_empty() {
			panic!("Duplicate entries in `NODE_REPLACEMENTS`: {duplicates:?}");
		}
	}

	// Segment IDs repeat across shapes, so a segment turned on load is followed only by the stored edits downstream of the node that turned it
	#[test]
	fn turned_segments_are_followed_only_downstream() {
		use graph_craft::document::NodeNetwork;
		use graphene_std::vector::misc::HandleId;
		use graphene_std::vector::{SegmentId, VectorModification, VectorModificationType};

		let segment = SegmentId::generate();
		let turned = VectorModification::default().with_segments_turned_on_load(HashSet::from([segment]));
		let edited_on = |handle: HandleId| {
			let mut modification = VectorModification::default();
			modification.modify(&VectorModificationType::SetHandlePosition { handle, relative_position: DVec2::X });
			modification
		};
		let value = |modification: VectorModification| NodeInput::value(TaggedValue::VectorModification(Box::new(modification)), false);
		let node = |inputs: Vec<NodeInput>| DocumentNode { inputs, ..Default::default() };

		// One turned Path node feeds another directly, and a second sits inside a nested network whose export feeds a third
		let [turning, following, unrelated, group, following_group] = [1, 2, 3, 4, 5].map(NodeId);
		let nested = NodeNetwork {
			exports: vec![NodeInput::node(turning, 0)],
			nodes: [(turning, node(vec![value(turned.clone())]))].into_iter().collect(),
			..Default::default()
		};
		let network = NodeNetwork {
			nodes: [
				(turning, node(vec![value(turned)])),
				(following, node(vec![NodeInput::node(turning, 0), value(edited_on(HandleId::primary(segment)))])),
				(unrelated, node(vec![value(edited_on(HandleId::primary(segment)))])),
				(
					group,
					DocumentNode {
						implementation: DocumentNodeImplementation::Network(nested),
						..Default::default()
					},
				),
				(following_group, node(vec![NodeInput::node(group, 0), value(edited_on(HandleId::primary(segment)))])),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};

		let rewritten = modifications_following_turned_segments(&network);
		let rewritten_at = |location: (&[NodeId], NodeId)| rewritten.iter().find(|(path, node_id, ..)| (path.as_slice(), *node_id) == location);

		assert!(rewritten_at((&[], turning)).is_some(), "the record of what turned is taken");
		assert!(rewritten_at((&[group], turning)).is_some(), "the nested record of what turned is taken");
		assert!(rewritten_at((&[], unrelated)).is_none(), "an edit not downstream stays as it was");

		let expected = value(edited_on(HandleId::end(segment)));
		for downstream in [following, following_group] {
			let (.., index, input) = rewritten_at((&[], downstream)).expect("an edit downstream follows the turn");
			assert_eq!(*index, 1);
			assert_eq!(*input, expected);
		}
	}

	// Inside a network fed by a turned node on one input and something else on another, only the edit reading the turned input follows
	#[test]
	fn turned_segments_follow_only_the_network_input_they_enter() {
		use graph_craft::document::NodeNetwork;
		use graphene_std::vector::misc::HandleId;
		use graphene_std::vector::{SegmentId, VectorModification, VectorModificationType};

		let segment = SegmentId::generate();
		let turned = VectorModification::default().with_segments_turned_on_load(HashSet::from([segment]));
		let mut edited = VectorModification::default();
		edited.modify(&VectorModificationType::SetHandlePosition {
			handle: HandleId::primary(segment),
			relative_position: DVec2::X,
		});
		let value = |modification: VectorModification| NodeInput::value(TaggedValue::VectorModification(Box::new(modification)), false);
		let import = |import_index: usize| NodeInput::Import {
			import_type: concrete!(()),
			import_index,
		};
		let node = |inputs: Vec<NodeInput>| DocumentNode { inputs, ..Default::default() };

		let [turning, other, group, reads_turned, reads_other] = [1, 2, 3, 4, 5].map(NodeId);
		let nested = NodeNetwork {
			exports: vec![NodeInput::node(reads_turned, 0)],
			nodes: [(reads_turned, node(vec![import(0), value(edited.clone())])), (reads_other, node(vec![import(1), value(edited)]))]
				.into_iter()
				.collect(),
			..Default::default()
		};
		let network = NodeNetwork {
			nodes: [
				(turning, node(vec![value(turned)])),
				(other, node(Vec::new())),
				(
					group,
					DocumentNode {
						inputs: vec![NodeInput::node(turning, 0), NodeInput::node(other, 0)],
						implementation: DocumentNodeImplementation::Network(nested),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};

		let rewritten = modifications_following_turned_segments(&network);
		let rewritten_at = |node_id: NodeId| rewritten.iter().any(|(path, rewritten_id, ..)| path.as_slice() == [group] && *rewritten_id == node_id);
		assert!(rewritten_at(reads_turned));
		assert!(!rewritten_at(reads_other));
	}

	// Values saved for Merge by Distance load as the methods of the Weld node it became
	#[test]
	fn merge_by_distance_values_load_as_weld_methods() {
		use graphene_std::vector::misc::WeldMethod;

		let load = |json: &str| serde_json::from_str::<TaggedValue>(json).ok();
		assert_eq!(load(r#"{"MergeByDistanceAlgorithm":"Spatial"}"#), Some(TaggedValue::WeldMethod(WeldMethod::All)));
		assert_eq!(load(r#"{"MergeByDistanceAlgorithm":"Topological"}"#), Some(TaggedValue::WeldMethod(WeldMethod::Connected)));
		assert_eq!(load(r#"{"WeldMethod":"FlowEnds"}"#), Some(TaggedValue::WeldMethod(WeldMethod::FlowEnds)));
	}

	// Every kind of Merge by Distance node older editors saved opens as a Weld node, with the method it had and Weld's own title and inputs
	#[test]
	fn merge_by_distance_nodes_open_as_weld_nodes() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::test_utils::capture_logged_errors;
		use graph_craft::application_io::resource::HashMapResourceStorage;
		use graphene_std::vector::misc::WeldMethod;

		let weld = graphene_std::vector::weld::IDENTIFIER;
		let definition = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(weld.clone())).expect("the Weld definition should exist");
		let content = definition.default_node_template().inputs[0].clone();
		let distance = NodeInput::value(TaggedValue::Number(2.), false);

		// Older nodes stored their title and input names, with any "Algorithm" choice saved here as a placeholder method
		let legacy_node = |identifier: &'static str, display_name: &str, placeholder_method: Option<WeldMethod>| {
			let mut inputs = vec![content.clone(), distance.clone()];
			inputs.extend(placeholder_method.map(|method| NodeInput::value(TaggedValue::WeldMethod(method), false)));
			let input_names: &[&str] = if placeholder_method.is_some() {
				&["Content", "Distance", "Algorithm"]
			} else {
				&["Source", "Distance"]
			};

			NodeTemplate {
				implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
				inputs,
				display_name: display_name.to_string(),
				input_metadata: input_names.iter().map(|name| (*name, "").into()).collect(),
				..Default::default()
			}
		};
		let legacy_nodes = [
			legacy_node("core_types::vector::MergeByDistanceNode", "Merge by Distance", Some(WeldMethod::All)),
			legacy_node("graphene_core::vector::MergeByDistanceNode", "Merge by Distance", Some(WeldMethod::Connected)),
			legacy_node("graphene_core::vector::MergeByDistanceNode", "Merge by Distance", None),
			legacy_node("graphene_core::vector::SpatialMergeByDistanceNode", "Spatial Merge by Distance", None),
		];
		let mut document = DocumentMessageHandler::default();
		for (index, node_template) in legacy_nodes.into_iter().enumerate() {
			document.network_interface.insert_node(NodeId(index as u64 + 1), node_template, &[]);
		}

		// The placeholder methods are saved under the old value names, which load as the methods they became
		let mut serialized = document.serialize_document();
		assert!(!serialized.contains("WeldNode"), "the saved document should name only the old node identifiers");
		for (placeholder, saved) in [("All", "Topological"), ("Connected", "Spatial")] {
			let placeholder = format!(r#"{{"WeldMethod":"{placeholder}"}}"#);
			assert_eq!(serialized.matches(&placeholder).count(), 1);
			serialized = serialized.replace(&placeholder, &format!(r#"{{"MergeByDistanceAlgorithm":"{saved}"}}"#));
		}

		let (opened, errors) = capture_logged_errors(|| load_legacy_document(serialized, &HashMapResourceStorage::new(), false));
		let opened = opened.expect("the document should open");
		assert!(errors.is_empty(), "opening the document logged errors: {errors:?}");

		let definition_inputs: Vec<(String, String)> = definition
			.node_template
			.input_metadata
			.iter()
			.map(|metadata| (metadata.persistent_metadata.input_name.clone(), metadata.persistent_metadata.input_description.clone()))
			.collect();
		assert_eq!(definition_inputs.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["Content", "Distance", "Method"]);

		for (node_id, method) in [(1, WeldMethod::Connected), (2, WeldMethod::All), (3, WeldMethod::Connected), (4, WeldMethod::All)].map(|(id, method)| (NodeId(id), method)) {
			let interface = &opened.network_interface;
			let node = interface.document_node(&node_id, &[]).expect("the node should still exist");
			assert_eq!(node.implementation, DocumentNodeImplementation::ProtoNode(weld.clone()), "node {node_id}");
			assert_eq!(node.inputs[1].as_value(), Some(&TaggedValue::Number(2.)), "node {node_id}");
			assert_eq!(node.inputs[2].as_value(), Some(&TaggedValue::WeldMethod(method)), "node {node_id}");
			assert_eq!(interface.display_name(&node_id, &[]), "Weld", "node {node_id}");

			let inputs: Vec<(String, String)> = (0..node.inputs.len()).map(|index| interface.displayed_input_name_and_description(&node_id, index, &[])).collect();
			assert_eq!(inputs, definition_inputs, "node {node_id}");
		}
	}

	// Cut Segments nodes open as Unweld nodes splitting every anchor, still wired to the node feeding them
	#[test]
	fn cut_segments_nodes_open_as_unweld_nodes() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::test_utils::capture_logged_errors;
		use graph_craft::application_io::resource::HashMapResourceStorage;
		use graphene_std::vector::misc::UnweldMethod;

		let unweld = graphene_std::vector::unweld::IDENTIFIER;
		let definition = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(unweld.clone())).expect("the Unweld definition should exist");
		let rectangle = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)).expect("the Rectangle definition should exist");

		// The current Cut Segments node and its oldest form, which stored their titles and single content input
		let content = definition.default_node_template().inputs[0].clone();
		let legacy_node = |identifier: &'static str, display_name: &str| NodeTemplate {
			implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
			inputs: vec![content.clone()],
			display_name: display_name.to_string(),
			input_metadata: vec![("Content", "").into()],
			..Default::default()
		};
		let (upstream, cut_segments, split_segments) = (NodeId(1), NodeId(2), NodeId(3));
		let mut document = DocumentMessageHandler::default();
		document.network_interface.insert_node(upstream, rectangle.default_node_template(), &[]);
		document
			.network_interface
			.insert_node(cut_segments, legacy_node("core_types::vector::CutSegmentsNode", "Cut Segments"), &[]);
		document
			.network_interface
			.insert_node(split_segments, legacy_node("graphene_core::vector::vector_nodes::SplitSegmentsNode", "Split Segments"), &[]);
		document.network_interface.set_input(&InputConnector::node_at_index(cut_segments, 0), NodeInput::node(upstream, 0), &[]);

		let serialized = document.serialize_document();
		assert!(!serialized.contains("UnweldNode"), "the saved document should name only the old node identifiers");

		let (opened, errors) = capture_logged_errors(|| load_legacy_document(serialized, &HashMapResourceStorage::new(), false));
		let mut opened = opened.expect("the document should open");
		assert!(errors.is_empty(), "opening the document logged errors: {errors:?}");

		let definition_inputs: Vec<(String, String)> = definition
			.node_template
			.input_metadata
			.iter()
			.map(|metadata| (metadata.persistent_metadata.input_name.clone(), metadata.persistent_metadata.input_description.clone()))
			.collect();
		for node_id in [cut_segments, split_segments] {
			let interface = &opened.network_interface;
			let node = interface.document_node(&node_id, &[]).expect("the node should still exist");
			assert_eq!(node.implementation, DocumentNodeImplementation::ProtoNode(unweld.clone()), "node {node_id}");
			assert_eq!(
				node.inputs.get(1).and_then(|input| input.as_value()),
				Some(&TaggedValue::UnweldMethod(UnweldMethod::All)),
				"node {node_id}"
			);
			assert_eq!(interface.display_name(&node_id, &[]), "Unweld", "node {node_id}");

			let inputs: Vec<(String, String)> = (0..node.inputs.len()).map(|index| interface.displayed_input_name_and_description(&node_id, index, &[])).collect();
			assert_eq!(inputs, definition_inputs, "node {node_id}");
		}

		// The wire from the upstream node survives in both directions
		let cut_segments_content = opened.network_interface.document_node(&cut_segments, &[]).map(|node| node.inputs[0].clone());
		assert!(matches!(cut_segments_content, Some(NodeInput::Node { node_id, output_index: 0, .. }) if node_id == upstream));
		let outward_wires = opened.network_interface.outward_wires(&[]).and_then(|wires| wires.get(&OutputConnector::node(upstream, 0)).cloned());
		assert_eq!(outward_wires.as_deref(), Some([InputConnector::node_at_index(cut_segments, 0)].as_slice()));
	}

	// Separate Subpaths and Close Path nodes open under their plural names and current input descriptions, still wired to the node feeding them
	#[test]
	fn renamed_path_nodes_open_under_their_new_names() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::test_utils::capture_logged_errors;
		use graph_craft::application_io::resource::HashMapResourceStorage;

		let rectangle = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)).expect("the Rectangle definition should exist");
		let renames = [
			(
				graphene_std::vector::separate_paths::IDENTIFIER,
				"SeparatePathsNode",
				"Separate Paths",
				["core_types::vector::SeparateSubpathsNode", "graphene_core::vector::SeparateSubpathsNode"],
				"Separate Subpaths",
				"Content",
			),
			(
				graphene_std::vector::close_paths::IDENTIFIER,
				"ClosePathsNode",
				"Close Paths",
				["core_types::vector::ClosePathNode", "graphene_core::vector::ClosePathNode"],
				"Close Path",
				"Source",
			),
		];

		for (identifier, identifier_name, title, [current_identifier, oldest_identifier], old_title, input_name) in renames {
			let definition = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(identifier.clone())).expect("the renamed node's definition should exist");

			// The node under its current identifier and its oldest one, both storing their old title
			let legacy_node = |old_identifier: &'static str| NodeTemplate {
				implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(old_identifier)),
				inputs: definition.default_node_template().inputs,
				display_name: old_title.to_string(),
				input_metadata: vec![(input_name, "").into()],
				..Default::default()
			};
			let (upstream, current, oldest) = (NodeId(1), NodeId(2), NodeId(3));
			let mut document = DocumentMessageHandler::default();
			document.network_interface.insert_node(upstream, rectangle.default_node_template(), &[]);
			document.network_interface.insert_node(current, legacy_node(current_identifier), &[]);
			document.network_interface.insert_node(oldest, legacy_node(oldest_identifier), &[]);
			document.network_interface.set_input(&InputConnector::node_at_index(current, 0), NodeInput::node(upstream, 0), &[]);

			let serialized = document.serialize_document();
			assert!(!serialized.contains(identifier_name), "the saved document should name only the old {title} identifiers");

			let (opened, errors) = capture_logged_errors(|| load_legacy_document(serialized, &HashMapResourceStorage::new(), false));
			let opened = opened.expect("the document should open");
			assert!(errors.is_empty(), "opening the document logged errors: {errors:?}");

			for node_id in [current, oldest] {
				let interface = &opened.network_interface;
				let node = interface.document_node(&node_id, &[]).expect("the node should still exist");
				assert_eq!(node.implementation, DocumentNodeImplementation::ProtoNode(identifier.clone()), "{title} node {node_id}");
				assert_eq!(interface.display_name(&node_id, &[]), title, "{title} node {node_id}");

				let expected = definition
					.node_template
					.input_metadata
					.first()
					.map(|metadata| (metadata.persistent_metadata.input_name.clone(), metadata.persistent_metadata.input_description.clone()));
				assert_eq!(Some(interface.displayed_input_name_and_description(&node_id, 0, &[])), expected, "{title} node {node_id}");
			}

			let current_content = opened.network_interface.document_node(&current, &[]).map(|node| node.inputs[0].clone());
			assert!(matches!(current_content, Some(NodeInput::Node { node_id, output_index: 0, .. }) if node_id == upstream), "{title}");
		}
	}

	// Nodes saved under their old identifiers and default titles open as the renamed nodes under their new titles
	#[test]
	fn renamed_nodes_open_under_their_new_names() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::test_utils::capture_logged_errors;
		use graph_craft::application_io::resource::HashMapResourceStorage;

		let renames = [
			(
				graphene_std::animation::step_animation_time::IDENTIFIER,
				"graphene_core::animation::QuantizeAnimationTimeNode",
				"Quantize Animation Time",
				"Step Animation Time",
			),
			(
				graphene_std::animation::step_real_time::IDENTIFIER,
				"graphene_core::animation::QuantizeRealTimeNode",
				"Quantize Real Time",
				"Step Real Time",
			),
			(graphene_std::debug::debug_log::IDENTIFIER, "graphene_core::debug::LogToConsoleNode", "Log to Console", "Debug Log"),
			(
				graphene_std::vector::smooth_anchors::IDENTIFIER,
				"core_types::vector::AutoTangentsNode",
				"Auto-Tangents",
				"Smooth Anchors",
			),
			(
				graphene_std::platform_application_io::bytes_to_image::IDENTIFIER,
				"graphene_std::platform_application_io::DecodeImageNode",
				"Decode Image",
				"Bytes to Image",
			),
			(graphene_std::graphic::indices::IDENTIFIER, "graphic_nodes::graphic::ListIndicesNode", "List Indices", "Indices"),
			(graphene_std::vector::count::IDENTIFIER, "graphene_core::vector::ListLengthNode", "List Length", "Count"),
			(graphene_std::graphic::slice::IDENTIFIER, "graphic_nodes::graphic::ListSliceNode", "List Slice", "Slice"),
			(graphene_std::text_nodes::string_map::IDENTIFIER, "text_nodes::MapStringNode", "Map String", "String Map"),
		];

		// Each node under its old identifier, storing its old default title
		let mut document = DocumentMessageHandler::default();
		for (index, (identifier, old_identifier, old_title, _)) in renames.iter().enumerate() {
			let definition = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(identifier.clone())).expect("the renamed node's definition should exist");
			let mut node_template = definition.default_node_template();
			node_template.implementation = NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(old_identifier));
			node_template.display_name = old_title.to_string();
			document.network_interface.insert_node(NodeId(index as u64), node_template, &[]);
		}

		let serialized = document.serialize_document();
		for (identifier, ..) in &renames {
			assert!(!serialized.contains(identifier.as_str()), "the saved document should name only the old identifiers, not {identifier}");
		}

		let (opened, errors) = capture_logged_errors(|| load_legacy_document(serialized, &HashMapResourceStorage::new(), false));
		let opened = opened.expect("the document should open");
		assert!(errors.is_empty(), "opening the document logged errors: {errors:?}");

		for (index, (identifier, _, _, title)) in renames.iter().enumerate() {
			let node_id = NodeId(index as u64);
			let node = opened.network_interface.document_node(&node_id, &[]).expect("the node should still exist");
			assert_eq!(node.implementation, DocumentNodeImplementation::ProtoNode(identifier.clone()), "{title}");
			assert_eq!(opened.network_interface.display_name(&node_id, &[]), *title);
		}
	}

	#[test]
	fn mix_and_blend_nodes_open_as_mix_colors_with_the_under_color_first() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use graph_craft::application_io::resource::HashMapResourceStorage;
		use graphene_std::raster_nodes::blending_nodes::mix_colors;

		let (over, under) = (Color::RED, Color::BLUE);
		let color_input = |color| NodeInput::value(TaggedValue::Color(color), false);

		// Old Mix and Blend nodes list the over color first, while a node already saved as Mix Colors lists the under color first
		let saved_nodes = [
			(ProtoNodeIdentifier::new("raster_nodes::blending_nodes::MixNode"), [over, under]),
			(ProtoNodeIdentifier::new("raster_nodes::blending_nodes::BlendNode"), [over, under]),
			(mix_colors::IDENTIFIER, [under, over]),
		];

		let mut document = DocumentMessageHandler::default();
		for (index, (identifier, colors)) in saved_nodes.iter().enumerate() {
			let mut node_template = resolve_document_node_type(&DefinitionIdentifier::ProtoNode(mix_colors::IDENTIFIER))
				.expect("the Mix Colors definition should exist")
				.default_node_template();
			node_template.implementation = NodeTemplateImplementation::ProtoNode(identifier.clone());
			node_template.inputs[0] = color_input(colors[0]);
			node_template.inputs[1] = color_input(colors[1]);
			document.network_interface.insert_node(NodeId(index as u64), node_template, &[]);
		}

		let opened = load_legacy_document(document.serialize_document(), &HashMapResourceStorage::new(), false).expect("the document should open");

		for (index, (identifier, _)) in saved_nodes.iter().enumerate() {
			let node = opened.network_interface.document_node(&NodeId(index as u64), &[]).expect("the node should still exist");
			assert_eq!(node.implementation, DocumentNodeImplementation::ProtoNode(mix_colors::IDENTIFIER), "{}", identifier.as_str());
			assert_eq!(node.inputs[0], color_input(under), "{} should take the under color first", identifier.as_str());
			assert_eq!(node.inputs[1], color_input(over), "{} should take the over color second", identifier.as_str());
		}
	}

	// Serialize, Sample Image, and Empty Image nodes open as As String, Passthrough, and Image nodes, keeping their wires and Serialize's stored text
	#[test]
	fn removed_nodes_open_as_their_stand_ins() {
		use crate::messages::portfolio::document_storage_io::load_legacy_document;
		use crate::test_utils::capture_logged_errors;
		use glam::DAffine2;
		use graph_craft::application_io::resource::HashMapResourceStorage;

		let legacy_node = |identifier: &'static str, display_name: &str, inputs: Vec<NodeInput>| NodeTemplate {
			implementation: NodeTemplateImplementation::ProtoNode(ProtoNodeIdentifier::new(identifier)),
			input_metadata: inputs.iter().map(|_| ("Value", "").into()).collect(),
			inputs,
			display_name: display_name.to_string(),
			..Default::default()
		};
		let default_template = |identifier| {
			resolve_document_node_type(&DefinitionIdentifier::ProtoNode(identifier))
				.expect("the definition should exist")
				.default_node_template()
		};

		let (number, image) = (NodeId(1), NodeId(2));
		let (serialize_stored_number, serialize_stored_string, serialize_wired) = (NodeId(3), NodeId(4), NodeId(5));
		let (sample_image, empty_image, oldest_empty_image) = (NodeId(6), NodeId(7), NodeId(8));
		let empty_image_inputs = || {
			vec![
				NodeInput::value(TaggedValue::DAffine2(DAffine2::from_scale(DVec2::splat(10.))), false),
				NodeInput::value(TaggedValue::Color(Color::WHITE), false),
			]
		};

		let mut document = DocumentMessageHandler::default();
		let interface = &mut document.network_interface;
		interface.insert_node(number, default_template(graphene_std::math_nodes::number_value::IDENTIFIER), &[]);
		interface.insert_node(image, default_template(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER), &[]);
		interface.insert_node(
			serialize_stored_number,
			legacy_node("text_nodes::SerializeNode", "Serialize", vec![NodeInput::value(TaggedValue::Number(2.5), false)]),
			&[],
		);
		interface.insert_node(
			serialize_stored_string,
			legacy_node("graphene_core::logic::SerializeNode", "Serialize", vec![NodeInput::value(TaggedValue::String("Hi".into()), false)]),
			&[],
		);
		interface.insert_node(
			serialize_wired,
			legacy_node("text_nodes::SerializeNode", "Serialize", vec![NodeInput::value(TaggedValue::None, false)]),
			&[],
		);
		interface.insert_node(
			sample_image,
			legacy_node("raster_nodes::std_nodes::SampleImageNode", "Sample Image", vec![NodeInput::value(TaggedValue::None, false)]),
			&[],
		);
		interface.insert_node(empty_image, legacy_node("raster_nodes::std_nodes::EmptyImageNode", "Empty Image", empty_image_inputs()), &[]);
		interface.insert_node(oldest_empty_image, legacy_node("graphene_std::raster::EmptyImageNode", "Empty Image", empty_image_inputs()), &[]);
		interface.set_input(&InputConnector::node_at_index(serialize_wired, 0), NodeInput::node(number, 0), &[]);
		interface.set_input(&InputConnector::node_at_index(sample_image, 0), NodeInput::node(image, 0), &[]);

		let serialized = document.serialize_document();
		let (opened, errors) = capture_logged_errors(|| load_legacy_document(serialized, &HashMapResourceStorage::new(), false));
		let opened = opened.expect("the document should open");
		assert!(errors.is_empty(), "opening the document logged errors: {errors:?}");

		let interface = &opened.network_interface;
		let node = |node_id| interface.document_node(&node_id, &[]).expect("the node should still exist");
		let as_string = DocumentNodeImplementation::ProtoNode(graphene_std::text_nodes::as_string::IDENTIFIER);

		// Serialize's stored values keep the JSON text it made of them, and its wire is kept for As String to convert
		for (node_id, expected) in [(serialize_stored_number, "2.5"), (serialize_stored_string, "\"Hi\"")] {
			assert_eq!(node(node_id).implementation, as_string, "node {node_id}");
			assert_eq!(node(node_id).inputs[0].as_value(), Some(&TaggedValue::String(expected.to_string())), "node {node_id}");
			assert_eq!(interface.display_name(&node_id, &[]), "As String", "node {node_id}");
		}
		assert_eq!(node(serialize_wired).implementation, as_string);
		assert!(matches!(node(serialize_wired).inputs[0], NodeInput::Node { node_id, output_index: 0, .. } if node_id == number));

		// Sample Image passes its image through
		assert_eq!(node(sample_image).implementation, DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER));
		assert!(matches!(node(sample_image).inputs[0], NodeInput::Node { node_id, output_index: 0, .. } if node_id == image));
		assert_ne!(interface.display_name(&sample_image, &[]), "Sample Image");

		// Empty Image becomes an Image node with no file
		for node_id in [empty_image, oldest_empty_image] {
			assert_eq!(
				node(node_id).implementation,
				DocumentNodeImplementation::ProtoNode(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER),
				"node {node_id}"
			);
			assert_eq!(
				node(node_id).inputs,
				default_template(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER).inputs,
				"node {node_id}"
			);
			assert_eq!(interface.display_name(&node_id, &[]), "Image", "node {node_id}");
		}
	}
}
