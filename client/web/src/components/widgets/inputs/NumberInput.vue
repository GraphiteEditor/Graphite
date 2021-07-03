<template>
	<div class="number-input">
		<button class="arrow left" @click="onIncrement(-1)"></button>
		<button class="arrow right" @click="onIncrement(1)"></button>
		<input type="text" spellcheck="false" v-model="text" @change="updateText($event.target.value)" /> />
	</div>
</template>

<style lang="scss">
.number-input {
	width: 80px;
	height: 24px;
	position: relative;
	border-radius: 2px;
	background: var(--color-1-nearblack);
	overflow: hidden;

	input {
		width: calc(100% - 8px);
		margin: 0 4px;
		outline: none;
		border: none;
		background: none;
		padding: 0;
		line-height: 24px;
		color: var(--color-e-nearwhite);
		font-size: inherit;
		text-align: center;
		font-family: inherit;

		&::selection {
			background: var(--color-accent);
		}
	}

	&:not(:hover) .arrow {
		display: none;
	}

	.arrow {
		position: absolute;
		top: 0;
		outline: none;
		border: none;
		background: none;
		padding: 9px 0;

		&:hover {
			background: var(--color-6-lowergray);

			&.right::before {
				border-color: transparent transparent transparent var(--color-f-white);
			}

			&.left::after {
				border-color: transparent var(--color-f-white) transparent transparent;
			}
		}

		&.right {
			right: 0;
			padding-left: 7px;
			padding-right: 6px;

			&::before {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 0 3px 3px;
				border-color: transparent transparent transparent var(--color-e-nearwhite);
				display: block;
			}
		}

		&.left {
			left: 0;
			padding-left: 6px;
			padding-right: 7px;

			&::after {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 3px 3px 0;
				border-color: transparent var(--color-e-nearwhite) transparent transparent;
				display: block;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {
		initial_value: { type: Number, default: 0, required: false },
		unit: { type: String, default: "", required: false },
		step: { type: Number, default: 1, required: false },
		min: { type: Number, required: false },
		max: { type: Number, required: false },
		callback: { type: Function, required: false },
	},
	data() {
		return {
			value: this.initial_value,
			text: this.initial_value.toString() + this.unit,
		};
	},
	methods: {
		onIncrement(direction: number) {
			this.updateValue(this.value + this.step * direction, true);
		},

		updateText(newText: string) {
			const new_value = parseInt(newText, 10);
			this.updateValue(new_value, true);
		},

		clampValue(newValue: number, resetOnClamp: boolean) {
			if (!Number.isFinite(newValue)) return this.value;
			let result = newValue;
			if (Number.isFinite(this.min) && typeof this.min === "number") {
				if (resetOnClamp && newValue < this.min) return this.value;
				result = Math.max(result, this.min);
			}
			if (Number.isFinite(this.max) && typeof this.max === "number") {
				if (resetOnClamp && newValue > this.max) return this.value;
				result = Math.max(result, this.max);
			}
			return result;
		},
		updateValue(newValue: number, resetOnClamp: boolean) {
			const old_value = this.value;
			this.value = this.clampValue(newValue, resetOnClamp);

			if (this.callback) this.callback(this.value, old_value);

			this.text = this.value + this.unit;
		},
	},
});
</script>
