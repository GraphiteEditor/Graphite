import type { Layout, LayoutGroup, Widget, WidgetDiff, WidgetInstance } from "@graphite/messages";

type UIItem = Layout | LayoutGroup | WidgetInstance[] | WidgetInstance;
export type WidgetColumn = Extract<LayoutGroup, { column: unknown }>;
export type WidgetRow = Extract<LayoutGroup, { row: unknown }>;
export type WidgetTable = Extract<LayoutGroup, { table: unknown }>;
export type WidgetSection = Extract<LayoutGroup, { section: unknown }>;
type ExtractWidgetKind<T> = T extends Record<infer K, unknown> ? K & string : never;
export type WidgetKind = ExtractWidgetKind<Widget>;

export function isWidgetColumn(layoutGroup: LayoutGroup): layoutGroup is WidgetColumn {
	return "column" in layoutGroup;
}

export function isWidgetRow(layoutGroup: LayoutGroup): layoutGroup is WidgetRow {
	return "row" in layoutGroup;
}

export function isWidgetTable(layoutGroup: LayoutGroup): layoutGroup is WidgetTable {
	return "table" in layoutGroup;
}

export function isWidgetSection(layoutGroup: LayoutGroup): layoutGroup is WidgetSection {
	return "section" in layoutGroup;
}

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
		const diffObject = update.widgetPath.reduce((targetLayout: UIItem | undefined, index: bigint): UIItem | undefined => {
			const i = Number(index);

			if (targetLayout && "column" in targetLayout) return targetLayout.column.columnWidgets[i];
			if (targetLayout && "row" in targetLayout) return targetLayout.row.rowWidgets[i];
			if (targetLayout && "table" in targetLayout) return targetLayout.table.tableWidgets[i];
			if (targetLayout && "section" in targetLayout) return targetLayout.section.layout[i];
			if (targetLayout && "widget" in targetLayout && "widgetId" in targetLayout) {
				if ("PopoverButton" in targetLayout.widget && targetLayout.widget.PopoverButton.popoverLayout) {
					return targetLayout.widget.PopoverButton.popoverLayout[i];
				}
				// eslint-disable-next-line no-console
				console.error("Tried to index widget");
				return targetLayout;
			}

			return targetLayout?.[i];
		}, layout as UIItem);

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
		Object.keys(diffObject).forEach((key) => delete (diffObject as Record<string, unknown>)[key]);

		// Assign keys to the new object
		// `Object.assign` works but `diffObject = update.newValue;` doesn't.
		// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/assign
		Object.assign(diffObject, newContent);
	});
}
