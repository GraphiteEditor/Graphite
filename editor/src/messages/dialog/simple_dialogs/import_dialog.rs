use crate::messages::frontend::utility_types::ImportDestination;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A simple dialog that asks how to import dropped or selected files
pub struct ImportDialog;

impl DialogLayoutHolder for ImportDialog {
    const ICON: &'static str = "Import";
    const TITLE: &'static str = "Import";

    fn layout_buttons(&self) -> Layout {
        let widgets = vec![TextButton::new("Cancel").on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];
        Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
    }
}

impl LayoutHolder for ImportDialog {
    fn layout(&self) -> Layout {
        let buttons = vec![
            TextButton::new("Import as New Layer")
                .emphasized(true)
                .min_width(220)
                .on_update(|_| {
                    DialogMessage::CloseDialogAndThen {
                        followups: vec![
                            FrontendMessage::TriggerImportWithDestination { destination: ImportDestination::NewLayer }.into(),
                        ],
                    }
                    .into()
                })
                .widget_holder(),
            TextButton::new("Import as New Document")
                .min_width(220)
                .on_update(|_| {
                    DialogMessage::CloseDialogAndThen {
                        followups: vec![
                            FrontendMessage::TriggerImportWithDestination { destination: ImportDestination::NewDocument }.into(),
                        ],
                    }
                    .into()
                })
                .widget_holder(),
        ];

        Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: buttons }]))
    }
}
