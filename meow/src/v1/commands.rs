use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase")]
pub enum CommandLoggedOut {
    /// Commands alternatives
    Help,
    /// Start
    Start,
    /// Sign Up
    SignUp { password: String },
    /// Log In
    LogIn { password: String },
    /// Log Out
    LogOut,
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase")]
pub enum CommandLoggedIn {
    /// Commands alternatives
    Help,
    /// Start
    Start,
    /// List
    List,
    /// Trade
    Trade,
    /// Create
    Create,
    /// Log Out
    LogOut,
    /// Print Keys
    PrintKeys,
}
