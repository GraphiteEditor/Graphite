declare global {
	namespace Graphite {
		type Platform = "Windows" | "Mac" | "Linux" | "Web";
		type MenuType = "Popover" | "Dropdown" | "Dialog" | "Cursor";
		type Axis = "Horizontal" | "Vertical";

		// interface Error {}
	}
}

export { Graphite };
