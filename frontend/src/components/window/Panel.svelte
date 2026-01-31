<script lang="ts" context="module">
	import Data from "@graphite/components/panels/Data.svelte";
	import Document from "@graphite/components/panels/Document.svelte";
	import Layers from "@graphite/components/panels/Layers.svelte";
	import Properties from "@graphite/components/panels/Properties.svelte";
	import Welcome from "@graphite/components/panels/Welcome.svelte";

	const PANEL_COMPONENTS = {
		Welcome,
		Document,
		Layers,
		Properties,
		Data,
	};
	type PanelType = keyof typeof PANEL_COMPONENTS;
</script>

<script lang="ts">
	import { getContext, onMount, onDestroy, tick } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { isEventSupported } from "@graphite/utility-functions/platform";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	type DragState = {
		startIndex: number;
		startX: number;
		startY: number;
	};

	const BUTTON_LEFT = 0;
	const BUTTON_MIDDLE = 1;
	const DRAG_THRESHOLD = 5;

	const editor = getContext<Editor>("editor");

	export let tabMinWidths = false;
	export let tabCloseButtons = false;
	export let tabLabels: { name: string; unsaved?: boolean; tooltipLabel?: string; tooltipDescription?: string; tooltipShortcut?: string }[];
	export let tabActiveIndex: number;
	export let panelType: PanelType | undefined = undefined;
	export let clickAction: ((index: number) => void) | undefined = undefined;
	export let closeAction: ((index: number) => void) | undefined = undefined;
	export let emptySpaceAction: (() => void) | undefined = undefined;
	export let reorderAction: ((fromIndex: number, toIndex: number) => void) | undefined = undefined;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};

	let tabElements: (LayoutRow | undefined)[] = [];
	let tabGroupElement: LayoutRow | undefined;

	// Drag-and-drop state
	let dragState: DragState | undefined = undefined;
	let dragInPanel = false;
	let dragDropIndex: number | undefined = undefined;
	let dragIndicatorLeft: number | undefined = undefined;
	let justFinishedDrag = false;

	function onEmptySpaceAction(e: MouseEvent) {
		if (e.target !== e.currentTarget) return;
		if (e.button === BUTTON_MIDDLE || (e.button === BUTTON_LEFT && e.detail === 2)) emptySpaceAction?.();
	}

	export async function scrollTabIntoView(newIndex: number) {
		await tick();
		tabElements[newIndex]?.div?.()?.scrollIntoView();
	}

	// --- Drag-and-drop ---

	function tabPointerDown(e: PointerEvent, tabIndex: number) {
		if (e.button !== BUTTON_LEFT || !reorderAction) return;
		dragState = { startIndex: tabIndex, startX: e.clientX, startY: e.clientY };
	}

	function calculateDropIndex(clientX: number): { index: number; left: number } | undefined {
		const groupDiv = tabGroupElement?.div?.();
		if (!groupDiv) return undefined;

		const groupRect = groupDiv.getBoundingClientRect();
		const scrollLeft = groupDiv.scrollLeft;

		for (let i = 0; i < tabLabels.length; i++) {
			const el = tabElements[i]?.div?.();
			if (!el) continue;

			const rect = el.getBoundingClientRect();
			if (clientX < rect.left || clientX > rect.right) continue;

			const pointerFraction = (clientX - rect.left) / rect.width;
			if (pointerFraction < 0.5) {
				return { index: i, left: rect.left - groupRect.left + scrollLeft };
			} else {
				return { index: i + 1, left: rect.right - groupRect.left + scrollLeft };
			}
		}

		return undefined;
	}

	function draggingPointerMove(e: PointerEvent) {
		if (!dragState || !tabGroupElement) return;

		if (!dragInPanel) {
			const distance = Math.hypot(e.clientX - dragState.startX, e.clientY - dragState.startY);
			if (distance <= DRAG_THRESHOLD) return;
			dragInPanel = true;
		}

		if (dragInPanel) {
			const result = calculateDropIndex(e.clientX);
			if (result) {
				// Adjust index to account for the item being removed from its original position
				let targetIndex = result.index;
				if (targetIndex > dragState.startIndex) targetIndex -= 1;

				if (targetIndex === dragState.startIndex) {
					dragDropIndex = undefined;
					dragIndicatorLeft = undefined;
				} else {
					dragDropIndex = targetIndex;
					dragIndicatorLeft = result.left;
				}
			} else {
				dragDropIndex = undefined;
				dragIndicatorLeft = undefined;
			}
		}
	}

	function draggingPointerUp() {
		if (dragInPanel && dragDropIndex !== undefined) {
			reorderAction?.(dragState.startIndex, dragDropIndex);
			justFinishedDrag = true;
			// Clear after the current tick so a same-tick click is still suppressed, but the next intentional click is not swallowed
			setTimeout(() => {
				justFinishedDrag = false;
			}, 0);
		} else if (justFinishedDrag) {
			// Avoid right-click abort getting stuck with `justFinishedDrag` set and blocking the first subsequent click
			setTimeout(() => {
				justFinishedDrag = false;
			}, 0);
		}

		abortDrag();
	}

	function draggingMouseDown(e: MouseEvent) {
		if (e.button === 2 && dragInPanel) {
			justFinishedDrag = true;
			abortDrag();
		}
	}

	function draggingKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape" && dragInPanel) {
			justFinishedDrag = true;
			abortDrag();
		}
	}

	function abortDrag() {
		dragState = undefined;
		dragInPanel = false;
		dragDropIndex = undefined;
		dragIndicatorLeft = undefined;
	}

	onMount(() => {
		addEventListener("pointermove", draggingPointerMove);
		addEventListener("pointerup", draggingPointerUp);
		addEventListener("mousedown", draggingMouseDown);
		addEventListener("keydown", draggingKeyDown);
	});

	onDestroy(() => {
		removeEventListener("pointermove", draggingPointerMove);
		removeEventListener("pointerup", draggingPointerUp);
		removeEventListener("mousedown", draggingMouseDown);
		removeEventListener("keydown", draggingKeyDown);
	});
