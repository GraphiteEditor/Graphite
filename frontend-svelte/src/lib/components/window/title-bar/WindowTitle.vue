<template>
	<LayoutRow class="window-title">
		<TextLabel>{{ text }}</TextLabel>
	</LayoutRow>
</template>

<style lang="scss" global>
.window-title {
	flex: 0 0 auto;
	align-items: center;
	white-space: nowrap;
	padding: 0 8px;
}
</style>

<script lang="ts">


import LayoutRow from "$lib/components/layout/LayoutRow.svelte";
import TextLabel from "$lib/components/widgets/labels/TextLabel.svelte";

export default defineComponent({
	props: {
		text: { type: String as PropType<string>, required: true },
	},
	components: {
		LayoutRow,
		TextLabel,
	},
});
</script>
