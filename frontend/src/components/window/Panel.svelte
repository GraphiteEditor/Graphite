<script lang="ts">
	import { getContext, onDestroy, tick } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import Data from "/src/components/panels/Data.svelte";
	import Document from "/src/components/panels/Document.svelte";
	import Layers from "/src/components/panels/Layers.svelte";
	import Properties from "/src/components/panels/Properties.svelte";
	import Welcome from "/src/components/panels/Welcome.svelte";
	import IconButton from "/src/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import { panelDrag, startCrossPanelDrag, endCrossPanelDrag, updateCrossPanelHover, updateDockingHover } from "/src/stores/panel-drag";
	import type { DockingEdge } from "/src/stores/panel-drag";
	import type { EditorWrapper, PanelType } from "/wrapper/pkg/graphite_wasm_wrapper";

	const PANEL_COMPONENTS = {
		Welcome,
		Document,
		Layers,
		Properties,
		Data,
	};
	const BUTTON_LEFT = 0;
	const BUTTON_MIDDLE = 1;
	const BUTTON_RIGHT = 2;
	const DRAG_ACTIVATION_DISTANCE = 5;

	const editor = getContext<EditorWrapper>("editor");

	export let tabMinWidths = false;
	export let tabCloseButtons = false;
	export let tabLabels: { name: string; unsaved?: boolean; tooltipLabel?: string; tooltipDescription?: string; tooltipShortcut?: string }[];
	export let tabActiveIndex: number;
	export let panelTypes: PanelType[];
	export let panelId: string;
	export let clickAction: ((index: number) => void) | undefined = undefined;
	export let closeAction: ((index: number) => void) | undefined = undefined;
	export let reorderAction: ((oldIndex: number, newIndex: number) => void) | undefined = undefined;
	export let emptySpaceAction: (() => void) | undefined = undefined;
	export let crossPanelDropAction: ((sourcePanelId: string, targetPanelId: string, insertIndex: number) => void) | undefined = undefined;
	export let groupDropAction: ((sourcePanelId: string, targetPanelId: string, insertIndex: number) => void) | undefined = undefined;
	export let splitDropAction: ((targetPanelId: string, direction: DockingEdge, tabs: PanelType[], activeTabIndex: number) => void) | undefined = undefined;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};

	let tabElements: (LayoutRow | undefined)[] = [];

	// Tab drag-and-drop state
	let dragStartState: { tabIndex: number; pointerX: number; pointerY: number; isGroupDrag: boolean } | undefined = undefined;
	let dragging = false;
	let insertionIndex: number | undefined = undefined;
	let insertionMarkerLeft: number | undefined = undefined;
	let lastPointerX = 0;
	let tabGroupElement: LayoutRow | undefined = undefined;

	onDestroy(() => {
		endDrag();
	});

	function onEmptySpaceAction(e: MouseEvent) {
		if (e.target !== e.currentTarget) return;
		if (e.button === BUTTON_MIDDLE || (e.button === BUTTON_LEFT && e.detail === 2)) emptySpaceAction?.();
	}

	function tabBarPointerDown(e: PointerEvent) {
		// Only start a group drag from the tab bar background (not from a tab or button)
		if (e.button !== BUTTON_LEFT) return;
		if (e.target !== e.currentTarget) return;
		if (!crossPanelDropAction) return;

		dragStartState = { tabIndex: tabActiveIndex, pointerX: e.clientX, pointerY: e.clientY, isGroupDrag: true };
		dragging = false;
		insertionIndex = undefined;
		insertionMarkerLeft = undefined;

		addDragListeners();
	}

	export async function scrollTabIntoView(newIndex: number) {
		await tick();
		tabElements[newIndex]?.div?.()?.scrollIntoView();
	}

	// Tab drag-and-drop handlers

	function tabPointerDown(e: PointerEvent, tabIndex: number) {
		if (e.button !== BUTTON_LEFT) return;
		if (e.target instanceof Element && e.target.closest("[data-close-button]")) return;

		// Activate the tab upon pointer down
		clickAction?.(tabIndex);

		// Allow within-panel reorder if there are multiple tabs, or cross-panel drag if this panel supports docking
		const canReorder = reorderAction && tabLabels.length > 1;
		const canCrossPanelDrag = crossPanelDropAction !== undefined;
		if (!canReorder && !canCrossPanelDrag) return;

		dragStartState = { tabIndex, pointerX: e.clientX, pointerY: e.clientY, isGroupDrag: false };
		dragging = false;
		insertionIndex = undefined;
		insertionMarkerLeft = undefined;

		addDragListeners();
	}

	function dragPointerMove(e: PointerEvent) {
		if (!dragStartState) return;

		// Activate drag after moving beyond threshold
		if (!dragging) {
			const deltaX = Math.abs(e.clientX - dragStartState.pointerX);
			const deltaY = Math.abs(e.clientY - dragStartState.pointerY);
			if (deltaX < DRAG_ACTIVATION_DISTANCE && deltaY < DRAG_ACTIVATION_DISTANCE) return;

			dragging = true;

			if (crossPanelDropAction) {
				if (dragStartState.isGroupDrag) {
					startCrossPanelDrag(panelId, [...panelTypes], tabActiveIndex, true);
				} else {
					const draggedTab = panelTypes[dragStartState.tabIndex];
					startCrossPanelDrag(panelId, [draggedTab], dragStartState.tabIndex, false);
				}
			}
		}

		lastPointerX = e.clientX;

		// Exit early in here after we show the insertion marker, if we're within our own tab bar
		if (pointerIsInsideTabBar(e)) {
			calculateInsertionIndex(lastPointerX);
			updateCrossPanelHover(undefined, undefined, undefined);
			return;
		}

		// Clear local insertion marker since we're outside our own tab bar
		insertionIndex = undefined;
		insertionMarkerLeft = undefined;

		// Check if the pointer is over any other dockable panel's tab bar
		if (crossPanelDropAction) {
			const tabBarTarget = Array.from(document.querySelectorAll("[data-panel-tab-bar]")).find((element) => {
				const targetPanelId = element.getAttribute("data-panel-tab-bar");
				if (!targetPanelId || targetPanelId === panelId) return false;

				const rect = element.getBoundingClientRect();
				return e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
			});

			const tabBarTargetId = tabBarTarget?.getAttribute("data-panel-tab-bar");
			if (tabBarTarget instanceof HTMLDivElement && tabBarTargetId) {
				calculateForeignInsertionIndex(e.clientX, tabBarTargetId, tabBarTarget);
				return;
			}

			// Check if the pointer is over any panel body's edge zone for split docking
			const panelBody = Array.from(document.querySelectorAll("[data-panel-body]")).find((element) => {
				const rect = element.getBoundingClientRect();
				return e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
			});

			const bodyPanelId = panelBody && panelBody.getAttribute("data-panel-body");
			if (bodyPanelId) {
				const rect = panelBody.getBoundingClientRect();
				let edge: DockingEdge | undefined = detectDockingEdge(e.clientX, e.clientY, rect);

				// Block center drops between document and non-document panels
				if (edge === "Center") {
					const targetIsDockable = panelBody.hasAttribute("data-panel-dockable");
					const sourceIsDockable = crossPanelDropAction !== undefined;
					if (targetIsDockable !== sourceIsDockable) edge = undefined;
				}

				if (edge) {
					updateDockingHover(bodyPanelId, edge);
					return;
				}
			}

			// Not hovering any drop target
			updateCrossPanelHover(undefined, undefined, undefined);
			updateDockingHover(undefined, undefined);
		}
	}

	function dragPointerUp() {
		if (dragging && dragStartState) {
			const crossPanelState = $panelDrag;

			// Center drop: append tabs to the target panel group
			if (crossPanelState.active && crossPanelState.hoverDockingPanelId && crossPanelState.hoverDockingEdge === "Center") {
				const dropAction = crossPanelState.draggingGroup ? groupDropAction : crossPanelDropAction;
				dropAction?.(panelId, crossPanelState.hoverDockingPanelId, Number.MAX_SAFE_INTEGER);
			}
			// Edge docking drop: create a new split adjacent to the target panel
			else if (crossPanelState.active && crossPanelState.hoverDockingPanelId && crossPanelState.hoverDockingEdge) {
				splitDropAction?.(
					crossPanelState.hoverDockingPanelId,
					crossPanelState.hoverDockingEdge,
					crossPanelState.draggedTabs,
					crossPanelState.draggingGroup ? crossPanelState.sourceTabIndex : 0,
				);
			}
			// Cross-panel tab bar drop: insert as a tab in the target panel group
			else if (
				crossPanelDropAction &&
				crossPanelState.active &&
				crossPanelState.hoverTargetPanelId &&
				crossPanelState.hoverTargetPanelId !== panelId &&
				crossPanelState.hoverInsertionIndex !== undefined
			) {
				const dropAction = crossPanelState.draggingGroup ? groupDropAction : crossPanelDropAction;
				dropAction?.(panelId, crossPanelState.hoverTargetPanelId, crossPanelState.hoverInsertionIndex);
			}
			// Within-panel reorder
			else if (insertionIndex !== undefined) {
				const oldIndex = dragStartState.tabIndex;

				// Adjust for the fact that removing the dragged tab shifts indices
				let newIndex = insertionIndex;
				if (newIndex > oldIndex) newIndex -= 1;

				if (oldIndex !== newIndex) {
					reorderAction?.(oldIndex, newIndex);
				}
			}
		}

		endDrag();
	}

	function dragAbort(e: MouseEvent | KeyboardEvent) {
		if (e instanceof MouseEvent && e.button === BUTTON_RIGHT) endDrag();
		if (e instanceof KeyboardEvent && e.key === "Escape") endDrag();
	}

	function dragScroll() {
		if (dragging && insertionIndex !== undefined) {
			calculateInsertionIndex(lastPointerX);
		}
	}

	function endDrag() {
		dragStartState = undefined;
		dragging = false;
		insertionIndex = undefined;
		insertionMarkerLeft = undefined;
		if (crossPanelDropAction) endCrossPanelDrag();
		removeDragListeners();
	}

	function pointerIsInsideTabBar(e: PointerEvent): boolean {
		const groupDiv = tabGroupElement?.div?.();
		if (!groupDiv) return false;

		const rect = groupDiv.getBoundingClientRect();
		return e.clientX >= rect.left && e.clientX <= rect.right && e.clientY >= rect.top && e.clientY <= rect.bottom;
	}

	/// Detect which zone the pointer is in: the nearest edge (by diagonal quadrant) if within the 25% border, or center if interior.
	function detectDockingEdge(clientX: number, clientY: number, rect: DOMRect): DockingEdge {
		const distLeft = clientX - rect.left;
		const distRight = rect.right - clientX;
		const distTop = clientY - rect.top;
		const distBottom = rect.bottom - clientY;
		const minDist = Math.min(distLeft, distRight, distTop, distBottom);

		// If the nearest edge is beyond the 25% threshold, it's the center zone
		const THRESHOLD = 0.25;
		const edgeThresholdX = rect.width * THRESHOLD;
		const edgeThresholdY = rect.height * THRESHOLD;
		if (distLeft > edgeThresholdX && distRight > edgeThresholdX && distTop > edgeThresholdY && distBottom > edgeThresholdY) return "Center";

		// Return whichever edge is closest (diagonal dividing lines between quadrants)
		if (minDist === distLeft) return "Left";
		if (minDist === distRight) return "Right";
		if (minDist === distTop) return "Top";
		return "Bottom";
	}

	// Calculate the insertion position for a foreign panel's tab bar
	function calculateForeignInsertionIndex(pointerX: number, targetPanelId: string, tabBarDiv: HTMLDivElement) {
		const tabBarRect = tabBarDiv.getBoundingClientRect();
		const tabs = tabBarDiv.querySelectorAll(":scope > [data-tab]");
		let bestIndex = 0;
		let bestMarkerLeft = 0;

		for (let i = 0; i < tabs.length; i++) {
			const tabRect = tabs[i].getBoundingClientRect();
			const tabCenter = tabRect.left + tabRect.width / 2;

			if (pointerX > tabCenter) {
				bestIndex = i + 1;
				bestMarkerLeft = tabRect.right - tabBarRect.left;
			} else {
				bestMarkerLeft = tabRect.left - tabBarRect.left;
				break;
			}
		}

		// Must be at least 2px from the left so its left half doesn't get cut off along the left of the tab bar
		updateCrossPanelHover(targetPanelId, bestIndex, Math.max(2, bestMarkerLeft));
	}

	function calculateInsertionIndex(pointerX: number) {
		const groupDiv = tabGroupElement?.div?.();
		if (!dragStartState || !groupDiv) return;

		const groupRect = groupDiv.getBoundingClientRect();
		let bestIndex = 0;
		let bestMarkerLeft = 0;

		// Walk through each tab to find the insertion point closest to the pointer
		for (let i = 0; i < tabLabels.length; i++) {
			const tabDiv = tabElements[i]?.div?.();
			if (!tabDiv) continue;

			const tabRect = tabDiv.getBoundingClientRect();
			const tabMidpoint = tabRect.left + tabRect.width / 2;

			if (pointerX > tabMidpoint) {
				bestIndex = i + 1;
				bestMarkerLeft = tabRect.right - groupRect.left;
			} else {
				bestIndex = i;
				bestMarkerLeft = tabRect.left - groupRect.left;
				break;
			}
		}

		insertionIndex = bestIndex;
		insertionMarkerLeft = Math.max(2, bestMarkerLeft);
	}

	function addDragListeners() {
		document.addEventListener("pointermove", dragPointerMove);
		document.addEventListener("pointerup", dragPointerUp);
		document.addEventListener("mousedown", dragAbort);
		document.addEventListener("keydown", dragAbort);
		tabGroupElement?.div?.()?.addEventListener("scroll", dragScroll);
	}

	function removeDragListeners() {
		document.removeEventListener("pointermove", dragPointerMove);
		document.removeEventListener("pointerup", dragPointerUp);
		document.removeEventListener("mousedown", dragAbort);
		document.removeEventListener("keydown", dragAbort);
		tabGroupElement?.div?.()?.removeEventListener("scroll", dragScroll);
	}
