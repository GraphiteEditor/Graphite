use super::utility_types::TransformOp;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::NodePropertiesContext;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::document::Document;
use document_legacy::layers::layer_info::{LegacyLayer, LegacyLayerType};
use document_legacy::layers::style::{RenderData, ViewMode};

use glam::{DAffine2, DVec2};
use std::f64::consts::PI;

pub fn apply_transform_operation(layer: &LegacyLayer, transform_op: TransformOp, value: f64, render_data: &RenderData) -> [f64; 6] {
	let transformation = match transform_op {
		TransformOp::X => DAffine2::update_x,
		TransformOp::Y => DAffine2::update_y,
		TransformOp::ScaleX | TransformOp::Width => DAffine2::update_scale_x,
		TransformOp::ScaleY | TransformOp::Height => DAffine2::update_scale_y,
		TransformOp::Rotation => DAffine2::update_rotation,
	};

	let scale = match transform_op {
		TransformOp::Width => layer.bounding_transform(render_data).scale_x() / layer.transform.scale_x(),
		TransformOp::Height => layer.bounding_transform(render_data).scale_y() / layer.transform.scale_y(),
		_ => 1.,
	};

	// Apply the operation
	let transform = transformation(layer.transform, value / scale);

	// Return this transform if it is not a dimensions change
	if !matches!(transform_op, TransformOp::ScaleX | TransformOp::Width | TransformOp::ScaleY | TransformOp::Height) {
		return transform.to_cols_array();
	}

	// Find the layerspace pivot
	let pivot = DAffine2::from_translation(layer.transform.transform_point2(layer.layerspace_pivot(render_data)));

	// Find the delta transform
	let mut delta = layer.transform.inverse() * transform;
	if !delta.is_finite() {
		return layer.transform.to_cols_array();
	}

	// Preserve aspect ratio
	if matches!(transform_op, TransformOp::ScaleX | TransformOp::Width) && layer.preserve_aspect {
		let scale_x = layer.transform.scale_x();
		if scale_x != 0. {
			delta = DAffine2::from_scale((1., (value / scale) / scale_x).into()) * delta;
		}
	} else if layer.preserve_aspect {
		let scale_y = layer.transform.scale_y();
		if scale_y != 0. {
			delta = DAffine2::from_scale(((value / scale) / scale_y, 1.).into()) * delta;
		}
	}

	// Transform around pivot
	((pivot * delta * pivot.inverse()) * layer.transform).to_cols_array()
}

pub fn register_artwork_layer_properties(
	document: &Document,
	layer_path: Vec<document_legacy::LayerId>,
	layer: &LegacyLayer,
	responses: &mut VecDeque<Message>,
	persistent_data: &PersistentData,
	node_graph_message_handler: &NodeGraphMessageHandler,
	executor: &mut NodeGraphExecutor,
) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			match &layer.data {
				LegacyLayerType::Folder(_) => IconLabel::new("Folder").tooltip("Folder").widget_holder(),
				LegacyLayerType::Layer(_) => IconLabel::new("Layer").tooltip("Layer").widget_holder(),
			},
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(layer.name.clone().unwrap_or_else(|| "Untitled Layer".to_string()))
				.on_update(|_text_input: &TextInput| panic!("This is presumed to be dead code, but if you are seeing this crash, please file a bug report."))
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			PopoverButton::new("Additional Options", "Coming soon").widget_holder(),
		],
	}];

	let properties_body = match &layer.data {
		LegacyLayerType::Layer(layer) => {
			let mut context = NodePropertiesContext {
				persistent_data,
				document,
				responses,
				nested_path: &node_graph_message_handler.network,
				layer_path: &layer_path,
				executor,
				network: &layer.network,
			};
			let properties_sections = node_graph_message_handler.collate_properties(&mut context);

			properties_sections
		}
		LegacyLayerType::Folder(_) => {
			vec![node_section_transform(layer, persistent_data)]
		}
	};

	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
		layout_target: LayoutTarget::PropertiesOptions,
	});
	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(properties_body)),
		layout_target: LayoutTarget::PropertiesSections,
	});
}

pub fn register_document_graph_properties(mut context: NodePropertiesContext, node_graph_message_handler: &NodeGraphMessageHandler, document_name: &str) {
	let properties_sections = node_graph_message_handler.collate_properties(&mut context);

	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			IconLabel::new("File").tooltip("Document").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(document_name)
				.on_update(|text_input| DocumentMessage::RenameDocument { new_name: text_input.value.clone() }.into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			PopoverButton::new("Additional Options", "Coming soon").widget_holder(),
		],
	}];

	context.responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
		layout_target: LayoutTarget::PropertiesOptions,
	});
	context.responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(properties_sections)),
		layout_target: LayoutTarget::PropertiesSections,
	});
}

