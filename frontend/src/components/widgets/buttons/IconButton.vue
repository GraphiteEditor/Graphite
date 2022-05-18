<template>
	<button :class="['icon-button', `size-${size}`, active && 'active']" @click="(e: MouseEvent) => action(e)">
		<IconLabel :icon="icon" />
	</button>
</template>

<style lang="scss">
.icon-button {
	display: flex;
	justify-content: center;
	align-items: center;
	flex: 0 0 auto;
	padding: 0;
	outline: none;
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

	&.active {
		background: var(--color-accent);
	}

	&:hover:not(.active) {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);

		svg {
			fill: var(--color-f-white);
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

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IconName, IconSize } from "@/utilities/icons";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	props: {
		action: { type: Function as PropType<(e?: MouseEvent) => void>, required: true },
		icon: { type: String as PropType<IconName>, required: true },
		size: { type: Number as PropType<IconSize>, required: true },
		active: { type: Boolean as PropType<boolean>, default: false },
		gapAfter: { type: Boolean as PropType<boolean>, default: false },
	},
	components: { IconLabel },
});
</script>
