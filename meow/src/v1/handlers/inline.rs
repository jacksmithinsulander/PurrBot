use crate::v1::processors::inline_processor::process_inline;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::InlineQuery;

pub async fn inline_query_handler(
    bot: Bot,
    q: InlineQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Inline got interacted with");
    process_inline(bot, q).await
}
