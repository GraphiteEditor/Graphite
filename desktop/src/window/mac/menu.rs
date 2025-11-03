use muda::Menu as MudaMenu;
use muda::accelerator::Accelerator;
use muda::{AboutMetadataBuilder, CheckMenuItem, IsMenuItem, MenuEvent, MenuId, MenuItem, MenuItemKind, PredefinedMenuItem, Result, Submenu};

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
		let new_entries = menu_items_from_wrapper(entries);
		let existing_entries = self.inner.items();

		let mut full_replacement_needed = false;

		// Skip first menu (app menu)
		if existing_entries.len() - 1 == new_entries.len() {
			for (old, new) in existing_entries.iter().skip(1).zip(new_entries.iter()) {
				match (old, new) {
					(muda::MenuItemKind::Submenu(old), muda::MenuItemKind::Submenu(new)) => {
						if old.text() != new.text() {
							full_replacement_needed = true;
							break;
						}

						replace_children(old, 0, new.items());
					}
					_ => {
						full_replacement_needed = true;
						break;
					}
				}
			}
		} else {
			full_replacement_needed = true;
		}

		if full_replacement_needed {
			replace_children(&self.inner, 1, new_entries);
		}
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

fn replace_children<'a, T: Into<MenuContainer<'a>>>(menu: T, skip: usize, new_items: Vec<MenuItemKind>) {
	let menu: MenuContainer = menu.into();
	let items = menu.items();
	for item in items.iter().skip(skip) {
		menu.remove(menu_item_kind_to_dyn(item)).unwrap();
	}
	let items = new_items.iter().map(|item| menu_item_kind_to_dyn(item)).collect::<Vec<&dyn IsMenuItem>>();
	menu.append_items(items.as_ref()).unwrap();
}

enum MenuContainer<'a> {
	Menu(&'a MudaMenu),
	Submenu(&'a Submenu),
}
impl<'a> MenuContainer<'a> {
	fn items(&self) -> Vec<MenuItemKind> {
		match self {
			MenuContainer::Menu(menu) => menu.items(),
			MenuContainer::Submenu(submenu) => submenu.items(),
		}
	}

	fn remove(&self, item: &dyn IsMenuItem) -> Result<()> {
		match self {
			MenuContainer::Menu(menu) => menu.remove(item),
			MenuContainer::Submenu(submenu) => submenu.remove(item),
		}
	}

	fn append_items(&self, items: &[&dyn IsMenuItem]) -> Result<()> {
		match self {
			MenuContainer::Menu(menu) => menu.append_items(items),
			MenuContainer::Submenu(submenu) => submenu.append_items(items),
		}
	}
}
impl<'a> From<&'a MudaMenu> for MenuContainer<'a> {
	fn from(menu: &'a MudaMenu) -> Self {
		MenuContainer::Menu(menu)
	}
}
impl<'a> From<&'a Submenu> for MenuContainer<'a> {
	fn from(submenu: &'a Submenu) -> Self {
		MenuContainer::Submenu(submenu)
	}
}
