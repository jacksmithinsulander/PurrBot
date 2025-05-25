use teloxide::utils::command::BotCommands;

#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    /// Display this text
    Help,
    /// Start
    Start,
}
