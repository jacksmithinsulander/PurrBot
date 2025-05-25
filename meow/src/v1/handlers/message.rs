use crate::v1::processors::message_processor::process_message;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::{Me, Message};

pub async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    process_message(bot, msg, me).await
}
