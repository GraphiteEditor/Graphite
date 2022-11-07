<template>
	<button
		class="text-button"
		:class="{ emphasized, disabled, 'sharp-right-corners': sharpRightCorners }"
		:data-emphasized="emphasized || undefined"
		:data-disabled="disabled || undefined"
		data-text-button
		:title="tooltip"
		:style="{ 'min-width': minWidth > 0 ? `${minWidth}px` : undefined }"
		@click="(e: MouseEvent) => action(e)"
		:tabindex="disabled ? -1 : 0"
	>
		<IconLabel v-if="icon" :icon="icon" />
		<TextLabel>{{ label }}</TextLabel>
	</button>
</template>

<style lang="scss">
.text-button {
	display: flex;
	justify-content: center;
	align-items: center;
	flex: 0 0 auto;
	height: 24px;
	margin: 0;
	padding: 0 8px;
	box-sizing: border-box;
	border: none;
	border-radius: 2px;
	background: var(--color-5-dullgray);
	color: var(--color-e-nearwhite);

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);
	}

	&.disabled {
		background: var(--color-4-dimgray);
		color: var(--color-8-uppergray);
	}

	&.emphasized {
		background: var(--color-e-nearwhite);
		color: var(--color-2-mildblack);

		&:hover {
			background: var(--color-f-white);
		}

		&.disabled {
			background: var(--color-8-uppergray);
		}
	}

	& + .text-button {
		margin-left: 8px;
	}

	.icon-label {
		position: relative;
		left: -4px;
	}
}
</style>


<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type IconName } from "@/utility-functions/icons";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	props: {
		label: { type: String as PropType<string>, required: true },
		icon: { type: String as PropType<IconName | undefined>, required: false },
		emphasized: { type: Boolean as PropType<boolean>, default: false },
		minWidth: { type: Number as PropType<number>, default: 0 },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },

		// Callbacks
		action: { type: Function as PropType<(e: MouseEvent) => void>, required: true },
	},
	components: {
		IconLabel,
		TextLabel,
	},
});
</script>
