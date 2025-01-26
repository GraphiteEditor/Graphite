<script lang="ts">
	import { getContext, onMount } from "svelte";

	import type { Editor } from "@graphite/editor";
	import { defaultWidgetLayout, patchWidgetLayout, UpdatePropertyPanelSectionsLayout } from "@graphite/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	const editor = getContext<Editor>("editor");

	let propertiesSectionsLayout = defaultWidgetLayout();

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelSectionsLayout, (updatePropertyPanelSectionsLayout) => {
			patchWidgetLayout(propertiesSectionsLayout, updatePropertyPanelSectionsLayout);
			propertiesSectionsLayout = propertiesSectionsLayout;
		});
	});
</script>

<LayoutCol class="properties">
	<LayoutCol class="sections" scrollableY={true}>
		<WidgetLayout layout={propertiesSectionsLayout} />
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
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
