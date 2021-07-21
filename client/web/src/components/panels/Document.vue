<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar'">
			<div class="left side">
				<DropdownInput :menuEntries="modeMenuEntries" :default="modeMenuEntries[0][0]" :callbackOnChange="notImplemented" :drawIcon="true" />

				<Separator :type="SeparatorType.Section" />

				<ToolOptions :activeTool="activeTool" />
			</div>
			<div class="spacer"></div>
			<div class="right side">
				<OptionalInput v-model:checked="snappingEnabled" :icon="'Snapping'" title="Snapping" />
				<PopoverButton>
					<h3>Snapping</h3>
					<p>More snapping options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Unrelated" />

				<OptionalInput v-model:checked="gridEnabled" :icon="'Grid'" title="Grid" />
				<PopoverButton>
					<h3>Grid</h3>
					<p>More grid options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Unrelated" />

				<OptionalInput v-model:checked="overlaysEnabled" :icon="'Overlays'" title="Overlays" />
				<PopoverButton>
					<h3>Overlays</h3>
					<p>More overlays options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Unrelated" />

				<RadioInput v-model:index="viewModeIndex">
					<IconButton :icon="'ViewModeNormal'" :size="24" title="View Mode: Normal" />
					<IconButton :icon="'ViewModeOutline'" :size="24" title="View Mode: Outline" />
					<IconButton :icon="'ViewModePixels'" :size="24" title="View Mode: Pixels" />
				</RadioInput>
				<PopoverButton>
					<h3>View Mode</h3>
					<p>More view mode options will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Section" />

				<NumberInput :callback="setRotation" :initialValue="0" :step="15" :unit="`°`" :updateOnCallback="false" ref="rotation" />

				<Separator :type="SeparatorType.Section" />

				<IconButton :icon="'ZoomIn'" :size="24" title="Zoom In" @click="this.$refs.zoom.onIncrement(1)" />
				<IconButton :icon="'ZoomOut'" :size="24" title="Zoom Out" @click="this.$refs.zoom.onIncrement(-1)" />
				<IconButton :icon="'ZoomReset'" :size="24" title="Zoom to 100%" @click="this.$refs.zoom.updateValue(100)" />

				<Separator :type="SeparatorType.Related" />

				<NumberInput :callback="setZoom" :initialValue="100" :min="0.001" :increaseMultiplier="1.25" :decreaseMultiplier="0.8" :unit="`%`" :updateOnCallback="false" ref="zoom" />
			</div>
		</LayoutRow>
		<LayoutRow :class="'shelf-and-viewport'">
			<LayoutCol :class="'shelf'">
				<div class="tools">
					<ShelfItemInput :icon="'LayoutSelectTool'" title="Select Tool (V)" :active="activeTool === 'Select'" @click="selectTool('Select')" />
					<ShelfItemInput :icon="'LayoutCropTool'" title="Crop Tool" :active="activeTool === 'Crop'" @click="'tool not implemented' || selectTool('Crop')" />
					<ShelfItemInput :icon="'LayoutNavigateTool'" title="Navigate Tool (Z)" :active="activeTool === 'Navigate'" @click="'tool not implemented' || selectTool('Navigate')" />
					<ShelfItemInput :icon="'LayoutEyedropperTool'" title="Eyedropper Tool (I)" :active="activeTool === 'Eyedropper'" @click="'tool not implemented' || selectTool('Eyedropper')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItemInput :icon="'ParametricTextTool'" title="Text Tool (T)" :active="activeTool === 'Text'" @click="'tool not implemented' || selectTool('Text')" />
					<ShelfItemInput :icon="'ParametricFillTool'" title="Fill Tool (F)" :active="activeTool === 'Fill'" @click="selectTool('Fill')" />
					<ShelfItemInput :icon="'ParametricGradientTool'" title="Gradient Tool (H)" :active="activeTool === 'Gradient'" @click="'tool not implemented' || selectTool('Gradient')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItemInput :icon="'RasterBrushTool'" title="Brush Tool (B)" :active="activeTool === 'Brush'" @click="'tool not implemented' || selectTool('Brush')" />
					<ShelfItemInput :icon="'RasterHealTool'" title="Heal Tool (J)" :active="activeTool === 'Heal'" @click="'tool not implemented' || selectTool('Heal')" />
					<ShelfItemInput :icon="'RasterCloneTool'" title="Clone Tool (C)" :active="activeTool === 'Clone'" @click="'tool not implemented' || selectTool('Clone')" />
					<ShelfItemInput :icon="'RasterPatchTool'" title="Patch Tool" :active="activeTool === 'Patch'" @click="'tool not implemented' || selectTool('Patch')" />
					<ShelfItemInput :icon="'RasterBlurSharpenTool'" title="Detail Tool (D)" :active="activeTool === 'BlurSharpen'" @click="'tool not implemented' || selectTool('BlurSharpen')" />
					<ShelfItemInput :icon="'RasterRelightTool'" title="Relight Tool (O)" :active="activeTool === 'Relight'" @click="'tool not implemented' || selectTool('Relight')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItemInput :icon="'VectorPathTool'" title="Path Tool (A)" :active="activeTool === 'Path'" @click="'tool not implemented' || selectTool('Path')" />
					<ShelfItemInput :icon="'VectorPenTool'" title="Pen Tool (P)" :active="activeTool === 'Pen'" @click="selectTool('Pen')" />
					<ShelfItemInput :icon="'VectorFreehandTool'" title="Freehand Tool (N)" :active="activeTool === 'Freehand'" @click="'tool not implemented' || selectTool('Freehand')" />
					<ShelfItemInput :icon="'VectorSplineTool'" title="Spline Tool" :active="activeTool === 'Spline'" @click="'tool not implemented' || selectTool('Spline')" />
					<ShelfItemInput :icon="'VectorLineTool'" title="Line Tool (L)" :active="activeTool === 'Line'" @click="selectTool('Line')" />
					<ShelfItemInput :icon="'VectorRectangleTool'" title="Rectangle Tool (M)" :active="activeTool === 'Rectangle'" @click="selectTool('Rectangle')" />
					<ShelfItemInput :icon="'VectorEllipseTool'" title="Ellipse Tool (E)" :active="activeTool === 'Ellipse'" @click="selectTool('Ellipse')" />
					<ShelfItemInput :icon="'VectorShapeTool'" title="Shape Tool (Y)" :active="activeTool === 'Shape'" @click="selectTool('Shape')" />
				</div>
				<div class="spacer"></div>
				<div class="working-colors">
					<SwatchPairInput />
					<div class="swap-and-reset">
						<IconButton @click="swapWorkingColors" :icon="'Swap'" title="Swap (Shift+X)" :size="16" />
						<IconButton @click="resetWorkingColors" :icon="'ResetColors'" title="Reset (Ctrl+Shift+X)" :size="16" />
					</div>
				</div>
			</LayoutCol>
			<LayoutCol :class="'viewport'">
				<LayoutRow :class="'bar-area'">
					<CanvasRuler :origin="0" :majorMarkSpacing="75" :direction="RulerDirection.Horizontal" :class="'top-ruler'" />
				</LayoutRow>
				<LayoutRow :class="'canvas-area'">
					<LayoutCol :class="'bar-area'">
						<CanvasRuler :origin="0" :majorMarkSpacing="75" :direction="RulerDirection.Vertical" />
					</LayoutCol>
					<LayoutCol :class="'canvas-area'">
						<div class="canvas" @mousedown="canvasMouseDown" @mouseup="canvasMouseUp" @mousemove="canvasMouseMove" ref="canvas">
							<svg v-html="viewportSvg"></svg>
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

				svg {
					background: #ffffff;
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
import { ResponseType, registerResponseHandler, Response, UpdateCanvas, SetActiveTool, ExportDocument, SetCanvasZoom, SetCanvasRotation } from "@/utilities/response-handler";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import { MenuDirection } from "@/components/widgets/floating-menus/FloatingMenu.vue";
import ShelfItemInput from "@/components/widgets/inputs/ShelfItemInput.vue";
import Separator, { SeparatorDirection, SeparatorType } from "@/components/widgets/separators/Separator.vue";
import PersistentScrollbar, { ScrollbarDirection } from "@/components/widgets/scrollbars/PersistentScrollbar.vue";
import CanvasRuler, { RulerDirection } from "@/components/widgets/rulers/CanvasRuler.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import PopoverButton from "@/components/widgets/buttons/PopoverButton.vue";
import RadioInput from "@/components/widgets/inputs/RadioInput.vue";
import NumberInput from "@/components/widgets/inputs/NumberInput.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import OptionalInput from "@/components/widgets/inputs/OptionalInput.vue";
import ToolOptions from "@/components/widgets/options/ToolOptions.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";

const modeMenuEntries: SectionsOfMenuListEntries = [
	[
		{ label: "Design Mode", icon: "ViewportDesignMode" },
		{ label: "Select Mode", icon: "ViewportSelectMode" },
		{ label: "Guide Mode", icon: "ViewportGuideMode" },
	],
];

const wasm = import("@/../wasm/pkg");

export default defineComponent({
	methods: {
		async viewportResize() {
			const { viewport_resize } = await wasm;
			const canvas = this.$refs.canvas as HTMLDivElement;
			viewport_resize(canvas.clientWidth, canvas.clientHeight);
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
		async viewModeChanged(toolIndex: number) {
			function todo(_: number) {
				return _;
			}
			todo(toolIndex);
		},
		notImplemented() {
			window.alert("Not implemented");
		},
		async swapWorkingColors() {
			const { swap_colors } = await wasm;
			swap_colors();
		},
		async resetWorkingColors() {
			const { reset_colors } = await wasm;
			reset_colors();
		},
		download(filename: string, svgData: string) {
			const svgBlob = new Blob([svgData], { type: "image/svg+xml;charset=utf-8" });
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
		registerResponseHandler(ResponseType.SetActiveTool, (responseData: Response) => {
			const toolData = responseData as SetActiveTool;
			if (toolData) this.activeTool = toolData.tool_name;
		});
		registerResponseHandler(ResponseType.SetCanvasZoom, (responseData: Response) => {
			const updateData = responseData as SetCanvasZoom;
			if (updateData) {
				const zoomWidget = this.$refs.zoom as typeof NumberInput;
				zoomWidget.setValue(updateData.new_zoom * 100);
			}
		});
		registerResponseHandler(ResponseType.SetCanvasRotation, (responseData: Response) => {
			const updateData = responseData as SetCanvasRotation;
			if (updateData) {
				const rotationWidget = this.$refs.rotation as typeof NumberInput;
				const newRotation = updateData.new_radians * (180 / Math.PI);
				rotationWidget.setValue((360 + (newRotation % 360)) % 360);
			}
		});

		// TODO: Move event listeners to `main.ts`
		const canvas = this.$refs.canvas as HTMLDivElement;
		canvas.addEventListener("wheel", this.canvasMouseScroll, { passive: false });

		window.addEventListener("resize", () => this.viewportResize());
		window.addEventListener("DOMContentLoaded", () => this.viewportResize());

		this.$watch("viewModeIndex", this.viewModeChanged);
	},
	data() {
		return {
			viewportSvg: "",
			activeTool: "Select",
			modeMenuEntries,
			viewModeIndex: 0,
			snappingEnabled: true,
			gridEnabled: true,
			overlaysEnabled: true,
			MenuDirection,
			SeparatorDirection,
			ScrollbarDirection,
			RulerDirection,
			SeparatorType,
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
