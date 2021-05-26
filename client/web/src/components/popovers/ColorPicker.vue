<template>
	<div class="popover-color-picker">
		<div class="saturation-picker" ref="saturationPicker" data-picker-type="saturation" @pointerdown="onPointerDown">
			<div ref="saturationCursor" class="selection-circle"></div>
		</div>
		<div class="hue-picker" ref="huePicker" data-picker-type="hue" @pointerdown="onPointerDown">
			<div ref="hueCursor" class="selection-pincers"></div>
		</div>
		<div class="opacity-picker" ref="opacityPicker" data-picker-type="opacity" @pointerdown="onPointerDown">
			<div ref="opacityCursor" class="selection-pincers"></div>
		</div>
	</div>
</template>

<style lang="scss">
.popover-color-picker {
	--hue: #ff0000;
	display: flex;

	.saturation-picker {
		width: 256px;
		background-blend-mode: multiply;
		background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--hue));
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
		background: linear-gradient(to bottom, var(--hue), transparent);

		&::before {
			content: "";
			display: block;
			width: 100%;
			height: 100%;
			background: linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%), linear-gradient(45deg, #cccccc 25%, transparent 25%, transparent 75%, #cccccc 75%),
				linear-gradient(#ffffff, #ffffff);
			background-size: 16px 16px;
			background-position: 0 0, 8px 8px;
			position: relative;
			z-index: -1;
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
import { defineComponent } from "vue";
import { clamp, HSV2RGB, RGB2HSV, RGB, HSV } from "../../lib/utils";

interface Rect {
	width: number;
	height: number;
	left: number;
	top: number;
}

interface ColorPickerData {
	color: HSV;
	hue: {
		rect: Rect;
	};
	saturation: {
		rect: Rect;
	};
	opacity: {
		rect: Rect;
	};
}

type ColorPickerState = "idle" | "move_hue" | "move_opacity" | "move_saturation";

export default defineComponent({
	colorPicker: {
		color: {
			h: 0,
			s: 0,
			v: 0,
			a: 1,
		},
		hue: {
			rect: {
				width: 0,
				height: 0,
				top: 0,
				left: 0,
			},
		},
		opacity: {
			rect: {
				width: 0,
				height: 0,
				top: 0,
				left: 0,
			},
		},
		saturation: {
			rect: {
				width: 0,
				height: 0,
				top: 0,
				left: 0,
			},
		},
	} as ColorPickerData,
	components: {},
	props: {
		color: {
			type: Object,
		},
	},
	data() {
		return {
			state: "idle" as ColorPickerState,
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
			const target = e.currentTarget as Element;
			const picker = target.getAttribute("data-picker-type");

			if (picker && this.state === "idle") {
				this.addEvents();
				this.state = `move_${picker}` as ColorPickerState;
				this.updateRects();
				this.onPointerMove(e);
			}
		},

		onPointerMove(e: PointerEvent) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };

			if (this.state === "move_hue") {
				this.setHuePosition(e.y - colorPicker.hue.rect.top);
			} else if (this.state === "move_opacity") {
				this.setOpacityPosition(e.y - colorPicker.opacity.rect.top);
			} else if (this.state === "move_saturation") {
				this.setSaturationPosition(e.x - colorPicker.saturation.rect.left, e.y - colorPicker.saturation.rect.top);
			}

			if (this.state !== "idle") {
				this.updateHue();
				this.$emit("update:color", HSV2RGB(colorPicker.color));
			}
		},

		onPointerUp() {
			if (this.state !== "idle") {
				this.state = "idle";
				this.removeEvents();
			}
		},

		updateRects() {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };

			const saturationPicker = this.$refs.saturationPicker as HTMLDivElement;
			const saturation = saturationPicker.getBoundingClientRect();
			colorPicker.saturation.rect.width = saturation.width;
			colorPicker.saturation.rect.height = saturation.height;
			colorPicker.saturation.rect.left = saturation.left;
			colorPicker.saturation.rect.top = saturation.top;

			const huePicker = this.$refs.huePicker as HTMLDivElement;
			const hue = huePicker.getBoundingClientRect();
			colorPicker.hue.rect.width = hue.width;
			colorPicker.hue.rect.height = hue.height;
			colorPicker.hue.rect.left = hue.left;
			colorPicker.hue.rect.top = hue.top;

			const opacityPicker = this.$refs.opacityPicker as HTMLDivElement;
			const opacity = opacityPicker.getBoundingClientRect();
			colorPicker.opacity.rect.width = opacity.width;
			colorPicker.opacity.rect.height = opacity.height;
			colorPicker.opacity.rect.left = opacity.left;
			colorPicker.opacity.rect.top = opacity.top;
		},

		setSaturationPosition(x: number, y: number) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const saturationCursor = this.$refs.saturationCursor as HTMLDivElement;
			const saturationPosition = [clamp(x, 0, colorPicker.saturation.rect.width), clamp(y, 0, colorPicker.saturation.rect.height)];
			saturationCursor.style.transform = `matrix(1, 0, 0, 1, ${saturationPosition[0]}, ${saturationPosition[1]})`;
			colorPicker.color.s = saturationPosition[0] / colorPicker.saturation.rect.width;
			colorPicker.color.v = (1 - saturationPosition[1] / colorPicker.saturation.rect.height) * 255;
		},

		setHuePosition(y: number) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const hueCursor = this.$refs.hueCursor as HTMLDivElement;
			const huePosition = clamp(y, 0, colorPicker.hue.rect.height);
			hueCursor.style.transform = `matrix(1, 0, 0, 1, 0, ${huePosition})`;
			colorPicker.color.h = clamp(1 - huePosition / colorPicker.hue.rect.height);
		},

		setOpacityPosition(y: number) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const opacityCursor = this.$refs.opacityCursor as HTMLDivElement;
			const opacityPosition = clamp(y, 0, colorPicker.opacity.rect.height);
			opacityCursor.style.transform = `matrix(1, 0, 0, 1, 0, ${opacityPosition})`;
			colorPicker.color.a = clamp(1 - opacityPosition / colorPicker.opacity.rect.height);
		},

		updateHue() {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const hueColor = HSV2RGB({
				h: colorPicker.color.h,
				s: 1,
				v: 255,
				a: 1,
			});
			this.$el.style.setProperty("--hue", `rgb(${hueColor.r}, ${hueColor.g}, ${hueColor.b})`);
		},

		updateColor() {
			if (this.state !== "idle") return;
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			colorPicker.color = RGB2HSV(this.color as RGB);
			this.updateRects();
			this.setSaturationPosition(colorPicker.color.s * colorPicker.saturation.rect.width, (1 - colorPicker.color.v / 255) * colorPicker.saturation.rect.height);
			this.setOpacityPosition((1 - colorPicker.color.a) * colorPicker.opacity.rect.height);
			this.setHuePosition((1 - colorPicker.color.h) * colorPicker.hue.rect.height);
			this.updateHue();
		},
	},
});
</script>