</script>

<LayoutCol on:pointerdown={() => panelType && editor.handle.setActivePanel(panelType)} class={`panel ${className}`.trim()} {classes} style={styleName} {styles}>
	<LayoutRow class="tab-bar" classes={{ "min-widths": tabMinWidths }}>
		<LayoutRow class="tab-group" classes={{ "drag-ongoing": dragInPanel }} scrollableX={true} on:click={onEmptySpaceAction} on:auxclick={onEmptySpaceAction} bind:this={tabGroupElement}>
			{#each tabLabels as tabLabel, tabIndex}
				<LayoutRow
					class="tab"
					classes={{ active: tabIndex === tabActiveIndex, dragging: dragInPanel && dragState?.startIndex === tabIndex }}
					tooltipLabel={tabLabel.tooltipLabel}
					tooltipDescription={tabLabel.tooltipDescription}
					on:pointerdown={(e) => tabPointerDown(e, tabIndex)}
					on:click={(e) => {
						e.stopPropagation();
						if (justFinishedDrag) {
							justFinishedDrag = false;
							return;
						}
						clickAction?.(tabIndex);
					}}
					on:auxclick={(e) => {
						// Middle mouse button click
						if (e.button === BUTTON_MIDDLE) {
							e.stopPropagation();
							closeAction?.(tabIndex);
						}
					}}
					on:mouseup={(e) => {
						// Middle mouse button click fallback for Safari:
						// https://developer.mozilla.org/en-US/docs/Web/API/Element/auxclick_event#browser_compatibility
						// The downside of using mouseup is that the mousedown didn't have to originate in the same element.
						// A possible future improvement could save the target element during mousedown and check if it's the same here.
						if (!isEventSupported("auxclick") && e.button === BUTTON_MIDDLE) {
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
						/>
					{/if}
				</LayoutRow>
			{/each}
			{#if dragInPanel && dragDropIndex !== undefined && dragIndicatorLeft !== undefined}
				<div class="drop-indicator" style:left={`${dragIndicatorLeft}px`} />
			{/if}
		</LayoutRow>
	</LayoutRow>
	<LayoutCol class="panel-body">
		{#if panelType}
			<svelte:component this={PANEL_COMPONENTS[panelType]} />
		{/if}
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.panel {
		background: var(--color-1-nearblack);
		border-radius: 6px;
		overflow: hidden;

		.tab-bar {
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

				&.drag-ongoing .tab {
					pointer-events: none;
				}

				.tab {
					flex: 0 1 auto;
					height: 28px;
					padding: 0 8px;
					align-items: center;
					position: relative;

					&.dragging {
						opacity: 0.5;
					}

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

				.drop-indicator {
					position: absolute;
					top: 4px;
					bottom: 4px;
					width: 2px;
					background: var(--color-e-nearwhite);
					pointer-events: none;
					z-index: 1;
					transform: translateX(-50%);
				}
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

		// Needed for the viewport hole punch on desktop
		.viewport-hole-punch &.document-panel,
		.viewport-hole-punch &.document-panel .panel-body:not(:has(.welcome-panel)) {
			background: none;
		}
	}
</style>
