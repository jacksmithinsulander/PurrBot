use crate::keyboard::logged_out_operations;
use crate::v1::commands::{CommandLoggedIn, CommandLoggedOut};
use crate::v1::models::log_in_state;
use std::error::Error;
use teloxide::{payloads::SendMessageSetters, prelude::*, types::Me, utils::command::BotCommands};

pub async fn process_message(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        match CommandLoggedOut::parse(text, me.username()) {
            Ok(CommandLoggedOut::Help) => {
                bot.send_message(msg.chat.id, CommandLoggedOut::descriptions().to_string())
                    .await?;
            }
            Ok(CommandLoggedOut::Start) => {
                let keyboard = logged_out_operations();
                bot.send_message(msg.chat.id, "💻 gm anon, whatchu wanna do? 🐈")
                    .reply_markup(keyboard)
                    .await?;
            }
            Ok(CommandLoggedOut::SignUp { password }) => {
                log::info!("{} password is {}", msg.chat.id.0, password);
            }
            Ok(CommandLoggedOut::LogIn { password }) => todo!(),
            Err(_) => {
                let states = log_in_state::USER_STATES.lock().await;
                match states
                    .get(&msg.chat.id.0)
                    .copied()
                    .unwrap_or(log_in_state::AwaitingState::None)
                {
                    log_in_state::AwaitingState::AwaitingSignUpPassword => {
                        log::info!("{} is signing up with pass {}", msg.chat.id.0, text);
                        return Ok(());
                    }
                    log_in_state::AwaitingState::AwaitingLoginPassword => {
                        log::info!("{} is logging in with pass {}", msg.chat.id.0, text);
                        return Ok(());
                    }
                    log_in_state::AwaitingState::None => {
                        bot.send_message(msg.chat.id, "Command not found!").await?;
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}
