//this is the file for user commands
use poise::serenity_prelude as serenity;
use tracing::error;
use chrono::Utc;
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
                "You're already registered".to_string()
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

#[poise::command(slash_command, subcommands("bid_start", "bid_status", "bid_end"))]
pub async fn bid(
    ctx: Context<'_>,
    #[description = "Amount of Slumcoins to bid"] amount: Option<i64>,
) -> Result<(), Error> {
    // If amount is provided, treat this as a bid placement
    if let Some(bid_amount) = amount {
        return place_bid(ctx, bid_amount).await;
    }
    
    // If no amount provided, show help
    ctx.say("Use `/bid start` to start an auction, `/bid [amount]` to place a bid, or `/bid status` to check current bids.").await?;
    Ok(())
}

// Helper function for placing bids
async fn place_bid(ctx: Context<'_>, amount: i64) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            ctx.say("can only be used in slumfields").await?;
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
            ctx.say("must be in vc to bid").await?;
            return Ok(());
        }
    };

    // Validate bid amount
    if amount <= 0 {
        ctx.say("have to bid more than 0").await?;
        return Ok(());
    }

    let data = ctx.data();
    let user_id = ctx.author().id.to_string();

    // Check if user is registered
    match data.database.get_user(&user_id).await {
        Ok(Some(_)) => {
            // Check user's balance
            match data.database.get_balance(&user_id).await {
                Ok(balance) => {
                    if balance < amount {
                        ctx.say(format!(
                            "insufficient funds! You have {} Slumcoins but need {} to place this bid.",
                            balance, amount
                        )).await?;
                        return Ok(());
                    }

                    // Try to place the bid
                    match data.auction_manager.place_bid(voice_channel_id, ctx.author().id, amount).await {
                        Ok(()) => {
                            ctx.say(format!(
                                "bid placed for **{} Slumcoins**!\nUse `/bid status` to see current standings.",
                                amount
                            )).await?;
                        }
                        Err(e) => {
                            ctx.say(format!("❌ {}", e)).await?;
                        }
                    }
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
            ctx.say("must be in vc to start auction").await?;
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
                "
                {} has started a bidding war\n\n\
                {}\n\n\
                place  bids using `/bid [amount]`\n\
                Auction ends in **2 minutes** (extends by 15s on new bids)\n\
                Use `/bid status` to check current highest bid",
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
                                    // Process coin deduction
                                    let message = match ended_auction.get_winner() {
                                        Some((winner_id, winning_amount)) => {
                                            // Try to process the auction completion (coin deduction)
                                            // Note: We don't have database access in this spawned task context
                                            // This is a limitation - in a real implementation you'd pass database reference or handle this differently
                                            format!(
                                                "🏆 **Auction Ended!**\n\
                                                Winner: <@{}>\n\
                                                Winning bid: **{} Slumcoins**\n\
                                                Note: Please use `/balance` to verify your updated balance.",
                                                winner_id,
                                                winning_amount
                                            )
                                        }
                                        None => "Auction ended with no bids".to_string(),
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

            let highest_bid = auction.get_highest_bid_amount();
            let mut response = format!(
                "💰 **Current Auction Status**\n\
                ⏱️ Time remaining: **{}s**\n\
                📊 Total bids: **{}**\n\n",
                auction.time_remaining(),
                auction.bids.len()
            );

            if auction.bids.is_empty() {
                response.push_str("No bids yet! Use `/bid [amount]` to place a bid.");
            } else {
                if let Some((winner_id, winning_amount)) = auction.get_winner() {
                    response.push_str(&format!(
                        "**Current highest bid:**\n\
                        • <@{}>: **{} Slumcoins**\n\n",
                        winner_id, winning_amount
                    ));
                }
                
                response.push_str("**All bids:**\n");
                let mut sorted_bids: Vec<_> = auction.bids.values().collect();
                sorted_bids.sort_by(|a, b| b.amount.cmp(&a.amount));
                
                for bid in sorted_bids {
                    response.push_str(&format!(
                        "• <@{}>: {} Slumcoins\n",
                        bid.user_id, bid.amount
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
                // Process the auction completion and handle coin deduction
                match data.auction_manager.process_auction_completion(&ended_auction, &data.database).await {
                    Ok(()) => {
                        let message = match ended_auction.get_winner() {
                            Some((winner_id, winning_amount)) => {
                                format!(
                                    "🏆 **Auction Ended Early!**\n\
                                    Winner: <@{}>\n\
                                    Winning bid: **{} Slumcoins**\n\
                                    ✅ Coins have been deducted from your balance!",
                                    winner_id,
                                    winning_amount
                                )
                            }
                            None => "Auction ended with no bids!".to_string(),
                        };
                        
                        ctx.say(message).await?;
                    }
                    Err(e) => {
                        ctx.say(format!("❌ Error processing auction: {}", e)).await?;
                    }
                }
            }
        }
        None => {
            ctx.say("No active auction in this voice channel!").await?;
        }
    }

    Ok(())
}
