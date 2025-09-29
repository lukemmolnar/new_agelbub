use poise::serenity_prelude as serenity;
use tracing::error;
use chrono::Utc;

use crate::{Context, Error, database::User};
use super::can_register_others;

#[poise::command(slash_command)]
pub async fn register(
    ctx: Context<'_>,
    #[description = "User to register (admin only)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let data = &ctx.data();
    let (target_user, is_registering_other) = match user {
        Some(mentioned_user) => {
            // Check if user has permission to register others
            if !can_register_others(ctx).await? {
                ctx.say("You don't have permission to register other users.\n\
                        **Required:** Bot owner, Administrator permission, or 'Currency Admin' role").await?;
                return Ok(());
            }
            (mentioned_user, true)
        }
        None => (ctx.author().clone(), false),
    };

    let user_id = target_user.id.to_string();
    let username = target_user.name.clone();

    match data.database.get_user(&user_id).await {
        Ok(Some(_)) => {
            let response = if is_registering_other {
                format!("{} is already registered", username)
            } else {
                "You're already registered!".to_string()
            };
            ctx.say(response).await?;
        }
        Ok(None) => {
            // Generate new keypair for user
            match data.crypto.generate_keypair() {
                Ok((public_key, private_key)) => {
                    // Encrypt private key
                    match data.crypto.encrypt_private_key(&private_key, &user_id) {
                        Ok(encrypted_private_key) => {
                            let user = User {
                                discord_id: user_id.clone(),
                                username: username.clone(),
                                public_key,
                                encrypted_private_key,
                                nonce: 0,
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            };

                            match data.database.create_user(&user).await {
                                Ok(()) => {
                                    let response = if is_registering_other {
                                        format!(
                                            "registered {} successfully. bub boils the seed\n\
                                            Starting balance: 0 coins.\n\
                                            {} can now use `/balance` and receive coins.",
                                            username, username
                                        )
                                    } else {
                                        "Registration successful. bub boils the seed".to_string()
                                    };
                                    ctx.say(response).await?;
                                }
                                Err(e) => {
                                    error!("Database error creating user: {}", e);
                                    ctx.say("Registration failed. Please try again.").await?;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error encrypting private key: {}", e);
                            ctx.say("Registration failed. Please try again.").await?;
                        }
                    }
                }
                Err(e) => {
                    error!("Error generating keypair: {}", e);
                    ctx.say("Registration failed. Please try again.").await?;
                }
            }
        }
        Err(e) => {
            error!("Database error checking user: {}", e);
            ctx.say("Registration failed. Please try again.").await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn balance(ctx: Context<'_>) -> Result<(), Error> {
    let data = &ctx.data();
    let user_id = ctx.author().id.to_string();

    match data.database.get_user(&user_id).await {
        Ok(Some(_)) => {
            match data.database.get_balance(&user_id).await {
                Ok(balance) => {
                    let response = format!("Your balance: {} coins", balance);
                    ctx.say(response).await?;
                }
                Err(e) => {
                    error!("Error getting balance: {}", e);
                    ctx.say("Error retrieving balance.").await?;
                }
            }
        }
        Ok(None) => {
            ctx.say("You're not registered! Use `/register` first.").await?;
        }
        Err(e) => {
            error!("Database error: {}", e);
            ctx.say("Database error occurred.").await?;
        }
    }

    Ok(())
}

// #[poise::command(slash_command)]
// pub async fn send(ctx: Context<'_>) -> Result<(), Error> {
//     let data = &ctx.data();
//     let from_user = ctx.author().id.to_string();
//     let to_user = user.id.to_string();

//     if from_user == to_user {
//         ctx.say("?").await?;
//         return Ok(());
// }

#[poise::command(slash_command)]
pub async fn baltop(ctx: Context<'_>) -> Result<(), Error> {
    let data = &ctx.data();

    match data.database.get_all_users_with_balances(None).await {
        Ok(users_with_balances) => {
            if users_with_balances.is_empty() {
                ctx.say("No registered users found!").await?;
                return Ok(());
            }

            let mut response = "Slumbank Leaderboard\n".to_string();
            
            for (rank, (username, balance)) in users_with_balances.iter().enumerate() {
                
                response.push_str(&format!(
                   "**{}. {} : ``{}``**\n",
                    rank + 1,
                    username,
                    balance
                ));
            }

            ctx.say(response).await?;
        }
        Err(e) => {
            error!("Error getting leaderboard: {}", e);
            ctx.say("Error retrieving leaderboard. Please try again.").await?;
        }
    }

    Ok(())
}
