use teloxide::prelude::*;
use teloxide::types::{Message, Me};
use crate::v1::processors::message_processor::process_message;
use std::error::Error;

pub async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    process_message(bot, msg, me).await
}