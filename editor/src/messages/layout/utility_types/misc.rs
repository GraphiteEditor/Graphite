use serde::{Deserialize, Serialize};

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Hash, Eq, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum LayoutTarget {
	DialogDetails,
	DocumentBar,
	DocumentMode,
	LayerTreeOptions,
	MenuBar,
	PropertiesOptions,
	PropertiesSections,
	ToolOptions,
	ToolShelf,
	WorkingColors,

	// KEEP THIS ENUM LAST
	// This is a marker that is used to define an array that is used to hold widgets
	#[remain::unsorted]
	LayoutTargetLength,
}
