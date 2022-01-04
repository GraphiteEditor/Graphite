use crate::consts::VIEWPORT_ROTATE_SNAP_INTERVAL;
pub use crate::document::layer_panel::*;
use crate::document::DocumentMessage;
use crate::input::keyboard::Key;
use crate::message_prelude::*;
use crate::{
	consts::{VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_LEVELS, VIEWPORT_ZOOM_MOUSE_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_WHEEL_RATE},
	input::{mouse::ViewportBounds, mouse::ViewportPosition, InputPreprocessor},
};
use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[impl_message(Message, DocumentMessage, Movement)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MovementMessage {
	MouseMove {
		snap_angle: Key,
		wait_for_snap_angle_release: bool,
		snap_zoom: Key,
		zoom_from_viewport: Option<DVec2>,
	},
	TranslateCanvasBegin,
	WheelCanvasTranslate {
		use_y_as_x: bool,
	},
	RotateCanvasBegin,
	ZoomCanvasBegin,
	TransformCanvasEnd,
	SetCanvasRotation(f64),
	SetCanvasZoom(f64),
	IncreaseCanvasZoom,
	DecreaseCanvasZoom,
	WheelCanvasZoom,
	ZoomCanvasToFitAll,
	TranslateCanvas(DVec2),
	TranslateCanvasByViewportFraction(DVec2),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementMessageHandler {
	translating: bool,
	pub translation: DVec2,

	rotating: bool,
	pub rotation: f64,

	zooming: bool,
	pub scale: f64,

	snap_rotate: bool,
	snap_scale: bool,
	released_snap_rotate: bool,

	mouse_pos: ViewportPosition,
}

impl Default for MovementMessageHandler {
	fn default() -> Self {
		Self {
			scale: 1.,
			translating: false,
			translation: DVec2::ZERO,
			rotating: false,
			rotation: 0.,
			zooming: false,
			snap_rotate: false,
			released_snap_rotate: false,
			snap_scale: false,
			mouse_pos: ViewportPosition::default(),
		}
	}
}

impl MovementMessageHandler {
	pub fn snapped_angle(&self) -> f64 {
		let increment_radians: f64 = VIEWPORT_ROTATE_SNAP_INTERVAL.to_radians();
		if self.snap_rotate {
			(self.rotation / increment_radians).round() * increment_radians
		} else {
			self.rotation
		}
	}
	pub fn snapped_scale(&self) -> f64 {
		if self.snap_scale {
			*VIEWPORT_ZOOM_LEVELS
				.iter()
				.min_by(|a, b| (**a - self.scale).abs().partial_cmp(&(**b - self.scale).abs()).unwrap())
				.unwrap_or(&self.scale)
		} else {
			self.scale
		}
	}
	pub fn calculate_offset_transform(&self, offset: DVec2) -> DAffine2 {
		// TODO: replace with DAffine2::from_scale_angle_translation and fix the errors
		let offset_transform = DAffine2::from_translation(offset);
		let scale_transform = DAffine2::from_scale(DVec2::splat(self.snapped_scale()));
		let angle_transform = DAffine2::from_angle(self.snapped_angle());
		let translation_transform = DAffine2::from_translation(self.translation);
		scale_transform * offset_transform * angle_transform * translation_transform
	}

	fn create_document_transform(&self, viewport_bounds: &ViewportBounds, responses: &mut VecDeque<Message>) {
		let half_viewport = viewport_bounds.size() / 2.;
		let scaled_half_viewport = half_viewport / self.snapped_scale();
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path: vec![],
				transform: self.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
			}
			.into(),
		);
	}
	pub fn centre_zoom(&self, viewport_bounds: DVec2, zoom_factor: f64, mouse: DVec2) -> [Message; 2] {
		let new_viewport_bounds = viewport_bounds / zoom_factor;
		let delta_size = viewport_bounds - new_viewport_bounds;
		let mouse_fraction = mouse / viewport_bounds;
		let delta = delta_size * (DVec2::splat(0.5) - mouse_fraction);

		[MovementMessage::TranslateCanvas(delta).into(), MovementMessage::SetCanvasZoom(self.scale * zoom_factor).into()]
	}
}

