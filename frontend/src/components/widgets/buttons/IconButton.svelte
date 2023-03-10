<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type IconName, type IconSize } from "@/utility-functions/icons";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	props: {
		icon: { type: String as PropType<IconName>, required: true },
		size: { type: Number as PropType<IconSize>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		active: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },

		// Callbacks
		action: { type: Function as PropType<(e?: MouseEvent) => void>, required: true },
	},
	components: { IconLabel },
});
</script>

<template>
	<button
		class="icon-button"
		:class="[`size-${size}`, { disabled, active, 'sharp-right-corners': sharpRightCorners }]"
		@click="(e: MouseEvent) => action(e)"
		:disabled="disabled"
		:title="tooltip"
		:tabindex="active ? -1 : 0"
	>
		<IconLabel :icon="icon" />
	</button>
</template>

<style lang="scss">
.icon-button {
	display: flex;
	justify-content: center;
	align-items: center;
	flex: 0 0 auto;
	margin: 0;
	padding: 0;
	border: none;
	border-radius: 2px;
	background: none;

	svg {
		fill: var(--color-e-nearwhite);
	}

	// The `where` pseudo-class does not contribtue to specificity
	& + :where(.icon-button) {
		margin-left: 0;
	}

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);

		svg {
			fill: var(--color-f-white);
		}
	}

	&.disabled {
		background: none;

		svg {
			fill: var(--color-8-uppergray);
		}
	}

	&.active {
		background: var(--color-e-nearwhite);

		svg {
			fill: var(--color-2-mildblack);
		}
	}

	&.size-12 {
		width: 12px;
		height: 12px;
	}

	&.size-16 {
		width: 16px;
		height: 16px;
	}

	&.size-24 {
		width: 24px;
		height: 24px;
	}

	&.size-32 {
		width: 32px;
		height: 32px;
	}
}
</style>
