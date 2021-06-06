<template>
	<div class="icon" :class="`size-${String(icons[icon].size)}`">
		<component :is="icon" />
	</div>
</template>

<style lang="scss">
.icon {
	display: block;
	flex: 0 0 auto;
	fill: var(--color-e-nearwhite);

	&.size-12 {
		width: 12px;
		height: 12px;
	}

	&.size-16 {
		width: 16px;
		height: 16px;
	}

	&.size-24 {
		width: 24px;
		height: 24px;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import SelectTool from "../../../../assets/24px-two-tone/document-tool-layout-select.svg";
import CropTool from "../../../../assets/24px-two-tone/document-tool-layout-crop.svg";
import NavigateTool from "../../../../assets/24px-two-tone/document-tool-layout-navigate.svg";
import EyedropperTool from "../../../../assets/24px-two-tone/document-tool-layout-eyedropper.svg";
import TextTool from "../../../../assets/24px-two-tone/document-tool-parametric-text.svg";
import FillTool from "../../../../assets/24px-two-tone/document-tool-parametric-fill.svg";
import GradientTool from "../../../../assets/24px-two-tone/document-tool-parametric-gradient.svg";
import BrushTool from "../../../../assets/24px-two-tone/document-tool-raster-brush.svg";
import HealTool from "../../../../assets/24px-two-tone/document-tool-raster-heal.svg";
import CloneTool from "../../../../assets/24px-two-tone/document-tool-raster-clone.svg";
import PatchTool from "../../../../assets/24px-two-tone/document-tool-raster-patch.svg";
import BlurSharpenTool from "../../../../assets/24px-two-tone/document-tool-raster-detail.svg";
import RelightTool from "../../../../assets/24px-two-tone/document-tool-raster-relight.svg";
import PathTool from "../../../../assets/24px-two-tone/document-tool-vector-path.svg";
import PenTool from "../../../../assets/24px-two-tone/document-tool-vector-pen.svg";
import FreehandTool from "../../../../assets/24px-two-tone/document-tool-vector-freehand.svg";
import SplineTool from "../../../../assets/24px-two-tone/document-tool-vector-spline.svg";
import LineTool from "../../../../assets/24px-two-tone/document-tool-vector-line.svg";
import RectangleTool from "../../../../assets/24px-two-tone/document-tool-vector-rectangle.svg";
import EllipseTool from "../../../../assets/24px-two-tone/document-tool-vector-ellipse.svg";
import ShapeTool from "../../../../assets/24px-two-tone/document-tool-vector-shape.svg";

import AlignHorizontalLeft from "../../../../assets/16px-solid/align-horizontal-left.svg";
import AlignHorizontalCenter from "../../../../assets/16px-solid/align-horizontal-center.svg";
import AlignHorizontalRight from "../../../../assets/16px-solid/align-horizontal-right.svg";
import AlignVerticalTop from "../../../../assets/16px-solid/align-vertical-top.svg";
import AlignVerticalCenter from "../../../../assets/16px-solid/align-vertical-center.svg";
import AlignVerticalBottom from "../../../../assets/16px-solid/align-vertical-bottom.svg";
import FlipHorizontal from "../../../../assets/16px-solid/flip-horizontal.svg";
import FlipVertical from "../../../../assets/16px-solid/flip-vertical.svg";
import BooleanUnion from "../../../../assets/16px-solid/boolean-union.svg";
import BooleanSubtractFront from "../../../../assets/16px-solid/boolean-subtract-front.svg";
import BooleanSubtractBack from "../../../../assets/16px-solid/boolean-subtract-back.svg";
import BooleanIntersect from "../../../../assets/16px-solid/boolean-intersect.svg";
import BooleanDifference from "../../../../assets/16px-solid/boolean-difference.svg";
import ZoomReset from "../../../../assets/16px-solid/zoom-reset.svg";
import ZoomIn from "../../../../assets/16px-solid/zoom-in.svg";
import ZoomOut from "../../../../assets/16px-solid/zoom-out.svg";
import ViewModeNormal from "../../../../assets/16px-solid/view-mode-normal.svg";
import ViewModeOutline from "../../../../assets/16px-solid/view-mode-outline.svg";
import ViewModePixels from "../../../../assets/16px-solid/view-mode-pixels.svg";
import EyeVisible from "../../../../assets/16px-solid/visibility-eye-visible.svg";
import EyeHidden from "../../../../assets/16px-solid/visibility-eye-hidden.svg";
import GraphiteLogo from "../../../../assets/16px-solid/graphite-logo.svg";
import File from "../../../../assets/16px-solid/file.svg";
import Copy from "../../../../assets/16px-solid/copy.svg";
import Paste from "../../../../assets/16px-solid/paste.svg";
import ViewportDesignMode from "../../../../assets/16px-solid/viewport-design-mode.svg";
import ViewportSelectMode from "../../../../assets/16px-solid/viewport-select-mode.svg";

import SwapButton from "../../../../assets/12px-solid/swap.svg";
import ResetColorsButton from "../../../../assets/12px-solid/reset-colors.svg";
import DropdownArrow from "../../../../assets/12px-solid/dropdown-arrow.svg";
import VerticalEllipsis from "../../../../assets/12px-solid/vertical-ellipsis.svg";
import CloseX from "../../../../assets/12px-solid/close-x.svg";
import WindowButtonWinMinimize from "../../../../assets/12px-solid/window-button-win-minimize.svg";
import WindowButtonWinMaximize from "../../../../assets/12px-solid/window-button-win-maximize.svg";
import WindowButtonWinRestoreDown from "../../../../assets/12px-solid/window-button-win-restore-down.svg";
import WindowButtonWinClose from "../../../../assets/12px-solid/window-button-win-close.svg";

import MouseHintNone from "../../../../assets/16px-two-tone/mouse-hint-none.svg";
import MouseHintLMB from "../../../../assets/16px-two-tone/mouse-hint-lmb.svg";
import MouseHintRMB from "../../../../assets/16px-two-tone/mouse-hint-rmb.svg";
import MouseHintMMB from "../../../../assets/16px-two-tone/mouse-hint-mmb.svg";
import MouseHintScrollUp from "../../../../assets/16px-two-tone/mouse-hint-scroll-up.svg";
import MouseHintScrollDown from "../../../../assets/16px-two-tone/mouse-hint-scroll-down.svg";
import MouseHintDrag from "../../../../assets/16px-two-tone/mouse-hint-drag.svg";
import MouseHintLMBDrag from "../../../../assets/16px-two-tone/mouse-hint-lmb-drag.svg";
import MouseHintRMBDrag from "../../../../assets/16px-two-tone/mouse-hint-rmb-drag.svg";
import MouseHintMMBDrag from "../../../../assets/16px-two-tone/mouse-hint-mmb-drag.svg";

import NodeTypePath from "../../../../assets/24px-full-color/node-type-path.svg";

const icons = {
	SelectTool: { component: SelectTool, size: 24 },
	CropTool: { component: CropTool, size: 24 },
	NavigateTool: { component: NavigateTool, size: 24 },
	EyedropperTool: { component: EyedropperTool, size: 24 },
	TextTool: { component: TextTool, size: 24 },
	FillTool: { component: FillTool, size: 24 },
	GradientTool: { component: GradientTool, size: 24 },
	BrushTool: { component: BrushTool, size: 24 },
	HealTool: { component: HealTool, size: 24 },
	CloneTool: { component: CloneTool, size: 24 },
	PatchTool: { component: PatchTool, size: 24 },
	BlurSharpenTool: { component: BlurSharpenTool, size: 24 },
	RelightTool: { component: RelightTool, size: 24 },
	PathTool: { component: PathTool, size: 24 },
	PenTool: { component: PenTool, size: 24 },
	FreehandTool: { component: FreehandTool, size: 24 },
	SplineTool: { component: SplineTool, size: 24 },
	LineTool: { component: LineTool, size: 24 },
	RectangleTool: { component: RectangleTool, size: 24 },
	EllipseTool: { component: EllipseTool, size: 24 },
	ShapeTool: { component: ShapeTool, size: 24 },
	AlignHorizontalLeft: { component: AlignHorizontalLeft, size: 16 },
	AlignHorizontalCenter: { component: AlignHorizontalCenter, size: 16 },
	AlignHorizontalRight: { component: AlignHorizontalRight, size: 16 },
	AlignVerticalTop: { component: AlignVerticalTop, size: 16 },
	AlignVerticalCenter: { component: AlignVerticalCenter, size: 16 },
	AlignVerticalBottom: { component: AlignVerticalBottom, size: 16 },
	FlipHorizontal: { component: FlipHorizontal, size: 16 },
	FlipVertical: { component: FlipVertical, size: 16 },
	BooleanUnion: { component: BooleanUnion, size: 16 },
	BooleanSubtractFront: { component: BooleanSubtractFront, size: 16 },
	BooleanSubtractBack: { component: BooleanSubtractBack, size: 16 },
	BooleanIntersect: { component: BooleanIntersect, size: 16 },
	BooleanDifference: { component: BooleanDifference, size: 16 },
	ZoomReset: { component: ZoomReset, size: 16 },
	ZoomIn: { component: ZoomIn, size: 16 },
	ZoomOut: { component: ZoomOut, size: 16 },
	ViewModeNormal: { component: ViewModeNormal, size: 16 },
	ViewModeOutline: { component: ViewModeOutline, size: 16 },
	ViewModePixels: { component: ViewModePixels, size: 16 },
	EyeVisible: { component: EyeVisible, size: 16 },
	EyeHidden: { component: EyeHidden, size: 16 },
	GraphiteLogo: { component: GraphiteLogo, size: 16 },
	File: { component: File, size: 16 },
	Copy: { component: Copy, size: 16 },
	Paste: { component: Paste, size: 16 },
	ViewportDesignMode: { component: ViewportDesignMode, size: 16 },
	ViewportSelectMode: { component: ViewportSelectMode, size: 16 },
	SwapButton: { component: SwapButton, size: 12 },
	ResetColorsButton: { component: ResetColorsButton, size: 12 },
	DropdownArrow: { component: DropdownArrow, size: 12 },
	VerticalEllipsis: { component: VerticalEllipsis, size: 12 },
	CloseX: { component: CloseX, size: 12 },
	WindowButtonWinMinimize: { component: WindowButtonWinMinimize, size: 12 },
	WindowButtonWinMaximize: { component: WindowButtonWinMaximize, size: 12 },
	WindowButtonWinRestoreDown: { component: WindowButtonWinRestoreDown, size: 12 },
	WindowButtonWinClose: { component: WindowButtonWinClose, size: 12 },
	MouseHintNone: { component: MouseHintNone, size: 16 },
	MouseHintLMB: { component: MouseHintLMB, size: 16 },
	MouseHintRMB: { component: MouseHintRMB, size: 16 },
	MouseHintMMB: { component: MouseHintMMB, size: 16 },
	MouseHintScrollUp: { component: MouseHintScrollUp, size: 16 },
	MouseHintScrollDown: { component: MouseHintScrollDown, size: 16 },
	MouseHintDrag: { component: MouseHintDrag, size: 16 },
	MouseHintLMBDrag: { component: MouseHintLMBDrag, size: 16 },
	MouseHintRMBDrag: { component: MouseHintRMBDrag, size: 16 },
	MouseHintMMBDrag: { component: MouseHintMMBDrag, size: 16 },
	NodeTypePath: { component: NodeTypePath, size: 24 },
};

const components = Object.fromEntries(Object.entries(icons).map(([name, data]) => [name, data.component]));

export default defineComponent({
	props: {
		icon: { type: String, required: true },
		gapAfter: { type: Boolean, default: false },
	},
	components,
	data() {
		return { icons };
	},
});
</script>
