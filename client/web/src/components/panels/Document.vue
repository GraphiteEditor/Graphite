<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar'">
			<div class="left side">
				<DropdownInput :menuEntries="modeMenuEntries" :default="modeMenuEntries[0][0]" :drawIcon="true" />

				<Separator :type="SeparatorType.Section" />

				<IconButton :icon="'AlignHorizontalLeft'" :size="24" title="Horizontal Align Left" />
				<IconButton :icon="'AlignHorizontalCenter'" :size="24" title="Horizontal Align Center" />
				<IconButton :icon="'AlignHorizontalRight'" :size="24" gapAfter title="Horizontal Align Right" />

				<Separator :type="SeparatorType.Unrelated" />

				<IconButton :icon="'AlignVerticalTop'" :size="24" title="Vertical Align Top" />
				<IconButton :icon="'AlignVerticalCenter'" :size="24" title="Vertical Align Center" />
				<IconButton :icon="'AlignVerticalBottom'" :size="24" title="Vertical Align Bottom" />

				<Separator :type="SeparatorType.Related" />

				<PopoverButton>
					<h3>Align</h3>
					<p>More alignment-related buttons will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Section" />

				<IconButton :icon="'FlipHorizontal'" :size="24" title="Flip Horizontal" />
				<IconButton :icon="'FlipVertical'" :size="24" title="Flip Vertical" />

				<Separator :type="SeparatorType.Related" />

				<PopoverButton>
					<h3>Flip</h3>
					<p>More flip-related buttons will be here</p>
				</PopoverButton>

				<Separator :type="SeparatorType.Section" />

				<IconButton :icon="'BooleanUnion'" :size="24" title="Boolean Union" />
				<IconButton :icon="'BooleanSubtractFront'" :size="24" title="Boolean Subtract Front" />
				<IconButton :icon="'BooleanSubtractBack'" :size="24" title="Boolean Subtract Back" />
				<IconButton :icon="'BooleanIntersect'" :size="24" title="Boolean Intersect" />
				<IconButton :icon="'BooleanDifference'" :size="24" title="Boolean Difference" />

				<Separator :type="SeparatorType.Related" />

				<PopoverButton>
					<h3>Boolean</h3>
					<p>More boolean-related buttons will be here</p>
				</PopoverButton>
			</div>
			<div class="spacer"></div>
			<div class="right side">
				<RadioInput v-model:index="viewModeIndex">
					<IconButton :icon="'ViewModeNormal'" :size="24" title="View Mode: Normal" />
					<IconButton :icon="'ViewModeOutline'" :size="24" title="View Mode: Outline" />
					<IconButton :icon="'ViewModePixels'" :size="24" title="View Mode: Pixels" />
					<PopoverButton>
						<h3>Display Mode</h3>
						<p>More display mode options will be here</p>
					</PopoverButton>
				</RadioInput>

				<Separator :type="SeparatorType.Section" />

				<IconButton :icon="'ZoomIn'" :size="24" title="Zoom In" />
				<IconButton :icon="'ZoomOut'" :size="24" title="Zoom Out" />
				<IconButton :icon="'ZoomReset'" :size="24" title="Zoom to 100%" />

				<Separator :type="SeparatorType.Related" />

				<NumberInput :value="25" :unit="`%`" />
			</div>
		</LayoutRow>
		<LayoutRow :class="'shelf-and-viewport'">
			<LayoutCol :class="'shelf'">
				<div class="tools">
					<ShelfItem :icon="'SelectTool'" title="Select Tool (V)" :active="activeTool === 'Select'" @click="selectTool('Select')" />
					<ShelfItem :icon="'CropTool'" title="Crop Tool" :active="activeTool === 'Crop'" @click="'tool not implemented' || selectTool('Crop')" />
					<ShelfItem :icon="'NavigateTool'" title="Navigate Tool (Z)" :active="activeTool === 'Navigate'" @click="'tool not implemented' || selectTool('Navigate')" />
					<ShelfItem :icon="'EyedropperTool'" title="Eyedropper Tool (I)" :active="activeTool === 'Eyedropper'" @click="'tool not implemented' || selectTool('Eyedropper')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItem :icon="'TextTool'" title="Text Tool (T)" :active="activeTool === 'Text'" @click="'tool not implemented' || selectTool('Text')" />
					<ShelfItem :icon="'FillTool'" title="Fill Tool (F)" :active="activeTool === 'Fill'" @click="'tool not implemented' || selectTool('Fill')" />
					<ShelfItem :icon="'GradientTool'" title="Gradient Tool (H)" :active="activeTool === 'Gradient'" @click="'tool not implemented' || selectTool('Gradient')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItem :icon="'BrushTool'" title="Brush Tool (B)" :active="activeTool === 'Brush'" @click="'tool not implemented' || selectTool('Brush')" />
					<ShelfItem :icon="'HealTool'" title="Heal Tool (J)" :active="activeTool === 'Heal'" @click="'tool not implemented' || selectTool('Heal')" />
					<ShelfItem :icon="'CloneTool'" title="Clone Tool (C)" :active="activeTool === 'Clone'" @click="'tool not implemented' || selectTool('Clone')" />
					<ShelfItem :icon="'PatchTool'" title="Patch Tool" :active="activeTool === 'Patch'" @click="'tool not implemented' || selectTool('Patch')" />
					<ShelfItem :icon="'BlurSharpenTool'" title="Detail Tool (D)" :active="activeTool === 'BlurSharpen'" @click="'tool not implemented' || selectTool('BlurSharpen')" />
					<ShelfItem :icon="'RelightTool'" title="Relight Tool (O)" :active="activeTool === 'Relight'" @click="'tool not implemented' || selectTool('Relight')" />

					<Separator :type="SeparatorType.Section" :direction="SeparatorDirection.Vertical" />

					<ShelfItem :icon="'PathTool'" title="Path Tool (A)" :active="activeTool === 'Path'" @click="'tool not implemented' || selectTool('Path')" />
					<ShelfItem :icon="'PenTool'" title="Pen Tool (P)" :active="activeTool === 'Pen'" @click="selectTool('Pen')" />
					<ShelfItem :icon="'FreehandTool'" title="Freehand Tool (N)" :active="activeTool === 'Freehand'" @click="'tool not implemented' || selectTool('Freehand')" />
					<ShelfItem :icon="'SplineTool'" title="Spline Tool" :active="activeTool === 'Spline'" @click="'tool not implemented' || selectTool('Spline')" />
					<ShelfItem :icon="'LineTool'" title="Line Tool (L)" :active="activeTool === 'Line'" @click="selectTool('Line')" />
					<ShelfItem :icon="'RectangleTool'" title="Rectangle Tool (M)" :active="activeTool === 'Rectangle'" @click="selectTool('Rectangle')" />
					<ShelfItem :icon="'EllipseTool'" title="Ellipse Tool (E)" :active="activeTool === 'Ellipse'" @click="selectTool('Ellipse')" />
					<ShelfItem :icon="'ShapeTool'" title="Shape Tool (Y)" :active="activeTool === 'Shape'" @click="selectTool('Shape')" />
				</div>
				<div class="spacer"></div>
				<WorkingColors />
			</LayoutCol>
			<LayoutCol :class="'viewport'">
				<div class="canvas" @mousedown="canvasMouseDown" @mouseup="canvasMouseUp" @mousemove="canvasMouseMove">
					<svg v-html="viewportSvg"></svg>
				</div>
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
		}

		.viewport {
			flex: 1 1 100%;

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
import { ResponseType, registerResponseHandler, Response, UpdateCanvas, SetActiveTool, ExportDocument } from "../../response-handler";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import WorkingColors from "../widgets/WorkingColors.vue";
import { MenuDirection } from "../widgets/floating-menus/FloatingMenu.vue";
import ShelfItem from "../widgets/ShelfItem.vue";
import Separator, { SeparatorDirection, SeparatorType } from "../widgets/Separator.vue";
import IconButton from "../widgets/buttons/IconButton.vue";
import PopoverButton from "../widgets/buttons/PopoverButton.vue";
import RadioInput from "../widgets/inputs/RadioInput.vue";
import NumberInput from "../widgets/inputs/NumberInput.vue";
import DropdownInput from "../widgets/inputs/DropdownInput.vue";
import { SectionsOfMenuListEntries } from "../widgets/floating-menus/MenuList.vue";

const modeMenuEntries: SectionsOfMenuListEntries = [
	[
		{ label: "Design Mode", icon: "ViewportDesignMode" },
		{ label: "Select Mode", icon: "ViewportSelectMode" },
		{ label: "Guide Mode", icon: "ViewportGuideMode" },
	],
];

const wasm = import("../../../wasm/pkg");

function redirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
	// Don't redirect user input from text entry into HTML elements
	const target = e.target as HTMLElement;
	if (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable) return false;

	// Don't redirect a fullscreen request
	if (e.key.toLowerCase() === "f11") return false;

	// Don't redirect debugging tools
	if (e.key.toLowerCase() === "f12") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "c") return false;

	return true;
}

