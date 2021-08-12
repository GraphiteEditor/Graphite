<template>
	<LayoutCol :class="'layer-tree-panel'">
		<LayoutRow :class="'options-bar'">
			<DropdownInput v-model:selectedIndex="blendModeSelectedIndex" @update:selectedIndex="setLayerBlendMode" :menuEntries="blendModeEntries" :disabled="blendModeDropdownDisabled" />

			<Separator :type="SeparatorType.Related" />

			<NumberInput v-model:value="opacity" @update:value="setLayerOpacity" :min="0" :max="100" :unit="`%`" :displayDecimalPlaces="2" :label="'Opacity'" :disabled="opacityNumberInputDisabled" />

			<Separator :type="SeparatorType.Related" />

			<PopoverButton>
				<h3>Compositing Options</h3>
				<p>The contents of this popover menu are coming soon</p>
			</PopoverButton>
		</LayoutRow>
		<LayoutRow :class="'layer-tree scrollable-y'">
			<LayoutCol :class="'list'" @click="deselectAllLayers">
				<div class="layer-row" v-for="layer in layers" :key="layer.path">
					<div class="layer-visibility">
						<IconButton
							:action="(e) => (toggleLayerVisibility(layer.path), e.stopPropagation())"
							:icon="layer.visible ? 'EyeVisible' : 'EyeHidden'"
							:size="24"
							:title="layer.visible ? 'Visible' : 'Hidden'"
						/>
					</div>
					<div
						class="layer"
						:class="{ selected: layer.layer_data.selected }"
						@click.shift.exact.stop="handleShiftClick(layer)"
						@click.ctrl.exact.stop="handleControlClick(layer)"
						@click.alt.exact.stop="handleControlClick(layer)"
						@click.exact.stop="handleClick(layer)"
					>
						<div class="layer-thumbnail" v-html="layer.thumbnail"></div>
						<div class="layer-type-icon">
							<IconLabel :icon="'NodeTypePath'" title="Path" />
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
	min-height: 0;

	.options-bar {
		height: 32px;
		flex: 0 0 auto;
		margin: 0 4px;
		align-items: center;

		.dropdown-input {
			max-width: 120px;
		}

		.dropdown-input,
		.number-input {
			flex: 1 1 auto;
		}
	}

	.layer-tree {
		.layer-row {
			display: flex;
			height: 36px;
			align-items: center;
			margin: 0 8px;
			flex: 0 0 auto;

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

				svg {
					width: calc(100% - 4px);
					height: calc(100% - 4px);
					margin: 2px;
				}
			}

			.layer-type-icon {
				margin: 0 8px;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { ResponseType, registerResponseHandler, Response, BlendMode, ExpandFolder, UpdateLayer, LayerPanelEntry } from "@/utilities/response-handler";
import { SeparatorType } from "@/components/widgets/widgets";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import Separator from "@/components/widgets/separators/Separator.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";

const wasm = import("@/../wasm/pkg");

const blendModeEntries: SectionsOfMenuListEntries = [
	[{ label: "Normal", value: BlendMode.Normal }],
	[
		{ label: "Multiply", value: BlendMode.Multiply },
		{ label: "Darken", value: BlendMode.Darken },
		{ label: "Color Burn", value: BlendMode.ColorBurn },
		// { label: "Linear Burn", value: "" }, // Not supported by SVG
		// { label: "Darker Color", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Screen", value: BlendMode.Screen },
		{ label: "Lighten", value: BlendMode.Lighten },
		{ label: "Color Dodge", value: BlendMode.ColorDodge },
		// { label: "Linear Dodge (Add)", value: "" }, // Not supported by SVG
		// { label: "Lighter Color", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Overlay", value: BlendMode.Overlay },
		{ label: "Soft Light", value: BlendMode.SoftLight },
		{ label: "Hard Light", value: BlendMode.HardLight },
		// { label: "Vivid Light", value: "" }, // Not supported by SVG
		// { label: "Linear Light", value: "" }, // Not supported by SVG
		// { label: "Pin Light", value: "" }, // Not supported by SVG
		// { label: "Hard Mix", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Difference", value: BlendMode.Difference },
		{ label: "Exclusion", value: BlendMode.Exclusion },
		// { label: "Subtract", value: "" }, // Not supported by SVG
		// { label: "Divide", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Hue", value: BlendMode.Hue },
		{ label: "Saturation", value: BlendMode.Saturation },
		{ label: "Color", value: BlendMode.Color },
		{ label: "Luminosity", value: BlendMode.Luminosity },
	],
];

export default defineComponent({
	props: {},
	methods: {
		async toggleLayerVisibility(path: BigUint64Array) {
			const { toggle_layer_visibility } = await wasm;
			toggle_layer_visibility(path);
		},
		async setLayerBlendMode() {
			const blendMode = this.blendModeEntries.flat()[this.blendModeSelectedIndex].value as BlendMode;
			if (blendMode) {
				const { set_blend_mode_for_selected_layers } = await wasm;
				set_blend_mode_for_selected_layers(blendMode);
			}
		},
		async setLayerOpacity() {
			const { set_opacity_for_selected_layers } = await wasm;
			set_opacity_for_selected_layers(this.opacity);
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

			this.sendSelectedLayers();
		},
		async handleShiftClick(clickedLayer: LayerPanelEntry) {
			// The two paths of the range are stored in selectionRangeStartLayer and selectionRangeEndLayer
			// So for a new Shift+Click, select all layers between selectionRangeStartLayer and selectionRangeEndLayer (stored in previous Shift+Click)
			this.clearSelection();

			this.selectionRangeEndLayer = clickedLayer;
			if (!this.selectionRangeStartLayer) this.selectionRangeStartLayer = clickedLayer;
			this.fillSelectionRange(this.selectionRangeStartLayer, this.selectionRangeEndLayer, true);

			this.sendSelectedLayers();
		},
		async handleClick(clickedLayer: LayerPanelEntry) {
			this.selectionRangeStartLayer = clickedLayer;
			this.selectionRangeEndLayer = clickedLayer;

			this.clearSelection();
			clickedLayer.layer_data.selected = true;

			this.sendSelectedLayers();
		},
		async deselectAllLayers() {
			this.selectionRangeStartLayer = undefined;
			this.selectionRangeEndLayer = undefined;

			const { deselect_all_layers } = await wasm;
			deselect_all_layers();
		},
		async fillSelectionRange(start: LayerPanelEntry, end: LayerPanelEntry, selected = true) {
			const startIndex = this.layers.findIndex((layer) => layer.path.join() === start.path.join());
			const endIndex = this.layers.findIndex((layer) => layer.path.join() === end.path.join());
			const [min, max] = [startIndex, endIndex].sort();

			if (min !== -1) {
				for (let i = min; i <= max; i += 1) {
					this.layers[i].layer_data.selected = selected;
				}
			}
		},
		async clearSelection() {
			this.layers.forEach((layer) => {
				layer.layer_data.selected = false;
			});
		},
		async sendSelectedLayers() {
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
		setBlendModeForSelectedLayers() {
			const selected = this.layers.filter((layer) => layer.layer_data.selected);

			if (selected.length < 1) {
				this.blendModeSelectedIndex = 0;
				this.blendModeDropdownDisabled = true;
				return;
			}
			this.blendModeDropdownDisabled = false;

			const firstEncounteredBlendMode = selected[0].blend_mode;
			const allBlendModesAlike = !selected.find((layer) => layer.blend_mode !== firstEncounteredBlendMode);

			if (allBlendModesAlike) {
				this.blendModeSelectedIndex = this.blendModeEntries.flat().findIndex((entry) => entry.value === firstEncounteredBlendMode);
			} else {
				// Display a dash when they are not all the same value
				this.blendModeSelectedIndex = NaN;
			}
		},
		setOpacityForSelectedLayers() {
			const selected = this.layers.filter((layer) => layer.layer_data.selected);

			if (selected.length < 1) {
				this.opacity = 100;
				this.opacityNumberInputDisabled = true;
				return;
			}
			this.opacityNumberInputDisabled = false;

			const firstEncounteredOpacity = selected[0].opacity;
			const allOpacitiesAlike = !selected.find((layer) => layer.opacity !== firstEncounteredOpacity);

			if (allOpacitiesAlike) {
				this.opacity = firstEncounteredOpacity;
			} else {
				// Display a dash when they are not all the same value
				this.opacity = NaN;
			}
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

				this.setBlendModeForSelectedLayers();
				this.setOpacityForSelectedLayers();
			}
		});
		registerResponseHandler(ResponseType.CollapseFolder, (responseData) => {
			console.log("CollapseFolder: ", responseData);
		});
		registerResponseHandler(ResponseType.UpdateLayer, (responseData) => {
			const updateData = responseData as UpdateLayer;
			if (updateData) {
				const responsePath = updateData.path;
				const responseLayer = updateData.data;

				const index = this.layers.findIndex((layer: LayerPanelEntry) => {
					const pathLengthsEqual = responsePath.length === layer.path.length;
					return pathLengthsEqual && responsePath.every((layer_id, i) => layer_id === layer.path[i]);
				});
				if (index >= 0) this.layers[index] = responseLayer;

				this.setBlendModeForSelectedLayers();
				this.setOpacityForSelectedLayers();
			}
		});
	},
	data() {
		return {
			blendModeEntries,
			blendModeSelectedIndex: 0,
			blendModeDropdownDisabled: true,
			opacityNumberInputDisabled: true,
			layers: [] as Array<LayerPanelEntry>,
			selectionRangeStartLayer: undefined as undefined | LayerPanelEntry,
			selectionRangeEndLayer: undefined as undefined | LayerPanelEntry,
			opacity: 100,
			MenuDirection,
			SeparatorType,
		};
	},
	components: {
		LayoutRow,
		LayoutCol,
		Separator,
		PopoverButton,
		NumberInput,
		IconButton,
		IconLabel,
		DropdownInput,
	},
});
</script>
