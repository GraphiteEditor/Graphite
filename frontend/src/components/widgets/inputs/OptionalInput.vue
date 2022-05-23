<template>
	<LayoutRow class="optional-input">
		<CheckboxInput :checked="checked" @input="(e) => $emit('update:checked', (e.target as HTMLInputElement).checked)" :icon="icon" />
	</LayoutRow>
</template>

<style lang="scss">
.optional-input {
	flex-grow: 0;

	label {
		align-items: center;
		justify-content: center;
		white-space: nowrap;
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
import { defineComponent, PropType } from "vue";

import { IconName } from "@/utility-functions/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";

export default defineComponent({
	emits: ["update:checked"],
	props: {
		checked: { type: Boolean as PropType<boolean>, required: true },
		icon: { type: String as PropType<IconName>, default: "Checkmark" },
	},
	components: {
		CheckboxInput,
		LayoutRow,
	},
});
</script>
