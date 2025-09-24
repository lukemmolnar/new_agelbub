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

pub struct Data {
    database: Database,
    crypto: CryptoManager,
}

#[poise::command(slash_command)]
async fn register(
    ctx: Context<'_>,
    #[description = "User to register (admin only)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let data = &ctx.data();
    let (target_user, is_registering_other) = match user {
        Some(mentioned_user) => {
            // TODO: Replace with actual admin check
            let is_admin = true;
            
            if !is_admin {
                ctx.say("You don't have permission to register other users.").await?;
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
    
    // TODO: Replace with actual admin check
    let is_admin = true;
    
    if !is_admin {
        ctx.say("You don't have permission to give coins.").await?;
        return Ok(());
    }

    if amount <= 0 {
        ctx.say("Amount must be positive.").await?;
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
        • `/info` or `!info` - Show this message\n\
        "
    );
    ctx.say(response).await?;
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
            commands: vec![register(), balance(), give(), info()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
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
