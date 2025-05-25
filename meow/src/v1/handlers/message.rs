use crate::v1::processors::message_processor::process_message;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::{Me, Message};

pub async fn message_handler(
    bot: Bot,
    msg: Message,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let me = bot.get_me().await?;
    process_message(bot, msg, me).await
}
