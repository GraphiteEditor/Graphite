<script lang="ts">
	import { getContext } from "svelte";

	import type { PropertiesState } from "@graphite/state-providers/properties";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const properties = getContext<PropertiesState>("properties");
</script>

<LayoutCol class="properties">
	<LayoutRow class="options-bar">
		<WidgetLayout layout={$properties.propertiesOptionsLayout} />
	</LayoutRow>
	<LayoutCol class="sections" scrollableY={true}>
		<WidgetLayout layout={$properties.propertiesSectionsLayout} />
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.properties {
		height: 100%;
		flex: 1 1 100%;

		.options-bar {
			min-height: 32px;
			margin: 0 4px;

			.widget-span {
				width: 100%;
			}
		}

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
