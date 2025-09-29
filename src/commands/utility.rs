use crate::{Context, Error};

#[poise::command(slash_command)]
pub async fn info(ctx: Context<'_>) -> Result<(), Error> {
    let response = format!(
        "
        • `/register` - Register yourself for Slumcoins\n\
        • `/register @user` - Register another user (admin)\n\
        • `/balance` - Check your Slumcoin balance\n\
        • `/give @user amount` - Give Slumcoins to a user (admin)\n\
        • `/baltop` - Show Slumcoin leaderboard\n\
        • `/info` - Show this message\n\
        "
    );
    ctx.say(response).await?;
    Ok(())
}

// pub async fn is_user_registered(ctx: &Context<'_>, user_id: &str) -> Result<bool, Error> {
//     let data = ctx.data();
//     match data.database.get_user(user_id).await {
//         Ok(Some(_)) => Ok(true),
//         Ok(None) => Ok(false),
//         Err(e) => {
//             error!("Database error checking user registration for {}: {}", user_id, e);
//             Err(e.into())
//         }
//     }
// }

// Check if the command author is registered, and send error message if not
// Returns Ok(true) if registered, Ok(false) if not registered (with error sent)
// pub async fn require_registration(ctx: &Context<'_>) -> Result<bool, Error> {
//     let user_id = ctx.author().id.to_string();
//     match is_user_registered(ctx, &user_id).await {
//         Ok(true) => Ok(true),
//         Ok(false) => {
//             ctx.say("You're not registered! Use `/register` first.").await?;
//             Ok(false)
//         }
//         Err(_) => {
//             ctx.say("Database error occurred.").await?;
//             Ok(false)
//         }
//     }
// }

// Check if a specific user is registered, and send error message if not
// Returns Ok(true) if registered, Ok(false) if not registered (with error sent)
// pub async fn require_user_registration(ctx: &Context<'_>, user: &serenity::User, user_type: &str) -> Result<bool, Error> {
//     let user_id = user.id.to_string();
//     match is_user_registered(ctx, &user_id).await {
//         Ok(true) => Ok(true),
//         Ok(false) => {
//             let message = match user_type {
//                 "recipient" => format!("{} is not registered! They need to use `/register` first.", user.name),
//                 "target" => format!("{} is not registered!", user.name),
//                 _ => format!("{} is not registered!", user.name),
//             };
//             ctx.say(message).await?;
//             Ok(false)
//         }
//         Err(_) => {
//             ctx.say("Database error occurred.").await?;
//             Ok(false)
//         }
//     }
// }