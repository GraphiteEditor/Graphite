<template>
	<div class="radio-input" ref="radioInput">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.radio-input {
	.popover-button button {
		border-radius: 0;
	}

	& > * {
		border-radius: 0;
		margin: 0;

		&:first-child,
		&:first-child button {
			border-radius: 2px 0 0 2px;
		}

		&:last-child,
		&:last-child button {
			border-radius: 0 2px 2px 0;
		}

		& + * {
			margin-left: 1px;
		}
	}

	& > button {
		background: var(--color-5-dullgray);
		fill: var(--color-e-nearwhite);

		&:hover {
			background: var(--color-6-lowergray);
		}

		&.active {
			background: var(--color-accent);
			fill: var(--color-f-white);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {
		index: { type: Number, required: true },
	},
	data() {
		return {
			activeIndex: this.index,
		};
	},
	mounted() {
		this.updateActiveIconButton();

		(this.$refs.radioInput as Element).querySelectorAll(".icon-button").forEach((iconButton, index) => {
			iconButton.addEventListener("click", () => {
				this.activeIndex = index;
				this.$emit("update:index", index);
				this.$emit("changed", index);
			});
		});
	},
	watch: {
		activeIndex() {
			this.updateActiveIconButton();
		},
	},
	methods: {
		// This method may be called by the user of this component by setting a `ref="radioInput"` attribute and calling `(this.$refs.viewModePicker as typeof RadioInput).setActive(...)`
		setActive(index: number) {
			this.activeIndex = index;
		},
		updateActiveIconButton() {
			const iconButtons = (this.$refs.radioInput as Element).querySelectorAll(".icon-button");
			iconButtons.forEach((iconButton) => iconButton.classList.remove("active"));
			iconButtons[this.activeIndex].classList.add("active");
		},
	},
});
</script>
