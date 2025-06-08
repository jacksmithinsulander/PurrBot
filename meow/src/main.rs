use std::error::Error;
use teloxide::{prelude::*, utils::command::BotCommands};
mod keyboard;
mod v1;
use std::sync::Arc;
use v1::services::user_config_store::UserConfigStore;

use nine_sdk::{Transport, connect};
use v1::commands::{CommandLoggedIn, CommandLoggedOut};
use v1::handlers::{callback_handler, message_handler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    log::info!("PurrBot is purring...");

    // Initialize the SQLite user config store
    let config_store = Arc::new(UserConfigStore::new("purrbot.sqlite")?);
    // Make it available globally if needed, or pass to handlers

    // Determine transport based on environment
    let transport = if std::env::var("ENCLAVE_MODE").as_deref() == Ok("enclave") {
        Transport::Tcp("127.0.0.1:5005".parse().unwrap()) // Use TCP for now
    } else {
        Transport::Tcp("127.0.0.1:5005".parse().unwrap())
    };
    log::info!("Using TCP transport");

    let bot = Bot::from_env();

    // Register commands with Telegram
    bot.set_my_commands(CommandLoggedOut::bot_commands())
        .await?;
    log::info!("Commands registered successfully");

    let handler = dptree::entry()
        .branch(Update::filter_message().branch(dptree::endpoint({
            let config_store = Arc::clone(&config_store);
            move |bot, msg| {
                let config_store = Arc::clone(&config_store);
                async move { v1::handlers::message_handler(bot, msg, config_store).await }
            }
        })))
        .branch(Update::filter_callback_query().branch(dptree::endpoint({
            let config_store = Arc::clone(&config_store);
            move |bot, q| {
                let config_store = Arc::clone(&config_store);
                async move { callback_handler(bot, q, config_store).await }
            }
        })));
    //.branch(Update::filter_inline_query().branch(dptree::endpoint(inline_query_handler)));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}
