use crate::consts::{VIEWPORT_ROTATE_SNAP_INTERVAL, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_LEVELS, VIEWPORT_ZOOM_MOUSE_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_WHEEL_RATE};
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::Key;
use crate::input::mouse::{ViewportBounds, ViewportPosition};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};

use graphene::document::Document;
use graphene::Operation as DocumentOperation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementMessageHandler {
	pub pan: DVec2,
	panning: bool,
	snap_tilt: bool,
	snap_tilt_released: bool,

	pub tilt: f64,
	tilting: bool,

	pub zoom: f64,
	zooming: bool,
	snap_zoom: bool,

	mouse_position: ViewportPosition,
}

impl Default for MovementMessageHandler {
	fn default() -> Self {
		Self {
			pan: DVec2::ZERO,
			panning: false,
			snap_tilt: false,
			snap_tilt_released: false,

			tilt: 0.,
			tilting: false,

			zoom: 1.,
			zooming: false,
			snap_zoom: false,

			mouse_position: ViewportPosition::default(),
		}
	}
}

impl MovementMessageHandler {
	pub fn snapped_angle(&self) -> f64 {
		let increment_radians: f64 = VIEWPORT_ROTATE_SNAP_INTERVAL.to_radians();
		if self.snap_tilt {
			(self.tilt / increment_radians).round() * increment_radians
		} else {
			self.tilt
		}
	}

	pub fn snapped_scale(&self) -> f64 {
		if self.snap_zoom {
			*VIEWPORT_ZOOM_LEVELS
				.iter()
				.min_by(|a, b| (**a - self.zoom).abs().partial_cmp(&(**b - self.zoom).abs()).unwrap())
				.unwrap_or(&self.zoom)
		} else {
			self.zoom
		}
	}

	pub fn calculate_offset_transform(&self, offset: DVec2) -> DAffine2 {
		// TODO: replace with DAffine2::from_scale_angle_translation and fix the errors
		let offset_transform = DAffine2::from_translation(offset);
		let scale_transform = DAffine2::from_scale(DVec2::splat(self.snapped_scale()));
		let angle_transform = DAffine2::from_angle(self.snapped_angle());
		let translation_transform = DAffine2::from_translation(self.pan);
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

		responses.push_back(
			ArtboardMessage::DispatchOperation(
				DocumentOperation::SetLayerTransform {
					path: vec![],
					transform: self.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
				}
				.into(),
			)
			.into(),
		);
	}

	pub fn center_zoom(&self, viewport_bounds: DVec2, zoom_factor: f64, mouse: DVec2) -> Message {
		let new_viewport_bounds = viewport_bounds / zoom_factor;
		let delta_size = viewport_bounds - new_viewport_bounds;
		let mouse_fraction = mouse / viewport_bounds;
		let delta = delta_size * (DVec2::splat(0.5) - mouse_fraction);

		MovementMessage::TranslateCanvas { delta }.into()
	}
}

impl MessageHandler<MovementMessage, (&Document, &InputPreprocessorMessageHandler)> for MovementMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: MovementMessage, data: (&Document, &InputPreprocessorMessageHandler), responses: &mut VecDeque<Message>) {
		use MovementMessage::*;

		let (document, ipp) = data;

