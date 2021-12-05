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
						@click="
							handleInputEvent($event, 'layerTreeLayerClick', {
								handleControlClick: () => handleControlClick(layer),
								handleShiftClick: () => handleShiftClick(layer),
								handleClick: () => handleClick(layer),
							})
						"
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
					top: 3px;
					left: 4px;
					border-style: solid;
					border-width: 3px 0 3px 6px;
					border-color: transparent transparent transparent var(--color-2-mildblack);
				}

				&.expanded::after {
					top: 4px;
					left: 3px;
					border-width: 6px 3px 0 3px;
					border-color: var(--color-2-mildblack) transparent transparent transparent;
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

import { ResponseType, registerResponseHandler, Response, BlendMode, DisplayFolderTreeStructure, UpdateLayer, LayerPanelEntry, LayerType } from "@/utilities/response-handler";
import { panicProxy } from "@/utilities/panic-proxy";
import { handleInputEvent } from "@/utilities/input";
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

const wasm = import("@/../wasm/pkg").then(panicProxy);

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
	data() {
		return {
			blendModeEntries,
			blendModeSelectedIndex: 0,
			blendModeDropdownDisabled: true,
			opacityNumberInputDisabled: true,
			// TODO: replace with BigUint64Array as index
			layerCache: new Map() as Map<string, LayerPanelEntry>,
			layers: [] as Array<LayerPanelEntry>,
			layerDepths: [] as Array<number>,
			selectionRangeStartLayer: undefined as undefined | LayerPanelEntry,
			selectionRangeEndLayer: undefined as undefined | LayerPanelEntry,
			opacity: 100,
			MenuDirection,
			SeparatorType,
			LayerType,
			handleInputEvent,
		};
	},
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
		registerResponseHandler(ResponseType.DisplayFolderTreeStructure, (responseData: Response) => {
			const expandData = responseData as DisplayFolderTreeStructure;
			if (!expandData) return;

			const path = [] as Array<bigint>;
			this.layers = [] as Array<LayerPanelEntry>;
			function recurse(folder: DisplayFolderTreeStructure, layers: Array<LayerPanelEntry>, cache: Map<string, LayerPanelEntry>) {
				folder.children.forEach((item) => {
					// TODO: fix toString
					path.push(BigInt(item.layerId.toString()));
					const mapping = cache.get(path.toString());
					if (mapping) layers.push(mapping);
					if (item.children.length > 1) recurse(item, layers, cache);
					path.pop();
				});
			}
			recurse(expandData, this.layers, this.layerCache);
		});

		registerResponseHandler(ResponseType.UpdateLayer, (responseData) => {
			const updateData = responseData as UpdateLayer;
			if (updateData) {
				const responsePath = updateData.path;
				const responseLayer = updateData.data;

				const layer = this.layerCache.get(responsePath.toString());
				if (layer) Object.assign(this.layerCache.get(responsePath.toString()), responseLayer);
				else this.layerCache.set(responsePath.toString(), responseLayer);
				this.setBlendModeForSelectedLayers();
				this.setOpacityForSelectedLayers();
			}
		});
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
