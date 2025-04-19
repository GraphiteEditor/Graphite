use crate::consts::{DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS, DRAG_BEYOND_VIEWPORT_SPEED_FACTOR};
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::tool_prelude::*;

#[derive(Clone, Debug, Default)]
pub struct AutoPanning {
	subscribed_to_animation_frame: bool,
}

impl AutoPanning {
	pub fn start(&mut self, messages: &[Message], responses: &mut VecDeque<Message>) {
		if !self.subscribed_to_animation_frame {
			self.subscribed_to_animation_frame = true;

			for message in messages {
				responses.add(BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::AnimationFrame,
					send: Box::new(message.clone()),
				});
			}
		}
	}

	pub fn stop(&mut self, messages: &[Message], responses: &mut VecDeque<Message>) {
		if self.subscribed_to_animation_frame {
			self.subscribed_to_animation_frame = false;

			for message in messages {
				responses.add(BroadcastMessage::UnsubscribeEvent {
					on: BroadcastEvent::AnimationFrame,
					message: Box::new(message.clone()),
				});
			}
		}
	}

	pub fn setup_by_mouse_position(&mut self, input: &InputPreprocessorMessageHandler, messages: &[Message], responses: &mut VecDeque<Message>) {
		let mouse_position = input.mouse.position;
		let viewport_size = input.viewport_bounds.size();
		let is_pointer_outside_edge = mouse_position.x < 0. || mouse_position.x > viewport_size.x || mouse_position.y < 0. || mouse_position.y > viewport_size.y;

		match is_pointer_outside_edge {
			true => self.start(messages, responses),
			false => self.stop(messages, responses),
		}
	}

	/// Shifts the viewport when the mouse reaches the edge of the viewport.
	///
	/// If the mouse was beyond any edge, it returns the amount shifted. Otherwise it returns None.
	/// The shift is proportional to the distance between edge and mouse, and to the duration of the frame.
	/// It is also guaranteed to be integral.
	pub fn shift_viewport(&self, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> Option<DVec2> {
		if !self.subscribed_to_animation_frame {
			return None;
		}

		let viewport_size = input.viewport_bounds.size();
		let mouse_position = input.mouse.position.clamp(
			DVec2::ZERO - DVec2::splat(DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS),
			viewport_size + DVec2::splat(DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS),
		);
		let mouse_position_percent = mouse_position / viewport_size;

		let mut shift_percent = DVec2::ZERO;

		if mouse_position_percent.x < 0. {
			shift_percent.x = -mouse_position_percent.x;
		} else if mouse_position_percent.x > 1. {
			shift_percent.x = 1. - mouse_position_percent.x;
		}

		if mouse_position_percent.y < 0. {
			shift_percent.y = -mouse_position_percent.y;
		} else if mouse_position_percent.y > 1. {
			shift_percent.y = 1. - mouse_position_percent.y;
		}

		if shift_percent.x == 0. && shift_percent.y == 0. {
			return None;
		}

		let time_delta = input.frame_time.frame_duration()?.as_secs_f64();
		let delta = (shift_percent * DRAG_BEYOND_VIEWPORT_SPEED_FACTOR * viewport_size * time_delta).round();
		responses.add(NavigationMessage::CanvasPan { delta });
		Some(delta)
	}
}

#[cfg(test)]
mod test_auto_panning {
	use crate::messages::input_mapper::utility_types::input_mouse::EditorMouseState;
	use crate::messages::input_mapper::utility_types::input_mouse::ScrollDelta;
	use crate::messages::tool::tool_messages::select_tool::SelectToolPointerKeys;
	use crate::test_utils::test_prelude::*;

