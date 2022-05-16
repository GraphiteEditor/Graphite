<template>
	<FloatingMenu class="dialog-modal" :type="'Dialog'" :direction="'Center'" data-dialog-modal>
		<LayoutRow ref="main">
			<LayoutCol class="icon-column">
				<!-- `dialog.state.icon` class exists to provide special sizing in CSS to specific icons -->
				<IconLabel :icon="dialog.state.icon" :class="dialog.state.icon.toLowerCase()" />
			</LayoutCol>
			<LayoutCol class="main-column">
				<WidgetLayout v-if="dialog.state.widgets.layout.length > 0" :layout="dialog.state.widgets" class="details" />
				<LayoutRow v-if="dialog.state.jsCallbackBasedButtons?.length" class="panic-buttons-row">
					<TextButton v-for="(button, index) in dialog.state.jsCallbackBasedButtons" :key="index" :action="() => button.callback?.()" v-bind="button.props" />
				</LayoutRow>
			</LayoutCol>
		</LayoutRow>
	</FloatingMenu>
</template>

<style lang="scss">
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
import { defineComponent } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import TextButton from "@/components/widgets/buttons/TextButton.vue";
import FloatingMenu from "@/components/widgets/floating-menus/FloatingMenu.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

export default defineComponent({
	inject: ["dialog"],
	components: {
		LayoutRow,
		LayoutCol,
		FloatingMenu,
		IconLabel,
		TextButton,
		WidgetLayout,
	},
	methods: {
		dismiss() {
			this.dialog.dismissDialog();
		},
	},
	mounted() {
		// Focus the first button in the popup
		const element = this.$el as Element | null;
		const emphasizedOrFirstButton = (element?.querySelector("button.emphasized") as HTMLButtonElement | null) || element?.querySelector("button");
		emphasizedOrFirstButton?.focus();
	},
});
</script>
