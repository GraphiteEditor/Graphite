<template>
	<LayoutCol :class="'layer-tree-panel'">
		<LayoutRow :class="'options-bar'">
			<DropdownInput :menuEntries="blendModeMenuEntries" :default="blendModeMenuEntries[0][0]" />

			<Separator :type="SeparatorType.Related" />

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
						:class="{ selected: layer.layer_data.selected }"
						@click.shift.exact="handleShiftClick(layer)"
						@click.ctrl.exact="handleControlClick(layer)"
						@click.exact="handleClick(layer)"
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
		flex: 0 0 auto;
		margin: 0 4px;
		align-items: center;

		.dropdown-input {
			flex: 0 0 auto;
		}

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
			background: var(--color-accent);
			color: var(--color-f-white);
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
import { MenuDirection } from "../widgets/floating-menus/FloatingMenu.vue";
import IconButton from "../widgets/buttons/IconButton.vue";
import Icon from "../widgets/labels/Icon.vue";
import DropdownInput from "../widgets/inputs/DropdownInput.vue";
import { SectionsOfMenuListEntries } from "../widgets/floating-menus/MenuList.vue";

const wasm = import("../../../wasm/pkg");

const blendModeMenuEntries: SectionsOfMenuListEntries = [
	[{ label: "Normal" }],
	[{ label: "Multiply" }, { label: "Darken" }, { label: "Color Burn" }, { label: "Linear Burn" }, { label: "Darker Color" }],
	[{ label: "Screen" }, { label: "Lighten" }, { label: "Color Dodge" }, { label: "Linear Dodge (Add)" }, { label: "Lighter Color" }],
	[{ label: "Overlay" }, { label: "Soft Light" }, { label: "Hard Light" }, { label: "Vivid Light" }, { label: "Linear Light" }, { label: "Pin Light" }, { label: "Hard Mix" }],
	[{ label: "Difference" }, { label: "Exclusion" }, { label: "Subtract" }, { label: "Divide" }],
	[{ label: "Hue" }, { label: "Saturation" }, { label: "Color" }, { label: "Luminosity" }],
];

export default defineComponent({
	props: {},
	methods: {
		async toggleLayerVisibility(path: BigUint64Array) {
			const { toggle_layer_visibility } = await wasm;
			toggle_layer_visibility(path);
		},
		async handleControlClick(clickedLayer: LayerPanelEntry) {
			const index = this.layers.indexOf(clickedLayer);
			clickedLayer.layer_data.selected = !clickedLayer.layer_data.selected;
			this.selectionRangeEndLayer = undefined;
			this.selectionRangeStartLayer =
				this.layers.slice(index).filter((layer) => layer.layer_data.selected)[0] ||
				this.layers
					.slice(0, index)
					.reverse()
					.filter((layer) => layer.layer_data.selected)[0];
			this.updateSelection();
		},
		async handleShiftClick(clickedLayer: LayerPanelEntry) {
			// The two paths of the range are stored in selectionRangeStartLayer and selectionRangeEndLayer
			// So for a new Shift+Click, select all layers between selectionRangeStartLayer and selectionRangeEndLayer(stored in prev Sft+C)
			this.selectionRangeEndLayer = clickedLayer;
			this.selectionRangeStartLayer = (this.selectionRangeStartLayer as LayerPanelEntry) || clickedLayer;
			this.clearSelection();
			this.fillSelectionRange(this.selectionRangeStartLayer, this.selectionRangeEndLayer, true);
			this.updateSelection();
		},

		async handleClick(clickedLayer: LayerPanelEntry) {
			this.selectionRangeStartLayer = clickedLayer;
			this.selectionRangeEndLayer = clickedLayer;
			this.clearSelection();
			clickedLayer.layer_data.selected = true;
			this.updateSelection();
		},
		async fillSelectionRange(start: LayerPanelEntry, end: LayerPanelEntry, selected = true) {
			const startIndex = this.layers.indexOf(start);
			const endIndex = this.layers.indexOf(end);
			const [min, max] = [startIndex, endIndex].sort();
			for (let i = min; i <= max; i += 1) {
				this.layers[i].layer_data.selected = selected;
			}
		},
		async clearSelection() {
			this.layers.forEach((layer) => {
				layer.layer_data.selected = false;
			});
		},
		async updateSelection() {
			const paths = this.layers.filter((layer) => layer.layer_data.selected).map((layer) => layer.path);
			const length = paths.reduce((acc, cur) => acc + cur.length, 0) + paths.length - 1;
			const output = new BigUint64Array(length);
			let i = 0;
			paths.forEach((path, index) => {
				output.set(path, i);
				i += path.length;
				if (index < paths.length) {
					// eslint-disable-next-line no-bitwise
					output[i] = (1n << 64n) - 1n;
				}
				i += 1;
			});
			const { select_layers } = await wasm;
			select_layers(output);
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
			blendModeMenuEntries,
			MenuDirection,
			SeparatorType,
			layers: [] as Array<LayerPanelEntry>,
			selectionRangeStartLayer: undefined as LayerPanelEntry | undefined,
			selectionRangeEndLayer: undefined as LayerPanelEntry | undefined,
		};
	},
	components: {
		LayoutRow,
		LayoutCol,
		Separator,
		PopoverButton,
		NumberInput,
		IconButton,
		Icon,
		DropdownInput,
	},
});
</script>
