<template>
	<LayoutRow class="icon-label" :class="`size-${icons[icon].size}`">
		<component :is="icon" />
	</LayoutRow>
</template>

<style lang="scss">
.icon-label {
	flex: 0 0 auto;
	fill: var(--color-e-nearwhite);

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
import { defineComponent, PropType } from "vue";

import { IconName, icons } from "@/utilities/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	props: {
		icon: { type: String as PropType<IconName>, required: true },
		gapAfter: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			icons,
		};
	},
	components: {
		LayoutRow,
		// Import the components of all the icons
		...Object.fromEntries(Object.entries(icons).map(([name, data]) => [name, data.component])),
	},
});
</script>
