use crate::commands::CommandLoggedIn;
use crate::constants::MAN_PAGE;
use crate::keyboard::{logged_in_operations, logged_out_operations};
use crate::models::{log_in_state, password_handler::PasswordHandler};
use crate::processors::message_processor::{logout, print_keys, CHAT_MESSAGE_IDS};
use crate::services::user_config_store::UserConfigStore;
use std::sync::Arc;
use teloxide::prelude::ResponseResult;
use teloxide::prelude::*;
use teloxide::types::{BotCommandScope, MessageId};
use teloxide::utils::command::BotCommands;

/// Represents different types of buttons in the bot interface
#[derive(Debug)]
pub enum Button {
    // Logged in buttons
    List,
    Trade,
    Create,
    LogOut,
    PrintKeys,
    // Logged out buttons
    LogIn,
    SignUp,
    Faq,
    // Unknown button
    UnRecognized,
}

impl Button {
    /// Creates a Button enum from a string input
    pub fn from_str(input: &str, is_logged_in: bool) -> Self {
        if is_logged_in {
            match input {
                "List" => Self::List,
                "Trade" => Self::Trade,
                "Create" => Self::Create,
                "Log Out" => Self::LogOut,
                "Print Keys" => Self::PrintKeys,
                _ => Self::UnRecognized,
            }
        } else {
            match input {
                "Log In" => Self::LogIn,
                "Sign Up" => Self::SignUp,
                "FAQ" => Self::Faq,
                _ => Self::UnRecognized,
            }
        }
    }

    /// Executes the action associated with the button
    pub async fn execute(
        &self,
        bot: Bot,
        chat_id: ChatId,
        config_store: Arc<UserConfigStore>,
        is_logged_in: bool,
    ) -> ResponseResult<()> {
        log::debug!("Executing Button: {:?}", self);

        match self {
            // Logged in buttons
            Button::List => handle_list_button(bot, chat_id).await,
            Button::Trade => handle_trade_button(bot, chat_id).await,
            Button::Create => handle_create_button(bot, chat_id).await,
            Button::LogOut => handle_logout_button(bot, chat_id).await,
            Button::PrintKeys => handle_print_keys_button(bot, chat_id).await,
            // Logged out buttons
            Button::Faq => handle_faq_button(bot, chat_id).await,
            Button::LogIn => handle_login_button(bot, chat_id).await,
            Button::SignUp => handle_signup_button(bot, chat_id, config_store).await,
            Button::UnRecognized => handle_unrecognized_button(bot, chat_id, is_logged_in).await,
        }
    }
}

/// Helper function to handle List button
async fn handle_list_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::debug!("Executing List button");
    let message = bot
        .send_message(chat_id, "📋 Listing your items...")
        .reply_markup(logged_in_operations())
        .await?;
    store_message_id(chat_id, message.id).await;
    log::debug!("List button execution completed");
    Ok(())
}

/// Helper function to handle Trade button
async fn handle_trade_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::debug!("Executing Trade button");
    let message = bot
        .send_message(chat_id, "🔄 Trading interface coming soon...")
        .reply_markup(logged_in_operations())
        .await?;
    store_message_id(chat_id, message.id).await;
    log::debug!("Trade button execution completed");
    Ok(())
}

/// Helper function to handle Create button
async fn handle_create_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::debug!("Executing Create button");
    let message = bot
        .send_message(chat_id, "✨ Create interface coming soon...")
        .reply_markup(logged_in_operations())
        .await?;
    store_message_id(chat_id, message.id).await;
    log::debug!("Create button execution completed");
    Ok(())
}

/// Helper function to handle LogOut button
async fn handle_logout_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::info!("Button::LogOut pressed for chat_id={}", chat_id);
    log::debug!("Executing LogOut button");

    match logout(chat_id, &bot).await {
        Ok(_) => {
            log::debug!("Logout successful");
        }
        Err(e) => {
            log::error!("Logout failed: {}", e);
            let message = bot
                .send_message(chat_id, format!("Failed to logout: {}", e))
                .reply_markup(logged_in_operations())
                .await?;
            store_message_id(chat_id, message.id).await;
        }
    }

    log::info!("Button::LogOut completed for chat_id={}", chat_id);
    log::debug!("LogOut button execution completed");
    Ok(())
}

