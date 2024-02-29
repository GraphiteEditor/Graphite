use crate::consts::{
	VIEWPORT_ROTATE_SNAP_INTERVAL, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_LEVELS, VIEWPORT_ZOOM_MIN_FRACTION_COVER, VIEWPORT_ZOOM_MOUSE_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN,
	VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR, VIEWPORT_ZOOM_WHEEL_RATE,
};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, MouseMotion};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::utility_types::document_metadata::DocumentMetadata;
use crate::messages::portfolio::document::utility_types::misc::PTZ;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
enum TransformOperation {
	#[default]
	None,
	Pan {
		pre_commit_pan: DVec2,
	},
	Rotate {
		pre_commit_tilt: f64,
		snap_tilt: bool,
		snap_tilt_released: bool,
	},
	Zoom {
		pre_commit_zoom: f64,
		snap_zoom_enabled: bool,
	},
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigationMessageHandler {
	transform_operation: TransformOperation,
	mouse_position: ViewportPosition,
	finish_operation_with_click: bool,
}

impl Default for NavigationMessageHandler {
	fn default() -> Self {
		Self {
			mouse_position: ViewportPosition::default(),
			finish_operation_with_click: false,
			transform_operation: TransformOperation::None,
		}
	}
}

impl MessageHandler<NavigationMessage, (&DocumentMetadata, Option<[DVec2; 2]>, &InputPreprocessorMessageHandler, Option<[DVec2; 2]>, &mut PTZ)> for NavigationMessageHandler {
	#[remain::check]
	fn process_message(
		&mut self,
		message: NavigationMessage,
		responses: &mut VecDeque<Message>,
		(metadata, document_bounds, ipp, selection_bounds, ptz): (&DocumentMetadata, Option<[DVec2; 2]>, &InputPreprocessorMessageHandler, Option<[DVec2; 2]>, &mut PTZ),
	) {
		use NavigationMessage::*;

		let old_zoom = ptz.zoom;

		#[remain::sorted]
		match message {
			DecreaseCanvasZoom { center_on_mouse } => {
				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().rev().find(|scale| **scale < ptz.zoom).unwrap_or(&ptz.zoom);
				if center_on_mouse {
					responses.add(self.center_zoom(ipp.viewport_bounds.size(), new_scale / ptz.zoom, ipp.mouse.position));
				}
				responses.add(SetCanvasZoom { zoom_factor: new_scale });
			}
			FitViewportToBounds {
				bounds: [pos1, pos2],
				prevent_zoom_past_100,
			} => {
				let v1 = metadata.document_to_viewport.inverse().transform_point2(DVec2::ZERO);
				let v2 = metadata.document_to_viewport.inverse().transform_point2(ipp.viewport_bounds.size());

				let center = v1.lerp(v2, 0.5) - pos1.lerp(pos2, 0.5);
				let size = (pos2 - pos1) / (v2 - v1);
				let size = 1. / size;
				let new_scale = size.min_element();

				ptz.pan += center;
				ptz.zoom *= new_scale * VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR;

				// Keep the canvas filling less than the full available viewport bounds if requested.
				// And if the zoom is close to the full viewport bounds, we ignore the padding because 100% is preferrable if it still fits.
				if prevent_zoom_past_100 && ptz.zoom > VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR {
					ptz.zoom = 1.;
				}

				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
			}
			FitViewportToSelection => {
				if let Some(bounds) = selection_bounds {
					let transform = metadata.document_to_viewport.inverse();
					responses.add(FitViewportToBounds {
						bounds: [transform.transform_point2(bounds[0]), transform.transform_point2(bounds[1])],
						prevent_zoom_past_100: false,
					})
				}
			}
			IncreaseCanvasZoom { center_on_mouse } => {
				let new_scale = *VIEWPORT_ZOOM_LEVELS.iter().find(|scale| **scale > ptz.zoom).unwrap_or(&ptz.zoom);
				if center_on_mouse {
					responses.add(self.center_zoom(ipp.viewport_bounds.size(), new_scale / ptz.zoom, ipp.mouse.position));
				}
				responses.add(SetCanvasZoom { zoom_factor: new_scale });
			}
			PointerMove {
				snap_angle,
				wait_for_snap_angle_release,
				snap_zoom,
				zoom_from_viewport,
			} => {
				match self.transform_operation {
					TransformOperation::None => {}
					TransformOperation::Pan { .. } => {
						let delta = ipp.mouse.position - self.mouse_position;
						responses.add(TranslateCanvas { delta });
					}
					TransformOperation::Rotate {
						snap_tilt,
						snap_tilt_released,
						pre_commit_tilt,
					} => {
						let new_snap = ipp.keyboard.get(snap_angle as usize);

						if !(wait_for_snap_angle_release && new_snap && !snap_tilt_released) {
							// When disabling snap, keep the viewed rotation as it was previously.
							if !new_snap && snap_tilt {
								ptz.tilt = self.snapped_angle(ptz.tilt);
							}
							self.transform_operation = TransformOperation::Rotate {
								pre_commit_tilt,
								snap_tilt: new_snap,
								snap_tilt_released: true,
							};
						}

						let half_viewport = ipp.viewport_bounds.size() / 2.;
						let rotation = {
							let start_offset = self.mouse_position - half_viewport;
							let end_offset = ipp.mouse.position - half_viewport;
							start_offset.angle_between(end_offset)
						};

						responses.add(SetCanvasTilt { angle_radians: ptz.tilt + rotation });
					}
					TransformOperation::Zoom { snap_zoom_enabled, pre_commit_zoom } => {
						let zoom_start = self.snapped_scale(ptz.zoom);

						let new_snap = ipp.keyboard.get(snap_zoom as usize);
						// When disabling snap, keep the viewed zoom as it was previously
						if !new_snap && snap_zoom_enabled {
							ptz.zoom = self.snapped_scale(ptz.zoom);
						}

						if snap_zoom_enabled != new_snap {
							self.transform_operation = TransformOperation::Zoom {
								pre_commit_zoom,
								snap_zoom_enabled: new_snap,
							};
						}

						let difference = self.mouse_position.y - ipp.mouse.position.y;
						let amount = 1. + difference * VIEWPORT_ZOOM_MOUSE_RATE;

						ptz.zoom *= amount;
						ptz.zoom *= Self::clamp_zoom(ptz.zoom, document_bounds, old_zoom, ipp);

						if let Some(mouse) = zoom_from_viewport {
							let zoom_factor = self.snapped_scale(ptz.zoom) / zoom_start;

							responses.add(SetCanvasZoom { zoom_factor: ptz.zoom });
							responses.add(self.center_zoom(ipp.viewport_bounds.size(), zoom_factor, mouse));
						} else {
							responses.add(SetCanvasZoom { zoom_factor: ptz.zoom });
						}
					}
				}

				self.mouse_position = ipp.mouse.position;
			}
			ResetCanvasTiltAndZoomTo100Percent => {
				ptz.tilt = 0.;
				ptz.zoom = 1.;
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
			}
			RotateCanvasBegin { was_dispatched_from_menu } => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
				responses.add(FrontendMessage::UpdateInputHints {
					hint_data: HintData(vec![
						HintGroup(vec![HintInfo {
							key_groups: vec![KeysGroup(vec![Key::Control]).into()],
							key_groups_mac: None,
							mouse: None,
							label: String::from("Snap 15°"),
							plus: false,
							slash: false,
						}]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Confirm")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, "Cancel")]),
					]),
				});

				self.transform_operation = TransformOperation::Rotate {
					pre_commit_tilt: ptz.tilt,
					snap_tilt_released: false,
					snap_tilt: false,
				};

				self.mouse_position = ipp.mouse.position;
				self.finish_operation_with_click = was_dispatched_from_menu;
			}
			SetCanvasTilt { angle_radians } => {
				ptz.tilt = angle_radians;
				self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
			}
			SetCanvasZoom { zoom_factor } => {
				ptz.zoom = zoom_factor.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				ptz.zoom *= Self::clamp_zoom(ptz.zoom, document_bounds, old_zoom, ipp);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
			}
			TransformCanvasEnd { abort_transform } => {
				if abort_transform {
					match self.transform_operation {
						TransformOperation::None => {}
						TransformOperation::Rotate { pre_commit_tilt, .. } => {
							responses.add(SetCanvasTilt { angle_radians: pre_commit_tilt });
						}
						TransformOperation::Pan { pre_commit_pan, .. } => {
							ptz.pan = pre_commit_pan;
							self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
						}
						TransformOperation::Zoom { pre_commit_zoom, .. } => {
							ptz.zoom = pre_commit_zoom;
							responses.add(PortfolioMessage::UpdateDocumentWidgets);
							self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
						}
					}
				}

				ptz.tilt = self.snapped_angle(ptz.tilt);
				ptz.zoom = self.snapped_scale(ptz.zoom);
				responses.add(BroadcastEvent::CanvasTransformed);
				responses.add(ToolMessage::UpdateCursor);
				responses.add(ToolMessage::UpdateHints);
				self.transform_operation = TransformOperation::None;
			}
			TransformFromMenuEnd { commit_key } => {
				let abort_transform = commit_key == Key::Rmb;
				self.finish_operation_with_click = false;
				responses.add(TransformCanvasEnd { abort_transform });
			}
			TranslateCanvas { delta } => {
				let transformed_delta = metadata.document_to_viewport.inverse().transform_vector2(delta);

				ptz.pan += transformed_delta;
				responses.add(BroadcastEvent::CanvasTransformed);
				self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
			}
			TranslateCanvasBegin => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Grabbing });

