use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn logged_out_operations() -> InlineKeyboardMarkup {
    let operations = [("Sign Up", "Sign Up"), ("Log In", "Log In"), ("FAQ", "FAQ")];
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    for row in operations.chunks(3) {
        keyboard.push(
            row.iter()
                .map(|&(text, data)| {
                    InlineKeyboardButton::callback(text.to_owned(), data.to_owned())
                })
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
                .map(|&(text, data)| {
                    InlineKeyboardButton::callback(text.to_owned(), data.to_owned())
                })
                .collect(),
        );
    }

    InlineKeyboardMarkup::new(keyboard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logged_out_operations_not_default() {
        let keyboard = logged_out_operations();
        let default_keyboard = InlineKeyboardMarkup::default();
        
        // Verify it's not the default (empty) keyboard
        assert!(!keyboard.inline_keyboard.is_empty(), "logged_out_operations should not return an empty keyboard");
        assert_ne!(keyboard.inline_keyboard.len(), default_keyboard.inline_keyboard.len());
    }

    #[test]
    fn test_logged_in_operations_not_default() {
        let keyboard = logged_in_operations();
        let default_keyboard = InlineKeyboardMarkup::default();
        
        // Verify it's not the default (empty) keyboard
        assert!(!keyboard.inline_keyboard.is_empty(), "logged_in_operations should not return an empty keyboard");
        assert_ne!(keyboard.inline_keyboard.len(), default_keyboard.inline_keyboard.len());
    }

    #[test]
    fn test_logged_out_operations_structure() {
        let keyboard = logged_out_operations();
        
        // Test that we have the expected structure
        assert_eq!(keyboard.inline_keyboard.len(), 1, "Should have 1 row");
        assert_eq!(keyboard.inline_keyboard[0].len(), 3, "Should have 3 buttons");
        
        // Verify button texts
        let row = &keyboard.inline_keyboard[0];
        assert_eq!(row[0].text, "Sign Up");
        assert_eq!(row[1].text, "Log In");
        assert_eq!(row[2].text, "FAQ");
    }

    #[test]
    fn test_logged_in_operations_structure() {
        let keyboard = logged_in_operations();
        
        // Test that we have the expected structure
        assert_eq!(keyboard.inline_keyboard.len(), 2, "Should have 2 rows");
        assert_eq!(keyboard.inline_keyboard[0].len(), 3, "First row should have 3 buttons");
        assert_eq!(keyboard.inline_keyboard[1].len(), 2, "Second row should have 2 buttons");
        
        // Verify button texts
        let row1 = &keyboard.inline_keyboard[0];
        assert_eq!(row1[0].text, "List");
        assert_eq!(row1[1].text, "Trade");
        assert_eq!(row1[2].text, "Create");
        
        let row2 = &keyboard.inline_keyboard[1];
        assert_eq!(row2[0].text, "Log Out");
        assert_eq!(row2[1].text, "Print Keys");
    }
}
