use std::error::Error;
use teloxide::prelude::*;
mod keyboard;
mod v1;

use v1::handlers::{callback_handler, inline_query_handler, message_handler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    log::info!("PurrBot is purring...");

    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().branch(dptree::endpoint(|bot, msg| async move {
            message_handler(bot, msg).await
        })))
        .branch(Update::filter_callback_query().branch(dptree::endpoint(|bot, q| async move {
            callback_handler(bot, q).await
        })));
    //.branch(Update::filter_inline_query().branch(dptree::endpoint(inline_query_handler)));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}
