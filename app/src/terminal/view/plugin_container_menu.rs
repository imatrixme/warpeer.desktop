use warpeer_plugin_protocol::{ChromeActionSource, ChromeMenuItem, ContainerId, PageId, Revision};

use super::*;
use crate::menu::{MenuItem, MenuItemFields, SubMenu};

impl TerminalView {
    pub(in crate::terminal::view) fn open_plugin_chrome_menu(
        &mut self,
        container_id: ContainerId,
        page_id: PageId,
        revision: Revision,
        action_id: String,
        label: String,
        position_id: String,
        items: &[ChromeMenuItem],
        ctx: &mut ViewContext<Self>,
    ) {
        let menu_items =
            plugin_menu_items(container_id, page_id, revision, &action_id, &label, items);
        self.plugin_hover.action_menu.update(ctx, |menu, ctx| {
            menu.set_items(menu_items, ctx);
        });
        self.plugin_hover.open_action_menu_position = Some(position_id);
        ctx.notify();
    }

    pub(in crate::terminal::view) fn activate_plugin_chrome_action(
        &mut self,
        container_id: ContainerId,
        page_id: PageId,
        revision: Revision,
        action_id: String,
        menu_item_id: Option<String>,
        source: ChromeActionSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.plugin_hover.client.container_action(
            container_id,
            page_id,
            revision,
            action_id,
            menu_item_id,
            source,
        );
        self.plugin_hover.open_action_menu_position = None;
        ctx.notify();
    }
}

fn plugin_menu_items(
    container_id: ContainerId,
    page_id: PageId,
    revision: Revision,
    action_id: &str,
    action_label: &str,
    items: &[ChromeMenuItem],
) -> Vec<MenuItem<TerminalAction>> {
    items
        .iter()
        .map(|item| {
            let mut fields = if item.children.is_empty() {
                MenuItemFields::new(item.label.clone())
            } else {
                MenuItemFields::new_submenu(item.label.clone())
            }
            .with_disabled(!item.enabled);
            if item.selected {
                fields = fields.with_icon(crate::ui_components::icons::Icon::Check);
            }
            if item.children.is_empty() {
                MenuItem::Item(fields.with_on_select_action(
                    TerminalAction::ActivatePluginChromeAction {
                        container_id,
                        page_id,
                        revision,
                        action_id: action_id.to_owned(),
                        menu_item_id: Some(item.id.clone()),
                        label: format!("{action_label}: {}", item.label),
                        source: ChromeActionSource::Menu,
                    },
                ))
            } else {
                MenuItem::Submenu {
                    fields,
                    menu: SubMenu::new(plugin_menu_items(
                        container_id,
                        page_id,
                        revision,
                        action_id,
                        action_label,
                        &item.children,
                    )),
                }
            }
        })
        .collect()
}
