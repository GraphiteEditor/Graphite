<template>
	<div class="optional-input">
		<CheckboxInput :checked="checked" @input="(e) => $emit('update:checked', e.target.checked)" :icon="icon" />
	</div>
</template>

<style lang="scss">
.optional-input {
	label {
		display: flex;
		align-items: center;
		white-space: nowrap;
		justify-content: center;
		margin-right: 1px;
		width: 23px;
		height: 24px;
		border: 1px solid var(--color-7-middlegray);
		border-radius: 2px 0 0 2px;
		box-sizing: border-box;
		border-right: none;

		&:hover {
			background: var(--color-6-lowergray);
		}
	}

	input:checked + label {
		border: 1px solid var(--color-accent);
		border-right: none;

		&:hover {
			border: 1px solid var(--color-accent-hover);
			border-right: none;
			background: none;
		}
		// 	border: none;
		// 	background: var(--color-e-nearwhite);

		// 	&:hover {
		// 		background: var(--color-f-white);
		// 	}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import CheckboxInput from "./CheckboxInput.vue";

export default defineComponent({
	props: {
		checked: { type: Boolean, required: true },
		icon: { type: String, default: "Checkmark" },
	},
	components: {
		CheckboxInput,
	},
});
</script>
