<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar'">
			<div class="left side">
				<span class="label">Select</span>

				<ItemDivider />

				<IconButton :size="24" title="Horizontal Align Left"><AlignHorizontalLeft /></IconButton>
				<IconButton :size="24" title="Horizontal Align Center"><AlignHorizontalCenter /></IconButton>
				<IconButton :size="24" gapAfter title="Horizontal Align Right"><AlignHorizontalRight /></IconButton>
				<IconButton :size="24" title="Vertical Align Top"><AlignVerticalTop /></IconButton>
				<IconButton :size="24" title="Vertical Align Center"><AlignVerticalCenter /></IconButton>
				<IconButton :size="24" title="Vertical Align Bottom"><AlignVerticalBottom /></IconButton>
				<DropdownButton />

				<ItemDivider />

				<IconButton :size="24" title="Flip Horizontal"><FlipHorizontal /></IconButton>
				<IconButton :size="24" title="Flip Vertical"><FlipVertical /></IconButton>
				<DropdownButton />

				<ItemDivider />

				<IconButton :size="24" title="Boolean Union"><BooleanUnion /></IconButton>
				<IconButton :size="24" title="Boolean Subtract Front"><BooleanSubtractFront /></IconButton>
				<IconButton :size="24" title="Boolean Subtract Back"><BooleanSubtractBack /></IconButton>
				<IconButton :size="24" title="Boolean Intersect"><BooleanIntersect /></IconButton>
				<IconButton :size="24" title="Boolean Difference"><BooleanDifference /></IconButton>
				<DropdownButton />
			</div>
			<div class="spacer"></div>
			<div class="right side">
				<RadioPicker :initialIndex="0" @changed="viewModeChanged">
					<IconButton :size="24" title="View Mode: Normal"><ViewModeNormal /></IconButton>
					<IconButton :size="24" title="View Mode: Outline"><ViewModeOutline /></IconButton>
					<IconButton :size="24" title="View Mode: Pixels"><ViewModePixels /></IconButton>
					<DropdownButton />
				</RadioPicker>

				<ItemDivider />

				<IconButton :size="24" title="Zoom In"><ZoomIn /></IconButton>
				<IconButton :size="24" title="Zoom Out"><ZoomOut /></IconButton>
				<IconButton :size="24" title="Zoom to 100%"><ZoomReset /></IconButton>
				<NumberInput />
			</div>
		</LayoutRow>
		<LayoutRow :class="'shelf-and-viewport'">
			<LayoutCol :class="'shelf'">
				<div class="tools">
					<ShelfItem title="Select Tool (V)" :active="activeTool === 'Select'" @click="selectTool('Select')"><SelectTool /></ShelfItem>
					<ShelfItem title="Crop Tool" :active="activeTool === 'Crop'" @click="'tool not implemented' || selectTool('Crop')"><CropTool /></ShelfItem>
					<ShelfItem title="Navigate Tool" :active="activeTool === 'Navigate'" @click="'tool not implemented' || selectTool('Navigate')"><NavigateTool /></ShelfItem>
					<ShelfItem title="Eyedropper Tool" :active="activeTool === 'Eyedropper'" @click="'tool not implemented' || selectTool('Eyedropper')"><EyedropperTool /></ShelfItem>

					<ItemDivider horizontal />

					<ShelfItem title="Text Tool" :active="activeTool === 'Text'" @click="'tool not implemented' || selectTool('Text')"><TextTool /></ShelfItem>
					<ShelfItem title="Fill Tool" :active="activeTool === 'Fill'" @click="'tool not implemented' || selectTool('Fill')"><FillTool /></ShelfItem>
					<ShelfItem title="Gradient Tool" :active="activeTool === 'Gradient'" @click="'tool not implemented' || selectTool('Gradient')"><GradientTool /></ShelfItem>

					<ItemDivider horizontal />

					<ShelfItem title="Brush Tool" :active="activeTool === 'Brush'" @click="'tool not implemented' || selectTool('Brush')"><BrushTool /></ShelfItem>
					<ShelfItem title="Heal Tool" :active="activeTool === 'Heal'" @click="'tool not implemented' || selectTool('Heal')"><HealTool /></ShelfItem>
					<ShelfItem title="Clone Tool" :active="activeTool === 'Clone'" @click="'tool not implemented' || selectTool('Clone')"><CloneTool /></ShelfItem>
					<ShelfItem title="Patch Tool" :active="activeTool === 'Patch'" @click="'tool not implemented' || selectTool('Patch')"><PatchTool /></ShelfItem>
					<ShelfItem title="Detail Tool" :active="activeTool === 'BlurSharpen'" @click="'tool not implemented' || selectTool('BlurSharpen')"><BlurSharpenTool /></ShelfItem>
					<ShelfItem title="Relight Tool" :active="activeTool === 'Relight'" @click="'tool not implemented' || selectTool('Relight')"><RelightTool /></ShelfItem>

					<ItemDivider horizontal />

					<ShelfItem title="Path Tool" :active="activeTool === 'Path'" @click="'tool not implemented' || selectTool('Path')"><PathTool /></ShelfItem>
					<ShelfItem title="Pen Tool (P)" :active="activeTool === 'Pen'" @click="selectTool('Pen')"><PenTool /></ShelfItem>
					<ShelfItem title="Freehand Tool" :active="activeTool === 'Freehand'" @click="'tool not implemented' || selectTool('Freehand')"><FreehandTool /></ShelfItem>
					<ShelfItem title="Spline Tool" :active="activeTool === 'Spline'" @click="'tool not implemented' || selectTool('Spline')"><SplineTool /></ShelfItem>
					<ShelfItem title="Line Tool (L)" :active="activeTool === 'Line'" @click="selectTool('Line')"><LineTool /></ShelfItem>
					<ShelfItem title="Rectangle Tool (M)" :active="activeTool === 'Rectangle'" @click="selectTool('Rectangle')"><RectangleTool /></ShelfItem>
					<ShelfItem title="Ellipse Tool (E)" :active="activeTool === 'Ellipse'" @click="selectTool('Ellipse')"><EllipseTool /></ShelfItem>
					<ShelfItem title="Shape Tool (Y)" :active="activeTool === 'Shape'" @click="selectTool('Shape')"><ShapeTool /></ShelfItem>
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
		flex: 0 0 32px;

		.side {
			height: 100%;
			flex: 0 0 auto;
			display: flex;
			align-items: center;
			margin: 0 8px;

			.label {
				white-space: nowrap;
				font-weight: bold;
			}
		}
	}

	.shelf-and-viewport {
		.shelf {
			flex: 0 0 32px;
		}

		.viewport {
			flex: 1 1 100%;

			.canvas {
				background: #111;
				width: 100%;
				height: 100%;

				svg {
					width: 100%;
					height: 100%;
				}
			}
		}
	}
}

