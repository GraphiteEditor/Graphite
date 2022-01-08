<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar scrollable-x'">
			<div class="left side">
				<DropdownInput :menuEntries="documentModeEntries" v-model:selectedIndex="documentModeSelectionIndex" :drawIcon="true" />

				<Separator :type="'Section'" />

				<ToolOptions :activeTool="activeTool" :activeToolOptions="activeToolOptions" />
			</div>
			<div class="spacer"></div>
			<div class="right side">
				<OptionalInput v-model:checked="snappingEnabled" @update:checked="(newStatus) => setSnap(newStatus)" :icon="'Snapping'" title="Snapping" />
				<PopoverButton>
					<h3>Snapping</h3>
					<p>The contents of this popover menu are coming soon</p>
				</PopoverButton>

				<Separator :type="'Unrelated'" />

				<OptionalInput v-model:checked="gridEnabled" @update:checked="() => dialog.comingSoon(318)" :icon="'Grid'" title="Grid" />
				<PopoverButton>
					<h3>Grid</h3>
					<p>The contents of this popover menu are coming soon</p>
				</PopoverButton>

				<Separator :type="'Unrelated'" />

				<OptionalInput v-model:checked="overlaysEnabled" @update:checked="() => dialog.comingSoon(99)" :icon="'Overlays'" title="Overlays" />
				<PopoverButton>
					<h3>Overlays</h3>
					<p>The contents of this popover menu are coming soon</p>
				</PopoverButton>

				<Separator :type="'Unrelated'" />

				<RadioInput :entries="viewModeEntries" v-model:selectedIndex="viewModeIndex" class="combined-after" />
				<PopoverButton>
					<h3>View Mode</h3>
					<p>The contents of this popover menu are coming soon</p>
				</PopoverButton>

				<Separator :type="'Section'" />

				<NumberInput @update:value="(newRotation) => setRotation(newRotation)" v-model:value="documentRotation" :incrementFactor="15" :unit="'°'" />

				<Separator :type="'Section'" />

				<IconButton :action="increaseCanvasZoom" :icon="'ZoomIn'" :size="24" title="Zoom In" />
				<IconButton :action="decreaseCanvasZoom" :icon="'ZoomOut'" :size="24" title="Zoom Out" />
				<IconButton :action="() => setCanvasZoom(100)" :icon="'ZoomReset'" :size="24" title="Zoom to 100%" />

				<Separator :type="'Related'" />

				<NumberInput
					v-model:value="documentZoom"
					@update:value="(newZoom) => setCanvasZoom(newZoom)"
					:min="0.000001"
					:max="1000000"
					:incrementBehavior="'Callback'"
					:incrementCallbackIncrease="increaseCanvasZoom"
					:incrementCallbackDecrease="decreaseCanvasZoom"
					:unit="'%'"
					:displayDecimalPlaces="4"
					ref="zoom"
				/>
			</div>
		</LayoutRow>
		<LayoutRow :class="'shelf-and-viewport'">
			<LayoutCol :class="'shelf'">
				<div class="tools scrollable-y">
					<ShelfItemInput icon="LayoutSelectTool" title="Select Tool (V)" :active="activeTool === 'Select'" :action="() => selectTool('Select')" />
					<ShelfItemInput icon="LayoutCropTool" title="Crop Tool" :active="activeTool === 'Crop'" :action="() => (dialog.comingSoon(289), false) && selectTool('Crop')" />
					<ShelfItemInput icon="LayoutNavigateTool" title="Navigate Tool (Z)" :active="activeTool === 'Navigate'" :action="() => selectTool('Navigate')" />
					<ShelfItemInput icon="LayoutEyedropperTool" title="Eyedropper Tool (I)" :active="activeTool === 'Eyedropper'" :action="() => selectTool('Eyedropper')" />

					<Separator :type="'Section'" :direction="'Vertical'" />

					<ShelfItemInput icon="ParametricTextTool" title="Text Tool (T)" :active="activeTool === 'Text'" :action="() => (dialog.comingSoon(153), false) && selectTool('Text')" />
					<ShelfItemInput icon="ParametricFillTool" title="Fill Tool (F)" :active="activeTool === 'Fill'" :action="() => selectTool('Fill')" />
					<ShelfItemInput
						icon="ParametricGradientTool"
						title="Gradient Tool (H)"
						:active="activeTool === 'Gradient'"
						:action="() => (dialog.comingSoon(), false) && selectTool('Gradient')"
					/>

					<Separator :type="'Section'" :direction="'Vertical'" />

					<ShelfItemInput icon="RasterBrushTool" title="Brush Tool (B)" :active="activeTool === 'Brush'" :action="() => (dialog.comingSoon(), false) && selectTool('Brush')" />
					<ShelfItemInput icon="RasterHealTool" title="Heal Tool (J)" :active="activeTool === 'Heal'" :action="() => (dialog.comingSoon(), false) && selectTool('Heal')" />
					<ShelfItemInput icon="RasterCloneTool" title="Clone Tool (C)" :active="activeTool === 'Clone'" :action="() => (dialog.comingSoon(), false) && selectTool('Clone')" />
					<ShelfItemInput icon="RasterPatchTool" title="Patch Tool" :active="activeTool === 'Patch'" :action="() => (dialog.comingSoon(), false) && selectTool('Patch')" />
					<ShelfItemInput icon="RasterDetailTool" title="Detail Tool (D)" :active="activeTool === 'Detail'" :action="() => (dialog.comingSoon(), false) && selectTool('Detail')" />
					<ShelfItemInput icon="RasterRelightTool" title="Relight Tool (O)" :active="activeTool === 'Relight'" :action="() => (dialog.comingSoon(), false) && selectTool('Relight')" />

					<Separator :type="'Section'" :direction="'Vertical'" />

					<ShelfItemInput icon="VectorPathTool" title="Path Tool (A)" :active="activeTool === 'Path'" :action="() => selectTool('Path')" />
					<ShelfItemInput icon="VectorPenTool" title="Pen Tool (P)" :active="activeTool === 'Pen'" :action="() => selectTool('Pen')" />
					<ShelfItemInput icon="VectorFreehandTool" title="Freehand Tool (N)" :active="activeTool === 'Freehand'" :action="() => (dialog.comingSoon(), false) && selectTool('Freehand')" />
					<ShelfItemInput icon="VectorSplineTool" title="Spline Tool" :active="activeTool === 'Spline'" :action="() => (dialog.comingSoon(), false) && selectTool('Spline')" />
					<ShelfItemInput icon="VectorLineTool" title="Line Tool (L)" :active="activeTool === 'Line'" :action="() => selectTool('Line')" />
					<ShelfItemInput icon="VectorRectangleTool" title="Rectangle Tool (M)" :active="activeTool === 'Rectangle'" :action="() => selectTool('Rectangle')" />
					<ShelfItemInput icon="VectorEllipseTool" title="Ellipse Tool (E)" :active="activeTool === 'Ellipse'" :action="() => selectTool('Ellipse')" />
					<ShelfItemInput icon="VectorShapeTool" title="Shape Tool (Y)" :active="activeTool === 'Shape'" :action="() => selectTool('Shape')" />
				</div>
				<div class="spacer"></div>
				<div class="working-colors">
					<SwatchPairInput />
					<div class="swap-and-reset">
						<IconButton :action="swapWorkingColors" :icon="'Swap'" title="Swap (Shift+X)" :size="16" />
						<IconButton :action="resetWorkingColors" :icon="'ResetColors'" title="Reset (Ctrl+Shift+X)" :size="16" />
					</div>
				</div>
			</LayoutCol>
			<LayoutCol :class="'viewport'">
				<LayoutRow :class="'bar-area'">
					<CanvasRuler :origin="rulerOrigin.x" :majorMarkSpacing="rulerSpacing" :numberInterval="rulerInterval" :direction="'Horizontal'" :class="'top-ruler'" />
				</LayoutRow>
				<LayoutRow :class="'canvas-area'">
					<LayoutCol :class="'bar-area'">
						<CanvasRuler :origin="rulerOrigin.y" :majorMarkSpacing="rulerSpacing" :numberInterval="rulerInterval" :direction="'Vertical'" />
					</LayoutCol>
					<LayoutCol :class="'canvas-area'">
						<div class="canvas" ref="canvas">
							<svg class="artboards" v-html="artboardSvg" :style="{ width: canvasSvgWidth, height: canvasSvgHeight }"></svg>
							<svg class="artwork" v-html="artworkSvg" :style="{ width: canvasSvgWidth, height: canvasSvgHeight }"></svg>
							<svg class="overlays" v-html="overlaysSvg" :style="{ width: canvasSvgWidth, height: canvasSvgHeight }"></svg>
						</div>
					</LayoutCol>
					<LayoutCol :class="'bar-area'">
						<PersistentScrollbar
							:direction="'Vertical'"
							:handlePosition="scrollbarPos.y"
							@update:handlePosition="(newValue) => translateCanvasY(newValue)"
							v-model:handleLength="scrollbarSize.y"
							@pressTrack="(delta) => pageY(delta)"
							:class="'right-scrollbar'"
						/>
					</LayoutCol>
				</LayoutRow>
				<LayoutRow :class="'bar-area'">
					<PersistentScrollbar
						:direction="'Horizontal'"
						:handlePosition="scrollbarPos.x"
						@update:handlePosition="(newValue) => translateCanvasX(newValue)"
						v-model:handleLength="scrollbarSize.x"
						@pressTrack="(delta) => pageX(delta)"
						:class="'bottom-scrollbar'"
					/>
				</LayoutRow>
			</LayoutCol>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.document {
	height: 100%;

	.options-bar {
		height: 32px;
		flex: 0 0 auto;

		.side {
			height: 100%;
			flex: 0 0 auto;
			display: flex;
			align-items: center;
			margin: 0 4px;
		}

		.spacer {
			min-width: 40px;
		}
	}

	.shelf-and-viewport {
		.shelf {
			flex: 0 0 auto;
			display: flex;
			flex-direction: column;

			.tools {
				flex: 0 1 auto;
			}

			.spacer {
				flex: 1 0 auto;
				min-height: 8px;
			}

			.working-colors .swap-and-reset {
				flex: 0 0 auto;
				display: flex;
			}
		}

		.viewport {
			flex: 1 1 100%;

			.canvas-area {
				flex: 1 1 100%;
			}

			.bar-area {
				flex: 0 0 auto;
			}

			.top-ruler {
				padding-left: 16px;
				margin-right: 16px;
			}

			.right-scrollbar {
				margin-top: -16px;
			}

			.bottom-scrollbar {
				margin-right: 16px;
			}

			.canvas {
				background: var(--color-1-nearblack);
				width: 100%;
				height: 100%;
				// Allows the SVG to be placed at explicit integer values of width and height to prevent non-pixel-perfect SVG scaling
				position: relative;
				overflow: hidden;

				svg {
					position: absolute;
					// Fallback values if JS hasn't set these to integers yet
					width: 100%;
					height: 100%;
					// Allows dev tools to select the artwork without being blocked by the SVG containers
					pointer-events: none;

					// Prevent inheritance from reaching the child elements
					> * {
						pointer-events: auto;
					}
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { UpdateArtwork, UpdateOverlays, UpdateScrollbars, UpdateRulers, SetActiveTool, SetCanvasZoom, SetCanvasRotation, ToolName, UpdateArtboards } from "@/dispatcher/js-messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import RadioInput, { RadioEntries } from "@/components/widgets/inputs/RadioInput.vue";
import ShelfItemInput from "@/components/widgets/inputs/ShelfItemInput.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import ToolOptions from "@/components/widgets/options/ToolOptions.vue";
import CanvasRuler from "@/components/widgets/rulers/CanvasRuler.vue";
import PersistentScrollbar from "@/components/widgets/scrollbars/PersistentScrollbar.vue";
import Separator from "@/components/widgets/separators/Separator.vue";

export default defineComponent({
	inject: ["editor", "dialog"],
	methods: {
		setSnap(newStatus: boolean) {
			this.editor.instance.set_snapping(newStatus);
		},
		setViewMode(newViewMode: string) {
			this.editor.instance.set_view_mode(newViewMode);
		},
		viewportResize() {
			const canvas = this.$refs.canvas as HTMLElement;
			// Get the width and height rounded up to the nearest even number because resizing is centered and dividing an odd number by 2 for centering causes antialiasing
			let width = Math.ceil(parseFloat(getComputedStyle(canvas).width));
			if (width % 2 === 1) width += 1;
			let height = Math.ceil(parseFloat(getComputedStyle(canvas).height));
			if (height % 2 === 1) height += 1;

			this.canvasSvgWidth = `${width}px`;
			this.canvasSvgHeight = `${height}px`;
		},
		setCanvasZoom(newZoom: number) {
			this.editor.instance.set_canvas_zoom(newZoom / 100);
		},
		increaseCanvasZoom() {
			this.editor.instance.increase_canvas_zoom();
		},
		decreaseCanvasZoom() {
			this.editor.instance.decrease_canvas_zoom();
		},
		setRotation(newRotation: number) {
			this.editor.instance.set_rotation(newRotation * (Math.PI / 180));
		},
		translateCanvasX(newValue: number) {
			const delta = newValue - this.scrollbarPos.x;
			this.scrollbarPos.x = newValue;
			this.editor.instance.translate_canvas(-delta * this.scrollbarMultiplier.x, 0);
		},
		translateCanvasY(newValue: number) {
			const delta = newValue - this.scrollbarPos.y;
			this.scrollbarPos.y = newValue;
			this.editor.instance.translate_canvas(0, -delta * this.scrollbarMultiplier.y);
		},
		pageX(delta: number) {
			const move = delta < 0 ? 1 : -1;
			this.editor.instance.translate_canvas_by_fraction(move, 0);
		},
		pageY(delta: number) {
			const move = delta < 0 ? 1 : -1;
			this.editor.instance.translate_canvas_by_fraction(0, move);
		},
		selectTool(toolName: string) {
			this.editor.instance.select_tool(toolName);
		},
		swapWorkingColors() {
			this.editor.instance.swap_colors();
		},
		resetWorkingColors() {
			this.editor.instance.reset_colors();
		},
	},
	mounted() {
		this.editor.dispatcher.subscribeJsMessage(UpdateArtwork, (UpdateArtwork) => {
			this.artworkSvg = UpdateArtwork.svg;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateOverlays, (updateOverlays) => {
			this.overlaysSvg = updateOverlays.svg;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateArtboards, (updateArtboards) => {
			this.artboardSvg = updateArtboards.svg;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateScrollbars, (updateScrollbars) => {
			this.scrollbarPos = updateScrollbars.position;
			this.scrollbarSize = updateScrollbars.size;
			this.scrollbarMultiplier = updateScrollbars.multiplier;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateRulers, (updateRulers) => {
			this.rulerOrigin = updateRulers.origin;
			this.rulerSpacing = updateRulers.spacing;
			this.rulerInterval = updateRulers.interval;
		});

		this.editor.dispatcher.subscribeJsMessage(SetActiveTool, (setActiveTool) => {
			this.activeTool = setActiveTool.tool_name;
			this.activeToolOptions = setActiveTool.tool_options;
		});

		this.editor.dispatcher.subscribeJsMessage(SetCanvasZoom, (setCanvasZoom) => {
			this.documentZoom = setCanvasZoom.new_zoom * 100;
		});

		this.editor.dispatcher.subscribeJsMessage(SetCanvasRotation, (setCanvasRotation) => {
			const newRotation = setCanvasRotation.new_radians * (180 / Math.PI);
			this.documentRotation = (360 + (newRotation % 360)) % 360;
		});

		window.addEventListener("resize", this.viewportResize);
		window.addEventListener("DOMContentLoaded", this.viewportResize);
	},
	data() {
		const documentModeEntries: SectionsOfMenuListEntries = [
			[
				{ label: "Design Mode", icon: "ViewportDesignMode" },
				{ label: "Select Mode", icon: "ViewportSelectMode", action: (): void => this.dialog.comingSoon(330) },
				{ label: "Guide Mode", icon: "ViewportGuideMode", action: (): void => this.dialog.comingSoon(331) },
			],
		];
		const viewModeEntries: RadioEntries = [
			{ value: "normal", icon: "ViewModeNormal", tooltip: "View Mode: Normal", action: (): void => this.setViewMode("Normal") },
			{ value: "outline", icon: "ViewModeOutline", tooltip: "View Mode: Outline", action: (): void => this.setViewMode("Outline") },
			{ value: "pixels", icon: "ViewModePixels", tooltip: "View Mode: Pixels", action: (): void => this.dialog.comingSoon(320) },
		];

		return {
			artworkSvg: "",
			artboardSvg: "",
			overlaysSvg: "",
			canvasSvgWidth: "100%",
			canvasSvgHeight: "100%",
			activeTool: "Select" as ToolName,
			activeToolOptions: {},
			documentModeEntries,
			viewModeEntries,
			documentModeSelectionIndex: 0,
			viewModeIndex: 0,
			snappingEnabled: true,
			gridEnabled: true,
			overlaysEnabled: true,
			documentRotation: 0,
			documentZoom: 100,
			scrollbarPos: { x: 0.5, y: 0.5 },
			scrollbarSize: { x: 0.5, y: 0.5 },
			scrollbarMultiplier: { x: 0, y: 0 },
			rulerOrigin: { x: 0, y: 0 },
			rulerSpacing: 100,
			rulerInterval: 100,
		};
	},
	components: {
		LayoutRow,
		LayoutCol,
		SwatchPairInput,
		ShelfItemInput,
		Separator,
		PersistentScrollbar,
		CanvasRuler,
		IconButton,
		PopoverButton,
		RadioInput,
		NumberInput,
		DropdownInput,
		OptionalInput,
		ToolOptions,
	},
});
</script>
