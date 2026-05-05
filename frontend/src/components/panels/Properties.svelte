<script lang="ts">
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { propertiesPanelLayout } from "/src/stores/portfolio";
</script>

<LayoutCol class="properties">
	<LayoutCol class="sections" scrollableY={true}>
		<WidgetLayout layout={$propertiesPanelLayout} layoutTarget="PropertiesPanel" />
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.properties {
		height: 100%;
		flex: 1 1 100%;

		.sections {
			flex: 1 1 100%;

			// Used as a placeholder for empty assist widgets
			.separator.section.horizontal {
				margin: 0;
				margin-left: 24px;

				div {
					width: 0;
				}
			}
		}

		.text-button {
			flex-basis: 0;
		}
	}
</style>
