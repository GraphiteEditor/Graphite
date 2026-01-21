use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to select import mode
pub struct ImportDialog;

impl DialogLayoutHolder for ImportDialog {
const ICON: &'static str = "File";
const TITLE: &'static str = "Import";

fn layout_buttons(&self) -> Layout {
let widgets = vec![
TextButton::new("New Document")
.on_update(|_| {
DialogMessage::CloseDialogAndThen {
followups: vec![FrontendMessage::TriggerFileImport { new_document: true }.into()],
}
.into()
})
.widget_instance(),
TextButton::new("New Layer")
.on_update(|_| {
DialogMessage::CloseDialogAndThen {
followups: vec![FrontendMessage::TriggerFileImport { new_document: false }.into()],
}
.into()
})
.widget_instance(),
TextButton::new("Cancel").on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_instance(),
];

Layout(vec![LayoutGroup::Row { widgets }])
}
}

impl LayoutHolder for ImportDialog {
fn layout(&self) -> Layout {
Layout(vec![
LayoutGroup::Row {
widgets: vec![TextLabel::new("How would you like to import the file(s)?").widget_instance()],
},
])
}
}

