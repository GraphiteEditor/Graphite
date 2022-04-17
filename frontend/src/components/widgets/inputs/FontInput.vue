<template>
	<LayoutRow class="dropdown-input">
		<LayoutRow class="dropdown-box" :class="{ disabled }" :style="{ minWidth: `${minWidth}px` }" @click="() => clickDropdownBox()" data-hover-menu-spawner>
			<span>{{ activeEntry.label }}</span>
			<IconLabel class="dropdown-arrow" :icon="'DropdownArrow'" />
		</LayoutRow>
		<MenuList
			v-model:activeEntry="activeEntry"
			@widthChanged="(newWidth: number) => onWidthChanged(newWidth)"
			:menuEntries="menuEntries"
			:direction="'Bottom'"
			:scrollableY="true"
			ref="menuList"
		/>
	</LayoutRow>
</template>

<style lang="scss">
.dropdown-input {
	position: relative;

	.dropdown-box {
		align-items: center;
		white-space: nowrap;
		background: var(--color-1-nearblack);
		height: 24px;
		border-radius: 2px;

		.dropdown-icon {
			margin: 4px;
			flex: 0 0 auto;
		}

		span {
			margin: 0;
			margin-left: 8px;
			flex: 1 1 100%;
		}

		.dropdown-icon + span {
			margin-left: 0;
		}

		.dropdown-arrow {
			margin: 6px 2px;
			flex: 0 0 auto;
		}

		&:hover,
		&.open {
			background: var(--color-6-lowergray);

			span {
				color: var(--color-f-white);
			}

			svg {
				fill: var(--color-f-white);
			}
		}

		&.open {
			border-radius: 2px 2px 0 0;
		}

		&.disabled {
			background: var(--color-2-mildblack);

			span {
				color: var(--color-8-uppergray);
			}

			svg {
				fill: var(--color-8-uppergray);
			}
		}
	}

	.menu-list .floating-menu-container .floating-menu-content {
		max-height: 400px;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { fontNames, getFontFile, getFontVariants } from "@/utilities/fonts";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import MenuList, { MenuListEntry, SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	emits: ["update:name", "update:variant", "changeFont"],
	props: {
		name: { type: String as PropType<string>, required: true },
		variant: { type: String as PropType<string>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		isVariant: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		const { menuEntries, activeEntry } = this.updateEntries();
		return {
			menuEntries,
			activeEntry,
			minWidth: 0,
		};
	},
	methods: {
		clickDropdownBox() {
			if (!this.disabled) (this.$refs.menuList as typeof MenuList).setOpen();
		},
		selectFont(newName: string) {
			if (this.isVariant) this.$emit("update:variant", newName);
			else this.$emit("update:name", newName);

			{
				const name = this.isVariant ? this.name : newName;
				const variant = this.isVariant ? newName : getFontVariants(newName)[0];
				this.$emit("changeFont", { name, variant, file: getFontFile(name, variant) });
			}
		},
		onWidthChanged(newWidth: number) {
			this.minWidth = newWidth;
		},
		updateEntries(): { menuEntries: SectionsOfMenuListEntries; activeEntry: MenuListEntry } {
			let selectedIndex = -1;
			const menuEntries: SectionsOfMenuListEntries = [
				(this.isVariant ? getFontVariants(this.name) : fontNames()).map((name, index) => {
					if (name === (this.isVariant ? this.variant : this.name)) selectedIndex = index;

					const x: MenuListEntry = {
						label: name,
						action: (): void => this.selectFont(name),
					};
					return x;
				}),
			];

			const activeEntry = selectedIndex < 0 ? { label: "-" } : menuEntries.flat()[selectedIndex];

			return { menuEntries, activeEntry };
		},
	},
	watch: {
		name() {
			const { menuEntries, activeEntry } = this.updateEntries();
			this.menuEntries = menuEntries;
			this.activeEntry = activeEntry;
		},
		variant() {
			const { menuEntries, activeEntry } = this.updateEntries();
			this.menuEntries = menuEntries;
			this.activeEntry = activeEntry;
		},
	},
	components: {
		IconLabel,
		MenuList,
		LayoutRow,
	},
});
</script>
