<template>
	<button class="text-button" :class="{ emphasized, disabled }" :style="minWidth > 0 ? `min-width: ${minWidth}px` : ''" @click="(e: MouseEvent) => action(e)">
		<TextLabel>{{ label }}</TextLabel>
	</button>
</template>

<style lang="scss">
.text-button {
	display: inline-flex;
	justify-content: center;
	align-items: center;
	flex: 0 0 auto;
	height: 24px;
	padding: 0 8px;
	box-sizing: border-box;
	outline: none;
	border: none;
	border-radius: 2px;
	background: var(--color-5-dullgray);
	color: var(--color-e-nearwhite);

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);
	}

	&.emphasized {
		background: var(--color-accent);
		color: var(--color-f-white);

		&:hover {
			background: var(--color-accent-hover);
		}

		&.disabled {
			background: var(--color-accent-disabled);
		}
	}

	&.disabled {
		background: var(--color-4-dimgray);
		color: var(--color-8-uppergray);
	}

	& + .text-button {
		margin-left: 8px;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	props: {
		action: { type: Function as PropType<(e: MouseEvent) => void>, required: true },
		label: { type: String as PropType<string>, required: true },
		emphasized: { type: Boolean as PropType<boolean>, default: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		minWidth: { type: Number as PropType<number>, default: 0 },
		gapAfter: { type: Boolean as PropType<boolean>, default: false },
	},
	components: { TextLabel },
});
</script>
