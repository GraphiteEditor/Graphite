<template>
	<div class="popover-button">
		<FloatingMenuToggleButton :type="MenuType.Popover" :direction="MenuDirection.Bottom">
			<template #button>
				<IconButton :icon="icon" :size="16" :action="() => {}" />
			</template>
			<template #menu>
				<slot></slot>
			</template>
		</FloatingMenuToggleButton>
	</div>
</template>

<style lang="scss">
.popover-button {
	display: inline-block;
	position: relative;
	width: 16px;
	height: 24px;
	flex: 0 0 auto;

	.floating-menu {
		left: 50%;
	}

	.icon-button {
		width: 100%;
		height: 100%;
		padding: 0;
		outline: none;
		border: none;
		border-radius: 2px;
		vertical-align: top;
		background: var(--color-1-nearblack);
		fill: var(--color-e-nearwhite);

		&:hover {
			background: var(--color-6-lowergray);
			fill: var(--color-f-white);
		}
	}

	// TODO: Refactor this and other complicated cases dealing with joined widget margins and border-radius by adding a single standard set of classes: joined-first, joined-inner, and joined-last
	div[class*="-input"] + & {
		margin-left: 1px;

		.icon-button {
			border-radius: 0 2px 2px 0;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import IconButton from "@/components/widgets/buttons/IconButton.vue";
import { MenuDirection, MenuType } from "@/components/widgets/floating-menus/FloatingMenu.vue";
import FloatingMenuToggleButton from "@/components/widgets/floating-menus/FloatingMenuToggleButton.vue";

export enum PopoverButtonIcon {
	"DropdownArrow" = "DropdownArrow",
	"VerticalEllipsis" = "VerticalEllipsis",
}

export default defineComponent({
	components: {
		FloatingMenuToggleButton,
		IconButton,
	},
	props: {
		action: { type: Function, required: false },
		icon: { type: String, default: PopoverButtonIcon.DropdownArrow },
	},
	data() {
		return {
			MenuDirection,
			MenuType,
		};
	},
});
</script>