</script>

<LayoutCol
	on:pointerdown={() => panelTypes[tabActiveIndex] && editor.setActivePanel(panelTypes[tabActiveIndex])}
	class={`panel ${className}`.trim()}
	{classes}
	style={styleName}
	{styles}
	data-panel-body={panelId}
	data-panel-dockable={crossPanelDropAction ? "" : undefined}
>
	<LayoutRow class="tab-bar" classes={{ "min-widths": tabMinWidths }}>
		<LayoutRow
			class="tab-group"
			scrollableX={true}
			data-panel-tab-bar={crossPanelDropAction ? panelId : undefined}
			on:pointerdown={tabBarPointerDown}
			on:click={onEmptySpaceAction}
			on:auxclick={onEmptySpaceAction}
			bind:this={tabGroupElement}
		>
			{#each tabLabels as tabLabel, tabIndex}
				<LayoutRow
					class="tab"
					classes={{ active: tabIndex === tabActiveIndex }}
					data-tab
					tooltipLabel={tabLabel.tooltipLabel}
					tooltipDescription={tabLabel.tooltipDescription}
					on:pointerdown={(e) => tabPointerDown(e, tabIndex)}
					on:click={(e) => e.stopPropagation()}
					on:auxclick={(e) => {
						// Middle mouse button click
						if (e.button === BUTTON_MIDDLE) {
							e.stopPropagation();
							closeAction?.(tabIndex);
						}
					}}
					bind:this={tabElements[tabIndex]}
				>
					<LayoutRow class="name">
						<TextLabel class="text">{tabLabel.name}</TextLabel>
						{#if tabLabel.unsaved}
							<TextLabel>*</TextLabel>
						{/if}
					</LayoutRow>
					{#if tabCloseButtons}
						<IconButton
							action={(e) => {
								e?.stopPropagation();
								closeAction?.(tabIndex);
							}}
							icon="CloseX"
							size={16}
							data-close-button
						/>
					{/if}
				</LayoutRow>
			{/each}
		</LayoutRow>
		{#if dragging && insertionMarkerLeft !== undefined}
			<div class="tab-insertion-mark" style:left={`${insertionMarkerLeft}px`}></div>
		{/if}
		{#if !dragging && crossPanelDropAction && $panelDrag.active && $panelDrag.hoverTargetPanelId === panelId && $panelDrag.hoverInsertionMarkerLeft !== undefined}
			<div class="tab-insertion-mark" style:left={`${$panelDrag.hoverInsertionMarkerLeft}px`}></div>
		{/if}
	</LayoutRow>
	<LayoutCol class="panel-body">
		{#if panelTypes[tabActiveIndex]}
			<svelte:component this={PANEL_COMPONENTS[panelTypes[tabActiveIndex]]} />
		{/if}
	</LayoutCol>
	{#if $panelDrag.active && $panelDrag.hoverDockingPanelId === panelId && $panelDrag.hoverDockingEdge}
		<div
			class="docking-ghost"
			class:left={$panelDrag.hoverDockingEdge === "Left"}
			class:right={$panelDrag.hoverDockingEdge === "Right"}
			class:top={$panelDrag.hoverDockingEdge === "Top"}
			class:bottom={$panelDrag.hoverDockingEdge === "Bottom"}
			class:center={$panelDrag.hoverDockingEdge === "Center"}
		></div>
	{/if}
</LayoutCol>

<style lang="scss">
	.panel {
		background: var(--color-1-nearblack);
		border-radius: 6px;
		overflow: hidden;
		position: relative;

		.tab-bar {
			position: relative;
			height: 28px;
			min-height: auto;
			background: var(--color-1-nearblack); // Needed for the viewport hole punch on desktop
			flex-shrink: 0;

			&.min-widths .tab-group .tab {
				min-width: 120px;
				max-width: 360px;
			}

			.tab-group {
				flex: 1 1 100%;
				position: relative;

				// This always hangs out at the end of the last tab, providing 16px (15px plus the 1px reserved for the separator line) to the right of the tabs.
				// When the last tab is selected, its bottom rounded fillet adds 16px to the width, which stretches the scrollbar width allocation in only that situation.
				// This pseudo-element ensures we always reserve that space to prevent the scrollbar from jumping when the last tab is selected.
				// There is unfortunately no apparent way to remove that 16px gap from the end of the scroll container, since negative margin does not reduce the scrollbar allocation.
				&::after {
					content: "";
					width: 15px;
					flex: 0 0 auto;
				}

				.tab {
					flex: 0 1 auto;
					height: 28px;
					padding: 0 8px;
					align-items: center;
					position: relative;

					&.active {
						background: var(--color-3-darkgray);
						border-radius: 6px 6px 0 0;
						position: relative;

						&:not(:first-child)::before,
						&::after {
							content: "";
							width: 16px;
							height: 8px;
							position: absolute;
							bottom: 0;
						}

						&:not(:first-child)::before {
							left: -16px;
							border-bottom-right-radius: 8px;
							box-shadow: 8px 0 0 0 var(--color-3-darkgray);
						}

						&::after {
							right: -16px;
							border-bottom-left-radius: 8px;
							box-shadow: -8px 0 0 0 var(--color-3-darkgray);
						}
					}

					.name {
						flex: 1 1 100%;

						.text-label {
							// Height and line-height required because https://stackoverflow.com/a/21611191/775283
							height: 28px;
							line-height: 28px;
							flex: 0 0 auto;

							&.text {
								overflow-x: hidden;
								white-space: nowrap;
								text-overflow: ellipsis;
								flex-shrink: 1;
							}
						}
					}

					.icon-button {
						margin-left: 8px;
					}

					& + .tab {
						margin-left: 1px;
					}

					&:not(.active) + .tab:not(.active)::before {
						content: "";
						position: absolute;
						left: -1px;
						width: 1px;
						height: 16px;
						background: var(--color-5-dullgray);
					}

					&:last-of-type {
						margin-right: 1px;

						&:not(.active)::after {
							content: "";
							position: absolute;
							right: -1px;
							width: 1px;
							height: 16px;
							background: var(--color-5-dullgray);
						}
					}
				}
			}

			&:has(.tab-insertion-mark) .tab .icon-button {
				pointer-events: none;
			}

			.tab-insertion-mark {
				position: absolute;
				top: 4px;
				bottom: 4px;
				width: 3px;
				margin-left: -2px;
				z-index: 1;
				background: var(--color-e-nearwhite);
				pointer-events: none;
			}
		}

		.panel-body {
			background: var(--color-3-darkgray);
			flex: 1 1 100%;
			flex-direction: column;

			> div {
				padding-bottom: 4px;
			}
		}

		&:has(.docking-ghost) .tab-bar,
		&:has(.docking-ghost) .panel-body {
			pointer-events: none;
		}

		.docking-ghost {
			position: absolute;
			background: rgba(var(--color-f-white-rgb), 0.2);
			border-radius: 6px;
			pointer-events: none;
			z-index: 1;
			transition:
				top 0.2s ease,
				left 0.2s ease,
				width 0.2s ease,
				height 0.2s ease;

			&.left {
				top: 0;
				left: 0;
				width: 50%;
				height: 100%;
			}

			&.right {
				top: 0;
				left: 50%;
				width: 50%;
				height: 100%;
			}

			&.top {
				top: 0;
				left: 0;
				width: 100%;
				height: 50%;
			}

			&.bottom {
				top: 50%;
				left: 0;
				width: 100%;
				height: 50%;
			}

			&.center {
				top: 6px;
				left: 6px;
				width: calc(100% - 12px);
				height: calc(100% - 12px);
			}
		}
	}

	// Needed for the viewport hole punch on desktop
	.viewport-hole-punch .panel.document-panel,
	.viewport-hole-punch .panel.document-panel .panel-body:not(:has(.welcome-panel)) {
		background: none;
	}
</style>