// The `where` pseduo-class does not contribtue to specificity
:where(.document .options-bar .side > :not(:first-child)) {
	margin-left: 8px;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import { ResponseType, registerResponseHandler, Response, UpdateCanvas, SetActiveTool } from "../../response-handler";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import WorkingColors from "../widgets/WorkingColors.vue";
import ShelfItem from "../widgets/ShelfItem.vue";
import ItemDivider from "../widgets/ItemDivider.vue";
import IconButton from "../widgets/IconButton.vue";
import DropdownButton from "../widgets/DropdownButton.vue";
import RadioPicker from "../widgets/RadioPicker.vue";
import NumberInput from "../widgets/NumberInput.vue";
import SelectTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-select.svg";
import CropTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-crop.svg";
import NavigateTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-navigate.svg";
import EyedropperTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-eyedropper.svg";
import TextTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-parametric-text.svg";
import FillTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-parametric-fill.svg";
import GradientTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-parametric-gradient.svg";
import BrushTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-brush.svg";
import HealTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-heal.svg";
import CloneTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-clone.svg";
import PatchTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-patch.svg";
import BlurSharpenTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-detail.svg";
import RelightTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-relight.svg";
import PathTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-path.svg";
import PenTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-pen.svg";
import FreehandTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-freehand.svg";
import SplineTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-spline.svg";
import LineTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-line.svg";
import RectangleTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-rectangle.svg";
import EllipseTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-ellipse.svg";
import ShapeTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-vector-shape.svg";
import AlignHorizontalLeft from "../../../assets/svg/24x24-bounds-16x16-icon/align-horizontal-left.svg";
import AlignHorizontalCenter from "../../../assets/svg/24x24-bounds-16x16-icon/align-horizontal-center.svg";
import AlignHorizontalRight from "../../../assets/svg/24x24-bounds-16x16-icon/align-horizontal-right.svg";
import AlignVerticalTop from "../../../assets/svg/24x24-bounds-16x16-icon/align-vertical-top.svg";
import AlignVerticalCenter from "../../../assets/svg/24x24-bounds-16x16-icon/align-vertical-center.svg";
import AlignVerticalBottom from "../../../assets/svg/24x24-bounds-16x16-icon/align-vertical-bottom.svg";
import FlipHorizontal from "../../../assets/svg/24x24-bounds-16x16-icon/flip-horizontal.svg";
import FlipVertical from "../../../assets/svg/24x24-bounds-16x16-icon/flip-vertical.svg";
import BooleanUnion from "../../../assets/svg/24x24-bounds-16x16-icon/boolean-union.svg";
import BooleanSubtractFront from "../../../assets/svg/24x24-bounds-16x16-icon/boolean-subtract-front.svg";
import BooleanSubtractBack from "../../../assets/svg/24x24-bounds-16x16-icon/boolean-subtract-back.svg";
import BooleanIntersect from "../../../assets/svg/24x24-bounds-16x16-icon/boolean-intersect.svg";
import BooleanDifference from "../../../assets/svg/24x24-bounds-16x16-icon/boolean-difference.svg";
import ZoomReset from "../../../assets/svg/24x24-bounds-16x16-icon/zoom-reset.svg";
import ZoomIn from "../../../assets/svg/24x24-bounds-16x16-icon/zoom-in.svg";
import ZoomOut from "../../../assets/svg/24x24-bounds-16x16-icon/zoom-out.svg";
import ViewModeNormal from "../../../assets/svg/24x24-bounds-16x16-icon/view-mode-normal.svg";
import ViewModeOutline from "../../../assets/svg/24x24-bounds-16x16-icon/view-mode-outline.svg";
import ViewModePixels from "../../../assets/svg/24x24-bounds-16x16-icon/view-mode-pixels.svg";

const wasm = import("../../../wasm/pkg");

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		WorkingColors,
		ShelfItem,
		ItemDivider,
		IconButton,
		DropdownButton,
		RadioPicker,
		NumberInput,
		SelectTool,
		CropTool,
		NavigateTool,
		EyedropperTool,
		TextTool,
		FillTool,
		GradientTool,
		BrushTool,
		HealTool,
		CloneTool,
		PatchTool,
		BlurSharpenTool,
		RelightTool,
		PathTool,
		PenTool,
		FreehandTool,
		SplineTool,
		LineTool,
		RectangleTool,
		EllipseTool,
		ShapeTool,
		AlignHorizontalLeft,
		AlignHorizontalCenter,
		AlignHorizontalRight,
		AlignVerticalTop,
		AlignVerticalCenter,
		AlignVerticalBottom,
		FlipHorizontal,
		FlipVertical,
		BooleanUnion,
		BooleanSubtractFront,
		BooleanSubtractBack,
		BooleanIntersect,
		BooleanDifference,
		ZoomReset,
		ZoomIn,
		ZoomOut,
		ViewModeNormal,
		ViewModeOutline,
		ViewModePixels,
	},
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
			const { on_key_down } = await wasm;
			on_key_down(e.key);
		},
		async keyUp(e: KeyboardEvent) {
			const { on_key_up } = await wasm;
			on_key_up(e.key);
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
		async updatePrimaryColor(c: { r: number; g: number; b: number; a: number }) {
			const { update_primary_color, Color } = await wasm;
			update_primary_color(new Color(c.r, c.g, c.b, c.a));
		},
	},
	mounted() {
		registerResponseHandler(ResponseType.UpdateCanvas, (responseData: Response) => {
			const updateData = responseData as UpdateCanvas;
			if (updateData) this.viewportSvg = updateData.document;
		});
		registerResponseHandler(ResponseType.SetActiveTool, (responseData: Response) => {
			const toolData = responseData as SetActiveTool;
			if (toolData) this.activeTool = toolData.tool_name;
		});

		window.addEventListener("keyup", (e: KeyboardEvent) => this.keyUp(e));
		window.addEventListener("keydown", (e: KeyboardEvent) => this.keyDown(e));

		// TODO: Implement an actual UI for chosing colors (this is completely temporary)
		this.updatePrimaryColor({ r: 247 / 255, g: 76 / 255, b: 0 / 255, a: 0.6 });
	},
	data() {
		return {
			viewportSvg: "",
			activeTool: "Select",
		};
	},
});
</script>
