use crate::application::Application;
use winit::event::*;
use winit::event_loop::ControlFlow;

pub fn window_event(application: &mut Application, control_flow: &mut ControlFlow, event: &WindowEvent) {
	match event {
		WindowEvent::Resized(physical_size) => resize(application, *physical_size),
		WindowEvent::Moved(_) => (),
		WindowEvent::CloseRequested => quit(control_flow),
		WindowEvent::Destroyed => (),
		WindowEvent::DroppedFile(_) => (),
		WindowEvent::HoveredFile(_) => (),
		WindowEvent::HoveredFileCancelled => (),
		WindowEvent::ReceivedCharacter(_) => (),
		WindowEvent::Focused(_) => (),
		WindowEvent::KeyboardInput { input, .. } => keyboard_event(application, control_flow, input),
		WindowEvent::CursorMoved { .. } => (),
		WindowEvent::CursorEntered { .. } => (),
		WindowEvent::CursorLeft { .. } => (),
		WindowEvent::MouseWheel { .. } => (),
		WindowEvent::MouseInput { .. } => (),
		WindowEvent::TouchpadPressure { .. } => (),
		WindowEvent::AxisMotion { .. } => (),
		WindowEvent::Touch(_) => (),
		WindowEvent::ScaleFactorChanged { new_inner_size, .. } => resize(application, **new_inner_size),
		WindowEvent::ThemeChanged(_) => (),
	}
}

fn keyboard_event(application: &mut Application, control_flow: &mut ControlFlow, input: &KeyboardInput) {
	match input {
		KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Escape), .. } => quit(control_flow),
		KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Space), .. } => {
			// const VERTICES: &[[f32; 2]] = &[
			// 	[-0.2, 0.0],
			// 	[0.2, 0.0],
			// 	[0.2, -0.5],
			// 	[-0.2, -0.5],
			// ];
			// const INDICES: &[u16] = &[
			// 	0, 1, 2,
			// 	0, 2, 3,
			// ];

			// application.example(VERTICES, INDICES);
		},
		_ => *control_flow = ControlFlow::Wait,
	}
}

fn quit(control_flow: &mut ControlFlow) {
	*control_flow = ControlFlow::Exit;
}

fn resize(application: &mut Application, new_size: winit::dpi::PhysicalSize<u32>) {
	application.swap_chain_descriptor.width = new_size.width;
	application.swap_chain_descriptor.height = new_size.height;

	application.swap_chain = application.device.create_swap_chain(&application.surface, &application.swap_chain_descriptor);

	// TODO: Mark root of GUI as dirty to force redraw of everything
}
