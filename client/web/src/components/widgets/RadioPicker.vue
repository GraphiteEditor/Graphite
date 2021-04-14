<template>
	<div class="radio-picker" ref="radioPicker">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.radio-picker {
	font-size: 0;

	button {
		fill: #fff;
		border-radius: 0;
		margin: 0;

		&:first-child {
			border-radius: 2px 0 0 2px;
		}

		&:last-child {
			border-radius: 0 2px 2px 0;
		}
	}

	.icon-button {
		background: #555;

		&:hover {
			background: #666;
		}

		&.active {
			background: #3194d6;
		}

		& + .icon-button {
			margin-left: 1px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {
		initialIndex: { type: Number, required: true },
		setIndex: { type: Function, required: false },
	},
	data() {
		return {
			activeIndex: this.initialIndex,
		};
	},
	mounted() {
		this.updateActiveIconButton();

		(this.$refs.radioPicker as Element).querySelectorAll(".icon-button").forEach((iconButton, index) => {
			iconButton.addEventListener("click", () => {
				this.activeIndex = index;
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
		// This method may be called by the user of this component by setting a `ref="radioPicker"` attribute and calling `(this.$refs.viewModePicker as typeof RadioPicker).setActive(...)`
		setActive(index: number) {
			this.activeIndex = index;
		},
		updateActiveIconButton() {
			const iconButtons = (this.$refs.radioPicker as Element).querySelectorAll(".icon-button");
			iconButtons.forEach((iconButton) => iconButton.classList.remove("active"));
			iconButtons[this.activeIndex].classList.add("active");
		},
	},
});
</script>
