use std::error::Error;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::*,
    sugar::bot::BotMessagesExt,
    types::{
        InlineKeyboardButton, InlineKeyboardMarkup, InlineQueryResultArticle, InputMessageContent,
        InputMessageContentText, Me,
    },
    utils::command::BotCommands,
};

/// These commands are supported:
#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Display this text
    Help,
    /// Start
    Start,
}

enum LoggedOutButtons {
    /// This is for making an account, directly in the TG bot
    LogIn,
    /// This is for logging into an existing account
    SignUp,
    /// $ man PurrBot
    Faq,
    /// Mess up
    UnRecognized,
}

#[allow(dead_code)]
impl LoggedOutButtons {
    pub fn from_str(input: &str) -> Self {
        match input {
            "Log In" => Self::LogIn,
            "Sign Up" => Self::SignUp,
            "FAQ" => Self::Faq,
            _ => Self::UnRecognized,
        }
    }

    /// Returns what should be printed to the user.
    pub fn perform(&self) -> &'static str {
        match self {
            LoggedOutButtons::Faq => MAN_PAGE,
            _ => "Not sure?",
        }
    }
}

static MAN_PAGE: &str = include_str!("../assets/purr.1");

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    log::info!("PurrBot is purring...");

    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler))
        .branch(Update::filter_inline_query().endpoint(inline_query_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

fn logged_out_operations() -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    let logged_out_operations = ["Sign Up", "Log In", "FAQ"];

    for operations in logged_out_operations.chunks(3) {
        let row = operations
            .iter()
            .map(|&operation| {
                InlineKeyboardButton::callback(operation.to_owned(), operation.to_owned())
            })
            .collect();

        keyboard.push(row);
    }

    InlineKeyboardMarkup::new(keyboard)
}

/// Parse the text wrote on Telegram and check if that text is a valid command
/// or not, then match the command. If the command is `/start` it writes a
/// markup with the `InlineKeyboardMarkup`.
async fn message_handler(
    bot: Bot,
    msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        match BotCommands::parse(text, me.username()) {
            Ok(Command::Help) => {
                // Just send the description of all commands.
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
            Ok(Command::Start) => {
                // Create a list of buttons and send them.
                let keyboard = logged_out_operations();
                bot.send_message(msg.chat.id, "Debian versions:")
                    .reply_markup(keyboard)
                    .await?;
            }

            Err(_) => {
                bot.send_message(msg.chat.id, "Command not found!").await?;
            }
        }
    }

    Ok(())
}

async fn inline_query_handler(
    bot: Bot,
    q: InlineQuery,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let choose_debian_version = InlineQueryResultArticle::new(
        "0",
        "Chose debian version",
        InputMessageContent::Text(InputMessageContentText::new("Debian versions:")),
    )
    .reply_markup(logged_out_operations());

    bot.answer_inline_query(q.id, vec![choose_debian_version.into()])
        .await?;

    Ok(())
}

async fn callback_handler(bot: Bot, q: CallbackQuery) -> Result<(), Box<dyn Error + Send + Sync>> {
    // 1. Do we have callback-data?
    if let Some(data) = q.data.as_deref() {
        // 2. Map it to the enum and obtain the reply text.
        let reply_text = LoggedOutButtons::from_str(data).perform();

        // 3. Acknowledge the callback so the hour-glass disappears.
        bot.answer_callback_query(&q.id).await?;

        // 4. Deliver the reply: edit the original message if it exists,
        //    otherwise edit the inline-message.
        if let Some(message) = q.regular_message() {
            bot.edit_text(message, reply_text).await?;
        } else if let Some(inline_id) = q.inline_message_id {
            bot.edit_message_text_inline(inline_id, reply_text).await?;
        }
    }
    Ok(())
}
