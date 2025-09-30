//this is the file for user commands
use poise::serenity_prelude as serenity;
use tracing::error;
use chrono::Utc;
use std::collections::HashMap;
use tokio::time::{sleep, Duration as TokioDuration};

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

#[poise::command(slash_command, subcommands("bid_start", "bid_vote", "bid_status", "bid_end"))]
pub async fn bid(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, rename = "start")]
pub async fn bid_start(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            ctx.say("This command can only be used in a server!").await?;
            return Ok(());
        }
    };

    // Get the user's current voice channel
    let voice_channel_id = match ctx.guild() {
        Some(guild) => {
            guild
                .voice_states
                .get(&ctx.author().id)
                .and_then(|vs| vs.channel_id)
        }
        None => None,
    };

    let voice_channel_id = match voice_channel_id {
        Some(id) => id,
        None => {
            ctx.say("You must be in a voice channel to start an auction!").await?;
            return Ok(());
        }
    };

    let data = ctx.data();
    
    // Start the auction (2 minute base, 15 second extensions)
    match data.auction_manager.start_auction(voice_channel_id, ctx.author().id, 120, 15).await {
        Ok(()) => {
            // Get all members in the voice channel
            let members_in_vc = match ctx.http().get_channel(voice_channel_id).await {
                Ok(serenity::Channel::Guild(channel)) => {
                    // Get the guild to access voice states
                    if let Some(guild) = ctx.guild() {
                        let mut members = Vec::new();
                        for (user_id, voice_state) in &guild.voice_states {
                            if voice_state.channel_id == Some(voice_channel_id) {
                                members.push(*user_id);
                            }
                        }
                        members
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };

            // Create mention string for all VC members
            let mentions = if members_in_vc.is_empty() {
                "everyone in the voice channel".to_string()
            } else {
                members_in_vc
                    .iter()
                    .map(|id| format!("<@{}>", id))
                    .collect::<Vec<_>>()
                    .join(" ")
            };

            ctx.say(format!(
                "🎮 **Game Auction Started!**\n\
                {} has started a game auction!\n\n\
                {}\n\n\
                Vote for which game to play using `/bid vote [game name]`\n\
                ⏱️ Auction ends in **2 minutes** (extends by 15s on new votes)\n\
                Use `/bid status` to check current votes",
                ctx.author().name,
                mentions
            )).await?;

            // Spawn a task to auto-end the auction
            let auction_manager = data.auction_manager.clone();
            let ctx_clone = ctx.serenity_context().clone();
            let channel_id = ctx.channel_id();
            
            tokio::spawn(async move {
                // Wait for the auction to expire
                sleep(TokioDuration::from_secs(120)).await;
                
                // Check and handle expired auction
                if let Some(auction) = auction_manager.get_auction(voice_channel_id).await {
                    if auction.is_expired() {
                        if let Some(ended_auction) = auction_manager.end_auction(voice_channel_id).await {
                            // Announce winner
                            let message = match ended_auction.get_winner() {
                                Some((game, voters)) => {
                                    let voter_mentions = voters
                                        .iter()
                                        .map(|id| format!("<@{}>", id))
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    
                                    format!(
                                        "🏆 **Auction Ended!**\n\
                                        Winning game: **{}**\n\
                                        Votes: {} ({})",
                                        game,
                                        voters.len(),
                                        voter_mentions
                                    )
                                }
                                None => "Auction ended with no votes!".to_string(),
                            };
                            
                            let _ = channel_id.say(&ctx_clone.http, message).await;
                        }
                    }
                }
            });
        }
        Err(e) => {
            ctx.say(format!("❌ {}", e)).await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, rename = "vote")]
pub async fn bid_vote(
    ctx: Context<'_>,
    #[description = "The game you want to vote for"] game: String,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            ctx.say("This command can only be used in a server!").await?;
            return Ok(());
        }
    };

    // Get the user's current voice channel
    let voice_channel_id = match ctx.guild() {
        Some(guild) => {
            guild
                .voice_states
                .get(&ctx.author().id)
                .and_then(|vs| vs.channel_id)
        }
        None => None,
    };

    let voice_channel_id = match voice_channel_id {
        Some(id) => id,
        None => {
            ctx.say("You must be in a voice channel to vote!").await?;
            return Ok(());
        }
    };

    let data = ctx.data();

    // Check if there's an active auction in this VC
    match data.auction_manager.place_bid(voice_channel_id, ctx.author().id, game.clone()).await {
        Ok(()) => {
            // Get updated auction info
            if let Some(auction) = data.auction_manager.get_auction(voice_channel_id).await {
                let time_remaining = auction.time_remaining();
                ctx.say(format!(
                    "✅ Vote recorded for **{}**!\n⏱️ Time remaining: **{}s**",
                    game,
                    time_remaining
                )).await?;
            } else {
                ctx.say(format!("✅ Vote recorded for **{}**!", game)).await?;
            }
        }
        Err(e) => {
            ctx.say(format!("❌ {}", e)).await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, rename = "status")]
pub async fn bid_status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            ctx.say("This command can only be used in a server!").await?;
            return Ok(());
        }
    };

    // Get the user's current voice channel
    let voice_channel_id = match ctx.guild() {
        Some(guild) => {
            guild
                .voice_states
                .get(&ctx.author().id)
                .and_then(|vs| vs.channel_id)
        }
        None => None,
    };

    let voice_channel_id = match voice_channel_id {
        Some(id) => id,
        None => {
            ctx.say("You must be in a voice channel to check auction status!").await?;
            return Ok(());
        }
    };

    let data = ctx.data();

    match data.auction_manager.get_auction(voice_channel_id).await {
        Some(auction) => {
            if auction.is_expired() {
                ctx.say("The auction in this voice channel has ended!").await?;
                return Ok(());
            }

            // Count votes for each game
            let mut vote_counts: HashMap<String, Vec<serenity::UserId>> = HashMap::new();
            for bid in auction.bids.values() {
                vote_counts
                    .entry(bid.game_name.clone())
                    .or_insert_with(Vec::new)
                    .push(bid.user_id);
            }

            let mut response = format!(
                "🎮 **Current Auction Status**\n\
                ⏱️ Time remaining: **{}s**\n\
                📊 Total votes: **{}**\n\n",
                auction.time_remaining(),
                auction.bids.len()
            );

            if vote_counts.is_empty() {
                response.push_str("No votes yet! Use `/bid vote [game]` to vote.");
            } else {
                response.push_str("**Current standings:**\n");
                let mut sorted_games: Vec<_> = vote_counts.iter().collect();
                sorted_games.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

                for (game, voters) in sorted_games {
                    response.push_str(&format!(
                        "• **{}**: {} vote{}\n",
                        game,
                        voters.len(),
                        if voters.len() == 1 { "" } else { "s" }
                    ));
                }
            }

            ctx.say(response).await?;
        }
        None => {
            ctx.say("No active auction in this voice channel! Use `/bid start` to begin one.").await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, rename = "end")]
