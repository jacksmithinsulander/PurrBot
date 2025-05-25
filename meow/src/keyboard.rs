use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn logged_out_operations() -> InlineKeyboardMarkup {
    let operations = ["Sign Up", "Log In", "FAQ"];
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    for row in operations.chunks(3) {
        keyboard.push(
            row.iter()
                .map(|&op| InlineKeyboardButton::callback(op.to_owned(), op.to_owned()))
                .collect(),
        );
    }

    InlineKeyboardMarkup::new(keyboard)
}
