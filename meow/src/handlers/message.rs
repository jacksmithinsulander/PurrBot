use crate::processors::message_processor::process_message;
use crate::services::user_config_store::UserConfigStore;
use std::error::Error;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Message;

pub async fn message_handler(
    bot: Bot,
    msg: Message,
    config_store: Arc<UserConfigStore>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let me = bot.get_me().await?;
    process_message(bot, msg, me, config_store).await
}
