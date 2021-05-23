<template>
	<div class="panel">
		<div class="tab-bar" :class="{ 'min-widths': tabMinWidths }">
			<div class="tab-group">
				<div class="tab" :class="{ active: tabIndex === tabActiveIndex }" v-for="(tabLabel, tabIndex) in tabLabels" :key="tabLabel">
					<span>{{ tabLabel }}</span>
					<IconButton :size="16" v-if="tabCloseButtons">
						<CloseX />
					</IconButton>
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

				&:not(.active) + .tab:not(.active) {
					margin-left: 1px;

					&::before {
						content: "";
						position: absolute;
						left: -1px;
						width: 1px;
						height: 16px;
						background: var(--color-4-dimgray);
					}
				}

				&:last-of-type:not(.active) {
					margin-right: 1px;

					&::after {
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
		flex-grow: 1;
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
import { PopoverDirection } from "../widgets/overlays/Popover.vue";
import VerticalEllipsis from "../../../assets/svg/16x24-bounds-8x16-icon/vertical-ellipsis.svg";
import CloseX from "../../../assets/svg/16x16-bounds-12x12-icon/close-x.svg";

export default defineComponent({
	components: {
		Document,
		Properties,
		LayerTree,
		Minimap,
		IconButton,
		PopoverButton,
		CloseX,
		VerticalEllipsis,
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
			PopoverDirection,
		};
	},
});
</script>
