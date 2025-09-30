use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};

mod database;
mod crypto;
mod commands;
mod funny;
mod auction;

use database::Database;
use crypto::CryptoManager;
use auction::AuctionManager;
use commands::*;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Debug)]
pub struct Data {
    database: Database,
    crypto: CryptoManager,
    auction_manager: AuctionManager
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in environment");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:currency.db".to_string());

    let database = Database::new(&database_url)
        .await
        .expect("Failed to connect to database");

    let crypto_key = env::var("CRYPTO_MASTER_KEY")
        .unwrap_or_else(|_| "default_dev_key_change_in_production".to_string());

    let crypto = CryptoManager::new(&crypto_key)
        .expect("Failed to initialize crypto manager");

    let auction_manager = AuctionManager::new();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![register(), balance(), give(), baltop()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            event_handler: |ctx, event, _framework, _data| {
                Box::pin(async move {
                    match event {
                        poise::serenity_prelude::FullEvent::Message { new_message } => {
                            // ignore agelbub messages to prevent loops
                            if !new_message.author.bot {
                                funny::handle_slumduke_messages(ctx, new_message).await;
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                })
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
                            let admin_role_name = std::env::var("ADMIN_ROLE_NAME")
                                .unwrap_or_else(|_| "Slumbanker".to_string());
                            let response = format!(
                                "
                                Required permissions:\n\
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
                let guild_id = serenity::GuildId::new(1078723086448349365);
                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
                                
                info!("registered commands to Slumfields {}", guild_id);
                
                Ok(Data { database, crypto, auction_manager })
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged() 
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILDS           
        | serenity::GatewayIntents::GUILD_VOICE_STATES;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    info!("Agelbub online");

    client.unwrap().start().await.unwrap();
}
