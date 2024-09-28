<script lang="ts" context="module">
	import Document from "@graphite/components/panels/Document.svelte";
	import Layers from "@graphite/components/panels/Layers.svelte";
	import Properties from "@graphite/components/panels/Properties.svelte";

	const PANEL_COMPONENTS = {
		Document,
		Layers,
		Properties,
	};
	type PanelType = keyof typeof PANEL_COMPONENTS;
</script>

<script lang="ts">
	import { getContext, tick } from "svelte";

	import { platformIsMac, isEventSupported } from "@graphite/utility-functions/platform";

	import { extractPixelData } from "@graphite/utility-functions/rasterization";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import { type LayoutKeysGroup, type Key } from "@graphite/wasm-communication/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	import UserInputLabel from "@graphite/components/widgets/labels/UserInputLabel.svelte";

	const editor = getContext<Editor>("editor");

	export let tabMinWidths = false;
	export let tabCloseButtons = false;
	export let tabLabels: { name: string; tooltip?: string }[];
	export let tabActiveIndex: number;
	export let panelType: PanelType | undefined = undefined;
	export let clickAction: ((index: number) => void) | undefined = undefined;
	export let closeAction: ((index: number) => void) | undefined = undefined;

	let tabElements: (LayoutRow | undefined)[] = [];

	function platformModifiers(reservedKey: boolean): LayoutKeysGroup {
		// TODO: Remove this by properly feeding these keys from a layout provided by the backend

		const ALT: Key = { key: "Alt", label: "Alt" };
		const COMMAND: Key = { key: "Command", label: "Command" };
		const CONTROL: Key = { key: "Control", label: "Ctrl" };

		if (platformIsMac()) return reservedKey ? [ALT, COMMAND] : [COMMAND];
		return reservedKey ? [CONTROL, ALT] : [CONTROL];
	}

	function dropFile(e: DragEvent) {
		if (!e.dataTransfer) return;

		e.preventDefault();

		Array.from(e.dataTransfer.items).forEach(async (item) => {
			const file = item.getAsFile();
			if (!file) return;

			if (file.type.includes("svg")) {
				const svgData = await file.text();
				editor.handle.pasteSvg(file.name, svgData);
				return;
			}

			if (file.type.startsWith("image")) {
				const imageData = await extractPixelData(file);
				editor.handle.pasteImage(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height);
				return;
			}

			if (file.name.endsWith(".graphite")) {
				const content = await file.text();
				editor.handle.openDocumentFile(file.name, content);
				return;
			}
		});
	}

	export async function scrollTabIntoView(newIndex: number) {
		await tick();
		tabElements[newIndex]?.div?.()?.scrollIntoView();
	}
</script>

<LayoutCol class="panel" on:pointerdown={() => panelType && editor.handle.setActivePanel(panelType)}>
	<LayoutRow class="tab-bar" classes={{ "min-widths": tabMinWidths }}>
		<LayoutRow class="tab-group" scrollableX={true}>
			{#each tabLabels as tabLabel, tabIndex}
				<LayoutRow
					class="tab"
					classes={{ active: tabIndex === tabActiveIndex }}
					tooltip={tabLabel.tooltip || undefined}
					on:click={(e) => {
						e.stopPropagation();
						clickAction?.(tabIndex);
					}}
					on:auxclick={(e) => {
						// Middle mouse button click
						if (e.button === 1) {
							e.stopPropagation();
							closeAction?.(tabIndex);
						}
					}}
					on:mouseup={(e) => {
						// Middle mouse button click fallback for Safari:
						// https://developer.mozilla.org/en-US/docs/Web/API/Element/auxclick_event#browser_compatibility
						// The downside of using mouseup is that the mousedown didn't have to originate in the same element.
						// A possible future improvement could save the target element during mousedown and check if it's the same here.
						if (!isEventSupported("auxclick") && e.button === 1) {
							e.stopPropagation();
							closeAction?.(tabIndex);
						}
					}}
					bind:this={tabElements[tabIndex]}
				>
					<TextLabel>{tabLabel.name}</TextLabel>
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
		<!-- <PopoverButton style="VerticalEllipsis">
			<TextLabel bold={true}>Panel Options</TextLabel>
			<TextLabel multiline={true}>Coming soon</TextLabel>
		</PopoverButton> -->
	</LayoutRow>
	<LayoutCol class="panel-body">
		{#if panelType}
			<svelte:component this={PANEL_COMPONENTS[panelType]} />
		{:else}
			<LayoutCol class="empty-panel" on:dragover={(e) => e.preventDefault()} on:drop={dropFile}>
				<LayoutCol class="content">
					<LayoutRow class="logotype">
						<IconLabel icon="GraphiteLogotypeSolid" />
					</LayoutRow>
					<LayoutRow class="actions">
						<table>
							<tr>
								<td>
									<TextButton label="New Document" icon="File" flush={true} action={() => editor.handle.newDocumentDialog()} />
								</td>
								<td>
									<UserInputLabel keysWithLabelsGroups={[[...platformModifiers(true), { key: "KeyN", label: "N" }]]} />
								</td>
							</tr>
							<tr>
								<td>
									<TextButton label="Open Document" icon="Folder" flush={true} action={() => editor.handle.openDocument()} />
								</td>
								<td>
									<UserInputLabel keysWithLabelsGroups={[[...platformModifiers(false), { key: "KeyO", label: "O" }]]} />
								</td>
							</tr>
							<tr>
								<td colspan="2">
									<TextButton label="Open Demo Artwork" icon="Image" flush={true} action={() => editor.handle.demoArtworkDialog()} />
								</td>
							</tr>
						</table>
					</LayoutRow>
				</LayoutCol>
			</LayoutCol>
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

					span {
						flex: 1 1 100%;
						overflow-x: hidden;
						white-space: nowrap;
						text-overflow: ellipsis;
						// Height and line-height required because https://stackoverflow.com/a/21611191/775283
						height: 28px;
						line-height: 28px;
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
						background: var(--color-4-dimgray);
					}

					&:last-of-type {
						margin-right: 1px;

						&:not(.active)::after {
							content: "";
							position: absolute;
							right: -1px;
							width: 1px;
							height: 16px;
							background: var(--color-4-dimgray);
						}
					}
				}
			}

			// .popover-button {
			// 	margin: 2px 4px;
			// }
		}

		.panel-body {
			background: var(--color-3-darkgray);
			flex: 1 1 100%;
			flex-direction: column;

			> div {
				padding-bottom: 4px;
			}

			.empty-panel {
				background: var(--color-2-mildblack);
				margin: 4px;
				border-radius: 2px;
				justify-content: center;

				.content {
					flex: 0 0 auto;
					align-items: center;

					.logotype {
						margin-bottom: 40px;

						svg {
							width: auto;
							height: 120px;
						}
					}

					.actions {
						table {
							border-spacing: 8px;
							margin: -8px;

							td {
								padding: 0;
							}
						}
					}
				}
			}
		}
	}
</style>
