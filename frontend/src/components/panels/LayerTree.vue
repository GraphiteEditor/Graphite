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
					<button
						v-if="layer.layer_type === LayerType.Folder"
						class="node-connector"
						:class="{ expanded: layer.layer_data.expanded }"
						@click.stop="handleNodeConnectorClick(layer.path)"
					></button>
					<div v-else class="node-connector-missing"></div>
					<div
						class="layer"
						:class="{ selected: layer.layer_data.selected }"
						:style="{ marginLeft: layerIndent(layer) }"
						@click.shift.exact.stop="handleShiftClick(layer)"
						@click.ctrl.exact.stop="handleControlClick(layer)"
						@click.alt.exact.stop="handleControlClick(layer)"
						@click.exact.stop="handleClick(layer)"
					>
						<div class="layer-thumbnail" v-html="layer.thumbnail"></div>
						<div class="layer-type-icon">
							<IconLabel v-if="layer.layer_type === LayerType.Folder" :icon="'NodeTypeFolder'" title="Folder" />
							<IconLabel v-else :icon="'NodeTypePath'" title="Path" />
						</div>
						<div class="layer-name">
							<span>{{ layer.name }}</span>
						</div>
					</div>
					<!-- <div class="glue" :style="{ marginLeft: layerIndent(layer) }"></div> -->
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
			flex: 0 0 auto;
			position: relative;

			& + .layer-row {
				margin-top: 2px;
			}

			.layer-visibility {
				flex: 0 0 auto;
				margin-left: 4px;
			}

			.node-connector {
				flex: 0 0 auto;
				width: 12px;
				height: 12px;
				margin: 0 2px;
				border-radius: 50%;
				background: var(--color-data-raster);
				outline: none;
				border: none;
				position: relative;

				&::after {
					content: "";
					position: absolute;
					width: 0;
					height: 0;
					top: 2px;
					left: 3px;
					border-style: solid;
					border-width: 0 3px 6px 3px;
					border-color: transparent transparent var(--color-2-mildblack) transparent;
				}

				&.expanded::after {
					top: 3px;
					left: 4px;
					border-width: 3px 0 3px 6px;
					border-color: transparent transparent transparent var(--color-2-mildblack);
				}
			}

			.node-connector-missing {
				width: 16px;
				flex: 0 0 auto;
			}

			.layer {
				display: flex;
				align-items: center;
				border-radius: 2px;
				background: var(--color-5-dullgray);
				margin-right: 16px;
				width: 100%;
				height: 100%;
				z-index: 1;

				&.selected {
					background: var(--color-7-middlegray);
					color: var(--color-f-white);
				}

				.layer-thumbnail {
					width: 64px;
					height: 100%;
					background: white;
					border-radius: 2px;

					svg {
						width: calc(100% - 4px);
						height: calc(100% - 4px);
						margin: 2px;
					}
				}

				.layer-type-icon {
					margin-left: 8px;
					margin-right: 4px;
				}
			}

			.glue {
				position: absolute;
				background: var(--color-data-raster);
				height: 6px;
				bottom: -4px;
				left: 44px;
				right: 16px;
				z-index: 0;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { ResponseType, registerResponseHandler, Response, BlendMode, ExpandFolder, CollapseFolder, UpdateLayer, LayerPanelEntry, LayerType } from "@/utilities/response-handler";
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
		layerIndent(layer: LayerPanelEntry): string {
			return `${(layer.path.length - 1) * 16}px`;
		},
		async toggleLayerVisibility(path: BigUint64Array) {
			(await wasm).toggle_layer_visibility(path);
		},
		async handleNodeConnectorClick(path: BigUint64Array) {
			(await wasm).toggle_layer_expansion(path);
		},
		async setLayerBlendMode() {
			const blendMode = this.blendModeEntries.flat()[this.blendModeSelectedIndex].value as BlendMode;
			if (blendMode) {
				(await wasm).set_blend_mode_for_selected_layers(blendMode);
			}
		},
		async setLayerOpacity() {
			(await wasm).set_opacity_for_selected_layers(this.opacity);
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

			(await wasm).deselect_all_layers();
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
			(await wasm).select_layers(output);
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
				if (responseLayers.length === 0) return;

				const mergeIntoExisting = (elements: Array<LayerPanelEntry>, layers: Array<LayerPanelEntry>) => {
					let lastInsertion = layers.findIndex((layer: LayerPanelEntry) => {
						const pathLengthsEqual = elements[0].path.length - 1 === layer.path.length;
						return pathLengthsEqual && elements[0].path.slice(0, -1).every((layerId, i) => layerId === layer.path[i]);
					});
					elements.forEach((nlayer) => {
						const index = layers.findIndex((layer: LayerPanelEntry) => {
							const pathLengthsEqual = nlayer.path.length === layer.path.length;
							return pathLengthsEqual && nlayer.path.every((layerId, i) => layerId === layer.path[i]);
						});
						if (index >= 0) {
							lastInsertion = index;
							layers[index] = nlayer;
						} else {
							lastInsertion += 1;
							layers.splice(lastInsertion, 0, nlayer);
						}
					});
				};
				mergeIntoExisting(responseLayers, this.layers);
				const newLayers: Array<LayerPanelEntry> = [];
				this.layers.forEach((layer) => {
					const index = responseLayers.findIndex((nlayer: LayerPanelEntry) => {
						const pathLengthsEqual = responsePath.length + 1 === layer.path.length;
						return pathLengthsEqual && nlayer.path.every((layerId, i) => layerId === layer.path[i]);
					});
					if (index >= 0 || layer.path.length !== responsePath.length + 1) {
						newLayers.push(layer);
					}
				});
				this.layers = newLayers;

				this.setBlendModeForSelectedLayers();
				this.setOpacityForSelectedLayers();
			}
		});
		registerResponseHandler(ResponseType.CollapseFolder, (responseData) => {
			const collapseData = responseData as CollapseFolder;
			if (collapseData) {
				const responsePath = collapseData.path;

				const newLayers: Array<LayerPanelEntry> = [];
				this.layers.forEach((layer) => {
					if (responsePath.length >= layer.path.length || !responsePath.every((layerId, i) => layerId === layer.path[i])) {
						newLayers.push(layer);
					}
				});
				this.layers = newLayers;

				this.setBlendModeForSelectedLayers();
				this.setOpacityForSelectedLayers();
			}
		});
		registerResponseHandler(ResponseType.UpdateLayer, (responseData) => {
			const updateData = responseData as UpdateLayer;
			if (updateData) {
				const responsePath = updateData.path;
				const responseLayer = updateData.data;

				const index = this.layers.findIndex((layer: LayerPanelEntry) => {
					const pathLengthsEqual = responsePath.length === layer.path.length;
					return pathLengthsEqual && responsePath.every((layerId, i) => layerId === layer.path[i]);
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
			LayerType,
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
