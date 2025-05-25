use crate::v1::commands::Command;
use crate::keyboard::logged_out_operations;
use std::error::Error;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    types::Me,
    utils::command::BotCommands,
};


pub async fn process_message(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        match Command::parse(text, me.username()) {
            Ok(Command::Help) => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
            }
            Ok(Command::Start) => {
                let keyboard = logged_out_operations();
                bot.send_message(msg.chat.id, "💻 gm anon, whatchu wanna do? 🐈")
                    .reply_markup(keyboard)
                    .await?;
            }
            Err(_) => {
                bot.send_message(msg.chat.id, "Command not found!").await?;
            }
        }
    }

    Ok(())
}