		#[remain::sorted]
		match message {
			DecreaseCanvasZoom { center_on_mouse } => {
				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().rev().find(|scale| **scale < self.zoom).unwrap_or(&self.zoom);
				if center_on_mouse {
					responses.push_back(self.center_zoom(ipp.viewport_bounds.size(), new_scale / self.zoom, ipp.mouse.position));
				}
				responses.push_back(SetCanvasZoom { zoom_factor: new_scale }.into());
			}
			FitViewportToBounds {
				bounds: [bounds_corner_a, bounds_corner_b],
				padding_scale_factor,
				prevent_zoom_past_100,
			} => {
				let pos1 = document.root.transform.inverse().transform_point2(bounds_corner_a);
				let pos2 = document.root.transform.inverse().transform_point2(bounds_corner_b);
				let v1 = document.root.transform.inverse().transform_point2(DVec2::ZERO);
				let v2 = document.root.transform.inverse().transform_point2(ipp.viewport_bounds.size());

				let center = v1.lerp(v2, 0.5) - pos1.lerp(pos2, 0.5);
				let size = (pos2 - pos1) / (v2 - v1);
				let size = 1. / size;
				let new_scale = size.min_element();

				self.pan += center;
				self.zoom *= new_scale;

				self.zoom /= padding_scale_factor.unwrap_or(1.) as f64;

				if self.zoom > 1. && prevent_zoom_past_100 {
					self.zoom = 1.
				}

				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				responses.push_back(DocumentMessage::DirtyRenderDocumentInOutlineView.into());
				responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			IncreaseCanvasZoom { center_on_mouse } => {
				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().find(|scale| **scale > self.zoom).unwrap_or(&self.zoom);
				if center_on_mouse {
					responses.push_back(self.center_zoom(ipp.viewport_bounds.size(), new_scale / self.zoom, ipp.mouse.position));
				}
				responses.push_back(SetCanvasZoom { zoom_factor: new_scale }.into());
			}
			PointerMove {
				snap_angle,
				wait_for_snap_angle_release,
				snap_zoom,
				zoom_from_viewport,
			} => {
				if self.panning {
					let delta = ipp.mouse.position - self.mouse_position;

					responses.push_back(TranslateCanvas { delta }.into());
				}

				if self.tilting {
					let new_snap = ipp.keyboard.get(snap_angle as usize);
					if !(wait_for_snap_angle_release && new_snap && !self.snap_tilt_released) {
						// When disabling snap, keep the viewed rotation as it was previously.
						if !new_snap && self.snap_tilt {
							self.tilt = self.snapped_angle();
						}
						self.snap_tilt = new_snap;
						self.snap_tilt_released = true;
					}

					let half_viewport = ipp.viewport_bounds.size() / 2.;
					let rotation = {
						let start_offset = self.mouse_position - half_viewport;
						let end_offset = ipp.mouse.position - half_viewport;
						start_offset.angle_between(end_offset)
					};

					responses.push_back(SetCanvasRotation { angle_radians: self.tilt + rotation }.into());
				}

				if self.zooming {
					let zoom_start = self.snapped_scale();

					let new_snap = ipp.keyboard.get(snap_zoom as usize);
					// When disabling snap, keep the viewed zoom as it was previously
					if !new_snap && self.snap_zoom {
						self.zoom = self.snapped_scale();
					}
					self.snap_zoom = new_snap;

					let difference = self.mouse_position.y as f64 - ipp.mouse.position.y as f64;
					let amount = 1. + difference * VIEWPORT_ZOOM_MOUSE_RATE;

					self.zoom *= amount;
					if let Some(mouse) = zoom_from_viewport {
						let zoom_factor = self.snapped_scale() / zoom_start;

						responses.push_back(SetCanvasZoom { zoom_factor: self.zoom }.into());
						responses.push_back(self.center_zoom(ipp.viewport_bounds.size(), zoom_factor, mouse));
					} else {
						responses.push_back(SetCanvasZoom { zoom_factor: self.zoom }.into());
					}
				}

				self.mouse_position = ipp.mouse.position;
			}
			RotateCanvasBegin => {
				responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
				responses.push_back(
					FrontendMessage::UpdateInputHints {
						hint_data: HintData(vec![HintGroup(vec![HintInfo {
							key_groups: vec![KeysGroup(vec![Key::KeyControl])],
							key_groups_mac: None,
							mouse: None,
							label: String::from("Snap 15°"),
							plus: false,
						}])]),
					}
					.into(),
				);

				self.tilting = true;
				self.mouse_position = ipp.mouse.position;
			}
			SetCanvasRotation { angle_radians } => {
				self.tilt = angle_radians;
				self.create_document_transform(&ipp.viewport_bounds, responses);
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
			}
			SetCanvasZoom { zoom_factor } => {
				self.zoom = zoom_factor.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				responses.push_back(DocumentMessage::DirtyRenderDocumentInOutlineView.into());
				responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			TransformCanvasEnd => {
				self.tilt = self.snapped_angle();
				self.zoom = self.snapped_scale();
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				responses.push_back(ToolMessage::UpdateCursor.into());
				responses.push_back(ToolMessage::UpdateHints.into());
				self.snap_tilt = false;
				self.snap_tilt_released = false;
				self.snap_zoom = false;
				self.panning = false;
				self.tilting = false;
				self.zooming = false;
			}
			TranslateCanvas { delta } => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta);

				self.pan += transformed_delta;
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			TranslateCanvasBegin => {
				responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Grabbing }.into());
				responses.push_back(FrontendMessage::UpdateInputHints { hint_data: HintData(Vec::new()) }.into());

				self.panning = true;
				self.mouse_position = ipp.mouse.position;
			}
			TranslateCanvasByViewportFraction { delta } => {
				let transformed_delta = document.root.transform.inverse().transform_vector2(delta * ipp.viewport_bounds.size());

				self.pan += transformed_delta;
				responses.push_back(BroadcastSignal::DocumentIsDirty.into());
				self.create_document_transform(&ipp.viewport_bounds, responses);
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				responses.push_back(TranslateCanvas { delta }.into());
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mut zoom_factor = 1. + scroll.abs() * VIEWPORT_ZOOM_WHEEL_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				};

				responses.push_back(self.center_zoom(ipp.viewport_bounds.size(), zoom_factor, ipp.mouse.position));
				responses.push_back(SetCanvasZoom { zoom_factor: self.zoom * zoom_factor }.into());
			}
			ZoomCanvasBegin => {
				responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::ZoomIn }.into());
				responses.push_back(
					FrontendMessage::UpdateInputHints {
						hint_data: HintData(vec![HintGroup(vec![HintInfo {
							key_groups: vec![KeysGroup(vec![Key::KeyControl])],
							key_groups_mac: None,
							mouse: None,
							label: String::from("Snap Increments"),
							plus: false,
						}])]),
					}
					.into(),
				);

				self.zooming = true;
				self.mouse_position = ipp.mouse.position;
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(MovementMessageDiscriminant;
			TranslateCanvasBegin,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasZoom,
			SetCanvasRotation,
			WheelCanvasZoom,
			IncreaseCanvasZoom,
			DecreaseCanvasZoom,
			WheelCanvasTranslate,
			TranslateCanvas,
			TranslateCanvasByViewportFraction,
		);

		if self.panning || self.tilting || self.zooming {
			let transforming = actions!(MovementMessageDiscriminant;
				PointerMove,
				TransformCanvasEnd,
			);
			common.extend(transforming);
		}
		common
	}
}
