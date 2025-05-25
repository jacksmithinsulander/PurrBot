use crate::v1::models::logged_out_buttons::LoggedOutButtons;
use std::error::Error;
use teloxide::{prelude::*, sugar::bot::BotMessagesExt};

pub async fn process_callback(
    bot: Bot,
    q: CallbackQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(data) = q.data.as_deref() {
        // Answer the callback to avoid the loading animation
        bot.answer_callback_query(&q.id).await?;

        // Send the button action response
        if let Some(user_id) = Some(q.from.id) {
            let chat_id = ChatId(user_id.0 as i64);
            LoggedOutButtons::from_str(data)
                .execute(bot, chat_id)
                .await?;
        }
    }

    Ok(())
}
