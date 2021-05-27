<template>
	<div class="popover-color-picker">
		<div class="saturation-picker" ref="saturationPicker" data-picker-action="MoveSaturation" @pointerdown="onPointerDown">
			<div ref="saturationCursor" class="selection-circle"></div>
		</div>
		<div class="hue-picker" ref="huePicker" data-picker-action="MoveHue" @pointerdown="onPointerDown">
			<div ref="hueCursor" class="selection-pincers"></div>
		</div>
		<div class="opacity-picker" ref="opacityPicker" data-picker-action="MoveOpacity" @pointerdown="onPointerDown">
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
import { clamp, hsvToRgb, rgbToHsv, RGB, isRGB } from "../../lib/utils";

const enum ColorPickerState {
	Idle = "Idle",
	MoveHue = "MoveHue",
	MoveOpacity = "MoveOpacity",
	MoveSaturation = "MoveSaturation",
}

export default defineComponent({
	components: {},
	props: {
		color: {
			type: Object,
		},
	},
	data() {
		return {
			state: ColorPickerState.Idle,
			_: {
				colorPicker: {
					color: { h: 0, s: 0, v: 0, a: 1 },
					hue: {
						rect: { width: 0, height: 0, top: 0, left: 0 },
					},
					opacity: {
						rect: { width: 0, height: 0, top: 0, left: 0 },
					},
					saturation: {
						rect: { width: 0, height: 0, top: 0, left: 0 },
					},
				},
			},
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

		getRef<T>(name: string) {
			return this.$refs[name] as T;
		},

		onPointerDown(e: PointerEvent) {
			if (!(e.currentTarget instanceof Element)) return;
			const picker = e.currentTarget.getAttribute("data-picker-action");
			this.state = (() => {
				switch (picker) {
					case "MoveHue":
						return ColorPickerState.MoveHue;
					case "MoveOpacity":
						return ColorPickerState.MoveOpacity;
					case "MoveSaturation":
						return ColorPickerState.MoveSaturation;
					default:
						return ColorPickerState.Idle;
				}
			})();

			if (this.state !== ColorPickerState.Idle) {
				this.addEvents();
				this.updateRects();
				this.onPointerMove(e);
			}
		},

		onPointerMove(e: PointerEvent) {
			const { colorPicker } = this.$data._;

			if (this.state === ColorPickerState.MoveHue) {
				this.setHuePosition(e.clientY - colorPicker.hue.rect.top);
			} else if (this.state === ColorPickerState.MoveOpacity) {
				this.setOpacityPosition(e.clientY - colorPicker.opacity.rect.top);
			} else if (this.state === ColorPickerState.MoveSaturation) {
				this.setSaturationPosition(e.clientX - colorPicker.saturation.rect.left, e.clientY - colorPicker.saturation.rect.top);
			}

			if (this.state !== ColorPickerState.Idle) {
				this.updateHue();
				this.$emit("update:color", hsvToRgb(colorPicker.color));
			}
		},

		onPointerUp() {
			if (this.state !== ColorPickerState.Idle) {
				this.state = ColorPickerState.Idle;
				this.removeEvents();
			}
		},

		updateRects() {
			const { colorPicker } = this.$data._;

			const saturationPicker = this.getRef<HTMLDivElement>("saturationPicker");
			const saturation = saturationPicker.getBoundingClientRect();
			colorPicker.saturation.rect.width = saturation.width;
			colorPicker.saturation.rect.height = saturation.height;
			colorPicker.saturation.rect.left = saturation.left;
			colorPicker.saturation.rect.top = saturation.top;

			const huePicker = this.getRef<HTMLDivElement>("huePicker");
			const hue = huePicker.getBoundingClientRect();
			colorPicker.hue.rect.width = hue.width;
			colorPicker.hue.rect.height = hue.height;
			colorPicker.hue.rect.left = hue.left;
			colorPicker.hue.rect.top = hue.top;

			const opacityPicker = this.getRef<HTMLDivElement>("opacityPicker");
			const opacity = opacityPicker.getBoundingClientRect();
			colorPicker.opacity.rect.width = opacity.width;
			colorPicker.opacity.rect.height = opacity.height;
			colorPicker.opacity.rect.left = opacity.left;
			colorPicker.opacity.rect.top = opacity.top;
		},

		setSaturationPosition(x: number, y: number) {
			const { colorPicker } = this.$data._;
			const saturationCursor = this.getRef<HTMLDivElement>("saturationCursor");
			const saturationPosition = [clamp(x, 0, colorPicker.saturation.rect.width), clamp(y, 0, colorPicker.saturation.rect.height)];
			saturationCursor.style.transform = `translate(${saturationPosition[0]}px, ${saturationPosition[1]}px)`;
			colorPicker.color.s = saturationPosition[0] / colorPicker.saturation.rect.width;
			colorPicker.color.v = (1 - saturationPosition[1] / colorPicker.saturation.rect.height) * 255;
		},

		setHuePosition(y: number) {
			const { colorPicker } = this.$data._;
			const hueCursor = this.getRef<HTMLDivElement>("hueCursor");
			const huePosition = clamp(y, 0, colorPicker.hue.rect.height);
			hueCursor.style.transform = `translateY(${huePosition}px)`;
			colorPicker.color.h = clamp(1 - huePosition / colorPicker.hue.rect.height);
		},

		setOpacityPosition(y: number) {
			const { colorPicker } = this.$data._;
			const opacityCursor = this.getRef<HTMLDivElement>("opacityCursor");
			const opacityPosition = clamp(y, 0, colorPicker.opacity.rect.height);
			opacityCursor.style.transform = `translateY(${opacityPosition}px)`;
			colorPicker.color.a = clamp(1 - opacityPosition / colorPicker.opacity.rect.height);
		},

		updateHue() {
			const { colorPicker } = this.$data._;
			const hueColor = hsvToRgb({ h: colorPicker.color.h, s: 1, v: 255, a: 1 });
			this.$el.style.setProperty("--hue", `rgb(${hueColor.r}, ${hueColor.g}, ${hueColor.b})`);
		},

		updateColor() {
			if (this.state !== ColorPickerState.Idle) return;
			const { color } = this;
			if (!isRGB(color)) return;
			const { colorPicker } = this.$data._;
			colorPicker.color = rgbToHsv(color);
			this.updateRects();
			this.setSaturationPosition(colorPicker.color.s * colorPicker.saturation.rect.width, (1 - colorPicker.color.v / 255) * colorPicker.saturation.rect.height);
			this.setOpacityPosition((1 - colorPicker.color.a) * colorPicker.opacity.rect.height);
			this.setHuePosition((1 - colorPicker.color.h) * colorPicker.hue.rect.height);
			this.updateHue();
		},
	},
});
</script>