pub async fn bid_end(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            ctx.say("This command can only be used in a server!").await?;
            return Ok(());
        }
    };

    // Get the user's current voice channel
    let voice_channel_id = match ctx.guild() {
        Some(guild) => {
            guild
                .voice_states
                .get(&ctx.author().id)
                .and_then(|vs| vs.channel_id)
        }
        None => None,
    };

    let voice_channel_id = match voice_channel_id {
        Some(id) => id,
        None => {
            ctx.say("You must be in a voice channel to end an auction!").await?;
            return Ok(());
        }
    };

    let data = ctx.data();

    match data.auction_manager.get_auction(voice_channel_id).await {
        Some(auction) => {
            // Only the creator can manually end the auction early
            if auction.creator_id != ctx.author().id {
                ctx.say("Only the auction creator can end it early!").await?;
                return Ok(());
            }

            if let Some(ended_auction) = data.auction_manager.end_auction(voice_channel_id).await {
                let message = match ended_auction.get_winner() {
                    Some((game, voters)) => {
                        let voter_mentions = voters
                            .iter()
                            .map(|id| format!("<@{}>", id))
                            .collect::<Vec<_>>()
                            .join(", ");
                        
                        format!(
                            "🏆 **Auction Ended Early!**\n\
                            Winning game: **{}**\n\
                            Votes: {} ({})",
                            game,
                            voters.len(),
                            voter_mentions
                        )
                    }
                    None => "Auction ended with no votes!".to_string(),
                };
                
                ctx.say(message).await?;
            }
        }
        None => {
            ctx.say("No active auction in this voice channel!").await?;
        }
    }

    Ok(())
}
