<template>
	<LayoutRow :class="['icon-label', iconSizeClass, { disabled }]" :title="tooltip">
		<component :is="icon" />
	</LayoutRow>
</template>

<style lang="scss">
.icon-label {
	flex: 0 0 auto;
	fill: var(--color-e-nearwhite);

	&.disabled {
		fill: var(--color-8-uppergray);
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
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type IconName, ICONS, ICON_COMPONENTS } from "@/utility-functions/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	props: {
		icon: { type: String as PropType<IconName>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
	},
	computed: {
		iconSizeClass(): string {
			return `size-${ICONS[this.icon].size}`;
		},
	},
	components: {
		LayoutRow,
		...ICON_COMPONENTS,
	},
});
</script>
