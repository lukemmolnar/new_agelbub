use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::env;
use tracing::{error, info};
use chrono::Utc;
use uuid::Uuid;

mod database;
mod crypto;

use database::{Database, User, Transaction};
use crypto::CryptoManager;

struct Handler {
    database: Database,
    crypto: CryptoManager,
}

impl Handler {
    fn new(database: Database, crypto: CryptoManager) -> Self {
        Self { database, crypto }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        // Basic test command
        if msg.content == "!test" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Bot is working").await {
                error!("Error sending message: {why}");
            }
        }

        // Ping command
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong").await {
                error!("Error sending message: {why}");
            }
        }



        // Register command
        if msg.content == "!register" || msg.content.starts_with("!register ") {
            let (user_id, username, is_registering_other) = if let Some(mentioned_user) = msg.mentions.first() {
                // Check if user has admin permissions (you'll need to implement this check)
                // For now, this is a placeholder - replace with your actual admin check
                let is_admin = true; // TODO: Implement actual admin check
                
                if !is_admin {
                    if let Err(why) = msg.channel_id.say(&ctx.http, "You don't have permission to register other users.").await {
                        error!("Error sending message: {why}");
                    }
                    return;
                }
                
                (mentioned_user.id.to_string(), mentioned_user.name.clone(), true)
            } else {
                (msg.author.id.to_string(), msg.author.name.clone(), false)
            };

            match self.database.get_user(&user_id).await {
                Ok(Some(_)) => {
                    let response = if is_registering_other {
                        format!("{} is already registered", username)
                    } else {
                        "You're already registered!".to_string()
                    };
                    if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                        error!("Error sending message: {why}");
                    }
                }
                Ok(None) => {
                    // Generate new keypair for user
                    match self.crypto.generate_keypair() {
                        Ok((public_key, private_key)) => {
                            // Encrypt private key
                            match self.crypto.encrypt_private_key(&private_key, &user_id) {
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

                                    match self.database.create_user(&user).await {
                                        Ok(()) => {
                                            let response = format!(
                                                "Registration successful. bub boils the seed"
                                            );
                                            if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                                                error!("Error sending message: {why}");
                                            }
                                        }
                                        Err(e) => {
                                            error!("Database error creating user: {}", e);
                                            if let Err(why) = msg.channel_id.say(&ctx.http, "Registration failed. Please try again.").await {
                                                error!("Error sending message: {why}");
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error encrypting private key: {}", e);
                                    if let Err(why) = msg.channel_id.say(&ctx.http, "Registration failed. Please try again.").await {
                                        error!("Error sending message: {why}");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error generating keypair: {}", e);
                            if let Err(why) = msg.channel_id.say(&ctx.http, "Registration failed. Please try again.").await {
                                error!("Error sending message: {why}");
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Database error checking user: {}", e);
                    if let Err(why) = msg.channel_id.say(&ctx.http, "Registration failed. Please try again.").await {
                        error!("Error sending message: {why}");
                    }
                }
            }
        }

        // Check balance
        if msg.content == "!balance" {
            let user_id = msg.author.id.to_string();

            match self.database.get_user(&user_id).await {
                Ok(Some(_)) => {
                    match self.database.get_balance(&user_id).await {
                        Ok(balance) => {
                            let response = format!("Your balance: {} coins", balance);
                            if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                                error!("Error sending message: {why}");
                            }
                        }
                        Err(e) => {
                            error!("Error getting balance: {}", e);
                            if let Err(why) = msg.channel_id.say(&ctx.http, "Error retrieving balance.").await {
                                error!("Error sending message: {why}");
                            }
                        }
                    }
                }
                Ok(None) => {
                    if let Err(why) = msg.channel_id.say(&ctx.http, "You're not registered! Use `!register` first.").await {
                        error!("Error sending message: {why}");
                    }
                }
                Err(e) => {
                    error!("Database error: {}", e);
                    if let Err(why) = msg.channel_id.say(&ctx.http, "Database error occurred.").await {
                        error!("Error sending message: {why}");
                    }
                }
            }
        }

        // Admin command to give coins (for testing)
        if msg.content.starts_with("!give ") {
            // Simple admin check - you might want to make this more robust
            let parts: Vec<&str> = msg.content.split_whitespace().collect();
            if parts.len() == 3 {
                if let (Ok(amount), Some(mentioned_user)) = (parts[2].parse::<i64>(), msg.mentions.first()) {
                    let to_user_id = mentioned_user.id.to_string();
                    let from_user_id = "SYSTEM".to_string();

                    // Check if target user is registered
                    match self.database.get_user(&to_user_id).await {
                        Ok(Some(_)) => {
                            // Create a system mint transaction
                            let transaction = Transaction {
                                id: Uuid::new_v4().to_string(),
                                from_user: from_user_id,
                                to_user: to_user_id.clone(),
                                amount,
                                transaction_type: "mint".to_string(),
                                message: Some(format!("Admin grant by {}", msg.author.name)),
                                nonce: 0,
                                signature: "system".to_string(), // System transactions don't need real signatures
                                timestamp_unix: Utc::now().timestamp(),
                                created_at: Utc::now(),
                            };

                            match self.database.add_transaction(&transaction).await {
                                Ok(()) => {
                                    // Update balance
                                    let current_balance = self.database.get_balance(&to_user_id).await.unwrap_or(0);
                                    let new_balance = current_balance + amount;

                                    match self.database.update_balance(&to_user_id, new_balance).await {
                                        Ok(()) => {
                                            let response = format!("Gave {} coins to {}. New balance: {}", amount, mentioned_user.name, new_balance);
                                            if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                                                error!("Error sending message: {why}");
                                            }
                                        }
                                        Err(e) => {
                                            error!("Error updating balance: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error adding transaction: {}", e);
                                    if let Err(why) = msg.channel_id.say(&ctx.http, "Error processing transaction.").await {
                                        error!("Error sending message: {why}");
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            if let Err(why) = msg.channel_id.say(&ctx.http, "Target user is not registered!").await {
                                error!("Error sending message: {why}");
                            }
                        }
                        Err(e) => {
                            error!("Database error: {}", e);
                        }
                    }
                } else {
                    if let Err(why) = msg.channel_id.say(&ctx.http, "Usage: `!give @user amount`").await {
                        error!("Error sending message: {why}");
                    }
                }
            }
        }

        // Bot info command
        if msg.content == "!info" {
            let response = format!(
                "**Available commands:**\n\
                • `!test` - Test if bot is working\n\
                • `!ping` - Ping pong!\n\
                • `!register` - Register for the currency system\n\
                • `!register @user` - Register another user (admin)\n\
                • `!balance` - Check your coin balance\n\
                • `!give @user amount` - Give coins to a user (admin)\n\
                • `!info` - Show this message"
            );

            if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                error!("Error sending message: {why}");
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected and ready!", ready.user.name);
    }
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

    // Set gateway intents - we need MESSAGE_CONTENT to read message content
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new(database, crypto))
        .await
        .expect("Error creating client");

    info!("Starting bot...");

    // Start the client
    if let Err(why) = client.start().await {
        error!("Client error: {why}");
    }
}
