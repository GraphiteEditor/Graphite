<template>
	<LayoutRow class="font-input">
		<LayoutRow class="dropdown-box" :class="{ disabled }" :style="{ minWidth: `${minWidth}px` }" @click="() => clickDropdownBox()" data-hover-menu-spawner>
			<span>{{ activeEntry.label }}</span>
			<IconLabel class="dropdown-arrow" :icon="'DropdownArrow'" />
		</LayoutRow>
		<MenuList v-model:activeEntry="activeEntry" @widthChanged="(newWidth: number) => onWidthChanged(newWidth)" :entries="entries" :direction="'Bottom'" :scrollableY="true" ref="menuList" />
	</LayoutRow>
</template>

<style lang="scss">
.font-input {
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

import LayoutRow from "@/components/layout/LayoutRow.vue";
import MenuList, { MenuListEntry, SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	inject: ["fonts"],
	emits: ["update:fontFamily", "update:fontStyle", "changeFont"],
	props: {
		fontFamily: { type: String as PropType<string>, required: true },
		fontStyle: { type: String as PropType<string>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		isStyle: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		const { entries, activeEntry } = this.updateEntries();
		return {
			entries,
			activeEntry,
			minWidth: 0,
		};
	},
	methods: {
		clickDropdownBox() {
			if (!this.disabled) (this.$refs.menuList as typeof MenuList).setOpen();
		},
		async selectFont(newName: string): Promise<void> {
			let fontFamily;
			let fontStyle;

			if (this.isStyle) {
				this.$emit("update:fontStyle", newName);

				fontFamily = this.fontFamily;
				fontStyle = newName;
			} else {
				this.$emit("update:fontFamily", newName);

				fontFamily = newName;
				fontStyle = (await this.fonts).getFontStyles(newName)[0];
			}

			const fontFile = (await this.fonts).getFontFile(fontFamily, fontStyle);
			this.$emit("changeFont", { fontFamily, fontStyle, fontFile });
		},
		onWidthChanged(newWidth: number) {
			this.minWidth = newWidth;
		},
		async updateEntries(): Promise<{ entries: SectionsOfMenuListEntries; activeEntry: MenuListEntry }> {
			const choices = this.isStyle ? (await this.fonts).getFontStyles(this.fontFamily) : (await this.fonts).state.fontNames;
			const selectedChoice = this.isStyle ? this.fontStyle : this.fontFamily;

			let selectedEntry: MenuListEntry | undefined;
			const menuListEntries = choices.map((name) => {
				const result: MenuListEntry = {
					label: name,
					action: async (): Promise<void> => this.selectFont(name),
				};

				if (name === selectedChoice) selectedEntry = result;

				return result;
			});

			const entries: SectionsOfMenuListEntries = [menuListEntries];
			const activeEntry = selectedEntry || { label: "-" };

			return { entries, activeEntry };
		},
	},
	watch: {
		async fontFamily() {
			const { entries, activeEntry } = await this.updateEntries();
			this.entries = entries;
			this.activeEntry = activeEntry;
		},
		async fontStyle() {
			const { entries, activeEntry } = await this.updateEntries();
			this.entries = entries;
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
