import type { Layout, LayoutGroup, UIItem, WidgetDiff, WidgetInstance, WidgetSection, WidgetSpanColumn, WidgetSpanRow, WidgetTable } from "@graphite/messages";

export function isWidgetSpanColumn(layoutColumn: LayoutGroup): layoutColumn is WidgetSpanColumn {
	return Boolean((layoutColumn as WidgetSpanColumn)?.columnWidgets);
}

export function isWidgetSpanRow(layoutRow: LayoutGroup): layoutRow is WidgetSpanRow {
	return Boolean((layoutRow as WidgetSpanRow)?.rowWidgets);
}

export function isWidgetTable(layoutTable: LayoutGroup): layoutTable is WidgetTable {
	return Boolean((layoutTable as WidgetTable)?.tableWidgets);
}

export function isWidgetSection(layoutRow: LayoutGroup): layoutRow is WidgetSection {
	return Boolean((layoutRow as WidgetSection)?.layout);
}

/// Unwraps the Serde tagged enum `{ widgetId, widget: { Kind: props } }` into `{ widgetId, props: { kind, ...props } }`
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseWidgetInstance(widgetInstance: any): WidgetInstance {
	const widgetId = widgetInstance.widgetId;

	const kind = Object.keys(widgetInstance.widget)[0];
	const props = widgetInstance.widget[kind];
	props.kind = kind;

	return { widgetId, props };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function parseWidgetDiffs(rawDiffs: any): WidgetDiff[] {
	return rawDiffs.map((diff: WidgetDiff) => {
		const { widgetPath, newValue } = diff;

		if ("layout" in newValue) return { widgetPath, newValue: newValue.layout.map(createLayoutGroup) };
		if ("layoutGroup" in newValue) return { widgetPath, newValue: createLayoutGroup(newValue.layoutGroup) };
		if ("widget" in newValue) return { widgetPath, newValue: parseWidgetInstance(newValue.widget) };

		// This code should be unreachable
		throw new Error("DiffUpdate invalid");
	});
}

// Updates a widget layout based on a list of updates, giving the new layout by mutating the `layout` argument
export function patchLayout(layout: /* &mut */ Layout, diffs: WidgetDiff[]) {
	diffs.forEach((update) => {
		// Find the object where the diff applies to
		const diffObject = update.widgetPath.reduce((targetLayout: UIItem | undefined, index: bigint): UIItem | undefined => {
			const i = Number(index);

			if (targetLayout && "columnWidgets" in targetLayout) return targetLayout.columnWidgets[i];
			if (targetLayout && "rowWidgets" in targetLayout) return targetLayout.rowWidgets[i];
			if (targetLayout && "tableWidgets" in targetLayout) return targetLayout.tableWidgets[i];
			if (targetLayout && "layout" in targetLayout) return targetLayout.layout[i];
			if (targetLayout && "props" in targetLayout && "widgetId" in targetLayout) {
				if (targetLayout.props.kind === "PopoverButton" && "popoverLayout" in targetLayout.props && targetLayout.props.popoverLayout) {
					targetLayout.props.popoverLayout = targetLayout.props.popoverLayout.map(createLayoutGroup);
					return targetLayout.props.popoverLayout[i];
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
		Object.assign(diffObject, update.newValue);
	});
}

// Unpacking a layout group
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function createLayoutGroup(layoutGroup: any): LayoutGroup {
	// Detect if this has already been parsed and, if so, return it as-is so this function can be idempotent
	if ("columnWidgets" in layoutGroup || "rowWidgets" in layoutGroup || "tableWidgets" in layoutGroup || ("name" in layoutGroup && "layout" in layoutGroup)) return layoutGroup;

	if (layoutGroup.column) {
		const columnWidgets = layoutGroup.column.columnWidgets.map(parseWidgetInstance);

		const result: WidgetSpanColumn = { columnWidgets };
		return result;
	}

	if (layoutGroup.row) {
		const result: WidgetSpanRow = { rowWidgets: layoutGroup.row.rowWidgets.map(parseWidgetInstance) };
		return result;
	}

	if (layoutGroup.section) {
		const result: WidgetSection = {
			name: layoutGroup.section.name,
			description: layoutGroup.section.description,
			visible: layoutGroup.section.visible,
			pinned: layoutGroup.section.pinned,
			id: layoutGroup.section.id,
			layout: layoutGroup.section.layout.map(createLayoutGroup),
		};
		return result;
	}

	if (layoutGroup.table) {
		const result: WidgetTable = {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			tableWidgets: layoutGroup.table.tableWidgets.map((row: any) => row.map(parseWidgetInstance)),
			unstyled: layoutGroup.table.unstyled,
		};
		return result;
	}

	throw new Error("Layout row type does not exist");
}
