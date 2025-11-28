use muda::Menu as MudaMenu;
use muda::accelerator::Accelerator;
use muda::{CheckMenuItem, IsMenuItem, MenuEvent, MenuId, MenuItem, MenuItemKind, PredefinedMenuItem, Result, Submenu};

use crate::event::{AppEvent, AppEventScheduler};
use crate::wrapper::messages::MenuItem as WrapperMenuItem;

pub(super) struct Menu {
	inner: MudaMenu,
}

impl Menu {
	pub(super) fn new(event_scheduler: AppEventScheduler) -> Self {
		// TODO: Remove as much app submenu special handling as possible
		let app_submenu = Submenu::with_items("", true, &[]).unwrap();

		let menu = MudaMenu::new();
		menu.prepend(&app_submenu).unwrap();

		menu.init_for_nsapp();

		MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
			let mtm = objc2::MainThreadMarker::new().expect("only ever called from main thread");
			let is_shortcut_triggered = objc2_app_kit::NSApplication::sharedApplication(mtm)
				.mainMenu()
				.map(|m| m.highlightedItem().is_some())
				.unwrap_or_default();
			if is_shortcut_triggered {
				tracing::error!("A keyboard input triggered a menu event. This is most likely a bug. Please report!");
				return;
			}

			if let Some(id) = menu_id_to_u64(event.id()) {
				event_scheduler.schedule(AppEvent::MenuEvent { id });
			}
		}));

		Menu { inner: menu }
	}

	pub(super) fn update(&self, entries: Vec<WrapperMenuItem>) {
		let new_entries = menu_items_from_wrapper(entries);
		let existing_entries = self.inner.items();

		let mut new_entries_iter = new_entries.iter();
		let mut existing_entries_iter = existing_entries.iter();

		let incremental_update_ok = std::iter::from_fn(move || match (existing_entries_iter.next(), new_entries_iter.next()) {
			(Some(MenuItemKind::Submenu(old)), Some(MenuItemKind::Submenu(new))) if old.text() == new.text() => {
				replace_children(old, new.items());
				Some(true)
			}
			(None, None) => None,
			_ => Some(false),
		})
		.all(|b| b);

		if !incremental_update_ok {
			// Fallback to full replace
			replace_children(&self.inner, new_entries);
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

fn replace_children<'a, T: Into<MenuContainer<'a>>>(menu: T, new_items: Vec<MenuItemKind>) {
	let menu: MenuContainer = menu.into();
	let items = menu.items();
	for item in items.iter() {
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
