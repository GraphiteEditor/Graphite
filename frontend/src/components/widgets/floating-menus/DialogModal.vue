<template>
	<div class="dialog-modal">
		<FloatingMenu :type="MenuType.Dialog" :direction="MenuDirection.Center">
			<LayoutRow>
				<LayoutCol :class="'icon-column'">
					<!-- `dialog.icon` class exists to provide special sizing in CSS to specific icons -->
					<IconLabel :icon="dialog.icon" :class="dialog.icon.toLowerCase()" />
				</LayoutCol>
				<LayoutCol :class="'main-column'">
					<TextLabel :bold="true" :class="'heading'">{{ dialog.heading }}</TextLabel>
					<TextLabel :class="'details'">{{ dialog.details }}</TextLabel>
					<LayoutRow :class="'buttons-row'">
						<TextButton v-for="(button, index) in dialog.buttons" :key="index" :title="button.tooltip" :action="button.callback" v-bind="button.props" />
					</LayoutRow>
				</LayoutCol>
			</LayoutRow>
		</FloatingMenu>
	</div>
</template>

<style lang="scss">
.dialog-modal {
	position: absolute;
	pointer-events: none;
	width: 100%;
	height: 100%;

	.dialog {
		width: 100%;
		height: 100%;
	}

	.floating-menu-container .floating-menu-content {
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
		.heading {
			white-space: pre-wrap;
			max-width: 400px;
			margin-bottom: 4px;
		}

		.details {
			white-space: pre-wrap;
			max-width: 400px;
		}

		.buttons-row {
			margin-top: 16px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { dismissDialog } from "@/state/dialog";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import FloatingMenu, { MenuDirection, MenuType } from "@/components/widgets/floating-menus/FloatingMenu.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";
import TextButton from "@/components/widgets/buttons/TextButton.vue";

export default defineComponent({
	inject: ["dialog"],
	components: {
		LayoutRow,
		LayoutCol,
		FloatingMenu,
		IconLabel,
		TextLabel,
		TextButton,
	},
	methods: {
		dismiss() {
			dismissDialog();
		},
	},
	data() {
		return {
			MenuDirection,
			MenuType,
		};
	},
});
</script>
