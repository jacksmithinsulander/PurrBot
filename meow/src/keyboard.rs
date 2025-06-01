use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn logged_out_operations() -> InlineKeyboardMarkup {
    let operations = [
        ("Sign Up", "Sign Up"),
        ("Log In", "Log In"),
        ("FAQ", "FAQ"),
    ];
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    for row in operations.chunks(3) {
        keyboard.push(
            row.iter()
                .map(|&(text, data)| InlineKeyboardButton::callback(text.to_owned(), data.to_owned()))
                .collect(),
        );
    }

    InlineKeyboardMarkup::new(keyboard)
}

pub fn logged_in_operations() -> InlineKeyboardMarkup {
    let operations = [
        ("List", "List"),
        ("Trade", "Trade"),
        ("Create", "Create"),
        ("Log Out", "Log Out"),
        ("Print Keys", "Print Keys"),
    ];
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    for row in operations.chunks(3) {
        keyboard.push(
            row.iter()
                .map(|&(text, data)| InlineKeyboardButton::callback(text.to_owned(), data.to_owned()))
                .collect(),
        );
    }

    InlineKeyboardMarkup::new(keyboard)
}
