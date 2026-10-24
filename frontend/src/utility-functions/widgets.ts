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

		// An empty path updates the whole layout
		const path = update.widgetPath.map(Number);
		const lastIndex = path.pop();
		if (lastIndex === undefined) {
			if (isPlainContainer(newContent)) reconcile(layout, newContent);
			return;
		}

		// Find the list holding the item the diff applies to
		const parent = path.reduce((list: UIItem[] | undefined, index) => indexedChildren(list?.[index]), indexedChildren(layout));

		// Exit if we failed to produce a valid patch for the existing layout.
		// This means that the backend assumed an existing layout that doesn't exist in the frontend. This can happen, for
		// example, if a panel is destroyed in the frontend but was never cleared in the backend, so the next time the backend
		// tries to update the layout, it attempts to insert only the changes against the old layout that no longer exists.
		if (parent?.[lastIndex] === undefined) {
			// eslint-disable-next-line no-console
			console.error("In `patchLayout`, the diff's target is undefined. The layout has not been updated. See the source code comment above this error for hints.");
			return;
		}

		reconcileEntry(parent, lastIndex, newContent);
	});
}

// The list that an index in a widget path selects from within the given layout item
function indexedChildren(item: UIItem | undefined): UIItem[] | undefined {
	if (!item) return undefined;
	if (Array.isArray(item)) return item;
	if ("Column" in item) return item.Column.columnWidgets;
	if ("Row" in item) return item.Row.rowWidgets;
	if ("Table" in item) return item.Table.tableWidgets;
	if ("NodeSection" in item) return item.NodeSection.layout;
	if ("PopoverButton" in item.widget) return item.widget.PopoverButton.popoverLayout;
	return undefined;
}

// Whether the value is a plain object or array, the only kinds a layout is built from
function isPlainContainer(value: unknown): value is object {
	if (typeof value !== "object" || !value) return false;
	const prototype = Object.getPrototypeOf(value);
	return prototype === Object.prototype || prototype === Array.prototype;
}

// Sets `container[key]` to `value`, changing the existing value in place where it keeps its shape so a reactive layout only invalidates what changed.
// A value that changes shape (like a widget changing kind) is replaced whole, so readers never see an object half-changed.
function reconcileEntry(container: object, key: PropertyKey, value: unknown) {
	const existing: unknown = Reflect.get(container, key);

	if (isPlainContainer(existing) && isPlainContainer(value) && haveSameShape(existing, value)) {
		reconcile(existing, value);
	} else if (!Object.is(existing, value)) {
		Reflect.set(container, key, value);
	}
}

// Changes `target` in place to deeply equal `source`, which has the same keys unless both are arrays
function reconcile(target: object, source: object) {
	if (Array.isArray(target) && Array.isArray(source) && target.length > source.length) target.length = source.length;
	Object.entries(source).forEach(([key, value]) => reconcileEntry(target, key, value));
}

function haveSameShape(first: object, second: object): boolean {
	if (Array.isArray(first) || Array.isArray(second)) return Array.isArray(first) && Array.isArray(second);

	const firstKeys = Object.keys(first);
	return firstKeys.length === Object.keys(second).length && firstKeys.every((key) => Object.hasOwn(second, key));
}
