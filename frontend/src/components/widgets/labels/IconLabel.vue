<template>
	<div class="icon-label" :class="`size-${String(icons[icon].size)}`">
		<component :is="icon" />
	</div>
</template>

<style lang="scss">
.icon-label {
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

import LayoutSelectTool from "@/../assets/24px-two-tone/layout-select-tool.svg";
import LayoutCropTool from "@/../assets/24px-two-tone/layout-crop-tool.svg";
import LayoutNavigateTool from "@/../assets/24px-two-tone/layout-navigate-tool.svg";
import LayoutEyedropperTool from "@/../assets/24px-two-tone/layout-eyedropper-tool.svg";
import ParametricTextTool from "@/../assets/24px-two-tone/parametric-text-tool.svg";
import ParametricFillTool from "@/../assets/24px-two-tone/parametric-fill-tool.svg";
import ParametricGradientTool from "@/../assets/24px-two-tone/parametric-gradient-tool.svg";
import RasterBrushTool from "@/../assets/24px-two-tone/raster-brush-tool.svg";
import RasterHealTool from "@/../assets/24px-two-tone/raster-heal-tool.svg";
import RasterCloneTool from "@/../assets/24px-two-tone/raster-clone-tool.svg";
import RasterPatchTool from "@/../assets/24px-two-tone/raster-patch-tool.svg";
import RasterBlurSharpenTool from "@/../assets/24px-two-tone/raster-detail-tool.svg";
import RasterRelightTool from "@/../assets/24px-two-tone/raster-relight-tool.svg";
import VectorPathTool from "@/../assets/24px-two-tone/vector-path-tool.svg";
import VectorPenTool from "@/../assets/24px-two-tone/vector-pen-tool.svg";
import VectorFreehandTool from "@/../assets/24px-two-tone/vector-freehand-tool.svg";
import VectorSplineTool from "@/../assets/24px-two-tone/vector-spline-tool.svg";
import VectorLineTool from "@/../assets/24px-two-tone/vector-line-tool.svg";
import VectorRectangleTool from "@/../assets/24px-two-tone/vector-rectangle-tool.svg";
import VectorEllipseTool from "@/../assets/24px-two-tone/vector-ellipse-tool.svg";
import VectorShapeTool from "@/../assets/24px-two-tone/vector-shape-tool.svg";

import AlignLeft from "@/../assets/16px-solid/align-left.svg";
import AlignHorizontalCenter from "@/../assets/16px-solid/align-horizontal-center.svg";
import AlignRight from "@/../assets/16px-solid/align-right.svg";
import AlignTop from "@/../assets/16px-solid/align-top.svg";
import AlignVerticalCenter from "@/../assets/16px-solid/align-vertical-center.svg";
import AlignBottom from "@/../assets/16px-solid/align-bottom.svg";
import FlipHorizontal from "@/../assets/16px-solid/flip-horizontal.svg";
import FlipVertical from "@/../assets/16px-solid/flip-vertical.svg";
import BooleanUnion from "@/../assets/16px-solid/boolean-union.svg";
import BooleanSubtractFront from "@/../assets/16px-solid/boolean-subtract-front.svg";
import BooleanSubtractBack from "@/../assets/16px-solid/boolean-subtract-back.svg";
import BooleanIntersect from "@/../assets/16px-solid/boolean-intersect.svg";
import BooleanDifference from "@/../assets/16px-solid/boolean-difference.svg";
import ZoomReset from "@/../assets/16px-solid/zoom-reset.svg";
import ZoomIn from "@/../assets/16px-solid/zoom-in.svg";
import ZoomOut from "@/../assets/16px-solid/zoom-out.svg";
import ViewModeNormal from "@/../assets/16px-solid/view-mode-normal.svg";
import ViewModeOutline from "@/../assets/16px-solid/view-mode-outline.svg";
import ViewModePixels from "@/../assets/16px-solid/view-mode-pixels.svg";
import EyeVisible from "@/../assets/16px-solid/eye-visible.svg";
import EyeHidden from "@/../assets/16px-solid/eye-hidden.svg";
import GraphiteLogo from "@/../assets/16px-solid/graphite-logo.svg";
import File from "@/../assets/16px-solid/file.svg";
import Copy from "@/../assets/16px-solid/copy.svg";
import Paste from "@/../assets/16px-solid/paste.svg";
import ViewportDesignMode from "@/../assets/16px-solid/viewport-design-mode.svg";
import ViewportSelectMode from "@/../assets/16px-solid/viewport-select-mode.svg";
import ViewportGuideMode from "@/../assets/16px-solid/viewport-guide-mode.svg";

import Checkmark from "@/../assets/12px-solid/checkmark.svg";
import Link from "@/../assets/12px-solid/link.svg";
import Grid from "@/../assets/12px-solid/grid.svg";
import Overlays from "@/../assets/12px-solid/overlays.svg";
import Snapping from "@/../assets/12px-solid/snapping.svg";
import Info from "@/../assets/12px-solid/info.svg";
import Warning from "@/../assets/12px-solid/warning.svg";
import Swap from "@/../assets/12px-solid/swap.svg";
import ResetColors from "@/../assets/12px-solid/reset-colors.svg";
import DropdownArrow from "@/../assets/12px-solid/dropdown-arrow.svg";
import VerticalEllipsis from "@/../assets/12px-solid/vertical-ellipsis.svg";
import CloseX from "@/../assets/12px-solid/close-x.svg";
import FullscreenEnter from "@/../assets/12px-solid/fullscreen-enter.svg";
import FullscreenExit from "@/../assets/12px-solid/fullscreen-exit.svg";
import WindowButtonWinMinimize from "@/../assets/12px-solid/window-button-win-minimize.svg";
import WindowButtonWinMaximize from "@/../assets/12px-solid/window-button-win-maximize.svg";
import WindowButtonWinRestoreDown from "@/../assets/12px-solid/window-button-win-restore-down.svg";
import WindowButtonWinClose from "@/../assets/12px-solid/window-button-win-close.svg";

import MouseHintNone from "@/../assets/16px-two-tone/mouse-hint-none.svg";
import MouseHintLMB from "@/../assets/16px-two-tone/mouse-hint-lmb.svg";
import MouseHintRMB from "@/../assets/16px-two-tone/mouse-hint-rmb.svg";
import MouseHintMMB from "@/../assets/16px-two-tone/mouse-hint-mmb.svg";
import MouseHintScrollUp from "@/../assets/16px-two-tone/mouse-hint-scroll-up.svg";
import MouseHintScrollDown from "@/../assets/16px-two-tone/mouse-hint-scroll-down.svg";
import MouseHintDrag from "@/../assets/16px-two-tone/mouse-hint-drag.svg";
import MouseHintLMBDrag from "@/../assets/16px-two-tone/mouse-hint-lmb-drag.svg";
import MouseHintRMBDrag from "@/../assets/16px-two-tone/mouse-hint-rmb-drag.svg";
import MouseHintMMBDrag from "@/../assets/16px-two-tone/mouse-hint-mmb-drag.svg";

import NodeTypePath from "@/../assets/24px-full-color/node-type-path.svg";

const icons = {
	LayoutSelectTool: { component: LayoutSelectTool, size: 24 },
	LayoutCropTool: { component: LayoutCropTool, size: 24 },
	LayoutNavigateTool: { component: LayoutNavigateTool, size: 24 },
	LayoutEyedropperTool: { component: LayoutEyedropperTool, size: 24 },
	ParametricTextTool: { component: ParametricTextTool, size: 24 },
	ParametricFillTool: { component: ParametricFillTool, size: 24 },
	ParametricGradientTool: { component: ParametricGradientTool, size: 24 },
	RasterBrushTool: { component: RasterBrushTool, size: 24 },
	RasterHealTool: { component: RasterHealTool, size: 24 },
	RasterCloneTool: { component: RasterCloneTool, size: 24 },
	RasterPatchTool: { component: RasterPatchTool, size: 24 },
	RasterBlurSharpenTool: { component: RasterBlurSharpenTool, size: 24 },
	RasterRelightTool: { component: RasterRelightTool, size: 24 },
	VectorPathTool: { component: VectorPathTool, size: 24 },
	VectorPenTool: { component: VectorPenTool, size: 24 },
	VectorFreehandTool: { component: VectorFreehandTool, size: 24 },
	VectorSplineTool: { component: VectorSplineTool, size: 24 },
	VectorLineTool: { component: VectorLineTool, size: 24 },
	VectorRectangleTool: { component: VectorRectangleTool, size: 24 },
	VectorEllipseTool: { component: VectorEllipseTool, size: 24 },
	VectorShapeTool: { component: VectorShapeTool, size: 24 },
	AlignLeft: { component: AlignLeft, size: 16 },
	AlignHorizontalCenter: { component: AlignHorizontalCenter, size: 16 },
	AlignRight: { component: AlignRight, size: 16 },
	AlignTop: { component: AlignTop, size: 16 },
	AlignVerticalCenter: { component: AlignVerticalCenter, size: 16 },
	AlignBottom: { component: AlignBottom, size: 16 },
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
	ViewportGuideMode: { component: ViewportGuideMode, size: 16 },
	Checkmark: { component: Checkmark, size: 12 },
	Link: { component: Link, size: 12 },
	Grid: { component: Grid, size: 12 },
	Overlays: { component: Overlays, size: 12 },
	Snapping: { component: Snapping, size: 12 },
	Info: { component: Info, size: 12 },
	Warning: { component: Warning, size: 12 },
	Swap: { component: Swap, size: 12 },
	ResetColors: { component: ResetColors, size: 12 },
	DropdownArrow: { component: DropdownArrow, size: 12 },
	VerticalEllipsis: { component: VerticalEllipsis, size: 12 },
	CloseX: { component: CloseX, size: 12 },
	FullscreenEnter: { component: FullscreenEnter, size: 12 },
	FullscreenExit: { component: FullscreenExit, size: 12 },
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