fn node_section_transform(layer: &LegacyLayer, persistent_data: &PersistentData) -> LayoutGroup {
	let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::default(), None);
	let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(&render_data));
	LayoutGroup::Section {
		name: "Transform".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Location").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					PivotInput::new(layer.pivot.into())
						.on_update(|pivot_input: &PivotInput| PropertiesPanelMessage::SetPivot { new_position: pivot_input.position }.into())
						.widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.transform.x() + pivot.x))
						.label("X")
						.unit(" px")
						.min(-((1u64 << std::f64::MANTISSA_DIGITS) as f64))
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.x,
								transform_op: TransformOp::X,
							}
							.into()
						})
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(layer.transform.y() + pivot.y))
						.label("Y")
						.unit(" px")
						.min(-((1u64 << std::f64::MANTISSA_DIGITS) as f64))
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.y,
								transform_op: TransformOp::Y,
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Rotation").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.transform.rotation() * 180. / PI))
						.unit("°")
						.mode(NumberInputMode::Range)
						.range_min(Some(-180.))
						.range_max(Some(180.))
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() / 180. * PI,
								transform_op: TransformOp::Rotation,
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Scale").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					CheckboxInput::new(layer.preserve_aspect)
						.icon("Link")
						.tooltip("Preserve Aspect Ratio")
						.on_update(|input: &CheckboxInput| PropertiesPanelMessage::ModifyPreserveAspect { preserve_aspect: input.checked }.into())
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.transform.scale_x()))
						.label("X")
						.unit("")
						.min(0.)
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleX,
							}
							.into()
						})
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(layer.transform.scale_y()))
						.label("Y")
						.unit("")
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleY,
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Dimensions").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.bounding_transform(&render_data).scale_x()))
						.label("W")
						.unit(" px")
						.max((1u64 << f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::Width,
							}
							.into()
						})
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(layer.bounding_transform(&render_data).scale_y()))
						.label("H")
						.unit(" px")
						.max((1u64 << f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::Height,
							}
							.into()
						})
						.widget_holder(),
				],
			},
		],
	}
}

trait DAffine2Utils {
	fn scale_x(&self) -> f64;
	fn update_scale_x(self, new_width: f64) -> Self;
	fn scale_y(&self) -> f64;
	fn update_scale_y(self, new_height: f64) -> Self;
	fn x(&self) -> f64;
	fn update_x(self, new_x: f64) -> Self;
	fn y(&self) -> f64;
	fn update_y(self, new_y: f64) -> Self;
	fn rotation(&self) -> f64;
	fn update_rotation(self, new_rotation: f64) -> Self;
}

impl DAffine2Utils for DAffine2 {
	fn scale_x(&self) -> f64 {
		self.transform_vector2((1., 0.).into()).length()
	}

	fn update_scale_x(self, new_width: f64) -> Self {
		let scale_x = self.scale_x();
		if scale_x != 0. {
			self * DAffine2::from_scale((new_width / scale_x, 1.).into())
		} else {
			self
		}
	}

	fn scale_y(&self) -> f64 {
		self.transform_vector2((0., 1.).into()).length()
	}

	fn update_scale_y(self, new_height: f64) -> Self {
		let scale_y = self.scale_y();
		if scale_y != 0. {
			self * DAffine2::from_scale((1., new_height / scale_y).into())
		} else {
			self
		}
	}

	fn x(&self) -> f64 {
		self.translation.x
	}

	fn update_x(mut self, new_x: f64) -> Self {
		self.translation.x = new_x;
		self
	}

	fn y(&self) -> f64 {
		self.translation.y
	}

	fn update_y(mut self, new_y: f64) -> Self {
		self.translation.y = new_y;
		self
	}

	fn rotation(&self) -> f64 {
		if self.scale_x() != 0. {
			let cos = self.matrix2.col(0).x / self.scale_x();
			let sin = self.matrix2.col(0).y / self.scale_x();
			sin.atan2(cos)
		} else if self.scale_y() != 0. {
			let sin = -self.matrix2.col(1).x / self.scale_y();
			let cos = self.matrix2.col(1).y / self.scale_y();
			sin.atan2(cos)
		} else {
			// Rotation information does not exists anymore in the matrix
			// return 0 for user experience.
			0.
		}
	}

	fn update_rotation(self, new_rotation: f64) -> Self {
		let width = self.scale_x();
		let height = self.scale_y();
		let half_width = width / 2.;
		let half_height = height / 2.;

		let angle_translation_offset = |angle: f64| DVec2::new(-half_width * angle.cos() + half_height * angle.sin(), -half_width * angle.sin() - half_height * angle.cos());
		let angle_translation_adjustment = angle_translation_offset(new_rotation) - angle_translation_offset(self.rotation());

		DAffine2::from_scale_angle_translation((width, height).into(), new_rotation, self.translation + angle_translation_adjustment)
	}
}
