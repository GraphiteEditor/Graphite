<template>
	<div class="working-colors">
		<div class="swatch-pair">
			<button
				class="secondary swatch"
				:style="{ 'background-color': secondaryColorCSS }"
				@click="onClick('secondary', $event)"
			></button>
			<button
				class="primary swatch"
				:style="{ 'background-color': primaryColorCSS }"
				@click="onClick('primary', $event)"
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
		<keep-alive>
			<ColorPicker
				ref="colorPicker"
				v-if="colorPickerOpened"
				v-model:color="colorPickerColor"
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
import IconButton from "../widgets/IconButton.vue";
import SwapButton from "../../../assets/svg/16x16-bounds-12x12-icon/swap.svg";
import ResetColorsButton from "../../../assets/svg/16x16-bounds-12x12-icon/reset-colors.svg";
import ColorPicker, { RGBAColor } from "./ColorPicker.vue";
import { NC } from "../../events/NotificationCenter";

type WorkingColorState = "none" | "primary" | "secondary";

export default defineComponent({
	data() {
		return {
			primaryColor: { r: 0, g: 0, b: 0 },
			secondaryColor: { r: 1, g: 1, b: 1 },
			colorPickerOpened: false,
			colorPickerColor: { r: 1, g: 1, b: 1 },
			active: "none" as WorkingColorState
		};
	},

	computed: {
		primaryColorCSS() {
			const r = Math.round(255 * this.primaryColor.r);
			const g = Math.round(255 * this.primaryColor.g);
			const b = Math.round(255 * this.primaryColor.b);
			return `rgb(${r}, ${g}, ${b})`;
		},
		secondaryColorCSS() {
			const r = Math.round(255 * this.secondaryColor.r);
			const g = Math.round(255 * this.secondaryColor.g);
			const b = Math.round(255 * this.secondaryColor.b);
			return `rgb(${r}, ${g}, ${b})`;
		}
	},

	components: {
		IconButton,
		ResetColorsButton,
		SwapButton,
		ColorPicker
	},

	mounted() {
		this.$watch("colorPickerColor", this.onColorChange, { deep: true });
	},

	methods: {
		onClose() {
			this.setColorPicker(this.active, false);
		},

		onClick(target: WorkingColorState, e: MouseEvent) {
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
						this.setColor(this.colorPickerColor, this.primaryColor);
						break;
					case "secondary":
						this.setColor(
							this.colorPickerColor,
							this.secondaryColor
						);
						break;
				}
			} else {
				switch (this.active) {
					case "primary":
						this.setColor(this.primaryColor, this.colorPickerColor);
						NC.dispatch("update_primary_color", {
							color: this.primaryColor
						});
						break;
					case "secondary":
						this.setColor(
							this.secondaryColor,
							this.colorPickerColor
						);
						NC.dispatch("update_secondary_color", {
							color: this.secondaryColor
						});
						break;
				}
				this.active = "none";
			}
		},

		setColor(c0: RGBAColor, c1: RGBAColor) {
			c0.r = c1.r;
			c0.g = c1.g;
			c0.b = c1.b;
		},

		onColorChange() {
			switch (this.active) {
				case "primary":
					this.setColor(this.primaryColor, this.colorPickerColor);
					break;
				case "secondary":
					this.secondaryColor = this.colorPickerColor;
					this.setColor(this.secondaryColor, this.colorPickerColor);
					break;
			}
		}
	}
});
</script>
