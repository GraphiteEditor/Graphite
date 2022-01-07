<template>
	<LayoutCol :class="'layer-tree-panel'">
		<LayoutRow :class="'options-bar'">
			<DropdownInput
				v-model:selectedIndex="blendModeSelectedIndex"
				@update:selectedIndex="(newSelectedIndex) => setLayerBlendMode(newSelectedIndex)"
				:menuEntries="blendModeEntries"
				:disabled="blendModeDropdownDisabled"
			/>

			<Separator :type="'Related'" />

			<NumberInput
				v-model:value="opacity"
				@update:value="(newOpacity) => setLayerOpacity(newOpacity)"
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
			<LayoutCol :class="'list'" ref="layerTreeList" @click="() => deselectAllLayers()" @dragover="updateLine($event)" @dragend="drop()">
				<div class="layer-row" v-for="(layer, index) in layers" :key="String(layer.path.slice(-1))">
					<div class="layer-visibility">
						<IconButton
							:action="(e) => (toggleLayerVisibility(layer.path), e && e.stopPropagation())"
							:icon="layer.visible ? 'EyeVisible' : 'EyeHidden'"
							:size="24"
							:title="layer.visible ? 'Visible' : 'Hidden'"
						/>
					</div>
					<button
						v-if="layer.layer_type === 'Folder'"
						class="node-connector"
						:class="{ expanded: layer.layer_metadata.expanded }"
						@click.stop="handleNodeConnectorClick(layer.path)"
					></button>
					<div v-else class="node-connector-missing"></div>
					<div
						class="layer"
						:class="{ selected: layer.layer_metadata.selected }"
						:style="{ marginLeft: layerIndent(layer) }"
						@click.shift.exact.stop="selectLayer(layer, false, true)"
						@click.shift.ctrl.exact.stop="selectLayer(layer, true, true)"
						@click.ctrl.exact.stop="selectLayer(layer, true, false)"
						@click.exact.stop="selectLayer(layer, false, false)"
						:data-index="index"
						draggable="true"
						@dragstart="dragStart($event, layer)"
						:title="layer.path"
					>
						<div class="layer-thumbnail" v-html="layer.thumbnail"></div>
						<div class="layer-type-icon">
							<IconLabel v-if="layer.layer_type === 'Folder'" :icon="'NodeTypeFolder'" title="Folder" />
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

			& + .layer-row,
			& + .insert-mark + .layer-row {
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
				min-width: 0;
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
					flex: 0 0 auto;

					svg {
						width: calc(100% - 4px);
						height: calc(100% - 4px);
						margin: 2px;
					}
				}

				.layer-type-icon {
					margin-left: 8px;
					margin-right: 4px;
					flex: 0 0 auto;
				}

				.layer-name {
					display: flex;
					min-width: 0;
					flex: 1 1 100%;
					margin-right: 8px;

					span {
						text-overflow: ellipsis;
						white-space: nowrap;
						overflow: hidden;
					}
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

		.insert-mark {
			position: relative;
			margin-right: 16px;
			height: 0;
			z-index: 2;

			&::after {
				content: "";
				position: absolute;
				background: var(--color-accent-hover);
				width: 100%;
				height: 6px;
			}

			&:not(:first-child, :last-child) {
				top: -2px;
			}

			&:first-child::after {
				top: 0;
			}

			&:last-child::after {
				bottom: 0;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { BlendMode, DisplayFolderTreeStructure, UpdateLayer, LayerPanelEntry } from "@/dispatcher/js-messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

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

const RANGE_TO_INSERT_WITHIN_BOTTOM_FOLDER_NOT_ROOT = 40;
const LAYER_LEFT_MARGIN_OFFSET = 28;
const LAYER_LEFT_INDENT_OFFSET = 16;

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
			layers: [] as LayerPanelEntry[],
			layerDepths: [] as number[],
			selectionRangeStartLayer: undefined as undefined | LayerPanelEntry,
			selectionRangeEndLayer: undefined as undefined | LayerPanelEntry,
			opacity: 100,
			draggingData: undefined as undefined | { path: BigUint64Array; above: boolean; nearestPath: BigUint64Array; insertLine: HTMLDivElement },
		};
	},
	methods: {
		layerIndent(layer: LayerPanelEntry) {
			return `${(layer.path.length - 1) * 16}px`;
		},
		async toggleLayerVisibility(path: BigUint64Array) {
			this.editor.instance.toggle_layer_visibility(path);
		},
		async handleNodeConnectorClick(path: BigUint64Array) {
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
				layer.layer_metadata.selected = false;
			});
		},
		closest(tree: HTMLElement, clientY: number): [BigUint64Array, boolean, Node] {
			const treeChildren = tree.children;

			// Closest distance to the middle of the row along the Y axis
			let closest = Infinity;

			// The nearest row parent (element of the tree)
			let nearestElement = tree.lastChild as Node;

			// The nearest element in the path to the mouse
			let nearestPath = new BigUint64Array();

			// Item goes above or below the mouse
			let above = false;

			Array.from(treeChildren).forEach((treeChild) => {
				if (treeChild.childElementCount <= 2) return;

				const child = treeChild.children[2] as HTMLElement;

				const indexAttribute = child.getAttribute("data-index");
				if (!indexAttribute) return;
				const layer = this.layers[parseInt(indexAttribute, 10)];

				const rect = child.getBoundingClientRect();
				const position = rect.top + rect.height / 2;
				const distance = position - clientY;

				// Inserting above current row
				if (distance > 0 && distance < closest) {
					closest = distance;
					nearestPath = layer.path;
					above = true;
					if (child.parentNode) {
						nearestElement = child.parentNode;
					}
				}
				// Inserting below current row
				else if (distance > -closest && distance > -RANGE_TO_INSERT_WITHIN_BOTTOM_FOLDER_NOT_ROOT && distance < 0 && layer.layer_type !== "Folder") {
					closest = -distance;
					nearestPath = layer.path;
					if (child.parentNode && child.parentNode.nextSibling) {
						nearestElement = child.parentNode.nextSibling;
					}
				}
				// Inserting with no nesting at the end of the panel
				else if (closest === Infinity) {
					nearestPath = layer.path.slice(0, 1);
				}
			});

			return [nearestPath, above, nearestElement];
		},
		async dragStart(event: DragEvent, layer: LayerPanelEntry) {
			// Set style of cursor for drag
			if (event.dataTransfer) {
				event.dataTransfer.dropEffect = "move";
				event.dataTransfer.effectAllowed = "move";
			}

			const tree = (this.$refs.layerTreeList as typeof LayoutCol).$el;

			// Create the insert line
			const insertLine = document.createElement("div") as HTMLDivElement;
			insertLine.classList.add("insert-mark");
			tree.appendChild(insertLine);

			const [nearestPath, above, nearestElement] = this.closest(tree, event.clientY);

			// Set the initial state of the line
			if (nearestElement.parentNode) {
				insertLine.style.marginLeft = `${LAYER_LEFT_MARGIN_OFFSET + LAYER_LEFT_INDENT_OFFSET * nearestPath.length}px`;
				tree.insertBefore(insertLine, nearestElement);
			}

			this.draggingData = { path: layer.path, above, nearestPath, insertLine };
		},
		updateLine(event: DragEvent) {
			// Stop the drag from being shown as cancelled
			event.preventDefault();

			const tree = (this.$refs.layerTreeList as typeof LayoutCol).$el as HTMLElement;

			const [nearestPath, above, nearestElement] = this.closest(tree, event.clientY);

			if (this.draggingData) {
				this.draggingData.nearestPath = nearestPath;
				this.draggingData.above = above;

				if (nearestElement.parentNode) {
					this.draggingData.insertLine.style.marginLeft = `${LAYER_LEFT_MARGIN_OFFSET + LAYER_LEFT_INDENT_OFFSET * nearestPath.length}px`;
					tree.insertBefore(this.draggingData.insertLine, nearestElement);
				}
			}
		},
		removeLine() {
			if (this.draggingData) {
				this.draggingData.insertLine.remove();
			}
		},
		async drop() {
			this.removeLine();
			if (this.draggingData) {
				this.editor.instance.move_layer_in_tree(this.draggingData.path, this.draggingData.above, this.draggingData.nearestPath);
			}
		},
		setBlendModeForSelectedLayers() {
			const selected = this.layers.filter((layer) => layer.layer_metadata.selected);

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
			// todo figure out why this is here
			const selected = this.layers.filter((layer) => layer.layer_metadata.selected);

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
		this.editor.dispatcher.subscribeJsMessage(DisplayFolderTreeStructure, (displayFolderTreeStructure) => {
			const path = [] as bigint[];
			this.layers = [] as LayerPanelEntry[];

			const recurse = (folder: DisplayFolderTreeStructure, layers: LayerPanelEntry[], cache: Map<string, LayerPanelEntry>): void => {
				folder.children.forEach((item) => {
					// TODO: fix toString
					path.push(BigInt(item.layerId.toString()));
					const mapping = cache.get(path.toString());
					if (mapping) layers.push(mapping);
					if (item.children.length >= 1) recurse(item, layers, cache);
					path.pop();
				});
			};

			recurse(displayFolderTreeStructure, this.layers, this.layerCache);
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateLayer, (updateLayer) => {
			const targetPath = updateLayer.data.path;
			const targetLayer = updateLayer.data;

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