				responses.add(FrontendMessage::UpdateInputHints {
					hint_data: HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, "Cancel")])]),
				});

				self.mouse_position = ipp.mouse.position;
				self.transform_operation = TransformOperation::Pan { pre_commit_pan: ptz.pan };
			}
			TranslateCanvasByViewportFraction { delta } => {
				let transformed_delta = metadata.document_to_viewport.inverse().transform_vector2(delta * ipp.viewport_bounds.size());

				ptz.pan += transformed_delta;
				self.create_document_transform(ipp.viewport_bounds.center(), ptz, responses);
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				responses.add(TranslateCanvas { delta });
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mut zoom_factor = 1. + scroll.abs() * VIEWPORT_ZOOM_WHEEL_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				}
				zoom_factor *= Self::clamp_zoom(ptz.zoom * zoom_factor, document_bounds, old_zoom, ipp);

				responses.add(self.center_zoom(ipp.viewport_bounds.size(), zoom_factor, ipp.mouse.position));
				responses.add(SetCanvasZoom { zoom_factor: ptz.zoom * zoom_factor });
			}
			ZoomCanvasBegin => {
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::ZoomIn });
				responses.add(FrontendMessage::UpdateInputHints {
					hint_data: HintData(vec![
						HintGroup(vec![HintInfo {
							key_groups: vec![KeysGroup(vec![Key::Control]).into()],
							key_groups_mac: None,
							mouse: None,
							label: String::from("Increments"),
							plus: false,
							slash: false,
						}]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, "Cancel")]),
					]),
				});

				self.transform_operation = TransformOperation::Zoom {
					pre_commit_zoom: ptz.zoom,
					snap_zoom_enabled: false,
				};
				self.mouse_position = ipp.mouse.position;
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(NavigationMessageDiscriminant;
			TranslateCanvasBegin,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasTilt,
			WheelCanvasZoom,
			IncreaseCanvasZoom,
			DecreaseCanvasZoom,
			WheelCanvasTranslate,
			TranslateCanvas,
			TranslateCanvasByViewportFraction,
			FitViewportToSelection,
		);

		if self.transform_operation != TransformOperation::None {
			let transforming = actions!(NavigationMessageDiscriminant;
				PointerMove,
				TransformCanvasEnd,
			);
			common.extend(transforming);
		}

		if self.finish_operation_with_click {
			let transforming_from_menu = actions!(NavigationMessageDiscriminant;
				TransformFromMenuEnd,
			);

			common.extend(transforming_from_menu);
		}

		common
	}
}

