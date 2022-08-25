<template>
	<LayoutRow class="color-picker">
		<LayoutCol class="saturation-picker" ref="saturationPicker" @pointerdown="(e: PointerEvent) => onPointerDown(e)">
			<div ref="saturationCursor" class="selection-circle"></div>
		</LayoutCol>
		<LayoutCol class="hue-picker" ref="huePicker" @pointerdown="(e: PointerEvent) => onPointerDown(e)">
			<div ref="hueCursor" class="selection-pincers"></div>
		</LayoutCol>
		<LayoutCol class="opacity-picker" ref="opacityPicker" @pointerdown="(e: PointerEvent) => onPointerDown(e)">
			<div ref="opacityCursor" class="selection-pincers"></div>
		</LayoutCol>
	</LayoutRow>
</template>

<style lang="scss">
.color-picker {
	--saturation-picker-hue: #ff0000;
	--opacity-picker-color: #ff0000;

	.saturation-picker {
		width: 256px;
		background-blend-mode: multiply;
		background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--saturation-picker-hue));
		position: relative;
	}

	.saturation-picker,
	.hue-picker,
	.opacity-picker {
		height: 256px;
		position: relative;
		overflow: hidden;
	}

	.hue-picker,
	.opacity-picker {
		width: 24px;
		margin-left: 8px;
		position: relative;
	}

	.hue-picker {
		background-blend-mode: screen;
		background: linear-gradient(to top, #ff0000ff 16.666%, #ff000000 33.333%, #ff000000 66.666%, #ff0000ff 83.333%),
			linear-gradient(to top, #00ff0000 0%, #00ff00ff 16.666%, #00ff00ff 50%, #00ff0000 66.666%), linear-gradient(to top, #0000ff00 33.333%, #0000ffff 50%, #0000ffff 83.333%, #0000ff00 100%);
	}

	.opacity-picker {
		background: linear-gradient(to bottom, var(--opacity-picker-color), transparent);

		&::before {
			content: "";
			width: 100%;
			height: 100%;
			z-index: -1;
			position: relative;
			// Checkered transparent pattern
			background: linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%), linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%),
				linear-gradient(#ffffff, #ffffff);
			background-size: 16px 16px;
			background-position: 0 0, 8px 8px;
		}
	}

	.selection-circle {
		position: absolute;
		left: 0%;
		top: 0%;
		width: 0;
		height: 0;
		pointer-events: none;

		&::after {
			content: "";
			display: block;
			position: relative;
			left: -6px;
			top: -6px;
			width: 12px;
			height: 12px;
			border-radius: 50%;
			border: 2px solid white;
			box-sizing: border-box;
			mix-blend-mode: difference;
		}
	}

	.selection-pincers {
		position: absolute;
		top: 0%;
		width: 100%;
		height: 0;
		pointer-events: none;

		&::before {
			content: "";
			position: absolute;
			top: -4px;
			left: 0;
			border-style: solid;
			border-width: 4px 0 4px 4px;
			border-color: transparent transparent transparent #000000;
		}

		&::after {
			content: "";
			position: absolute;
			top: -4px;
			right: 0;
			border-style: solid;
			border-width: 4px 4px 4px 0;
			border-color: transparent #000000 transparent transparent;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { hsvaToRgba, rgbaToHsva } from "@/utility-functions/color";
import { clamp } from "@/utility-functions/math";
import { type RGBA } from "@/wasm-communication/messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";

type ColorPickerState = "Idle" | "MoveHue" | "MoveOpacity" | "MoveSaturation";

// TODO: Clean up the fundamental code design in this file to simplify it and use better practices.
// TODO: Such as removing the `picker*` data variables and reducing the number of functions which call each other in weird, non-obvious ways.

export default defineComponent({
	emits: ["update:color"],
	props: {
		color: { type: Object as PropType<RGBA>, required: true },
	},
	data() {
		return {
			state: "Idle" as ColorPickerState,
			pickerHSVA: { h: 0, s: 0, v: 0, a: 1 },
			pickerHueRect: { width: 0, height: 0, top: 0, left: 0 },
			pickerOpacityRect: { width: 0, height: 0, top: 0, left: 0 },
			pickerSaturationRect: { width: 0, height: 0, top: 0, left: 0 },
		};
	},
	mounted() {
		this.$watch("color", this.updateColor, { immediate: true });
	},
	unmounted() {
		this.removeEvents();
	},
	methods: {
		addEvents() {
			document.addEventListener("pointermove", this.onPointerMove);
			document.addEventListener("pointerup", this.onPointerUp);
		},
		removeEvents() {
			document.removeEventListener("pointermove", this.onPointerMove);
			document.removeEventListener("pointerup", this.onPointerUp);
		},
		onPointerDown(e: PointerEvent) {
			const saturationPicker = this.$refs.saturationPicker as typeof LayoutCol;
			const saturationPickerElement = saturationPicker?.$el as HTMLElement | undefined;

			const huePicker = this.$refs.huePicker as typeof LayoutCol;
			const huePickerElement = huePicker?.$el as HTMLElement | undefined;

			const opacityPicker = this.$refs.opacityPicker as typeof LayoutCol;
			const opacityPickerElement = opacityPicker?.$el as HTMLElement | undefined;

			if (!(e.currentTarget instanceof HTMLElement) || !saturationPickerElement || !huePickerElement || !opacityPickerElement) return;

			if (saturationPickerElement.contains(e.currentTarget)) {
				this.state = "MoveSaturation";
			} else if (huePickerElement.contains(e.currentTarget)) {
				this.state = "MoveHue";
			} else if (opacityPickerElement.contains(e.currentTarget)) {
				this.state = "MoveOpacity";
			} else {
				this.state = "Idle";
			}

			if (this.state === "Idle") return;

			this.addEvents();
			this.updateRects();
			this.onPointerMove(e);
		},
		onPointerMove(e: PointerEvent) {
			switch (this.state) {
				case "MoveHue":
					this.setHueCursorPosition(e.clientY - this.pickerHueRect.top);
					break;
				case "MoveOpacity":
					this.setOpacityCursorPosition(e.clientY - this.pickerOpacityRect.top);
					break;
				case "MoveSaturation":
					this.setSaturationCursorPosition(e.clientX - this.pickerSaturationRect.left, e.clientY - this.pickerSaturationRect.top);
					break;
				default:
					return;
			}

			this.updateHue();

			// The `color` prop's watcher calls `this.updateColor()`
			this.$emit("update:color", hsvaToRgba(this.pickerHSVA));
		},
		onPointerUp() {
			if (this.state === "Idle") return;

			this.state = "Idle";

			this.removeEvents();
		},
		updateRects() {
			const saturationPicker = this.$refs.saturationPicker as typeof LayoutCol;
			const saturationPickerElement = saturationPicker?.$el as HTMLElement | undefined;

			const huePicker = this.$refs.huePicker as typeof LayoutCol;
			const huePickerElement = huePicker?.$el as HTMLElement | undefined;

			const opacityPicker = this.$refs.opacityPicker as typeof LayoutCol;
			const opacityPickerElement = opacityPicker?.$el as HTMLElement | undefined;

			if (!saturationPickerElement || !huePickerElement || !opacityPickerElement) return;

			// Saturation
			const saturation = saturationPickerElement.getBoundingClientRect();

			this.pickerSaturationRect.width = saturation.width;
			this.pickerSaturationRect.height = saturation.height;
			this.pickerSaturationRect.left = saturation.left;
			this.pickerSaturationRect.top = saturation.top;

			// Hue
			const hue = huePickerElement.getBoundingClientRect();

			this.pickerHueRect.width = hue.width;
			this.pickerHueRect.height = hue.height;
			this.pickerHueRect.left = hue.left;
			this.pickerHueRect.top = hue.top;

			// Opacity
			const opacity = opacityPickerElement.getBoundingClientRect();

			this.pickerOpacityRect.width = opacity.width;
			this.pickerOpacityRect.height = opacity.height;
			this.pickerOpacityRect.left = opacity.left;
			this.pickerOpacityRect.top = opacity.top;
		},
		setSaturationCursorPosition(x: number, y: number) {
			const saturationPositionX = clamp(x, 0, this.pickerSaturationRect.width);
			const saturationPositionY = clamp(y, 0, this.pickerSaturationRect.height);

			const saturationCursor = this.$refs.saturationCursor as HTMLElement;
			saturationCursor.style.transform = `translate(${saturationPositionX}px, ${saturationPositionY}px)`;

			this.pickerHSVA.s = saturationPositionX / this.pickerSaturationRect.width;
			this.pickerHSVA.v = (1 - saturationPositionY / this.pickerSaturationRect.height) * 255;
		},
		setHueCursorPosition(y: number) {
			const huePosition = clamp(y, 0, this.pickerHueRect.height);

			const hueCursor = this.$refs.hueCursor as HTMLElement;
			hueCursor.style.transform = `translateY(${huePosition}px)`;

			this.pickerHSVA.h = clamp(1 - huePosition / this.pickerHueRect.height);
		},
		setOpacityCursorPosition(y: number) {
			const opacityPosition = clamp(y, 0, this.pickerOpacityRect.height);

			const opacityCursor = this.$refs.opacityCursor as HTMLElement;
			opacityCursor.style.transform = `translateY(${opacityPosition}px)`;

			this.pickerHSVA.a = clamp(1 - opacityPosition / this.pickerOpacityRect.height);
		},
		updateHue() {
			const hsva = hsvaToRgba({ h: this.pickerHSVA.h, s: 1, v: 255, a: 1 });
			const rgba = hsvaToRgba(this.pickerHSVA);

			this.$el.style.setProperty("--saturation-picker-hue", `rgb(${hsva.r}, ${hsva.g}, ${hsva.b})`);
			this.$el.style.setProperty("--opacity-picker-color", `rgb(${rgba.r}, ${rgba.g}, ${rgba.b})`);
		},
		updateColor() {
			if (this.state !== "Idle") return;

			this.pickerHSVA = rgbaToHsva(this.color);

			this.updateRects();

			this.setSaturationCursorPosition(this.pickerHSVA.s * this.pickerSaturationRect.width, (1 - this.pickerHSVA.v / 255) * this.pickerSaturationRect.height);
			this.setOpacityCursorPosition((1 - this.pickerHSVA.a) * this.pickerOpacityRect.height);
			this.setHueCursorPosition((1 - this.pickerHSVA.h) * this.pickerHueRect.height);

			this.updateHue();
		},
	},
	components: {
		LayoutCol,
		LayoutRow,
	},
});
</script>
