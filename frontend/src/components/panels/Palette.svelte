<script lang="ts">
	// TODO: The content here is copied from inspector and hence it needs to be
	// updated based on the requirements of palette itself.
	import { getContext, onMount, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { paletteLayout } from "/src/stores/portfolio";
	import { createAutoScroller } from "/src/utility-functions/auto-scroll";
	import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

	const _editor = getContext<EditorWrapper>("editor");

	let sectionsCol: LayoutCol | undefined;

	// Interactive dragging to reorder Palette node sections (a selected layer's chain nodes, or pinned nodes)
	type DragState = { nodeId: bigint; startX: number; startY: number; active: boolean };
	let dragState: DragState | undefined = undefined;
	let dragging = false;
	let fromIndex: number | undefined = undefined;
	let insertIndex: number | undefined = undefined;
	let insertMarker: { top: number; left: number; width: number } | undefined = undefined;
	let justFinishedDrag = false; // Used to suppress the click event that follows a drag release (which would otherwise toggle a section)

	// Scrolling the sections near the panel's edges during a drag shifts them under the pointer, so the gap is found again where the pointer last was
	let lastDragClientY = 0;
	const autoScroller = createAutoScroller(() => calculateInsertIndex(lastDragClientY));

	onMount(() => {
		addEventListener("pointermove", draggingPointerMove);
		addEventListener("pointerup", draggingPointerUp);
		// A drag cut short by the pointer being canceled or the window losing focus won't get the release that ends it
		addEventListener("pointercancel", abortDrag);
		addEventListener("blur", abortDrag);
		addEventListener("keydown", draggingKeyDown);
		addEventListener("mousedown", draggingMouseDown);
		// Capture phase so this runs before a section header's own click handler
		addEventListener("click", suppressClickAfterDrag, true);
	});

	onDestroy(() => {
		removeEventListener("pointermove", draggingPointerMove);
		removeEventListener("pointerup", draggingPointerUp);
		removeEventListener("pointercancel", abortDrag);
		removeEventListener("blur", abortDrag);
		removeEventListener("keydown", draggingKeyDown);
		removeEventListener("mousedown", draggingMouseDown);
		removeEventListener("click", suppressClickAfterDrag, true);

		autoScroller.stop();
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
		return Array.from(container.querySelectorAll("[data-palette-reorderable-section]"));
	}

	function sectionPointerDown(e: PointerEvent) {
		// Only left click drags
		if (e.button !== 0) return;

		const target = e.target instanceof Element ? e.target : undefined;
		if (!target) return;

		// The drag handle is the section header
		const handle = target.closest("[data-palette-reorder-handle]");
		if (!handle) return;

		// Don't begin a drag when pressing one of the header's own buttons (pin/delete/visibility); only the header itself grabs
		if (target.closest("[data-icon-button]")) return;

		const section = target.closest("[data-palette-reorderable-section]");
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

		lastDragClientY = e.clientY;
		calculateInsertIndex(e.clientY);

		const container = sectionsCol?.div();
		if (container) autoScroller.update(container, e.clientX, e.clientY, 1);
	}

	function calculateInsertIndex(clientY: number) {
		const container = sectionsCol?.div();
		const sections = reorderableSections();
		if (!container || sections.length === 0) {
			insertIndex = undefined;
			insertMarker = undefined;
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
		// Above the first, it's centered in the room left below what comes before, and below the last, in a gap as wide as the sections leave between each other
		const halfGap = (parseFloat(getComputedStyle(sections[0]).getPropertyValue("--section-gap")) || 0) / 2;
		let markerViewportY;
		if (index <= 0) {
			markerViewportY = (bottomOfContentAbove(sections[0], container) + sections[0].getBoundingClientRect().top) / 2;
		} else if (index >= sections.length) {
			markerViewportY = sections[sections.length - 1].getBoundingClientRect().bottom + halfGap;
		} else {
			markerViewportY = (sections[index - 1].getBoundingClientRect().bottom + sections[index].getBoundingClientRect().top) / 2;
		}

		// Spanning the sections, which a layer's frame around them indents
		const firstRect = sections[0].getBoundingClientRect();

		insertIndex = index;
		insertMarker = { top: toContentOffset(markerViewportY), left: firstRect.left - containerRect.left, width: firstRect.width };
	}

	// The bottom of what's shown right above a section, which is the last row or section before it (even outside the frame it's in), or else the top of the panel
	function bottomOfContentAbove(section: Element, container: HTMLElement): number {
		let current = section;
		while (current !== container && current.parentElement) {
			let previous = current.previousElementSibling;
			if (previous) {
				// Down through the frames and section bodies to the last row or section within
				while (previous.lastElementChild && previous.hasAttribute("data-section-contents")) previous = previous.lastElementChild;
				return previous.getBoundingClientRect().bottom;
			}

			current = current.parentElement;
		}
		return container.getBoundingClientRect().top;
	}

	function draggingPointerUp(_e: PointerEvent) {}

	function draggingKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape" && dragState?.active) {
			abortDrag();
		}
	}

	function draggingMouseDown(e: MouseEvent) {
		// Abort an in-progress drag if the user presses the right mouse button
		if (e.button === 2 && dragState?.active) {
			abortDrag();
		}
	}

	function abortDrag() {
		autoScroller.stop();
		dragState = undefined;
		dragging = false;
		fromIndex = undefined;
		insertIndex = undefined;
		insertMarker = undefined;
	}
</script>

<LayoutCol class="palette">
	<LayoutCol class="sections" classes={{ dragging }} scrollableY={true} bind:this={sectionsCol} on:pointerdown={sectionPointerDown}>
		<WidgetLayout layout={paletteLayout} layoutTarget="Palette" />
		{#if dragging && insertMarker}
			<div class="insert-mark" style:top={`${insertMarker.top}px`} style:left={`${insertMarker.left}px`} style:width={`${insertMarker.width}px`}></div>
		{/if}
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.palette {
		height: 100%;
		flex: 1 1 100%;

		.sections {
			flex: 1 1 100%;
			position: relative;
			padding-top: 4px;

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
