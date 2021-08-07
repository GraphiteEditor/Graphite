<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar'">
			<div class="left side">
				<DropdownInput :menuEntries="documentModeEntries" v-model:selectedIndex="documentModeSelectionIndex" :drawIcon="true" />

				<Separator :type="SeparatorType.Section" />

				<ToolOptions :activeTool="activeTool" />
			</div>
			<div class="spacer"></div>
			<div class="right side">
				<OptionalInput v-model:checked="snappingEnabled" @update:checked="comingSoon(200)" :icon="'Snapping'" title="Snapping" />
				<PopoverButton>
					<h3>Snapping</h3>
					<p>More snapping options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Unrelated" />

				<OptionalInput v-model:checked="gridEnabled" @update:checked="comingSoon(318)" :icon="'Grid'" title="Grid" />
				<PopoverButton>
					<h3>Grid</h3>
					<p>More grid options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Unrelated" />

				<OptionalInput v-model:checked="overlaysEnabled" @update:checked="comingSoon(99)" :icon="'Overlays'" title="Overlays" />
				<PopoverButton>
					<h3>Overlays</h3>
					<p>More overlays options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Unrelated" />

				<RadioInput :entries="viewModeEntries" v-model:selectedIndex="viewModeIndex" />
				<PopoverButton>
					<h3>View Mode</h3>
					<p>More view mode options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Section" />

				<NumberInput @update:value="setRotation" v-model:value="documentRotation" :step="15" :unit="`°`" ref="rotation" />

				<Separator :type="SeparatorType.Section" />

				<IconButton :action="() => this.$refs.zoom.onIncrement(IncrementDirection.Increase)" :icon="'ZoomIn'" :size="24" title="Zoom In" />
				<IconButton :action="() => this.$refs.zoom.onIncrement(IncrementDirection.Decrease)" :icon="'ZoomOut'" :size="24" title="Zoom Out" />
				<IconButton :action="() => this.$refs.zoom.updateValue(100)" :icon="'ZoomReset'" :size="24" title="Zoom to 100%" />

				<Separator :type="SeparatorType.Related" />

				<NumberInput
					v-model:value="documentZoom"
					@update:value="setZoom"
					:min="0.000001"
					:max="1000000"
					:step="1.25"
					:stepIsMultiplier="true"
					:unit="`%`"
					:displayDecimalPlaces="4"
					ref="zoom"
				/>
			</div>
		</LayoutRow>
		<LayoutRow :class="'shelf-and-viewport'">
			<LayoutCol :class="'shelf'">
				<div class="tools">
					<ShelfItemInput icon="LayoutSelectTool" title="Select Tool (V)" :active="activeTool === 'Select'" :action="() => selectTool('Select')" />
					<ShelfItemInput icon="LayoutCropTool" title="Crop Tool" :active="activeTool === 'Crop'" :action="() => comingSoon(289) && selectTool('Crop')" />
					<ShelfItemInput icon="LayoutNavigateTool" title="Navigate Tool (Z)" :active="activeTool === 'Navigate'" :action="() => comingSoon(155) && selectTool('Navigate')" />
					<ShelfItemInput icon="LayoutEyedropperTool" title="Eyedropper Tool (I)" :active="activeTool === 'Eyedropper'" :action="() => selectTool('Eyedropper')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItemInput icon="ParametricTextTool" title="Text Tool (T)" :active="activeTool === 'Text'" :action="() => comingSoon(153) && selectTool('Text')" />
					<ShelfItemInput icon="ParametricFillTool" title="Fill Tool (F)" :active="activeTool === 'Fill'" :action="() => selectTool('Fill')" />
					<ShelfItemInput icon="ParametricGradientTool" title="Gradient Tool (H)" :active="activeTool === 'Gradient'" :action="() => comingSoon() && selectTool('Gradient')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItemInput icon="RasterBrushTool" title="Brush Tool (B)" :active="activeTool === 'Brush'" :action="() => comingSoon() && selectTool('Brush')" />
					<ShelfItemInput icon="RasterHealTool" title="Heal Tool (J)" :active="activeTool === 'Heal'" :action="() => comingSoon() && selectTool('Heal')" />
					<ShelfItemInput icon="RasterCloneTool" title="Clone Tool (C)" :active="activeTool === 'Clone'" :action="() => comingSoon() && selectTool('Clone')" />
					<ShelfItemInput icon="RasterPatchTool" title="Patch Tool" :active="activeTool === 'Patch'" :action="() => comingSoon() && selectTool('Patch')" />
					<ShelfItemInput icon="RasterBlurSharpenTool" title="Detail Tool (D)" :active="activeTool === 'BlurSharpen'" :action="() => comingSoon() && selectTool('BlurSharpen')" />
					<ShelfItemInput icon="RasterRelightTool" title="Relight Tool (O)" :active="activeTool === 'Relight'" :action="() => comingSoon() && selectTool('Relight')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItemInput icon="VectorPathTool" title="Path Tool (A)" :active="activeTool === 'Path'" :action="() => comingSoon(82) && selectTool('Path')" />
					<ShelfItemInput icon="VectorPenTool" title="Pen Tool (P)" :active="activeTool === 'Pen'" :action="() => selectTool('Pen')" />
					<ShelfItemInput icon="VectorFreehandTool" title="Freehand Tool (N)" :active="activeTool === 'Freehand'" :action="() => comingSoon() && selectTool('Freehand')" />
					<ShelfItemInput icon="VectorSplineTool" title="Spline Tool" :active="activeTool === 'Spline'" :action="() => comingSoon() && selectTool('Spline')" />
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
					<CanvasRuler :origin="0" :majorMarkSpacing="100" :direction="RulerDirection.Horizontal" :class="'top-ruler'" />
				</LayoutRow>
				<LayoutRow :class="'canvas-area'">
					<LayoutCol :class="'bar-area'">
						<CanvasRuler :origin="0" :majorMarkSpacing="100" :direction="RulerDirection.Vertical" />
					</LayoutCol>
					<LayoutCol :class="'canvas-area'">
						<div class="canvas" @mousedown="canvasMouseDown" @mouseup="canvasMouseUp" @mousemove="canvasMouseMove" ref="canvas">
							<svg v-html="viewportSvg" :style="{ width: canvasSvgWidth, height: canvasSvgHeight }"></svg>
						</div>
					</LayoutCol>
					<LayoutCol :class="'bar-area'">
						<PersistentScrollbar :direction="ScrollbarDirection.Vertical" :class="'right-scrollbar'" />
					</LayoutCol>
				</LayoutRow>
				<LayoutRow :class="'bar-area'">
					<PersistentScrollbar :direction="ScrollbarDirection.Horizontal" :class="'bottom-scrollbar'" />
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
	}

	.shelf-and-viewport {
		.shelf {
			flex: 0 0 auto;
			display: flex;
			flex-direction: column;

			.working-colors .swap-and-reset {
				font-size: 0;
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
					background: #ffffff;
					position: absolute;
					// Fallback values if JS hasn't set these to integers yet
					width: 100%;
					height: 100%;
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { makeModifiersBitfield } from "@/utilities/input";
import { ResponseType, registerResponseHandler, Response, UpdateCanvas, SetActiveTool, ExportDocument, SetCanvasZoom, SetCanvasRotation, SaveDocument } from "@/utilities/response-handler";
import { SeparatorDirection, SeparatorType } from "@/components/widgets/widgets";
import { comingSoon } from "@/utilities/errors";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";
import ShelfItemInput from "@/components/widgets/inputs/ShelfItemInput.vue";
import Separator from "@/components/widgets/separators/Separator.vue";
import PersistentScrollbar, { ScrollbarDirection } from "@/components/widgets/scrollbars/PersistentScrollbar.vue";
import CanvasRuler, { RulerDirection } from "@/components/widgets/rulers/CanvasRuler.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import RadioInput, { RadioEntries } from "@/components/widgets/inputs/RadioInput.vue";
import NumberInput, { IncrementDirection } from "@/components/widgets/inputs/NumberInput.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import ToolOptions from "@/components/widgets/options/ToolOptions.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";

const documentModeEntries: SectionsOfMenuListEntries = [
	[
		{ label: "Design Mode", icon: "ViewportDesignMode" },
		{ label: "Select Mode", icon: "ViewportSelectMode", action: () => comingSoon(330) },
		{ label: "Guide Mode", icon: "ViewportGuideMode", action: () => comingSoon(331) },
	],
];
const viewModeEntries: RadioEntries = [
	{ value: "normal", icon: "ViewModeNormal", tooltip: "View Mode: Normal" },
	{ value: "outline", icon: "ViewModeOutline", tooltip: "View Mode: Outline", action: () => comingSoon(319) },
	{ value: "pixels", icon: "ViewModePixels", tooltip: "View Mode: Pixels", action: () => comingSoon(320) },
];

const wasm = import("@/../wasm/pkg");

export default defineComponent({
	methods: {
		async viewportResize() {
			const canvas = this.$refs.canvas as HTMLElement;
			// Get the width and height rounded up to the nearest even number because resizing is centered and dividing an odd number by 2 for centering causes antialiasing
			let width = Math.ceil(parseFloat(getComputedStyle(canvas).width));
			if (width % 2 === 1) width += 1;
			let height = Math.ceil(parseFloat(getComputedStyle(canvas).height));
			if (height % 2 === 1) height += 1;

			this.canvasSvgWidth = `${width}px`;
			this.canvasSvgHeight = `${height}px`;

			const { viewport_resize } = await wasm;
			viewport_resize(width, height);
		},
		async canvasMouseDown(e: MouseEvent) {
			const { on_mouse_down } = await wasm;
			const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
			on_mouse_down(e.offsetX, e.offsetY, e.buttons, modifiers);
		},
		async canvasMouseUp(e: MouseEvent) {
			const { on_mouse_up } = await wasm;
			const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
			on_mouse_up(e.offsetX, e.offsetY, e.buttons, modifiers);
		},
		async canvasMouseMove(e: MouseEvent) {
			const { on_mouse_move } = await wasm;
			const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
			on_mouse_move(e.offsetX, e.offsetY, modifiers);
		},
		async canvasMouseScroll(e: WheelEvent) {
			e.preventDefault();
			const { on_mouse_scroll } = await wasm;
			const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
			on_mouse_scroll(e.deltaX, e.deltaY, e.deltaZ, modifiers);
		},
		async setZoom(newZoom: number) {
			const { set_zoom } = await wasm;
			set_zoom(newZoom / 100);
		},
		async setRotation(newRotation: number) {
			const { set_rotation } = await wasm;
			set_rotation(newRotation * (Math.PI / 180));
		},
		async selectTool(toolName: string) {
			const { select_tool } = await wasm;
			select_tool(toolName);
		},
		async swapWorkingColors() {
			const { swap_colors } = await wasm;
			swap_colors();
		},
		async resetWorkingColors() {
			const { reset_colors } = await wasm;
			reset_colors();
		},
		download(filename: string, fileData: string) {
			let type = "text/plain;charset=utf-8";
			if (filename.endsWith(".svg")) {
				type = "image/svg+xml;charset=utf-8";
			}
			const svgBlob = new Blob([fileData], { type });
			const svgUrl = URL.createObjectURL(svgBlob);
			const element = document.createElement("a");

			element.href = svgUrl;
			element.setAttribute("download", filename);
			element.style.display = "none";

			element.click();
		},
	},
	mounted() {
		registerResponseHandler(ResponseType.UpdateCanvas, (responseData: Response) => {
			const updateData = responseData as UpdateCanvas;
			if (updateData) this.viewportSvg = updateData.document;
		});
		registerResponseHandler(ResponseType.ExportDocument, (responseData: Response) => {
			const updateData = responseData as ExportDocument;
			if (updateData) this.download("canvas.svg", updateData.document);
		});
		registerResponseHandler(ResponseType.SaveDocument, (responseData: Response) => {
			const saveData = responseData as SaveDocument;
			if (saveData) this.download("canvas.gsvg", saveData.document);
		});
		registerResponseHandler(ResponseType.SetActiveTool, (responseData: Response) => {
			const toolData = responseData as SetActiveTool;
			if (toolData) this.activeTool = toolData.tool_name;
		});
		registerResponseHandler(ResponseType.SetCanvasZoom, (responseData: Response) => {
			const updateData = responseData as SetCanvasZoom;
			if (updateData) {
				this.documentZoom = updateData.new_zoom * 100;
			}
		});
		registerResponseHandler(ResponseType.SetCanvasRotation, (responseData: Response) => {
			const updateData = responseData as SetCanvasRotation;
			if (updateData) {
				const newRotation = updateData.new_radians * (180 / Math.PI);
				this.documentRotation = (360 + (newRotation % 360)) % 360;
			}
		});

		// TODO: Move event listeners to `main.ts`
		const canvas = this.$refs.canvas as HTMLDivElement;
		canvas.addEventListener("wheel", this.canvasMouseScroll, { passive: false });

		window.addEventListener("resize", () => this.viewportResize());
		window.addEventListener("DOMContentLoaded", () => this.viewportResize());
	},
	data() {
		return {
			viewportSvg: "",
			canvasSvgWidth: "100%",
			canvasSvgHeight: "100%",
			activeTool: "Select",
			documentModeEntries,
			viewModeEntries,
			documentModeSelectionIndex: 0,
			viewModeIndex: 0,
			snappingEnabled: true,
			gridEnabled: true,
			overlaysEnabled: true,
			documentRotation: 0,
			documentZoom: 100,
			IncrementDirection,
			MenuDirection,
			SeparatorDirection,
			ScrollbarDirection,
			RulerDirection,
			SeparatorType,
			comingSoon,
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
