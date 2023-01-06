<template>
	<FloatingMenu :open="true" class="dialog-modal" :type="'Dialog'" :direction="'Center'" data-dialog-modal>
		<LayoutRow>
			<LayoutCol class="icon-column">
				<!-- `dialog.state.icon` class exists to provide special sizing in CSS to specific icons -->
				<IconLabel :icon="dialog.state.icon" :class="dialog.state.icon.toLowerCase()" />
			</LayoutCol>
			<LayoutCol class="main-column">
				<WidgetLayout v-if="dialog.state.widgets.layout.length > 0" :layout="dialog.state.widgets" class="details" />
				<LayoutRow v-if="(dialog.state.jsCallbackBasedButtons?.length || NaN) > 0" class="panic-buttons-row">
					<TextButton v-for="(button, index) in dialog.state.jsCallbackBasedButtons" :key="index" :action="() => button.callback?.()" v-bind="button.props" />
				</LayoutRow>
			</LayoutCol>
		</LayoutRow>
	</FloatingMenu>
</template>

<style lang="scss" global>
.dialog-modal {
	position: absolute;
	pointer-events: none;
	width: 100%;
	height: 100%;

	> .floating-menu-container > .floating-menu-content {
		pointer-events: auto;
		padding: 24px;
	}

	.icon-column {
		margin-right: 24px;

		.icon-label {
			width: 80px;
			height: 80px;

			&.file,
			&.copy {
				width: 60px;

				svg {
					width: 80px;
					height: 80px;
					margin: 0 -10px;
				}
			}
		}
	}

	.main-column {
		margin: -4px 0;

		.details.text-label {
			-webkit-user-select: text; // Required as of Safari 15.0 (Graphite's minimum version) through the latest release
			user-select: text;
			white-space: pre-wrap;
			max-width: 400px;
			height: auto;
		}

		.panic-buttons-row {
			height: 32px;
			align-items: center;
		}
	}
}
</style>

<script lang="ts">


import FloatingMenu from "$lib/components/layout/FloatingMenu.svelte";
import LayoutCol from "$lib/components/layout/LayoutCol.svelte";
import LayoutRow from "$lib/components/layout/LayoutRow.svelte";
import TextButton from "$lib/components/widgets/buttons/TextButton.svelte";
import IconLabel from "$lib/components/widgets/labels/IconLabel.svelte";
import WidgetLayout from "$lib/components/widgets/WidgetLayout.svelte";

export default defineComponent({
	inject: ["dialog"],
	methods: {
		dismiss() {
			this.dialog.dismissDialog();
		},
	},
	mounted() {
		// Focus the first button in the popup
		const dialogModal: HTMLDivElement | undefined = this.$el;
		const emphasizedOrFirstButton = (dialogModal?.querySelector("[data-emphasized]") || dialogModal?.querySelector("[data-text-button]") || undefined) as HTMLButtonElement | undefined;
		emphasizedOrFirstButton?.focus();
	},
	components: {
		FloatingMenu,
		IconLabel,
		LayoutCol,
		LayoutRow,
		TextButton,
		WidgetLayout,
	},
});
</script>