impl MessageHandler<MovementMessage, (&Document, &InputPreprocessor)> for MovementMessageHandler {
	fn process_action(&mut self, message: MovementMessage, data: (&Document, &InputPreprocessor), responses: &mut VecDeque<Message>) {
		let (document, ipp) = data;
		use MovementMessage::*;
		match message {
			TranslateCanvasBegin => {
				self.translating = true;
				self.mouse_pos = ipp.mouse.position;
			}
			RotateCanvasBegin => {
				self.rotating = true;
				self.mouse_pos = ipp.mouse.position;
			}
			ZoomCanvasBegin => {
				self.zooming = true;
				self.mouse_pos = ipp.mouse.position;
			}
			TransformCanvasEnd => {
				self.rotation = self.snapped_angle();
				self.scale = self.snapped_scale();
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				self.snap_rotate = false;
				self.released_snap_rotate = false;
				self.snap_scale = false;
				self.translating = false;
				self.rotating = false;
				self.zooming = false;
			}
			MouseMove {
				snap_angle,
				wait_for_snap_angle_release,
				snap_zoom,
				zoom_from_viewport,
			} => {
				if self.translating {
					let delta = ipp.mouse.position - self.mouse_pos;

					responses.push_back(TranslateCanvas(delta).into());
				}

				if self.rotating {
					let new_snap = ipp.keyboard.get(snap_angle as usize);
					if !(wait_for_snap_angle_release && new_snap && !self.released_snap_rotate) {
						// When disabling snap, keep the viewed rotation as it was previously.
						if !new_snap && self.snap_rotate {
							self.rotation = self.snapped_angle();
						}
						self.snap_rotate = new_snap;
						self.released_snap_rotate = true;
					}

					let half_viewport = ipp.viewport_bounds.size() / 2.;
					let rotation = {
						let start_vec = self.mouse_pos - half_viewport;
						let end_vec = ipp.mouse.position - half_viewport;
						start_vec.angle_between(end_vec)
					};

					responses.push_back(SetCanvasRotation(self.rotation + rotation).into());
				}
				if self.zooming {
					let new_snap = ipp.keyboard.get(snap_zoom as usize);
					// When disabling snap, keep the viewed zoom as it was previously.
					if !new_snap && self.snap_scale {
						self.scale = self.snapped_scale();
					}
					self.snap_scale = new_snap;

					let difference = self.mouse_pos.y as f64 - ipp.mouse.position.y as f64;
					let amount = 1. + difference * VIEWPORT_ZOOM_MOUSE_RATE;

					if let Some(mouse) = zoom_from_viewport {
						responses.extend(self.centre_zoom(ipp.viewport_bounds.size(), amount, mouse));
					} else {
						responses.push_back(SetCanvasZoom(self.scale * amount).into());
					}
				}
				self.mouse_pos = ipp.mouse.position;
			}
			SetCanvasZoom(new) => {
				self.scale = new.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: self.snapped_scale() }.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(DocumentMessage::DirtyRenderDocumentInOutlineView.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			IncreaseCanvasZoom => {
				responses.push_back(SetCanvasZoom(*VIEWPORT_ZOOM_LEVELS.iter().find(|scale| **scale > self.scale).unwrap_or(&self.scale)).into());
			}
			DecreaseCanvasZoom => {
				responses.push_back(SetCanvasZoom(*VIEWPORT_ZOOM_LEVELS.iter().rev().find(|scale| **scale < self.scale).unwrap_or(&self.scale)).into());
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mut zoom_factor = 1. + scroll.abs() * VIEWPORT_ZOOM_WHEEL_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				};

				responses.extend(self.centre_zoom(ipp.viewport_bounds.size(), zoom_factor, ipp.mouse.position));
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				responses.push_back(TranslateCanvas(delta).into());
			}
			SetCanvasRotation(new_radians) => {
				self.rotation = new_radians;
				self.create_document_transform(&ipp.viewport_bounds, responses);
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(FrontendMessage::SetCanvasRotation { new_radians: self.snapped_angle() }.into());
			}
			ZoomCanvasToFitAll => {
				if let Some([pos1, pos2]) = document.visible_layers_bounding_box() {
					let pos1 = document.root.transform.inverse().transform_point2(pos1);
					let pos2 = document.root.transform.inverse().transform_point2(pos2);
					let v1 = document.root.transform.inverse().transform_point2(DVec2::ZERO);
					let v2 = document.root.transform.inverse().transform_point2(ipp.viewport_bounds.size());

					let center = v1.lerp(v2, 0.5) - pos1.lerp(pos2, 0.5);
					let size = (pos2 - pos1) / (v2 - v1);
					let size = 1. / size;
					let new_scale = size.min_element();

					self.translation += center;
					self.scale *= new_scale;
					responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: self.scale }.into());
					responses.push_back(ToolMessage::DocumentIsDirty.into());
					responses.push_back(DocumentMessage::DirtyRenderDocumentInOutlineView.into());
					self.create_document_transform(&ipp.viewport_bounds, responses);
				}
			}
			TranslateCanvas(delta) => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);

				self.translation += transformed_delta;
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			TranslateCanvasByViewportFraction(delta) => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta * ipp.viewport_bounds.size());

				self.translation += transformed_delta;
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(MovementMessageDiscriminant;
			MouseMove,
			TranslateCanvasBegin,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasZoom,
			SetCanvasRotation,
			WheelCanvasZoom,
			IncreaseCanvasZoom,
			DecreaseCanvasZoom,
			WheelCanvasTranslate,
			ZoomCanvasToFitAll,
			TranslateCanvas,
			TranslateCanvasByViewportFraction,
		);

		if self.translating || self.rotating || self.zooming {
			let transforming = actions!(MovementMessageDiscriminant;
				TransformCanvasEnd,
			);
			common.extend(transforming);
		}
		common
	}
}
