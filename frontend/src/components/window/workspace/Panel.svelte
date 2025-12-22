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
	import { getContext, tick } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { isEventSupported } from "@graphite/utility-functions/platform";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const BUTTON_LEFT = 0;
	const BUTTON_MIDDLE = 1;

	const editor = getContext<Editor>("editor");

	export let tabMinWidths = false;
	export let tabCloseButtons = false;
	export let tabLabels: { name: string; unsaved?: boolean; tooltipLabel?: string; tooltipDescription?: string; tooltipShortcut?: string }[];
	export let tabActiveIndex: number;
	export let panelType: PanelType | undefined = undefined;
	export let clickAction: ((index: number) => void) | undefined = undefined;
	export let closeAction: ((index: number) => void) | undefined = undefined;
	export let emptySpaceAction: (() => void) | undefined = undefined;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};

	let tabElements: (LayoutRow | undefined)[] = [];

	function onEmptySpaceAction(e: MouseEvent) {
		if (e.target !== e.currentTarget) return;
		if (e.button === BUTTON_MIDDLE || (e.button === BUTTON_LEFT && e.detail === 2)) emptySpaceAction?.();
	}

	export async function scrollTabIntoView(newIndex: number) {
		await tick();
		tabElements[newIndex]?.div?.()?.scrollIntoView();
	}
</script>

<LayoutCol on:pointerdown={() => panelType && editor.handle.setActivePanel(panelType)} class={`panel ${className}`.trim()} {classes} style={styleName} {styles}>
	<LayoutRow class="tab-bar" classes={{ "min-widths": tabMinWidths }}>
		<LayoutRow class="tab-group" scrollableX={true} on:click={onEmptySpaceAction} on:auxclick={onEmptySpaceAction}>
			{#each tabLabels as tabLabel, tabIndex}
				<LayoutRow
					class="tab"
					classes={{ active: tabIndex === tabActiveIndex }}
					tooltipLabel={tabLabel.tooltipLabel}
					tooltipDescription={tabLabel.tooltipDescription}
					on:click={(e) => {
						e.stopPropagation();
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
