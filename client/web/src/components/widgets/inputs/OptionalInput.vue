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
		width: 24px;
		height: 24px;
		border: 1px solid var(--color-7-middlegray);
		border-radius: 2px 0 0 2px;
		box-sizing: border-box;

		&:hover {
			background: var(--color-6-lowergray);
		}
	}

	input:checked + label {
		border: 1px solid var(--color-accent);

		&:hover {
			border: 1px solid var(--color-accent-hover);
			background: none;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";

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
