use muda::Menu as MudaMenu;
use muda::accelerator::Accelerator;
use muda::{AboutMetadataBuilder, CheckMenuItem, IsMenuItem, MenuEvent, MenuId, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};

use crate::event::{AppEvent, AppEventScheduler};
use crate::wrapper::messages::MenuItem as WrapperMenuItem;

pub(super) struct Menu {
	inner: MudaMenu,
}

impl Menu {
	pub(super) fn new(event_scheduler: AppEventScheduler, app_name: &str) -> Self {
		let about = PredefinedMenuItem::about(None, Some(AboutMetadataBuilder::new().name(Some(app_name)).build()));
		let hide = PredefinedMenuItem::hide(None);
		let hide_others = PredefinedMenuItem::hide_others(None);
		let show_all = PredefinedMenuItem::show_all(None);
		let quit = PredefinedMenuItem::quit(None);
		let app_submenu = Submenu::with_items(
			"",
			true,
			&[&about, &PredefinedMenuItem::separator(), &hide, &hide_others, &show_all, &PredefinedMenuItem::separator(), &quit],
		)
		.unwrap();

		let menu = MudaMenu::new();
		menu.prepend(&app_submenu).unwrap();

		menu.init_for_nsapp();

		MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
			if let Some(id) = menu_id_to_u64(event.id()) {
				event_scheduler.schedule(AppEvent::MenuEvent { id });
			}
		}));

		Menu { inner: menu }
	}

	pub(super) fn update(&self, entries: Vec<WrapperMenuItem>) {
		// remove all items except the first (app menu)
		self.inner.items().iter().skip(1).for_each(|item: &muda::MenuItemKind| {
			self.inner.remove(menu_item_kind_to_dyn(item)).unwrap();
		});

		let items = menu_items_from_wrapper(entries);
		let items = items.iter().map(|item| menu_item_kind_to_dyn(item)).collect::<Vec<&dyn IsMenuItem>>();
		self.inner.append_items(items.as_ref()).unwrap();
	}
}

fn menu_items_from_wrapper(entries: Vec<WrapperMenuItem>) -> Vec<MenuItemKind> {
	let mut menu_items: Vec<MenuItemKind> = Vec::new();
	for entry in entries {
		match entry {
			WrapperMenuItem::Action { id, text, enabled, shortcut } => {
				let id = u64_to_menu_id(id);
				let accelerator = shortcut.map(|s| Accelerator::new(Some(s.modifiers), s.key));
				let item = MenuItem::with_id(id, text, enabled, accelerator);
				menu_items.push(MenuItemKind::MenuItem(item));
			}
			WrapperMenuItem::Checkbox { id, text, enabled, shortcut, checked } => {
				let id = u64_to_menu_id(id);
				let accelerator = shortcut.map(|s| Accelerator::new(Some(s.modifiers), s.key));
				let check = CheckMenuItem::with_id(id, text, enabled, checked, accelerator);
				menu_items.push(MenuItemKind::Check(check));
			}
			WrapperMenuItem::SubMenu { text: name, items, .. } => {
				let items = menu_items_from_wrapper(items);
				let items = items.iter().map(|item| menu_item_kind_to_dyn(item)).collect::<Vec<&dyn IsMenuItem>>();
				let submenu = Submenu::with_items(name, true, &items).unwrap();
				menu_items.push(MenuItemKind::Submenu(submenu));
			}
			WrapperMenuItem::Separator => {
				let separator = PredefinedMenuItem::separator();
				menu_items.push(MenuItemKind::Predefined(separator));
			}
		}
	}
	menu_items
}

fn menu_item_kind_to_dyn(item: &MenuItemKind) -> &dyn IsMenuItem {
	match item {
		MenuItemKind::MenuItem(i) => i,
		MenuItemKind::Submenu(i) => i,
		MenuItemKind::Predefined(i) => i,
		MenuItemKind::Check(i) => i,
		MenuItemKind::Icon(i) => i,
	}
}

fn u64_to_menu_id(id: u64) -> String {
	format!("{id:08x}")
}

fn menu_id_to_u64(id: &MenuId) -> Option<u64> {
	u64::from_str_radix(&id.0, 16).ok()
}
