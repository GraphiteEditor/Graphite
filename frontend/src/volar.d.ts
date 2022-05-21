import { type RGBA as RGBA_ } from "@/interop/messages";

import FloatingMenu from "@/components/widgets/floating-menus/FloatingMenu.vue";
import MenuList from "@/components/widgets/floating-menus/MenuList.vue";

// TODO: When a Volar bug is fixed (likely in v0.34.16):
// TODO: - Uncomment this block
// TODO: - Remove the `MenuList` and `FloatingMenu` lines from the `declare global` section below
// TODO: - And possibly add the empty export line of code `export {};` to the bottom of this file, for some reason
// declare module "vue" {
// 	interface ComponentCustomProperties {
// 		const MenuList: MenuList;
// 		const FloatingMenu: FloatingMenu;
// 	}
// }

// Satisfies Volar
// TODO: Move this back into `DropdownInput.vue` and `SwatchPairInput.vue` after https://github.com/johnsoncodehk/volar/issues/1321 is fixed
declare global {
	const MenuList: MenuList;
	const FloatingMenu: FloatingMenu;
	type RGBA = RGBA_;
}
