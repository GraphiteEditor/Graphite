<template>
	<LayoutCol class="main-window">
		<LayoutRow :class="['header-bar']">
			<HeaderBar />
		</LayoutRow>
		<LayoutRow :class="['panel-container']">
			<PanelArea />
		</LayoutRow>
		<LayoutRow :class="['footer-bar']">
			<FooterBar />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.main-window {
	height: 100%;
}

.header-bar {
	height: 28px;
	flex: 0 0 auto;
}

.panel-container {
	flex: 1 1 100%;
	height: 100%;
}

.footer-bar {
	height: 24px;
	flex: 0 0 auto;
}
</style>

<script lang="ts">
import { Options, Vue } from "vue-class-component";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import HeaderBar from "../header/HeaderBar.vue";
import PanelArea from "../panel-system/PanelArea.vue";
import FooterBar from "../footer/FooterBar.vue";

@Options({
	components: {
		LayoutRow,
		LayoutCol,
		HeaderBar,
		PanelArea,
		FooterBar,
	},
	props: {},
})
export default class MainWindow extends Vue {}
</script>
