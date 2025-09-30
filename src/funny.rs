use poise::serenity_prelude as serenity;
use tracing::{error};

const TARGET_USER_ID: u64 = 339829749218017281;

pub async fn handle_slumduke_messages(ctx: &serenity::Context, msg: &serenity::Message) {
    if msg.author.id.get() != TARGET_USER_ID {
        return;
    }

    if msg.content.to_lowercase().contains("right agelbub?") {
        if let Err(e) = msg.channel_id.say(&ctx.http, "yes").await {
            error!("Failed to send joke response: {}", e);
        }
    }
}
