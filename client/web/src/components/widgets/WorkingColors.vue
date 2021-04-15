<template>
	<div class="working-colors">
		<div class="swatch-pair">
			<button class="secondary swatch" :style="{ 'background-color': secondaryColorCSS }" @click="onClick('secondary', $event)"></button>
			<button class="primary swatch" :style="{ 'background-color': primaryColorCSS }" @click="onClick('primary', $event)"></button>
		</div>
		<div class="swap-and-reset">
			<IconButton :size="16">
				<SwapButton />
			</IconButton>
			<IconButton :size="16">
				<ResetColorsButton />
			</IconButton>
		</div>
		<keep-alive>
			<ColorPicker
				ref="colorPicker"
				v-if="colorPickerOpened"
				v-model:red="colorPickerColor.red"
				v-model:green="colorPickerColor.green"
				v-model:blue="colorPickerColor.blue"
				v-model:alpha="colorPickerColor.alpha"
				@mouseleave="onClose"
			></ColorPicker>
		</keep-alive>
	</div>
</template>

<style lang="scss">
.working-colors {
	position: relative;

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

	.swatch-pair {
		display: flex;
		// Reversed order of elements paired with `column-reverse` allows primary to overlap secondary without relying on `z-index`
		flex-direction: column-reverse;
	}

	.primary.swatch {
		margin-bottom: -8px;
	}

	.swap-and-reset {
		font-size: 0;
	}

	.color-picker {
		position: absolute;
		bottom: 0;
		left: 100%;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import IconButton from "./IconButton.vue";
import SwapButton from "../../../assets/svg/16x16-bounds-12x12-icon/swap.svg";
import ResetColorsButton from "../../../assets/svg/16x16-bounds-12x12-icon/reset-colors.svg";
import ColorPicker from "./ColorPicker.vue";
import { NC } from "../../events/notification-center";

type WorkingColorState = "none" | "primary" | "secondary";

export default defineComponent({
	data() {
		return {
			primaryColor: {
				red: 0,
				green: 0,
				blue: 0,
				alpha: 1,
			},
			secondaryColor: {
				red: 1,
				green: 1,
				blue: 1,
				alpha: 1,
			},
			tmpColor: {
				red: 1,
				green: 1,
				blue: 1,
				alpha: 1,
			},
			colorPickerOpened: false,
			colorPickerColor: {
				red: 1,
				green: 1,
				blue: 1,
				alpha: 1,
			},
			active: "none" as WorkingColorState,
		};
	},

	computed: {
		primaryColorCSS() {
			const r = Math.round(255 * this.primaryColor.red);
			const g = Math.round(255 * this.primaryColor.green);
			const b = Math.round(255 * this.primaryColor.blue);
			return `rgb(${r}, ${g}, ${b})`;
		},
		secondaryColorCSS() {
			const r = Math.round(255 * this.secondaryColor.red);
			const g = Math.round(255 * this.secondaryColor.green);
			const b = Math.round(255 * this.secondaryColor.blue);
			return `rgb(${r}, ${g}, ${b})`;
		},
	},

	components: {
		IconButton,
		ResetColorsButton,
		SwapButton,
		ColorPicker,
	},

	methods: {
		onClose() {
			this.setColorPicker(this.active, false);
		},

		onClick(target: WorkingColorState) {
			this.toggle(target);
		},

		toggle(target: WorkingColorState) {
			let enabled = !this.colorPickerOpened;
			if (target !== this.active) {
				enabled = true;
			}

			this.setColorPicker(target, enabled);
		},

		setColorPicker(target: WorkingColorState, enabled: boolean) {
			this.colorPickerOpened = enabled;

			if (enabled) {
				this.active = target;
				switch (this.active) {
				case "primary":
					this.colorPickerColor = this.primaryColor;
					break;
				case "secondary":
					this.colorPickerColor = this.secondaryColor;
					break;
				default:
					break;
				}
			} else {
				switch (this.active) {
				case "primary":
					this.colorPickerColor = this.tmpColor;
					NC.dispatch("update_primary_color", {
						color: {
							r: this.primaryColor.red,
							g: this.primaryColor.green,
							b: this.primaryColor.blue,
							a: this.primaryColor.alpha,
						},
					});
					break;
				case "secondary":
					this.colorPickerColor = this.tmpColor;
					NC.dispatch("update_secondary_color", {
						color: {
							r: this.secondaryColor.red,
							g: this.secondaryColor.green,
							b: this.secondaryColor.blue,
							a: this.secondaryColor.alpha,
						},
					});
					break;
				default:
					break;
				}
				this.active = "none";
			}
		},
	},
});
</script>
