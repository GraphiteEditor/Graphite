import type { Layout, LayoutGroup, WidgetDiff, WidgetInstance, WidgetSizing } from "/wrapper/pkg/graphite_wasm_wrapper";

// The flexbox styles for the parts of a widget's sizing that are set, leaving the rest to the defaults of the layout it appears in
export function widgetSizingStyles(sizing: Partial<WidgetSizing> | undefined): Record<string, string> {
	const styles: Record<string, string> = {};
	if (typeof sizing?.basis === "number") styles["flex-basis"] = `${sizing.basis}px`;
	if (typeof sizing?.grow === "boolean") styles["flex-grow"] = sizing.grow ? "1" : "0";
	if (typeof sizing?.shrink === "boolean") styles["flex-shrink"] = sizing.shrink ? "1" : "0";
	if (typeof sizing?.minWidth === "number") styles["min-width"] = `${sizing.minWidth}px`;
	if (typeof sizing?.maxWidth === "number") styles["max-width"] = `${sizing.maxWidth}px`;
	return styles;
}

// Writes out styles as the text of an element's `style` attribute
export function styleText(styles: Record<string, string>): string | undefined {
	return (
		Object.entries(styles)
			.map(([property, value]) => `${property}: ${value};`)
			.join(" ") || undefined
	);
}

type UIItem = Layout | LayoutGroup | WidgetInstance[][] | WidgetInstance[] | WidgetInstance;
// Updates a widget layout based on a list of updates, giving the new layout by mutating the `layout` argument
export function patchLayout(layout: /* &mut */ Layout, diffs: WidgetDiff[]) {
	diffs.forEach((update) => {
		// Extract the actual content from the DiffUpdate tagged enum
		const { newValue } = update;
		let newContent: Layout | LayoutGroup | WidgetInstance;
		if ("layout" in newValue) newContent = newValue.layout;
		else if ("layoutGroup" in newValue) newContent = newValue.layoutGroup;
		else if ("widget" in newValue) newContent = newValue.widget;
		else throw new Error("DiffUpdate invalid");

		// Find the object where the diff applies to
		const diffObject = update.widgetPath.reduce((targetLayout, index: bigint): UIItem | undefined => {
			const i = Number(index);

			if (targetLayout && "Column" in targetLayout) return targetLayout.Column.columnWidgets[i];
			if (targetLayout && "Row" in targetLayout) return targetLayout.Row.rowWidgets[i];
			if (targetLayout && "Table" in targetLayout) return targetLayout.Table.tableWidgets[i];
			if (targetLayout && "NodeSection" in targetLayout) return targetLayout.NodeSection.layout[i];
			if (targetLayout && "widget" in targetLayout && "widgetId" in targetLayout) {
				if ("PopoverButton" in targetLayout.widget && targetLayout.widget.PopoverButton.popoverLayout) {
					return targetLayout.widget.PopoverButton.popoverLayout[i];
				}
				// eslint-disable-next-line no-console
				console.error("Tried to index widget");
				return targetLayout;
			}

			return targetLayout?.[i];
		}, layout);

		// Exit if we failed to produce a valid patch for the existing layout.
		// This means that the backend assumed an existing layout that doesn't exist in the frontend. This can happen, for
		// example, if a panel is destroyed in the frontend but was never cleared in the backend, so the next time the backend
		// tries to update the layout, it attempts to insert only the changes against the old layout that no longer exists.
		if (diffObject === undefined) {
			// eslint-disable-next-line no-console
			console.error("In `patchLayout`, the `diffObject` is undefined. The layout has not been updated. See the source code comment above this error for hints.");
			return;
		}

		// If this is a list with a length, then set the length to 0 to clear the list
		if ("length" in diffObject) {
			diffObject.length = 0;
		}
		// Remove all of the keys from the old object
		Object.keys(diffObject).forEach((key) => Reflect.deleteProperty(diffObject, key));

		// Assign keys to the new object
		// `Object.assign` works but `diffObject = update.newValue;` doesn't.
		// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/assign
		Object.assign(diffObject, newContent);
	});
}
