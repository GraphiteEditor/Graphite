<script lang="ts">
	import LayoutRow from "~/src/components/layout/LayoutRow.svelte";
	import TextLabel from "~/src/components/widgets/labels/TextLabel.svelte";

	export let text: string;
</script>

<LayoutRow class="window-title">
	<TextLabel>{text}</TextLabel>
</LayoutRow>

<style lang="scss" global>
	.window-title {
		flex: 0 0 auto;
		align-items: center;
		white-space: nowrap;
		padding: 0 8px;
	}
</style>
