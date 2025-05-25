use teloxide::prelude::*;
use teloxide::types::{InlineQuery, InputMessageContent, InputMessageContentText, InlineQueryResultArticle};
use crate::keyboard::logged_out_operations;
use std::error::Error;

pub async fn process_inline(
    bot: Bot,
    q: InlineQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let article = InlineQueryResultArticle::new(
        "0",
        "💻 gm anon, whatchu wanna do? 🐈",
        InputMessageContent::Text(InputMessageContentText::new(
            "💻 gm anon, whatchu wanna do? 🐈",
        )),
    )
    .reply_markup(logged_out_operations());

    bot.answer_inline_query(q.id, vec![article.into()]).await?;

    Ok(())
}
