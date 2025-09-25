use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};
use chrono::Utc;
use uuid::Uuid;

mod database;
mod crypto;

use database::{Database, User, Transaction};
use crypto::CryptoManager;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Debug)]
pub struct Data {
    database: Database,
    crypto: CryptoManager,
}

/// Check if user is an admin (bot owner, has admin role, or has ADMINISTRATOR permission)
async fn is_admin(ctx: Context<'_>) -> Result<bool, Error> {
    let user_id = ctx.author().id;
    
    // Check if user is bot application owner
    if let Ok(app_info) = ctx.http().get_current_application_info().await {
        if let Some(owner) = &app_info.owner {
            if owner.id == user_id {
                return Ok(true);
            }
        }
    }
    
    // Check if we're in a guild (server)
    if let Some(guild_id) = ctx.guild_id() {
        // Check if user has ADMINISTRATOR permission
        if let Some(member) = ctx.author_member().await {
            if let Ok(perms) = member.permissions(&ctx.cache()) {
                if perms.administrator() {
                    return Ok(true);
                }
            }
        }
        
        // Check for admin role (configurable via environment variable)
        let admin_role_name = env::var("ADMIN_ROLE_NAME")
            .unwrap_or_else(|_| "Currency Admin".to_string());
            
        if let Ok(guild) = guild_id.to_partial_guild(&ctx.http()).await {
            if let Ok(member) = guild.member(&ctx.http(), user_id).await {
                for role_id in &member.roles {
                    if let Some(role) = guild.roles.get(role_id) {
                        if role.name == admin_role_name {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }
    
    Ok(false)
}

/// Check if user can register others (stricter admin check)
async fn can_register_others(ctx: Context<'_>) -> Result<bool, Error> {
    // For now, same as admin check, but could be made more restrictive
    is_admin(ctx).await
}

#[poise::command(slash_command, prefix_command)]
async fn test(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Bot is working!").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Pong!").await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn register(
    ctx: Context<'_>,
    #[description = "User to register (admin only)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let data = &ctx.data();
    let (target_user, is_registering_other) = match user {
        Some(mentioned_user) => {
            // Check if user has permission to register others
            if !can_register_others(ctx).await? {
                ctx.say("You don't have permission to register other users.\n\
                        ").await?;
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
async fn balance(ctx: Context<'_>) -> Result<(), Error> {
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

#[poise::command(slash_command)]
async fn give(
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
            \n\
            **Required permissions (any of the following):**\n\
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
                            let response = format!("Gave {} coins to {}. New balance: {}", amount, user.name, new_balance);
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

#[poise::command(slash_command)]
async fn info(ctx: Context<'_>) -> Result<(), Error> {
    let response = format!(
        "
        • `/register`\n\
        • `/register @user` - Register another user (admin)\n\
        • `/balance` - Check your Slumcoin balance\n\
        • `/give @user amount` - Give Slumcoins to a user (admin)\n\
        • `/baltop` - Show Slumcoin leaderboard\n\
        • `/info` Show this message\n\
        "
    );
    ctx.say(response).await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn baltop(ctx: Context<'_>) -> Result<(), Error> {
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

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Initialize the logger
    tracing_subscriber::fmt::init();

    // Get the Discord token from environment
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in environment");

    // Get database URL from environment or use default
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:currency.db".to_string());

    // Initialize database
    let database = Database::new(&database_url)
        .await
        .expect("Failed to connect to database");

    // Initialize crypto manager
    let crypto_key = env::var("CRYPTO_MASTER_KEY")
        .unwrap_or_else(|_| "default_dev_key_change_in_production".to_string());

    let crypto = CryptoManager::new(&crypto_key)
        .expect("Failed to initialize crypto manager");

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![test(), ping(), register(), balance(), give(), info(), baltop()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            on_error: |error| Box::pin(async move {
                match error {
                    poise::FrameworkError::Command { error, ctx, .. } => {
                        error!("Error in command '{}': {}", ctx.command().name, error);
                    }
                    poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
                        if let Some(error) = error {
                            error!("Command check failed for '{}': {}", ctx.command().name, error);
                        } else {
                            // This is a permission check failure - send a user-friendly message
                            let admin_role_name = std::env::var("ADMIN_ROLE_NAME")
                                .unwrap_or_else(|_| "Currency Admin".to_string());
                            let response = format!(
                                "
                                You don't have permission to use this command.\n\
                                \n\
                                **Required permissions (any of the following):**\n\
                                • '{}' role",
                                admin_role_name
                            );
                            if let Err(e) = ctx.say(response).await {
                                error!("Failed to send permission denied message: {}", e);
                            }
                        }
                    }
                    error => {
                        error!("Framework error: {:?}", error);
                    }
                }
            }),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { database, crypto })
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    info!("Starting bot...");

    client.unwrap().start().await.unwrap();
}
