<template>
	<div class="popover-color-picker" @pointermove="onPointerMove">
		<div class="saturation-picker" ref="saturationPicker" data-picker-type="saturation" @pointerdown="onPointerDown" @pointerup="onPointerUp" @pointerout="onPointerUp">
			<div ref="saturationCursor" class="selection-circle"></div>
		</div>
		<div class="hue-picker" ref="huePicker" data-picker-type="hue" @pointerdown="onPointerDown" @pointerup="onPointerUp" @pointerout="onPointerUp">
			<div ref="hueCursor" class="selection-pincers"></div>
		</div>
		<div class="opacity-picker" ref="opacityPicker" data-picker-type="opacity" @pointerdown="onPointerDown" @pointerup="onPointerUp" @pointerout="onPointerUp">
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
import { clamp, HSV2RGB, RGB, RGB2HSV } from "../../lib/utils";

interface ColorPickerData {
	color: {
		h: number;
		s: number;
		v: number;
		a: number;
	};
	hue: {
		size: {
			width: number;
			height: number;
		};
	};
	saturation: {
		size: {
			width: number;
			height: number;
		};
	};
	opacity: {
		size: {
			width: number;
			height: number;
		};
	};
}

export default defineComponent({
	colorPicker: {
		color: {
			h: 0,
			s: 0,
			v: 0,
			a: 1,
		},
		hue: {
			size: {
				width: 0,
				height: 0,
			},
		},
		opacity: {
			size: {
				width: 0,
				height: 0,
			},
		},
		saturation: {
			size: {
				width: 0,
				height: 0,
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
			state: "idle",
		};
	},
	mounted() {
		this.$watch(
			"color",
			() => {
				if (this.state === "idle") {
					this.updateColor();
				}
			},
			{ immediate: true }
		);
	},
	methods: {
		onPointerDown(e: PointerEvent) {
			const target = e.currentTarget as Element;
			const picker = target.getAttribute("data-picker-type");

			if (picker) {
				this.state = `move_${picker}`;
				this.updateRects();
				this.onPointerMove(e);
			}
		},

		onPointerMove(e: PointerEvent) {
			if (this.state === "move_hue") {
				this.setHuePosition(e.offsetY);
			} else if (this.state === "move_opacity") {
				this.setOpacityPosition(e.offsetY);
			} else if (this.state === "move_saturation") {
				this.setSaturationPosition(e.offsetX, e.offsetY);
			}

			if (this.state !== "idle") {
				const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
				this.updateHue();
				console.log("SEND", colorPicker.color, HSV2RGB(colorPicker.color));
				this.$emit("update:color", HSV2RGB(colorPicker.color));
			}
		},

		onPointerUp(_: PointerEvent) {
			this.state = "idle";
		},

		updateRects() {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };

			const saturationPicker = this.$refs.saturationPicker as HTMLDivElement;
			const saturation = saturationPicker.getBoundingClientRect();
			colorPicker.saturation.size.width = saturation.width;
			colorPicker.saturation.size.height = saturation.height;

			const huePicker = this.$refs.huePicker as HTMLDivElement;
			const hue = huePicker.getBoundingClientRect();
			colorPicker.hue.size.width = hue.width;
			colorPicker.hue.size.height = hue.height;

			const opacityPicker = this.$refs.opacityPicker as HTMLDivElement;
			const opacity = opacityPicker.getBoundingClientRect();
			colorPicker.opacity.size.width = opacity.width;
			colorPicker.opacity.size.height = opacity.height;
		},

		setSaturationPosition(x: number, y: number) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const saturationCursor = this.$refs.saturationCursor as HTMLDivElement;
			const saturationPosition = [clamp(x, 0, colorPicker.saturation.size.width), clamp(y, 0, colorPicker.saturation.size.height)];
			saturationCursor.style.transform = `matrix(1, 0, 0, 1, ${saturationPosition[0]}, ${saturationPosition[1]})`;
			colorPicker.color.s = saturationPosition[0] / colorPicker.saturation.size.width;
			colorPicker.color.v = 1 - saturationPosition[1] / colorPicker.saturation.size.height;
		},

		setHuePosition(y: number) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const hueCursor = this.$refs.hueCursor as HTMLDivElement;
			const huePosition = clamp(y, 0, colorPicker.hue.size.height);
			hueCursor.style.transform = `matrix(1, 0, 0, 1, 0, ${huePosition})`;
			colorPicker.color.h = clamp(1 - huePosition / colorPicker.hue.size.height);
		},

		setOpacityPosition(y: number) {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const opacityCursor = this.$refs.opacityCursor as HTMLDivElement;
			const opacityPosition = clamp(y, 0, colorPicker.opacity.size.height);
			opacityCursor.style.transform = `matrix(1, 0, 0, 1, 0, ${opacityPosition})`;
			colorPicker.color.a = clamp(1 - opacityPosition / colorPicker.opacity.size.height);
		},

		updateHue() {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			const hueColor = HSV2RGB({
				h: colorPicker.color.h,
				s: 1,
				v: 1,
				a: 1,
			});
			console.log(colorPicker.color.h);
			this.$el.style.setProperty("--hue", `rgb(${hueColor.r}, ${hueColor.g}, ${hueColor.b})`);
		},

		updateColor() {
			const { colorPicker } = this.$options as { colorPicker: ColorPickerData };
			colorPicker.color = RGB2HSV(this.color as RGB);
			this.updateRects();
			this.setSaturationPosition(colorPicker.color.s * colorPicker.saturation.size.width, (1 - colorPicker.color.v) * colorPicker.saturation.size.height);
			this.setOpacityPosition((1 - colorPicker.color.a) * colorPicker.opacity.size.height);
			this.setHuePosition((1 - colorPicker.color.h) * colorPicker.hue.size.height);
			console.log(colorPicker.color);
			this.updateHue();
		},
	},
});
</script>
