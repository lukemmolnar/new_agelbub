use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};

mod database;
mod crypto;
mod commands;

use database::Database;
use crypto::CryptoManager;
use commands::*;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Debug)]
pub struct Data {
    database: Database,
    crypto: CryptoManager,
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
            commands: vec![register(), balance(), give(), baltop()],
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
