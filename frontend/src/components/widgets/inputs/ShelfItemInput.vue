<template>
	<LayoutRow class="shelf-item-input" :class="{ active: active }">
		<IconButton :action="action" :icon="icon" :size="32" />
	</LayoutRow>
</template>

<style lang="scss">
.shelf-item-input {
	flex: 0 0 auto;
	border-radius: 2px;

	&:hover {
		background: var(--color-6-lowergray);
	}

	&.active {
		background: var(--color-accent);
	}

	.icon-button {
		background: unset;
	}

	svg {
		width: 24px;
		height: 24px;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IconName } from "@/utilities/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";

export default defineComponent({
	components: {
		IconButton,
		LayoutRow,
	},
	props: {
		icon: { type: String as PropType<IconName>, required: true },
		action: { type: Function as PropType<(e?: MouseEvent) => void>, required: true },
		active: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
