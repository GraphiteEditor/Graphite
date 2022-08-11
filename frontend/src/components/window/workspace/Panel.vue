<template>
	<LayoutCol class="panel">
		<LayoutRow class="tab-bar" data-tab-bar :class="{ 'min-widths': tabMinWidths }">
			<LayoutRow class="tab-group" :scrollableX="true">
				<LayoutRow
					class="tab"
					:class="{ active: tabIndex === tabActiveIndex }"
					data-tab
					v-for="(tabLabel, tabIndex) in tabLabels"
					:key="tabIndex"
					@click="(e) => (e?.stopPropagation(), clickAction?.(tabIndex))"
					@click.middle="(e) => (e?.stopPropagation(), closeAction?.(tabIndex))"
				>
					<span>{{ tabLabel }}</span>
					<IconButton :action="(e) => (e?.stopPropagation(), closeAction?.(tabIndex))" :icon="'CloseX'" :size="16" v-if="tabCloseButtons" />
				</LayoutRow>
			</LayoutRow>
			<PopoverButton :icon="'VerticalEllipsis'">
				<h3>Panel Options</h3>
				<p>The contents of this popover menu are coming soon</p>
			</PopoverButton>
		</LayoutRow>
		<LayoutCol class="panel-body">
			<component :is="panelType" v-if="panelType" />
			<LayoutCol class="empty-panel" v-else>
				<LayoutCol class="content">
					<LayoutRow class="logotype">
						<IconLabel :icon="'GraphiteLogotypeSolid'" />
					</LayoutRow>
					<LayoutRow class="actions">
						<table>
							<tr>
								<td>
									<TextButton :label="'New Document:'" :icon="'File'" :action="() => newDocument()" />
								</td>
								<td>
									<UserInputLabel :keysWithLabelsGroups="[[...platformModifiers(true), { key: 'KeyN', label: 'N' }]]" />
								</td>
							</tr>
							<tr>
								<td>
									<TextButton :label="'Open Document:'" :icon="'Folder'" :action="() => openDocument()" />
								</td>
								<td>
									<UserInputLabel :keysWithLabelsGroups="[[...platformModifiers(false), { key: 'KeyO', label: 'O' }]]" />
								</td>
							</tr>
						</table>
					</LayoutRow>
				</LayoutCol>
			</LayoutCol>
		</LayoutCol>
	</LayoutCol>
</template>

<style lang="scss">
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
				height: 100%;
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
					height: 100%;
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

		.popover-button {
			margin: 2px 4px;
		}
	}

	.panel-body {
		background: var(--color-3-darkgray);
		flex: 1 1 100%;
		flex-direction: column;
		min-height: 0;

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

						.text-button:not(:hover) {
							background: none;
						}
					}
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { platformIsMac } from "@/utility-functions/platform";

import { KeysGroup, Key } from "@/wasm-communication/messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import Document from "@/components/panels/Document.vue";
import LayerTree from "@/components/panels/LayerTree.vue";
import NodeGraph from "@/components/panels/NodeGraph.vue";
import Properties from "@/components/panels/Properties.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import TextButton from "@/components/widgets/buttons/TextButton.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";
import UserInputLabel from "@/components/widgets/labels/UserInputLabel.vue";

const panelComponents = {
	Document,
	IconButton,
	LayerTree,
	NodeGraph,
	PopoverButton,
	Properties,
	TextButton,
};
type PanelTypes = keyof typeof panelComponents;

export default defineComponent({
	inject: ["editor"],
	props: {
		tabMinWidths: { type: Boolean as PropType<boolean>, default: false },
		tabCloseButtons: { type: Boolean as PropType<boolean>, default: false },
		tabLabels: { type: Array as PropType<string[]>, required: true },
		tabActiveIndex: { type: Number as PropType<number>, required: true },
		panelType: { type: String as PropType<PanelTypes>, required: false },
		clickAction: { type: Function as PropType<(index: number) => void>, required: false },
		closeAction: { type: Function as PropType<(index: number) => void>, required: false },
	},
	methods: {
		newDocument() {
			this.editor.instance.new_document_dialog();
		},
		openDocument() {
			this.editor.instance.document_open();
		},
		platformModifiers(reservedKey: boolean): KeysGroup {
			// TODO: Remove this by properly feeding these keys from a layout provided by the backend

			const ALT: Key = { key: "Alt", label: "Alt" };
			const COMMAND: Key = { key: "Command", label: "Command" };
			const CONTROL: Key = { key: "Control", label: "Control" };

			if (platformIsMac()) return reservedKey ? [ALT, COMMAND] : [COMMAND];
			return reservedKey ? [CONTROL, ALT] : [CONTROL];
		},
	},
	components: {
		LayoutCol,
		LayoutRow,
		IconLabel,
		TextLabel,
		UserInputLabel,
		...panelComponents,
	},
});
</script>