export default defineComponent({
	methods: {
		async canvasMouseDown(e: MouseEvent) {
			const { on_mouse_down } = await wasm;
			on_mouse_down(e.offsetX, e.offsetY, e.buttons);
		},
		async canvasMouseUp(e: MouseEvent) {
			const { on_mouse_up } = await wasm;
			on_mouse_up(e.offsetX, e.offsetY, e.buttons);
		},
		async canvasMouseMove(e: MouseEvent) {
			const { on_mouse_move } = await wasm;
			on_mouse_move(e.offsetX, e.offsetY);
		},
		async keyDown(e: KeyboardEvent) {
			if (redirectKeyboardEventToBackend(e)) {
				e.preventDefault();
				const { on_key_down } = await wasm;
				on_key_down(e.key);
			}
		},
		async keyUp(e: KeyboardEvent) {
			if (redirectKeyboardEventToBackend(e)) {
				e.preventDefault();
				const { on_key_up } = await wasm;
				on_key_up(e.key);
			}
		},
		async selectTool(toolName: string) {
			const { select_tool } = await wasm;
			select_tool(toolName);
		},
		async viewModeChanged(toolIndex: number) {
			console.log(toolIndex);
			function todo(_: number) {
				return _;
			}
			todo(toolIndex);
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

		window.addEventListener("keyup", (e: KeyboardEvent) => this.keyUp(e));
		window.addEventListener("keydown", (e: KeyboardEvent) => this.keyDown(e));

		this.$watch("viewModeIndex", this.viewModeChanged);
	},
	data() {
		return {
			viewportSvg: "",
			activeTool: "Select",
			MenuDirection,
			SeparatorDirection,
			SeparatorType,
			modeMenuEntries,
			viewModeIndex: 0,
		};
	},
	components: {
		LayoutRow,
		LayoutCol,
		WorkingColors,
		ShelfItem,
		Separator,
		IconButton,
		PopoverButton,
		RadioInput,
		NumberInput,
		DropdownInput,
	},
});
</script>
