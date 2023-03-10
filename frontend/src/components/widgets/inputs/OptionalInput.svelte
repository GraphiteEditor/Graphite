<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type IconName } from "@/utility-functions/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import CheckboxInput from "@/components/widgets/inputs/CheckboxInput.vue";

export default defineComponent({
	emits: ["update:checked"],
	props: {
		checked: { type: Boolean as PropType<boolean>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		icon: { type: String as PropType<IconName>, default: "Checkmark" },
		tooltip: { type: String as PropType<string | undefined>, required: false },
	},
	components: {
		CheckboxInput,
		LayoutRow,
	},
});
</script>

<template>
	<LayoutRow class="optional-input" :class="disabled">
		<CheckboxInput :checked="checked" :disabled="disabled" @input="(e: Event) => $emit('update:checked', (e.target as HTMLInputElement).checked)" :icon="icon" :tooltip="tooltip" />
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
		border: 1px solid var(--color-5-dullgray);
		border-radius: 2px 0 0 2px;
		box-sizing: border-box;
	}

	&.disabled label {
		border: 1px solid var(--color-4-dimgray);
	}
}
</style>
