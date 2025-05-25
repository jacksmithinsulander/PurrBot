use teloxide::prelude::*;
use teloxide::types::InlineQuery;
use crate::v1::processors::inline_processor::process_inline;
use std::error::Error;

pub async fn inline_query_handler(
    bot: Bot,
    q: InlineQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    process_inline(bot, q).await
}
