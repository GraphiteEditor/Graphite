<template>
	<div class="panel">
		<div class="tab-bar" :class="{ 'min-widths': tabMinWidths }">
			<div class="tab-group">
				<div
					class="tab"
					:class="{ active: tabIndex === tabActiveIndex }"
					v-for="(tabLabel, tabIndex) in tabLabels"
					:key="tabLabel"
					@click.middle="closeTab(tabIndex)"
					@click="handleTabClick(tabIndex)"
				>
					<span>{{ tabLabel }}</span>
					<IconButton :icon="'CloseX'" :size="16" v-if="tabCloseButtons" @click.stop="closeTab(tabIndex)" />
				</div>
			</div>
			<PopoverButton :icon="PopoverButtonIcon.VerticalEllipsis">
				<h3>Panel Options</h3>
				<p>More panel-related options will be here</p>
			</PopoverButton>
		</div>
		<div class="panel-body">
			<component :is="panelType" />
		</div>
	</div>
</template>

<style lang="scss">
.panel {
	background: var(--color-1-nearblack);
	border-radius: 8px;
	flex-grow: 1;
	display: flex;
	flex-direction: column;
	overflow: hidden;

	.tab-bar {
		height: 28px;
		display: flex;
		flex-direction: row;

		&.min-widths .tab-group .tab {
			min-width: 124px;
			max-width: 360px;
		}

		.tab-group {
			flex: 1 1 100%;
			display: flex;
			flex-direction: row;
			overflow: hidden;

			.tab {
				height: 100%;
				padding: 0 8px;
				display: flex;
				align-items: center;
				position: relative;

				&.active {
					background: var(--color-3-darkgray);
					border-radius: 8px 8px 0 0;
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
		display: flex;
		flex-direction: column;
		min-height: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Document from "../panels/Document.vue";
import Properties from "../panels/Properties.vue";
import LayerTree from "../panels/LayerTree.vue";
import Minimap from "../panels/Minimap.vue";
import IconButton from "../widgets/buttons/IconButton.vue";
import PopoverButton, { PopoverButtonIcon } from "../widgets/buttons/PopoverButton.vue";
import { MenuDirection } from "../widgets/floating-menus/FloatingMenu.vue";
import { ResponseType, registerResponseHandler, Response } from "../../response-handler";

const wasm = import("../../../wasm/pkg");

export default defineComponent({
	components: {
		Document,
		Properties,
		LayerTree,
		Minimap,
		IconButton,
		PopoverButton,
	},
	methods: {
		async handleTabClick(tabIndex: number) {
			if (this.panelType !== "Document") return;

			const { select_document } = await wasm;
			select_document(tabIndex);
		},
		async closeTab(tabIndex: number) {
			if (this.panelType !== "Document") return;

			const { close_document } = await wasm;
			// eslint-disable-next-line no-alert
			const result = window.confirm("Closing this document will permanently discard all work. Continue?");
			if (result) close_document(tabIndex);
		},
	},
	mounted() {
		registerResponseHandler(ResponseType.PromptCloseConfirmationModal, (_responseData: Response) => {
			this.closeTab(this.tabActiveIndex);
		});
	},
	props: {
		tabMinWidths: { type: Boolean, default: false },
		tabCloseButtons: { type: Boolean, default: false },
		tabLabels: { type: Array, required: true },
		tabActiveIndex: { type: Number, required: true },
		panelType: { type: String, required: true },
	},
	data() {
		return {
			PopoverButtonIcon,
			MenuDirection,
		};
	},
});
</script>
