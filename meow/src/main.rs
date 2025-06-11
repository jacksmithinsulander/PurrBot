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
    let transport = if std::env::var("USE_VSOCK").as_deref() == Ok("true") {
        // Parent instance always has CID 3 in Nitro Enclaves
        // The enclave will have a CID assigned (e.g., 16)
        let enclave_cid = std::env::var("ENCLAVE_CID")
            .unwrap_or_else(|_| "16".to_string())
            .parse::<u32>()
            .expect("Invalid ENCLAVE_CID");
        let port = std::env::var("VSOCK_PORT")
            .unwrap_or_else(|_| "5005".to_string())
            .parse::<u32>()
            .expect("Invalid VSOCK_PORT");
        
        log::info!("Using vsock transport to enclave: CID={}, Port={}", enclave_cid, port);
        
        #[cfg(feature = "vsock")]
        {
            Transport::Vsock { cid: enclave_cid, port }
        }
        #[cfg(not(feature = "vsock"))]
        {
            log::error!("vsock feature not enabled!");
            return Err("vsock feature not enabled".into());
        }
    } else {
        let addr = std::env::var("ENCLAVE_ADDRESS")
            .unwrap_or_else(|_| "127.0.0.1:5005".to_string());
        log::info!("Using TCP transport to enclave: {}", addr);
        Transport::Tcp(addr.parse().unwrap())
    };

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
