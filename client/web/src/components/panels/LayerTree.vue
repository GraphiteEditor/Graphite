<template>
	<LayoutCol :class="'layer-tree-panel'">
		<LayoutRow :class="'options-bar'">
			<NumberInput :value="100" :unit="`%`" />

			<Separator :type="SeparatorType.Related" />

			<PopoverButton>
				<h3>Compositing Options</h3>
				<p>More blend and compositing options will be here</p>
			</PopoverButton>
		</LayoutRow>
		<LayoutRow :class="'layer-tree'">
			<LayoutCol :class="'list'">
				<div class="layer-row" v-for="layer in layers" :key="layer.path">
					<div class="layer-visibility">
						<IconButton :icon="layer.visible ? 'EyeVisible' : 'EyeHidden'" @click="toggleLayerVisibility(layer.path)" :size="24" :title="layer.visible ? 'Visible' : 'Hidden'" />
					</div>
					<div
						class="layer"
						:class="layer.selected ? 'selected' : ''"
						@click.shift.exact="handleShiftClick(layer.path)"
						@click.alt.exact="handleControlClick(layer.path)"
						@click.exact="handleClick(layer.path)"
					>
						<div class="layer-thumbnail"></div>
						<div class="layer-type-icon">
							<Icon :icon="'NodeTypePath'" title="Path" />
						</div>
						<div class="layer-name">
							<span>{{ layer.name }}</span>
						</div>
					</div>
				</div>
			</LayoutCol>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.layer-tree-panel {
	.options-bar {
		height: 32px;
		margin: 0 4px;
		align-items: center;

		.number-input {
			flex: 1 1 100%;
		}
	}

	.layer-row {
		display: flex;
		height: 36px;
		align-items: center;
		margin: 0 8px;

		.layer {
			display: flex;
			align-items: center;
			background: var(--color-5-dullgray);
			border-radius: 4px;
			width: 100%;
			height: 100%;
			margin-left: 4px;
			padding-left: 16px;
		}
		.selected {
			background: #ff0000;
		}

		& + .layer-row {
			margin-top: 2px;
		}

		.layer-thumbnail {
			width: 64px;
			height: 100%;
			background: white;
		}

		.layer-type-icon {
			margin: 0 8px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import { ResponseType, registerResponseHandler, Response, ExpandFolder, LayerPanelEntry } from "../../response-handler";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import Separator, { SeparatorType } from "../widgets/Separator.vue";
import NumberInput from "../widgets/inputs/NumberInput.vue";
import PopoverButton from "../widgets/buttons/PopoverButton.vue";
import { PopoverDirection } from "../widgets/overlays/Popover.vue";
import IconButton from "../widgets/buttons/IconButton.vue";
import Icon from "../widgets/labels/Icon.vue";

const wasm = import("../../../wasm/pkg");

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		Separator,
		PopoverButton,
		NumberInput,
		IconButton,
		Icon,
	},
	props: {},
	methods: {
		async toggleLayerVisibility(path: BigUint64Array) {
			const { toggle_layer_visibility } = await wasm;
			toggle_layer_visibility(path);
		},
		async handleControlClick(path: BigUint64Array) {
			let i = 0;
			this.endPath = -1n;
			this.layers.forEach((layer, idx, layers) => {
				if (layer.path === path) {
					layers[idx].selected = !layer.selected;
					if (layer.selected) {
						[this.startPath] = path;
					} else {
						let j = i + 1;
						while (j < this.layers.length) {
							// Look for a selected layer below to assign to startPath
							if (this.layers[j].selected) {
								[this.startPath] = this.layers[j].path;
								break;
							}
							j += 1;
						}
						if (j >= this.layers.length) {
							// Look above
							j = i - 1;
							while (j >= 0) {
								if (this.layers[j].selected) {
									console.log("ABOVE");
									[this.startPath] = this.layers[j].path;
									break;
								}
								j -= 1;
							}
						}
						if (j < 0) {
							// RESET
							this.startPath = -1n;
						}
					}
				}
				i += 1;
			});
		},
		async handleShiftClick(path: BigUint64Array) {
			// The two paths of the range are stored in startPath and endPath
			// So for a new Shift+Click, unselect all paths between startPath and endPath(stored in prev Sft+C)
			// Then select all paths between startPath and path(new endPath) and assign path to endPath
			if (this.startPath === -1n) {
				// If nothing was selected before, usually at the start of the app
				// Also if the user manually deselects all the layers
				this.layers.forEach((layer) => {
					if (layer.path[0] <= path[0]) {
						layer.selected = true;
					}
				});
			} else {
				[this.endPath] = path;
				this.layers.forEach((layer) => {
					if ((layer.path[0] >= path[0] && layer.path[0] <= this.startPath) || (layer.path[0] <= path[0] && layer.path[0] >= this.startPath)) {
						layer.selected = true;
					}
				});
			}
		},

		async handleClick(path: BigUint64Array) {
			[this.startPath] = path;
			[this.endPath] = path;
			this.layers.forEach((layer) => {
				// Can we directly index into `layers`? Is the path `i` at the `i`th index in layers?
				// Delete layer op may affect the order of layers and the paths.
				layer.selected = layer.path === path;
			});
		},
	},
	mounted() {
		registerResponseHandler(ResponseType.ExpandFolder, (responseData: Response) => {
			const expandData = responseData as ExpandFolder;
			if (expandData) {
				const responsePath = expandData.path;
				const responseLayers = expandData.children as Array<LayerPanelEntry>;
				if (responsePath.length > 0) console.error("Non root paths are currently not implemented");

				this.layers = responseLayers;
			}
		});
		registerResponseHandler(ResponseType.CollapseFolder, (responseData) => {
			console.log("CollapseFolder: ", responseData);
		});
	},
	data() {
		return {
			PopoverDirection,
			SeparatorType,
			layers: [] as Array<LayerPanelEntry>,
			startPath: -1n,
			endPath: -1n,
		};
	},
});
</script>