impl NavigationMessageHandler {
	pub fn snapped_angle(&self, tilt: f64) -> f64 {
		let increment_radians: f64 = VIEWPORT_ROTATE_SNAP_INTERVAL.to_radians();
		if let TransformOperation::Rotate { snap_tilt: true, .. } = self.transform_operation {
			(tilt / increment_radians).round() * increment_radians
		} else {
			tilt
		}
	}

	pub fn snapped_scale(&self, zoom: f64) -> f64 {
		if let TransformOperation::Zoom { snap_zoom_enabled: true, .. } = self.transform_operation {
			*VIEWPORT_ZOOM_LEVELS.iter().min_by(|a, b| (**a - zoom).abs().partial_cmp(&(**b - zoom).abs()).unwrap()).unwrap_or(&zoom)
		} else {
			zoom
		}
	}

	pub fn calculate_offset_transform(&self, viewport_center: DVec2, pan: DVec2, tilt: f64, zoom: f64) -> DAffine2 {
		let scaled_center = viewport_center / self.snapped_scale(zoom);

		// Try to avoid fractional coordinates to reduce anti aliasing.
		let scale = self.snapped_scale(zoom);
		let rounded_pan = ((pan + scaled_center) * scale).round() / scale - scaled_center;

		// TODO: replace with DAffine2::from_scale_angle_translation and fix the errors
		let offset_transform = DAffine2::from_translation(scaled_center);
		let scale_transform = DAffine2::from_scale(DVec2::splat(scale));
		let angle_transform = DAffine2::from_angle(self.snapped_angle(tilt));
		let translation_transform = DAffine2::from_translation(rounded_pan);
		scale_transform * offset_transform * angle_transform * translation_transform
	}

	fn create_document_transform(&self, viewport_center: DVec2, ptz: &PTZ, responses: &mut VecDeque<Message>) {
		let transform = self.calculate_offset_transform(viewport_center, ptz.pan, ptz.tilt, ptz.zoom);
		responses.add(DocumentMessage::UpdateDocumentTransform { transform });
	}

	pub fn center_zoom(&self, viewport_bounds: DVec2, zoom_factor: f64, mouse: DVec2) -> Message {
		let new_viewport_bounds = viewport_bounds / zoom_factor;
		let delta_size = viewport_bounds - new_viewport_bounds;
		let mouse_fraction = mouse / viewport_bounds;
		let delta = delta_size * (DVec2::splat(0.5) - mouse_fraction);

		NavigationMessage::TranslateCanvas { delta }.into()
	}

	pub fn clamp_zoom(zoom: f64, document_bounds: Option<[DVec2; 2]>, old_zoom: f64, ipp: &InputPreprocessorMessageHandler) -> f64 {
		let document_size = (document_bounds.map(|[min, max]| max - min).unwrap_or_default() / old_zoom) * zoom;
		let scale_factor = (document_size / ipp.viewport_bounds.size()).max_element();

		if scale_factor > f64::EPSILON * 100. && scale_factor.is_finite() && scale_factor < VIEWPORT_ZOOM_MIN_FRACTION_COVER {
			VIEWPORT_ZOOM_MIN_FRACTION_COVER / scale_factor
		} else {
			1.
		}
	}
}