/// Helper function to handle PrintKeys button
async fn handle_print_keys_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::info!("Button::PrintKeys pressed for chat_id={}", chat_id);
    log::debug!("Executing PrintKeys button");

    match print_keys(chat_id, &bot).await {
        Ok(_) => {
            log::debug!("Print keys successful");
            bot.set_my_commands(CommandLoggedIn::bot_commands())
                .scope(BotCommandScope::Chat {
                    chat_id: chat_id.into(),
                })
                .await?;

            let keyboard = logged_in_operations();
            let message = bot
                .send_message(
                    chat_id,
                    "🔑 Keys printed above. What else would you like to do?",
                )
                .reply_markup(keyboard)
                .await?;
            store_message_id(chat_id, message.id).await;
        }
        Err(e) => {
            log::error!("Print keys failed: {}", e);
            let message = bot
                .send_message(chat_id, format!("Failed to print keys: {}", e))
                .reply_markup(logged_in_operations())
                .await?;
            store_message_id(chat_id, message.id).await;
        }
    }

    log::info!("Button::PrintKeys completed for chat_id={}", chat_id);
    log::debug!("PrintKeys button execution completed");
    Ok(())
}

/// Helper function to handle FAQ button
async fn handle_faq_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::debug!("Executing FAQ button");
    let message = bot
        .send_message(chat_id, MAN_PAGE)
        .reply_markup(logged_out_operations())
        .await?;
    store_message_id(chat_id, message.id).await;
    log::debug!("FAQ button execution completed");
    Ok(())
}

/// Helper function to handle LogIn button
async fn handle_login_button(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    log::debug!("Executing LogIn button");
    let message = bot
        .send_message(chat_id, "Please enter your password:")
        .reply_markup(logged_out_operations())
        .await?;
    store_message_id(chat_id, message.id).await;
    log::debug!("LogIn button execution completed");
    Ok(())
}

/// Helper function to handle SignUp button
async fn handle_signup_button(
    bot: Bot,
    chat_id: ChatId,
    config_store: Arc<UserConfigStore>,
) -> ResponseResult<()> {
    log::debug!("Executing SignUp button");
    let message = bot
        .send_message(chat_id, "Choose your password:")
        .reply_markup(logged_out_operations())
        .await?;
    store_message_id(chat_id, message.id).await;

    if let Err(e) = PasswordHandler::new(config_store.clone()) {
        log::error!("Failed to create password handler: {}", e);
        let error_message = bot
            .send_message(chat_id, "Failed to initialize password handler")
            .reply_markup(logged_out_operations())
            .await?;
        store_message_id(chat_id, error_message.id).await;
        return Ok(());
    }

    let mut states = log_in_state::USER_STATES.lock().await;
    states.insert(
        chat_id.0,
        log_in_state::AwaitingState::AwaitingSignUpPassword,
    );
    log::debug!("SignUp button execution completed");
    Ok(())
}

/// Helper function to handle unrecognized button
async fn handle_unrecognized_button(
    bot: Bot,
    chat_id: ChatId,
    is_logged_in: bool,
) -> ResponseResult<()> {
    log::debug!("Executing unrecognized button");
    let message = bot
        .send_message(chat_id, "❌ Not a valid command")
        .reply_markup(if is_logged_in {
            logged_in_operations()
        } else {
            logged_out_operations()
        })
        .await?;
    store_message_id(chat_id, message.id).await;
    log::debug!("Unrecognized button execution completed");
    Ok(())
}

/// Helper function to store message ID
async fn store_message_id(chat_id: ChatId, message_id: MessageId) {
    let mut chat_message_ids = CHAT_MESSAGE_IDS.lock().await;
    chat_message_ids.insert(chat_id, vec![message_id]);
}
