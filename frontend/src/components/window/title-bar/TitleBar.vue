<template>
	<LayoutRow class="title-bar">
		<LayoutRow class="header-part">
			<WindowButtonsMac :maximized="maximized" v-if="platform === 'Mac'" />
			<MenuBarInput v-if="platform !== 'Mac'" />
		</LayoutRow>
		<LayoutRow class="header-part">
			<WindowTitle :text="`${activeDocumentDisplayName} - Graphite`" />
		</LayoutRow>
		<LayoutRow class="header-part">
			<WindowButtonsWindows :maximized="maximized" v-if="platform === 'Windows' || platform === 'Linux'" />
			<WindowButtonsWeb :maximized="maximized" v-if="platform === 'Web'" />
		</LayoutRow>
	</LayoutRow>
</template>

<style lang="scss">
.title-bar {
	height: 28px;
	flex: 0 0 auto;

	.header-part {
		flex: 1 1 100%;

		&:nth-child(1) {
			justify-content: flex-start;
		}

		&:nth-child(2) {
			justify-content: center;
		}

		&:nth-child(3) {
			justify-content: flex-end;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import MenuBarInput from "@/components/widgets/inputs/MenuBarInput.vue";
import WindowButtonsMac from "@/components/window/title-bar/WindowButtonsMac.vue";
import WindowButtonsWeb from "@/components/window/title-bar/WindowButtonsWeb.vue";
import WindowButtonsWindows from "@/components/window/title-bar/WindowButtonsWindows.vue";
import WindowTitle from "@/components/window/title-bar/WindowTitle.vue";

export type Platform = "Windows" | "Mac" | "Linux" | "Web";

export default defineComponent({
	inject: ["portfolio"],
	props: {
		platform: { type: String as PropType<Platform>, required: true },
		maximized: { type: Boolean as PropType<boolean>, required: true },
	},
	computed: {
		activeDocumentDisplayName() {
			return this.portfolio.state.documents[this.portfolio.state.activeDocumentIndex].displayName;
		},
	},
	components: {
		MenuBarInput,
		WindowTitle,
		WindowButtonsWindows,
		WindowButtonsMac,
		WindowButtonsWeb,
		LayoutRow,
	},
});
</script>
