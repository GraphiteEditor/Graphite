<template>
	<LayoutCol :class="'layer-tree-panel'">
		<LayoutRow :class="'options-bar'">
			<DropdownInput
				v-model:selectedIndex="blendModeSelectedIndex"
				@update:selectedIndex="(newSelectedIndex: number) => setLayerBlendMode(newSelectedIndex)"
				:menuEntries="blendModeEntries"
				:disabled="blendModeDropdownDisabled"
			/>

			<Separator :type="'Related'" />

			<NumberInput
				v-model:value="opacity"
				@update:value="(newOpacity: number) => setLayerOpacity(newOpacity)"
				:min="0"
				:max="100"
				:unit="'%'"
				:displayDecimalPlaces="2"
				:label="'Opacity'"
				:disabled="opacityNumberInputDisabled"
			/>

			<Separator :type="'Related'" />

			<PopoverButton>
				<h3>Compositing Options</h3>
				<p>The contents of this popover menu are coming soon</p>
			</PopoverButton>
		</LayoutRow>
		<LayoutRow :class="'layer-tree scrollable-y'">
			<LayoutCol :class="'list'" ref="layerTreeList" @click="() => deselectAllLayers()" @dragover="updateInsertLine($event)" @dragend="drop()">
				<LayoutRow
					class="layer-row"
					v-for="(listing, index) in layers"
					:key="String(listing.entry.path.slice(-1))"
					v-bind:class="{ 'insert-folder': draggingData && draggingData.highlightFolder && draggingData.insertFolder === listing.entry.path }"
				>
					<div class="visibility">
						<IconButton
							:action="(e) => (toggleLayerVisibility(listing.entry.path), e && e.stopPropagation())"
							:size="24"
							:icon="listing.entry.visible ? 'EyeVisible' : 'EyeHidden'"
							:title="listing.entry.visible ? 'Visible' : 'Hidden'"
						/>
					</div>

					<div class="indent" :style="{ marginLeft: layerIndent(listing.entry) }"></div>

					<button
						v-if="listing.entry.layer_type === 'Folder'"
						class="expand-arrow"
						:class="{ expanded: listing.entry.layer_metadata.expanded }"
						@click.stop="handleExpandArrowClick(listing.entry.path)"
					></button>
					<div
						class="layer"
						:class="{ selected: listing.entry.layer_metadata.selected }"
						@click.shift.exact.stop="selectLayer(listing.entry, false, true)"
						@click.shift.ctrl.exact.stop="selectLayer(listing.entry, true, true)"
						@click.ctrl.exact.stop="selectLayer(listing.entry, true, false)"
						@click.exact.stop="selectLayer(listing.entry, false, false)"
						:data-index="index"
						draggable="true"
						@dragstart="dragStart($event, listing.entry)"
						:title="String(listing.entry.path)"
					>
						<div class="layer-type-icon">
							<IconLabel v-if="listing.entry.layer_type === 'Folder'" :icon="'NodeTypeFolder'" title="Folder" />
							<IconLabel v-else :icon="'NodeTypePath'" title="Path" />
						</div>
						<div class="layer-name">
							<span>{{ listing.entry.name }}</span>
						</div>
						<div class="thumbnail" v-html="listing.entry.thumbnail"></div>
					</div>
				</LayoutRow>
			</LayoutCol>
			<div class="insert-mark" v-if="draggingData && !draggingData.highlightFolder" v-bind:style="{ left: markIndent(draggingData!.insertFolder), top: `${draggingData.markerHeight}px` }"></div>
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
		// Crop away the 1px border below the bottom layer entry when it uses the full space of this panel
		margin-bottom: -1px;

		position: relative;

		.insert-mark {
			right: 0px;
			position: absolute;
			background: var(--color-accent-hover);
			margin-top: -2px;
			height: 5px;
			z-index: 1;
		}

		.layer-row {
			flex: 0 0 auto;
			align-items: center;
			position: relative;
			height: 36px;
			margin: 0 4px;
			border-bottom: 1px solid var(--color-4-dimgray);

			.visibility {
				height: 100%;
				flex: 0 0 auto;
				display: flex;
				align-items: center;

				.icon-button {
					height: 100%;
					width: calc(24px + 2 * 4px);
				}
			}

			.expand-arrow {
				margin-left: -16px;
				width: 16px;
				height: 100%;
				padding: 0;
				outline: none;
				border: none;
				position: relative;
				background: none;
				flex: 0 0 auto;
				display: flex;
				align-items: center;
				justify-content: center;
				border-radius: 2px;

				&:hover {
					background: var(--color-6-lowergray);
				}

				&::after {
					content: "";
					position: absolute;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 3px 0 3px 6px;
					border-color: transparent transparent transparent var(--color-e-nearwhite);

					&:hover {
						color: var(--color-f-white);
					}
				}

				&.expanded::after {
					border-width: 6px 3px 0 3px;
					border-color: var(--color-e-nearwhite) transparent transparent transparent;

					&:hover {
						color: var(--color-f-white);
					}
				}
			}

			.layer {
				display: flex;
				align-items: center;
				z-index: 1;
				min-width: 0;
				width: 100%;
				height: 100%;
				border-radius: 2px;
				padding: 0 4px;
				margin-right: 8px;

				&.selected {
					background: var(--color-5-dullgray);
					color: var(--color-f-white);
				}

				.layer-type-icon {
					flex: 0 0 auto;
					margin: 0 4px;
				}

				.layer-name {
					flex: 1 1 100%;
					display: flex;
					min-width: 0;
					margin: 0 4px;

					span {
						text-overflow: ellipsis;
						white-space: nowrap;
						overflow: hidden;
					}
				}

				.thumbnail {
					height: calc(100% - 4px);
					margin: 2px 0;
					margin-left: 4px;
					width: 64px;
					background: white;
					border-radius: 2px;
					flex: 0 0 auto;

					svg {
						width: calc(100% - 4px);
						height: calc(100% - 4px);
						margin: 2px;
					}
				}
			}

			&.insert-folder .layer {
				outline: 3px solid var(--color-accent-hover);
				outline-offset: -3px;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { BlendMode, DisplayDocumentLayerTreeStructure, UpdateDocumentLayer, LayerPanelEntry } from "@/dispatcher/js-messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

type LayerListingInfo = { entry: LayerPanelEntry; bottomLayer: boolean; folderIndex: number };

const blendModeEntries: SectionsOfMenuListEntries<BlendMode> = [
	[{ label: "Normal", value: "Normal" }],
	[
		{ label: "Multiply", value: "Multiply" },
		{ label: "Darken", value: "Darken" },
		{ label: "Color Burn", value: "ColorBurn" },
		// { label: "Linear Burn", value: "" }, // Not supported by SVG
		// { label: "Darker Color", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Screen", value: "Screen" },
		{ label: "Lighten", value: "Lighten" },
		{ label: "Color Dodge", value: "ColorDodge" },
		// { label: "Linear Dodge (Add)", value: "" }, // Not supported by SVG
		// { label: "Lighter Color", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Overlay", value: "Overlay" },
		{ label: "Soft Light", value: "SoftLight" },
		{ label: "Hard Light", value: "HardLight" },
		// { label: "Vivid Light", value: "" }, // Not supported by SVG
		// { label: "Linear Light", value: "" }, // Not supported by SVG
		// { label: "Pin Light", value: "" }, // Not supported by SVG
		// { label: "Hard Mix", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Difference", value: "Difference" },
		{ label: "Exclusion", value: "Exclusion" },
		// { label: "Subtract", value: "" }, // Not supported by SVG
		// { label: "Divide", value: "" }, // Not supported by SVG
	],
	[
		{ label: "Hue", value: "Hue" },
		{ label: "Saturation", value: "Saturation" },
		{ label: "Color", value: "Color" },
		{ label: "Luminosity", value: "Luminosity" },
	],
];

const RANGE_TO_INSERT_WITHIN_BOTTOM_FOLDER_NOT_ROOT = 20;
const LAYER_LEFT_MARGIN_OFFSET = 52;
const LAYER_LEFT_INDENT_OFFSET = 16;
const INSERT_MARK_OFFSET = 2;

type DraggingData = { insertFolder: BigUint64Array; insertIndex: number; highlightFolder: boolean; markerHeight: number };

export default defineComponent({
	inject: ["editor"],
	data() {
		return {
			blendModeEntries,
			blendModeSelectedIndex: 0,
			blendModeDropdownDisabled: true,
			opacityNumberInputDisabled: true,
			// TODO: replace with BigUint64Array as index
			layerCache: new Map() as Map<string, LayerPanelEntry>,
			layers: [] as LayerListingInfo[],
			layerDepths: [] as number[],
			selectionRangeStartLayer: undefined as undefined | LayerPanelEntry,
			selectionRangeEndLayer: undefined as undefined | LayerPanelEntry,
			opacity: 100,
			draggingData: undefined as undefined | DraggingData,
		};
	},
	methods: {
		layerIndent(layer: LayerPanelEntry): string {
			return `${layer.path.length * LAYER_LEFT_INDENT_OFFSET}px`;
		},
		markIndent(path: BigUint64Array): string {
			return `${LAYER_LEFT_MARGIN_OFFSET + path.length * LAYER_LEFT_INDENT_OFFSET}px`;
		},
		async toggleLayerVisibility(path: BigUint64Array) {
			this.editor.instance.toggle_layer_visibility(path);
		},
		async handleExpandArrowClick(path: BigUint64Array) {
			this.editor.instance.toggle_layer_expansion(path);
		},
		async setLayerBlendMode(newSelectedIndex: number) {
			const blendMode = this.blendModeEntries.flat()[newSelectedIndex].value;
			if (blendMode) this.editor.instance.set_blend_mode_for_selected_layers(blendMode);
		},
		async setLayerOpacity(newOpacity: number) {
			this.editor.instance.set_opacity_for_selected_layers(newOpacity);
		},
		async selectLayer(clickedLayer: LayerPanelEntry, ctrl: boolean, shift: boolean) {
			this.editor.instance.select_layer(clickedLayer.path, ctrl, shift);
		},
		async deselectAllLayers() {
			this.selectionRangeStartLayer = undefined;
			this.selectionRangeEndLayer = undefined;

			this.editor.instance.deselect_all_layers();
		},
		async clearSelection() {
			this.layers.forEach((layer) => {
				layer.entry.layer_metadata.selected = false;
			});
		},
		closest(tree: HTMLElement, clientY: number): DraggingData {
			const treeChildren = tree.children;

			// Closest distance to the middle of the row along the Y axis
			let closest = Infinity;

			// Folder to insert into
			let insertFolder = new BigUint64Array();

			// Insert index
			let insertIndex = -1;

			// Whether you are inserting into a folder and should show the folder outline
			let highlightFolder = false;

			const treeOffset = tree.getBoundingClientRect().top;
			// Marker height
			let markerHeight = 0;

			let previousHeight = undefined as undefined | number;

			Array.from(treeChildren).forEach((treeChild, index) => {
				const layerComponents = treeChild.getElementsByClassName("layer");
				if (layerComponents.length !== 1) return;
				const child = layerComponents[0];

				const indexAttribute = child.getAttribute("data-index");
				if (!indexAttribute) return;
				const { folderIndex, entry: layer } = this.layers[parseInt(indexAttribute, 10)];

				const rect = child.getBoundingClientRect();
				const position = rect.top + rect.height / 2;
				const distance = position - clientY;

				// Inserting above current row
				if (distance > 0 && distance < closest) {
					insertFolder = layer.path.slice(0, layer.path.length - 1);
					insertIndex = folderIndex;
					highlightFolder = false;
					closest = distance;
					markerHeight = previousHeight || treeOffset + INSERT_MARK_OFFSET;
				}
				// Inserting below current row
				else if (distance > -closest && distance > -RANGE_TO_INSERT_WITHIN_BOTTOM_FOLDER_NOT_ROOT && distance < 0) {
					insertFolder = layer.layer_type === "Folder" ? layer.path : layer.path.slice(0, layer.path.length - 1);
					insertIndex = layer.layer_type === "Folder" ? 0 : folderIndex + 1;
					highlightFolder = layer.layer_type === "Folder";
					closest = -distance;
					markerHeight = index === treeChildren.length - 1 ? rect.bottom - INSERT_MARK_OFFSET : rect.bottom;
				}
				// Inserting with no nesting at the end of the panel
				else if (closest === Infinity) {
					if (layer.path.length === 1) insertIndex = folderIndex + 1;

					markerHeight = rect.bottom - INSERT_MARK_OFFSET;
				}
				previousHeight = rect.bottom;
			});

			markerHeight -= treeOffset;

			return { insertFolder, insertIndex, highlightFolder, markerHeight };
		},
		async dragStart(event: DragEvent, layer: LayerPanelEntry) {
			if (!layer.layer_metadata.selected) this.selectLayer(layer, event.ctrlKey, event.shiftKey);

			// Set style of cursor for drag
			if (event.dataTransfer) {
				event.dataTransfer.dropEffect = "move";
				event.dataTransfer.effectAllowed = "move";
			}
			const tree = (this.$refs.layerTreeList as typeof LayoutCol).$el;

			this.draggingData = this.closest(tree, event.clientY);
		},
		updateInsertLine(event: DragEvent) {
			// Stop the drag from being shown as cancelled
			event.preventDefault();

			const tree = (this.$refs.layerTreeList as typeof LayoutCol).$el as HTMLElement;
			this.draggingData = this.closest(tree, event.clientY);
		},
		async drop() {
			if (this.draggingData) {
				const { insertFolder, insertIndex } = this.draggingData;

				this.editor.instance.move_layer_in_tree(insertFolder, insertIndex);

				this.draggingData = undefined;
			}
		},
		setBlendModeForSelectedLayers() {
			const selected = this.layers.filter((layer) => layer.entry.layer_metadata.selected);

			if (selected.length < 1) {
				this.blendModeSelectedIndex = 0;
				this.blendModeDropdownDisabled = true;
				return;
			}
			this.blendModeDropdownDisabled = false;

			const firstEncounteredBlendMode = selected[0].entry.blend_mode;
			const allBlendModesAlike = !selected.find((layer) => layer.entry.blend_mode !== firstEncounteredBlendMode);

			if (allBlendModesAlike) {
				this.blendModeSelectedIndex = this.blendModeEntries.flat().findIndex((entry) => entry.value === firstEncounteredBlendMode);
			} else {
				// Display a dash when they are not all the same value
				this.blendModeSelectedIndex = NaN;
			}
		},
		setOpacityForSelectedLayers() {
			// todo figure out why this is here
			const selected = this.layers.filter((layer) => layer.entry.layer_metadata.selected);

			if (selected.length < 1) {
				this.opacity = 100;
				this.opacityNumberInputDisabled = true;
				return;
			}
			this.opacityNumberInputDisabled = false;

			const firstEncounteredOpacity = selected[0].entry.opacity;
			const allOpacitiesAlike = !selected.find((layer) => layer.entry.opacity !== firstEncounteredOpacity);

			if (allOpacitiesAlike) {
				this.opacity = firstEncounteredOpacity;
			} else {
				// Display a dash when they are not all the same value
				this.opacity = NaN;
			}
		},
	},
	mounted() {
		this.editor.dispatcher.subscribeJsMessage(DisplayDocumentLayerTreeStructure, (displayDocumentLayerTreeStructure) => {
			const path = [] as bigint[];
			this.layers = [] as { folderIndex: number; bottomLayer: boolean; entry: LayerPanelEntry }[];

			const recurse = (folder: DisplayDocumentLayerTreeStructure, layers: { folderIndex: number; bottomLayer: boolean; entry: LayerPanelEntry }[], cache: Map<string, LayerPanelEntry>): void => {
				folder.children.forEach((item, index) => {
					// TODO: fix toString
					path.push(BigInt(item.layerId.toString()));
					const mapping = cache.get(path.toString());
					if (mapping) layers.push({ folderIndex: index, bottomLayer: index === folder.children.length - 1, entry: mapping });
					if (item.children.length >= 1) recurse(item, layers, cache);
					path.pop();
				});
			};

			recurse(displayDocumentLayerTreeStructure, this.layers, this.layerCache);
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentLayer, (updateDocumentLayer) => {
			const targetPath = updateDocumentLayer.data.path;
			const targetLayer = updateDocumentLayer.data;

			const layer = this.layerCache.get(targetPath.toString());
			if (layer) {
				Object.assign(this.layerCache.get(targetPath.toString()), targetLayer);
			} else {
				this.layerCache.set(targetPath.toString(), targetLayer);
			}
			this.setBlendModeForSelectedLayers();
			this.setOpacityForSelectedLayers();
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