	#[tokio::test]
	async fn test_select_tool_drawing_box_auto_panning() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 50., 50., 150., 150., ModifierKeys::empty()).await;
		editor.select_tool(ToolType::Select).await;
		// Starting selection box inside viewport
		editor.left_mousedown(100., 100., ModifierKeys::empty()).await;
		// Moving cursor far outside viewport to trigger auto-panning
		editor.move_mouse(2000., 100., ModifierKeys::empty(), MouseKeys::LEFT).await;

		let pointer_keys = SelectToolPointerKeys {
			axis_align: Key::Shift,
			snap_angle: Key::Control,
			center: Key::Alt,
			duplicate: Key::Alt,
		};

		// Sending multiple pointer outside events to simulate auto-panning over time
		for _ in 0..5 {
			editor.handle_message(SelectToolMessage::PointerOutsideViewport(pointer_keys.clone())).await;
		}

		editor
			.mouseup(
				EditorMouseState {
					editor_position: DVec2::new(2000., 100.),
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let document = editor.active_document();
		let selected_node_count = document.network_interface.selected_nodes().selected_nodes_ref().len();
		assert!(selected_node_count > 0, "Selection should have included at least one object");
	}

	#[tokio::test]
	async fn test_select_tool_dragging_auto_panning() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 50., 50., 150., 150., ModifierKeys::empty()).await;
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		let initial_transform = editor.active_document().metadata().transform_to_viewport(layer);
		// Select and start dragging the rectangle
		editor.select_tool(ToolType::Select).await;
		editor.left_mousedown(100., 100., ModifierKeys::empty()).await;

		// Moving cursor outside viewport to trigger auto-panning
		editor.move_mouse(2000., 100., ModifierKeys::empty(), MouseKeys::LEFT).await;

		let pointer_keys = SelectToolPointerKeys {
			axis_align: Key::Shift,
			snap_angle: Key::Control,
			center: Key::Alt,
			duplicate: Key::Alt,
		};

		// Sending multiple outside viewport events to simulate continuous auto-panning
		for _ in 0..5 {
			editor.handle_message(SelectToolMessage::PointerOutsideViewport(pointer_keys.clone())).await;
		}

		editor
			.mouseup(
				EditorMouseState {
					editor_position: DVec2::new(2000., 100.),
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Verifying the rectngle has moved significantly due to auto-panning
		let final_transform = editor.active_document().metadata().transform_to_viewport(layer);
		let translation_diff = (final_transform.translation - initial_transform.translation).length();

		assert!(translation_diff > 10., "Rectangle should have moved significantly due to auto-panning (moved by {})", translation_diff);
	}

	#[tokio::test]
	async fn test_select_tool_resizing_auto_panning() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 50., 50., 150., 150., ModifierKeys::empty()).await;
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		let initial_transform = editor.active_document().metadata().transform_to_viewport(layer);

		editor.select_tool(ToolType::Select).await;
		editor.left_mousedown(150., 150., ModifierKeys::empty()).await; // Click near edge for resize handle
		editor
			.mouseup(
				EditorMouseState {
					editor_position: DVec2::new(150., 150.),
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		editor.handle_message(TransformLayerMessage::BeginScale).await;

		// Moving cursor to trigger auto-panning
		editor.move_mouse(2000., 2000., ModifierKeys::empty(), MouseKeys::LEFT).await;

		let pointer_keys = SelectToolPointerKeys {
			axis_align: Key::Shift,
			snap_angle: Key::Control,
			center: Key::Alt,
			duplicate: Key::Alt,
		};

		// Simulatiing auto-panning for several frames
		for _ in 0..5 {
			editor.handle_message(SelectToolMessage::PointerOutsideViewport(pointer_keys.clone())).await;
		}

		editor.handle_message(TransformLayerMessage::ApplyTransformOperation { final_transform: true }).await;

		// Verifying the transform has changed (scale component should be different)
		let final_transform = editor.active_document().metadata().transform_to_viewport(layer);

		// Comparing transform matrices to detect scale changes
		let initial_scale = initial_transform.matrix2.determinant().sqrt();
		let final_scale = final_transform.matrix2.determinant().sqrt();
		let scale_ratio = final_scale / initial_scale;

		assert!(
			scale_ratio > 1.1 || scale_ratio < 0.9,
			"Rectangle should have been resized due to auto-panning (scale ratio: {})",
			scale_ratio
		);
	}

	#[tokio::test]
	async fn test_artboard_tool_drawing_auto_panning() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.select_tool(ToolType::Artboard).await;
		editor.left_mousedown(100., 100., ModifierKeys::empty()).await;

		// Moving cursor outside viewport to trigger auto-panning
		editor.move_mouse(2000., 100., ModifierKeys::empty(), MouseKeys::LEFT).await;

		// Sending pointer outside event multiple times to simulate auto-panning
		for _ in 0..5 {
			editor
				.handle_message(ArtboardToolMessage::PointerOutsideViewport {
					constrain_axis_or_aspect: Key::Shift,
					center: Key::Alt,
				})
				.await;
		}

		editor
			.mouseup(
				EditorMouseState {
					editor_position: DVec2::new(2000., 100.),
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Verifying that an artboard was created with significant width due to auto-panning
		let artboards = get_artboards(&mut editor).await;
		assert!(!artboards.is_empty(), "Artboard should have been created");
		assert!(
			artboards[0].dimensions.x > 200,
			"Artboard should have significant width due to auto-panning: {}",
			artboards[0].dimensions.x
		);
	}

	#[tokio::test]
	async fn test_artboard_tool_dragging_auto_panning() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 50., 50., 150., 150., ModifierKeys::empty()).await;

		let artboards = get_artboards(&mut editor).await;
		assert!(!artboards.is_empty(), "Artboard should have been created");
		let initial_location = artboards[0].location;

		editor.select_tool(ToolType::Artboard).await;
		editor.left_mousedown(100., 100., ModifierKeys::empty()).await;

		// Moving cursor outside viewport to trigger auto-panning
		editor.move_mouse(2000., 100., ModifierKeys::empty(), MouseKeys::LEFT).await;

		// Sending pointer outside event multiple times to simulate auto-panning
		for _ in 0..5 {
			editor
				.handle_message(ArtboardToolMessage::PointerOutsideViewport {
					constrain_axis_or_aspect: Key::Shift,
					center: Key::Alt,
				})
				.await;
		}

		editor
			.mouseup(
				EditorMouseState {
					editor_position: DVec2::new(2000., 100.),
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Verifying the artboard moved due to auto-panning
		let artboards = get_artboards(&mut editor).await;
		let final_location = artboards[0].location;

		assert!(
			(final_location.x - initial_location.x).abs() > 10 || (final_location.y - initial_location.y).abs() > 10,
			"Artboard should have moved significantly due to auto-panning: {:?} -> {:?}",
			initial_location,
			final_location
		);
	}

	#[tokio::test]
	async fn test_artboard_tool_resizing_auto_panning() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 50., 50., 150., 150., ModifierKeys::empty()).await;

		let artboards = get_artboards(&mut editor).await;
		assert!(!artboards.is_empty(), "Artboard should have been created");
		let initial_dimensions = artboards[0].dimensions;

		// Selecting the artboard and click on edge to prepare for resizing
		editor.select_tool(ToolType::Artboard).await;
		editor.left_mousedown(150., 150., ModifierKeys::empty()).await;

		// Moving cursor outside viewport to trigger auto-panning
		editor.move_mouse(2000., 2000., ModifierKeys::empty(), MouseKeys::LEFT).await;

		// Sending pointer outside event multiple times to simulate auto-panning
		for _ in 0..5 {
			editor
				.handle_message(ArtboardToolMessage::PointerOutsideViewport {
					constrain_axis_or_aspect: Key::Shift,
					center: Key::Alt,
				})
				.await;
		}

		editor
			.mouseup(
				EditorMouseState {
					editor_position: DVec2::new(2000., 2000.),
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Verifying the artboard was resized due to auto-panning
		let artboards = get_artboards(&mut editor).await;
		let final_dimensions = artboards[0].dimensions;

		assert!(
			final_dimensions.x > initial_dimensions.x || final_dimensions.y > initial_dimensions.y,
			"Artboard should have been resized due to auto-panning: {:?} -> {:?}",
			initial_dimensions,
			final_dimensions
		);
	}

	// Helper function to get artboards
	async fn get_artboards(editor: &mut EditorTestUtils) -> Vec<graphene_core::Artboard> {
		let instrumented = editor.eval_graph().await;
		instrumented.grab_all_input::<graphene_core::append_artboard::ArtboardInput>(&editor.runtime).collect()
	}
}
