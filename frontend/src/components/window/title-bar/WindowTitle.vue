<template>
	<LayoutRow class="window-title">
		<span>{{ text }}</span>
	</LayoutRow>
</template>

<style lang="scss">
.window-title {
	flex: 0 0 auto;
	align-items: center;
	white-space: nowrap;
	padding: 0 8px;
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	props: {
		text: { type: String as PropType<string>, required: true },
	},
	components: { LayoutRow },
});
</script>
