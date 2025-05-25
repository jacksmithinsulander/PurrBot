use crate::v1::constants::MAN_PAGE;
use nine_sdk::password_handler;

pub enum LoggedOutButtons {
    LogIn,
    SignUp,
    Faq,
    UnRecognized,
}

impl LoggedOutButtons {
    pub fn from_str(input: &str) -> Self {
        match input {
            "Log In" => Self::LogIn,
            "Sign Up" => Self::SignUp,
            "FAQ" => Self::Faq,
            _ => Self::UnRecognized,
        }
    }

    pub fn execute(&self) -> &'static str {
        match self {
            LoggedOutButtons::Faq => MAN_PAGE,
            LoggedOutButtons::LogIn => password_handler(),
            _ => "Not sure?",
        }
    }
}
