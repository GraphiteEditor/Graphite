<template>
	<div class="panel">
		<div class="tab-bar" :class="{ 'min-widths': tabMinWidths }">
			<div class="tab-group">
				<div class="tab" :class="{ active: tabIndex === tabActiveIndex }" v-for="(tabLabel, tabIndex) in tabLabels" :key="tabLabel">
					<span>{{tabLabel}}</span>
					<button v-if="tabCloseButtons">
						<svg width="16" height="16" viewBox="0 0 16 16">
							<polygon points="12,5 11,4 8,7 5,4 4,5 7,8 4,11 5,12 8,9 11,12 12,11 9,8" />
						</svg>
					</button>
				</div>
			</div>
			<div class="panel-options-ellipsis">
				<svg viewBox="0 0 16 24">
					<circle cx="8" cy="8" r="1.5" />
					<circle cx="8" cy="12" r="1.5" />
					<circle cx="8" cy="16" r="1.5" />
				</svg>
			</div>
		</div>
		<div class="panel-content">
			<component :is="panelType" />
		</div>
	</div>
</template>

<style lang="scss">
.panel {
	background: #111;
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
			min-width: 120px;
			max-width: 360px;
		}

		.tab-group {
			flex: 1 1 100%;
			display: flex;
			flex-direction: row;
			overflow: hidden;

			.tab {
				height: 100%;
				padding: 0 10px;
				display: flex;
				align-items: center;
				position: relative;

				&.active {
					background: #333;
					border-radius: 8px 8px 0 0;
					position: relative;

					&::before, &::after {
						content: "";
						width: 16px;
						height: 8px;
						position: absolute;
						bottom: 0;
						box-shadow: #333;
					}

					&::before {
						left: -16px;
						border-bottom-right-radius: 8px;
						box-shadow: 8px 0 0 0 #333;
					}

					&::after {
						right: -16px;
						border-bottom-left-radius: 8px;
						box-shadow: -8px 0 0 0 #333;
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

				button {
					flex: 0 0 auto;
					outline: none;
					border: none;
					padding: 0;
					width: 16px;
					height: 16px;
					background: none;
					color: #ddd;
					font-weight: bold;
					font-size: 10px;
					border-radius: 2px;
					margin-left: 8px;
					fill: #ddd;

					&:hover {
						background: #555;
						color: white;
						fill: white;
					}
				}

				&:not(.active) + .tab:not(.active) {
					margin-left: 1px;

					&::before {
						content: "";
						position: absolute;
						left: -1px;
						width: 1px;
						height: 16px;
						background: #444;
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
						background: #444;
					}
				}
			}
		}

		.panel-options-ellipsis {
			width: 16px;
			height: 24px;
			margin: 2px 4px;

			svg {
				width: 16px;
				height: 24px;
				border-radius: 2px;
				fill: #ddd;
			}

			&:hover svg {
				background: #555;
				fill: #fff;
			}
		}
	}

	.panel-content {
		background: #333;
		flex-grow: 1;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Viewport from "../panels/ViewportPanel.vue";
import Properties from "../panels/PropertiesPanel.vue";
import Layers from "../panels/LayersPanel.vue";
import Minimap from "../panels/MinimapPanel.vue";

export default defineComponent({
	components: {
		Viewport,
		Properties,
		Layers,
		Minimap,
	},
	props: {
		tabMinWidths: { type: Boolean, default: false },
		tabCloseButtons: { type: Boolean, default: false },
		tabLabels: { type: Array, required: true },
		tabActiveIndex: { type: Number, required: true },
		panelType: { type: String, required: true },
	},
});
</script>
