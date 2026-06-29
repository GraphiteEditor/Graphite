<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { propertiesPanelLayout } from "/src/stores/portfolio";
	import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

	const editor = getContext<EditorWrapper>("editor");

	let sectionsCol: LayoutCol | undefined;

	// Interactive dragging to reorder Properties panel node sections (a selected layer's chain nodes, or pinned nodes)
	type DragState = { nodeId: bigint; startX: number; startY: number; active: boolean };
	let dragState: DragState | undefined = undefined;
	let dragging = false;
	let fromIndex: number | undefined = undefined;
	let insertIndex: number | undefined = undefined;
	let insertMarkerTop: number | undefined = undefined;
	let justFinishedDrag = false; // Used to suppress the click event that follows a drag release (which would otherwise toggle a section)

	onMount(() => {
		addEventListener("pointermove", draggingPointerMove);
		addEventListener("pointerup", draggingPointerUp);
		addEventListener("keydown", draggingKeyDown);
		addEventListener("mousedown", draggingMouseDown);
		// Capture phase so this runs before a section header's own click handler
		addEventListener("click", suppressClickAfterDrag, true);
	});

	onDestroy(() => {
		removeEventListener("pointermove", draggingPointerMove);
		removeEventListener("pointerup", draggingPointerUp);
		removeEventListener("keydown", draggingKeyDown);
		removeEventListener("mousedown", draggingMouseDown);
		removeEventListener("click", suppressClickAfterDrag, true);
	});

	function suppressClickAfterDrag(e: MouseEvent) {
		if (!justFinishedDrag) return;
		justFinishedDrag = false;
		e.stopPropagation();
		e.preventDefault();
	}

	function reorderableSections(): Element[] {
		const container = sectionsCol?.div();
		if (!container) return [];
		return Array.from(container.querySelectorAll("[data-properties-reorderable-section]"));
	}

	function sectionPointerDown(e: PointerEvent) {
		// Only left click drags
		if (e.button !== 0) return;

		const target = e.target instanceof Element ? e.target : undefined;
		if (!target) return;

		// The drag handle is the section header
		const handle = target.closest("[data-properties-reorder-handle]");
		if (!handle) return;

		// Don't begin a drag when pressing one of the header's own buttons (pin/delete/visibility); only the header itself grabs
		if (target.closest("button") !== handle) return;

		const section = target.closest("[data-properties-reorderable-section]");
		const nodeIdAttribute = section?.getAttribute("data-node-id");
		if (!section || !nodeIdAttribute) return;

		dragState = { nodeId: BigInt(nodeIdAttribute), startX: e.clientX, startY: e.clientY, active: false };
	}

	function draggingPointerMove(e: PointerEvent) {
		if (!dragState) return;

		// Wait until the cursor has moved beyond the threshold before treating it as a drag (so a click still toggles the section)
		if (!dragState.active) {
			const distance = Math.hypot(e.clientX - dragState.startX, e.clientY - dragState.startY);
			const DRAG_THRESHOLD = 5;
			if (distance <= DRAG_THRESHOLD) return;

			dragState.active = true;
			dragging = true;
			fromIndex = reorderableSections().findIndex((section) => section.getAttribute("data-node-id") === String(dragState?.nodeId));
		}

		calculateInsertIndex(e.clientY);
	}

	function calculateInsertIndex(clientY: number) {
		const container = sectionsCol?.div();
		const sections = reorderableSections();
		if (!container || sections.length === 0) {
			insertIndex = undefined;
			insertMarkerTop = undefined;
			return;
		}

		const containerRect = container.getBoundingClientRect();
		const scrollTop = container.scrollTop;
		// Convert a viewport Y to a position within the (scrollable) container's content, so the marker tracks the content when scrolled
		const toContentOffset = (viewportY: number) => viewportY - containerRect.top + scrollTop;

		// The insertion index is the number of sections whose vertical midpoint sits above the cursor. The index flips only when
		// the cursor crosses a section's midpoint, so each gap maps to exactly one index (and gaps between sections are handled).
		let index = 0;
		for (let i = 0; i < sections.length; i += 1) {
			const rect = sections[i].getBoundingClientRect();
			if (clientY < (rect.top + rect.bottom) / 2) break;
			index = i + 1;
		}

		// Position the marker purely from the gap index so it has one fixed spot per gap, rather than snapping between adjacent
		// sections' edges as the cursor crosses their shared boundary.
		let markerViewportY;
		if (index <= 0) {
			markerViewportY = sections[0].getBoundingClientRect().top - 2;
		} else if (index >= sections.length) {
			markerViewportY = sections[sections.length - 1].getBoundingClientRect().bottom + 2;
		} else {
			markerViewportY = (sections[index - 1].getBoundingClientRect().bottom + sections[index].getBoundingClientRect().top) / 2;
		}

		insertIndex = index;
		insertMarkerTop = toContentOffset(markerViewportY);
	}

	function draggingPointerUp() {
		if (dragState?.active) {
			// Suppress the click that the browser fires after the drag release, so it doesn't toggle the dropped section
			justFinishedDrag = true;

			// Skip drops that don't actually move the node (into its own slot)
			if (insertIndex !== undefined && fromIndex !== undefined && insertIndex !== fromIndex && insertIndex !== fromIndex + 1) {
				editor.reorderPropertiesSection(dragState.nodeId, insertIndex);
			}
		}

		abortDrag();
	}

	function draggingKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape" && dragState?.active) {
			justFinishedDrag = true;
			abortDrag();
		}
	}

	function draggingMouseDown(e: MouseEvent) {
		// Abort an in-progress drag if the user presses the right mouse button
		if (e.button === 2 && dragState?.active) {
			justFinishedDrag = true;
			abortDrag();
		}
	}

	function abortDrag() {
		dragState = undefined;
		dragging = false;
		fromIndex = undefined;
		insertIndex = undefined;
		insertMarkerTop = undefined;
	}
</script>

<LayoutCol class="properties">
	<LayoutCol class="sections" classes={{ dragging }} scrollableY={true} bind:this={sectionsCol} on:pointerdown={sectionPointerDown}>
		<WidgetLayout layout={$propertiesPanelLayout} layoutTarget="PropertiesPanel" />
		{#if dragging && insertMarkerTop !== undefined}
			<div class="insert-mark" style:top={`${insertMarkerTop}px`}></div>
		{/if}
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.properties {
		height: 100%;
		flex: 1 1 100%;

		.sections {
			flex: 1 1 100%;
			position: relative;

			// While dragging a section, disable pointer events (which inherit down to the sections) so the drop doesn't toggle a section's expansion, and prevent text selection
			&.dragging {
				user-select: none;
				pointer-events: none;
			}

			// Used as a placeholder for empty assist widgets
			.separator.section.horizontal {
				margin: 0;
				margin-left: 24px;

				div {
					width: 0;
				}
			}

			.insert-mark {
				position: absolute;
				left: 4px;
				right: 4px;
				background: var(--color-e-nearwhite);
				height: 5px;
				// The marker's `top` is the center of the gap, so shift up by half its height to straddle that line
				transform: translateY(-50%);
				z-index: 1;
				pointer-events: none;
			}
		}

		.text-button {
			flex-basis: 0;
		}
	}
</style>
