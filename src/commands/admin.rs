use poise::serenity_prelude as serenity;
use std::env;
use tracing::error;
use chrono::Utc;
use uuid::Uuid;

use crate::{Context, Error, database::Transaction};
use super::is_admin;

#[poise::command(slash_command)]
pub async fn give(
    ctx: Context<'_>,
    #[description = "User to give coins to"] user: serenity::User,
    #[description = "Amount of coins to give"] amount: i64,
) -> Result<(), Error> {
    let data = &ctx.data();

    // Check if user has admin permissions
    if !is_admin(ctx).await? {
        let admin_role_name = env::var("ADMIN_ROLE_NAME")
            .unwrap_or_else(|_| "Currency Admin".to_string());
        let response = format!(
            "
            You don't have permission to use this command.\n\
            **Required permissions:**\n\
            • '{}' role",
            admin_role_name
        );
        ctx.say(response).await?;
        return Ok(());
    }

    let to_user_id = user.id.to_string();
    let from_user_id = "SYSTEM".to_string();

    // Check if target user is registered
    match data.database.get_user(&to_user_id).await {
        Ok(Some(_)) => {
            // Create a system mint transaction
            let transaction = Transaction {
                id: Uuid::new_v4().to_string(),
                from_user: from_user_id,
                to_user: to_user_id.clone(),
                amount,
                transaction_type: "mint".to_string(),
                message: Some(format!("Admin grant by {}", ctx.author().name)),
                nonce: 0,
                signature: "system".to_string(),
                timestamp_unix: Utc::now().timestamp(),
                created_at: Utc::now(),
            };

            match data.database.add_transaction(&transaction).await {
                Ok(()) => {
                    // Update balance
                    let current_balance = data.database.get_balance(&to_user_id).await.unwrap_or(0);
                    let new_balance = current_balance + amount;

                    match data.database.update_balance(&to_user_id, new_balance).await {
                        Ok(()) => {
                            let response = format!("Gave {} Slumcoins to {}. New balance: {}", amount, user.name, new_balance);
                            ctx.say(response).await?;
                        }
                        Err(e) => {
                            error!("Error updating balance: {}", e);
                            ctx.say("Error updating balance.").await?;
                        }
                    }
                }
                Err(e) => {
                    error!("Error adding transaction: {}", e);
                    ctx.say("Error processing transaction.").await?;
                }
            }
        }
        Ok(None) => {
            ctx.say("Target user is not registered!").await?;
        }
        Err(e) => {
            error!("Database error: {}", e);
            ctx.say("Database error occurred.").await?;
        }
    }

    Ok(())
}
