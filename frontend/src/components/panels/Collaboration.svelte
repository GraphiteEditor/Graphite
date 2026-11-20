<script lang="ts">
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { collaborationPanelLayout } from "/src/stores/portfolio";
</script>

<LayoutCol class="collaboration-panel">
	<LayoutCol class="body" scrollableY={true}>
		<WidgetLayout layout={collaborationPanelLayout} layoutTarget="CollaborationPanel" />
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.collaboration-panel {
		flex-grow: 1;
		padding: 4px 8px;

		.widget-span:has(.text-label.multiline) {
			flex: 1 1 100%;
		}
	}
</style>
