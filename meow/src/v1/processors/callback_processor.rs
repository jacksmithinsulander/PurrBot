use crate::v1::models::logged_out_buttons::LoggedOutButtons;
use std::error::Error;
use teloxide::{prelude::*,sugar::bot::BotMessagesExt};


pub async fn process_callback(
    bot: Bot,
    q: CallbackQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(data) = q.data.as_deref() {
        let reply_text = LoggedOutButtons::from_str(data).execute();

        bot.answer_callback_query(&q.id).await?;

        if let Some(message) = q.regular_message() {
            bot.edit_text(message, reply_text).await?;
        } else if let Some(inline_id) = q.inline_message_id {
            bot.edit_message_text_inline(inline_id, reply_text).await?;
        }
    }

    Ok(())
}
