<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar'">
			<div class="left side">
				<span class="label">Select</span>

				<ItemDivider />

				<IconButton :size="24" title="Horizontal Align Left">
					<AlignHorizontalLeft />
				</IconButton>
				<IconButton :size="24" title="Horizontal Align Center">
					<AlignHorizontalCenter />
				</IconButton>
				<IconButton :size="24" gapAfter title="Horizontal Align Right">
					<AlignHorizontalRight />
				</IconButton>
				<IconButton :size="24" title="Vertical Align Top">
					<AlignVerticalTop />
				</IconButton>
				<IconButton :size="24" title="Vertical Align Center">
					<AlignVerticalCenter />
				</IconButton>
				<IconButton :size="24" title="Vertical Align Bottom">
					<AlignVerticalBottom />
				</IconButton>
				<DropdownButton />

				<ItemDivider />

				<IconButton :size="24" title="Flip Horizontal">
					<FlipHorizontal />
				</IconButton>
				<IconButton :size="24" title="Flip Vertical">
					<FlipVertical />
				</IconButton>
				<DropdownButton />

				<ItemDivider />

				<IconButton :size="24" title="Boolean Union">
					<BooleanUnion />
				</IconButton>
				<IconButton :size="24" title="Boolean Subtract Front">
					<BooleanSubtractFront />
				</IconButton>
				<IconButton :size="24" title="Boolean Subtract Back">
					<BooleanSubtractBack />
				</IconButton>
				<IconButton :size="24" title="Boolean Intersect">
					<BooleanIntersect />
				</IconButton>
				<IconButton :size="24" title="Boolean Difference">
					<BooleanDifference />
				</IconButton>
				<DropdownButton />
			</div>
			<div class="spacer"></div>
			<div class="right side"></div>
		</LayoutRow>
		<LayoutRow :class="'shelf-and-viewport'">
			<LayoutCol :class="'shelf'">
				<div class="tools">
					<ShelfItem active title="Select Tool (V)">
						<SelectTool />
					</ShelfItem>
					<ShelfItem title="Crop Tool">
						<CropTool />
					</ShelfItem>
					<ShelfItem title="Navigate Tool">
						<NavigateTool />
					</ShelfItem>
					<ShelfItem title="Sample Tool">
						<SampleTool />
					</ShelfItem>
					<ItemDivider horizontal />
					<ShelfItem title="Text Tool">
						<TextTool />
					</ShelfItem>
					<ShelfItem title="Fill Tool">
						<FillTool />
					</ShelfItem>
					<ShelfItem title="Gradient Tool">
						<GradientTool />
					</ShelfItem>
					<ItemDivider horizontal />
					<ShelfItem title="Brush Tool">
						<BrushTool />
					</ShelfItem>
					<ShelfItem title="Heal Tool">
						<HealTool />
					</ShelfItem>
					<ShelfItem title="Clone Tool">
						<CloneTool />
					</ShelfItem>
					<ShelfItem title="Patch Tool">
						<PatchTool />
					</ShelfItem>
					<ShelfItem title="Blur/Sharpen Tool">
						<BlurSharpenTool />
					</ShelfItem>
					<ShelfItem title="Relight Tool">
						<RelightTool />
					</ShelfItem>
					<ItemDivider horizontal />
					<ShelfItem title="Path Tool">
						<PathTool />
					</ShelfItem>
					<ShelfItem title="Pen Tool">
						<PenTool />
					</ShelfItem>
					<ShelfItem title="Freehand Tool">
						<FreehandTool />
					</ShelfItem>
					<ShelfItem title="Spline Tool">
						<SplineTool />
					</ShelfItem>
					<ShelfItem title="Line Tool">
						<LineTool />
					</ShelfItem>
					<ShelfItem title="Rectangle Tool (M)">
						<RectangleTool />
					</ShelfItem>
					<ShelfItem title="Ellipse Tool (E)">
						<EllipseTool />
					</ShelfItem>
					<ShelfItem title="Shape Tool">
						<ShapeTool />
					</ShelfItem>
				</div>
				<div class="spacer"></div>
				<div class="working-colors">
					<div class="swatch-pair">
						<button
							class="secondary swatch"
							style="background: white;"
						></button>
						<button
							class="primary swatch"
							style="background: black;"
						></button>
					</div>
					<div class="swap-and-reset">
						<IconButton :size="16">
							<SwapButton />
						</IconButton>
						<IconButton :size="16">
							<ResetColorsButton />
						</IconButton>
					</div>
				</div>
			</LayoutCol>
			<LayoutCol :class="'viewport'">
				<div
					class="canvas"
					@mousedown="canvasMouseDown"
					@mouseup="canvasMouseUp"
					@mousemove="canvasMouseMove"
				>
					<svg></svg>
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
			flex: 0 1 auto;
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

			.swatch-pair {
				display: flex;
				// Reversed order of elements paired with `column-reverse` allows primary to overlap secondary without relying on `z-index`
				flex-direction: column-reverse;
			}

			.working-colors {
				.swatch {
					width: 24px;
					height: 24px;
					border-radius: 50%;
					border: 2px #888 solid;
					box-shadow: 0 0 0 2px #333;
					margin: 2px;
					padding: 0;
					box-sizing: unset;
					outline: none;
				}

				.primary.swatch {
					margin-bottom: -8px;
				}
			}

			.swap-and-reset {
				font-size: 0;
			}
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
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import ShelfItem from "../widgets/ShelfItem.vue";
import ItemDivider from "../widgets/ItemDivider.vue";
import IconButton from "../widgets/IconButton.vue";
import DropdownButton from "../widgets/DropdownButton.vue";
import SwapButton from "../../../assets/svg/16x16-bounds-12x12-icon/swap.svg";
import ResetColorsButton from "../../../assets/svg/16x16-bounds-12x12-icon/reset-colors.svg";
import SelectTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-select.svg";
import CropTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-crop.svg";
import NavigateTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-navigate.svg";
import SampleTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-layout-sample.svg";
import TextTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-parametric-text.svg";
import FillTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-parametric-fill.svg";
import GradientTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-parametric-gradient.svg";
import BrushTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-brush.svg";
import HealTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-heal.svg";
import CloneTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-clone.svg";
import PatchTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-patch.svg";
import BlurSharpenTool from "../../../assets/svg/24x24-bounds-24x24-icon/document-tool-raster-blur-sharpen.svg";
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

const wasm = import("../../../wasm/pkg");

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		ShelfItem,
		ItemDivider,
		IconButton,
		DropdownButton,
		SwapButton,
		ResetColorsButton,
		SelectTool,
		CropTool,
		NavigateTool,
		SampleTool,
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
	},
	mounted() {
		window.addEventListener("keyup", (e: KeyboardEvent) => this.keyUp(e));
		window.addEventListener("keydown", (e: KeyboardEvent) => this.keyDown(e));
	},
});
</script>
