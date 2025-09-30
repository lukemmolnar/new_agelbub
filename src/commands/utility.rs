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
