use serde::{Deserialize, Serialize};

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Hash, Eq, Copy, Serialize, Deserialize, specta::Type)]
#[repr(u8)]
pub enum LayoutTarget {
	/// Contains the contents of the dialog, including the title and action buttons. Must be shown with the `FrontendMessage::DisplayDialog` message.
	DialogDetails,
	/// Contains the widgets located directly above the canvas to the right, for example the zoom in and out buttons.
	DocumentBar,
	/// Contains the dropdown for design / select / guide mode found on the top left of the canvas.
	DocumentMode,
	/// Options for opacity seen at the top of the layer tree panel.
	LayerTreeOptions,
	/// The dropdown menu at the very top of the application.
	MenuBar,
	/// Bar at the top of the node graph containing the location and the preview and hide buttons.
	NodeGraphBar,
	/// The bar at the top of the properties panel containing the layer name and icon.
	PropertiesOptions,
	/// The body of the properties panel containing many colapsable sections.
	PropertiesSections,
	/// The bar directly above the canvas, left alligned and to the right of the document mode dropdown.
	ToolOptions,
	/// The vertical buttons for all of the tools on the left of the canvas.
	ToolShelf,
	/// The color swatch for the working colors and a flip and reset button found at the bottom of the tool buttons.
	WorkingColors,

	// KEEP THIS ENUM LAST
	// This is a marker that is used to define an array that is used to hold widgets
	#[remain::unsorted]
	LayoutTargetLength,
}
