mod color;
mod color_palette;
mod layout_abstract_syntax;
mod layout_abstract_types;
mod layout_attribute_parser;
mod layout_system;
mod resource_cache;
mod window_dom;

use bevy::{prelude::*, render::pass::ClearColor};

use layout_system::LayoutSystem;

// Function initializing the logging system
fn logger() {
	// Display graphics API errors (requires Vulkan SDK is installed)
	#[cfg(feature = "debug")]
	env_logger::init();
}

// Function creating the layout system components
fn layout(mut commands: Commands) {
	// Main window in the XML layout language
	let mut main_window_layout = LayoutSystem::new();
	main_window_layout.add_window(("window", "main"));

	// The layout system has a single component.
	let components = (main_window_layout,);

	commands.spawn(components);
}

// Function initializing the 2D graphics system
fn graphics(mut commands: Commands, asset_server: Res<AssetServer>, mut materials: ResMut<Assets<ColorMaterial>>) {
	// Create a new 2D camera for our window's viewport
	commands.spawn(UiCameraComponents::default());

	// Load a sample texture and render it
	let texture_handle = asset_server.load("textures/grid.png").unwrap();
	commands
		// Create a node for the window
		.spawn(NodeComponents {
			style: Style {
				size: Size {
					width: Val::Percent(100.0),
					height: Val::Percent(100.0),
				},
				flex_direction: FlexDirection::Column,
				..Default::default()
			},
			..Default::default()
		})
		.with_children(|parent| {
			// Header
			parent.spawn(NodeComponents {
				material: materials.add(Color::rgb(1.0, 0.0, 0.0).into()),
				style: Style {
					size: Size { width: Val::Percent(100.0), height: Val::Px(28.0), },
					..Default::default()
				},
				..Default::default()
			});
			// Viewport
			parent.spawn(ImageComponents {
				material: materials.add(texture_handle.into()),
				style: Style {
					size: Size {
						width: Val::Percent(100.0),
						height: Val::Percent(100.0),
					},
					..Default::default()
					},
				..Default::default()
			});
			// Footer
			parent.spawn(NodeComponents {
				material: materials.add(Color::rgb(0.0, 0.0, 1.0).into()),
				style: Style {
					size: Size { width: Val::Percent(100.0), height: Val::Px(14.0), },
					..Default::default()
				},
				..Default::default()
			});
		});
}

fn main() {
	App::build()
        .add_resource(ClearColor(Color::BLACK))
        .add_resource(WindowDescriptor {
            title: "Graphite".to_string(),
            ..Default::default()
        })
		// TODO: we might not need all of the default plugins
		.add_default_plugins()
		.add_startup_system(logger.system())
		.add_startup_system(layout.system())
		.add_startup_system(graphics.system())
		.run();
}
