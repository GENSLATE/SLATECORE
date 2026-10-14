//! The message box for a start-up failure (`rfd`). A plain message box does not touch the
//! common-dialog "recent" lists the way file pickers do.

use rfd::{MessageButtons, MessageDialog, MessageLevel};

use crate::fatal::{self, Fatal};

/// Shows `fatal` in an error message box and waits for it to be closed.
pub fn show(fatal: &Fatal) {
    let message = fatal::message(fatal);
    MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(message.title)
        .set_description(message.body)
        .set_buttons(MessageButtons::Ok)
        .show();
}
