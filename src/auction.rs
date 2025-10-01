use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use poise::serenity_prelude as serenity;
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone)]
pub struct AuctionBid {
    pub user_id: serenity::UserId,
    pub amount: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Auction {
    pub voice_channel_id: serenity::ChannelId,
    pub creator_id: serenity::UserId,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub bids: HashMap<serenity::UserId, AuctionBid>,
    pub base_duration_seconds: i64,
    pub extension_seconds: i64,
}

impl Auction {
    pub fn new(
        voice_channel_id: serenity::ChannelId,
        creator_id: serenity::UserId,
        base_duration_seconds: i64,
        extension_seconds: i64,
    ) -> Self {
        let start_time = Utc::now();
        let end_time = start_time + Duration::seconds(base_duration_seconds);

        Auction {
            voice_channel_id,
            creator_id,
            start_time,
            end_time,
            bids: HashMap::new(),
            base_duration_seconds,
            extension_seconds,
        }
    }

    pub fn add_or_update_bid(&mut self, user_id: serenity::UserId, amount: i64) -> Result<(), String> {
        let now = Utc::now();
        
        // Check if auction has expired
        if self.is_expired() {
            return Err("This auction has already ended!".to_string());
        }
        
        // Get current highest bid
        let current_highest = self.get_highest_bid_amount();
        
        // Require bid to be higher than current highest (minimum increment of 1)
        if amount <= current_highest {
            return Err(format!("Bid must be higher than current highest bid of {} Slumcoins", current_highest));
        }
        
        // Extend the auction if this is a new bid or higher bid from same user
        let should_extend = !self.bids.contains_key(&user_id) || 
                           self.bids.get(&user_id).map_or(false, |b| b.amount != amount);
        
        if should_extend {
            // Only extend if we're close to the end (within 30 seconds)
            let time_remaining = self.end_time.signed_duration_since(now).num_seconds();
            if time_remaining < 30 {
                self.end_time = now + Duration::seconds(self.extension_seconds);
            }
        }

        self.bids.insert(user_id, AuctionBid {
            user_id,
            amount,
            timestamp: now,
        });
        
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.end_time
    }

    pub fn time_remaining(&self) -> i64 {
        self.end_time.signed_duration_since(Utc::now()).num_seconds().max(0)
    }

    pub fn get_winner(&self) -> Option<(serenity::UserId, i64)> {
        if self.bids.is_empty() {
            return None;
        }

        // Find the highest bidder
        self.bids
            .values()
            .max_by_key(|bid| bid.amount)
            .map(|bid| (bid.user_id, bid.amount))
    }
    
    pub fn get_highest_bid_amount(&self) -> i64 {
        self.bids
            .values()
            .map(|bid| bid.amount)
            .max()
            .unwrap_or(0)
    }
    
    pub fn get_user_bid(&self, user_id: serenity::UserId) -> Option<i64> {
        self.bids.get(&user_id).map(|bid| bid.amount)
    }
}

#[derive(Debug, Clone)]
pub struct AuctionManager {
    // Map of voice channel ID to active auction
    auctions: Arc<RwLock<HashMap<serenity::ChannelId, Auction>>>,
}

impl AuctionManager {
    pub fn new() -> Self {
        AuctionManager {
            auctions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_auction(
        &self,
        voice_channel_id: serenity::ChannelId,
        creator_id: serenity::UserId,
        base_duration_seconds: i64,
        extension_seconds: i64,
    ) -> Result<(), String> {
        let mut auctions = self.auctions.write().await;

        if auctions.contains_key(&voice_channel_id) {
            return Err("auction already running".to_string());
        }

        let auction = Auction::new(
            voice_channel_id,
            creator_id,
            base_duration_seconds,
            extension_seconds,
        );

        auctions.insert(voice_channel_id, auction);
        Ok(())
    }

    pub async fn place_bid(
        &self,
        voice_channel_id: serenity::ChannelId,
        user_id: serenity::UserId,
        amount: i64,
    ) -> Result<(), String> {
        let mut auctions = self.auctions.write().await;

        match auctions.get_mut(&voice_channel_id) {
            Some(auction) => {
                auction.add_or_update_bid(user_id, amount)
            }
            None => Err("No active auction in this voice channel!".to_string()),
        }
    }

    pub async fn get_auction(&self, voice_channel_id: serenity::ChannelId) -> Option<Auction> {
        let auctions = self.auctions.read().await;
        auctions.get(&voice_channel_id).cloned()
    }

    pub async fn end_auction(&self, voice_channel_id: serenity::ChannelId) -> Option<Auction> {
        let mut auctions = self.auctions.write().await;
        auctions.remove(&voice_channel_id)
    }
    
    // Process auction completion and handle coin deduction
    pub async fn process_auction_completion(
        &self, 
        auction: &Auction, 
        database: &crate::database::Database
    ) -> Result<(), String> {
        if let Some((winner_id, winning_amount)) = auction.get_winner() {
            let winner_id_str = winner_id.to_string();
            
            // Get current balance
            match database.get_balance(&winner_id_str).await {
                Ok(current_balance) => {
                    if current_balance >= winning_amount {
                        // Deduct the winning bid from winner's balance
                        let new_balance = current_balance - winning_amount;
                        match database.update_balance(&winner_id_str, new_balance).await {
                            Ok(()) => {
                                // Create transaction record for the auction win
                                let transaction = crate::database::Transaction {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    from_user: winner_id_str,
                                    to_user: "AUCTION_SYSTEM".to_string(),
                                    amount: winning_amount,
                                    transaction_type: "auction_win".to_string(),
                                    message: Some("Auction win deduction".to_string()),
                                    nonce: 0,
                                    signature: "system".to_string(),
                                    timestamp_unix: chrono::Utc::now().timestamp(),
                                    created_at: chrono::Utc::now(),
                                };
                                
                                if let Err(e) = database.add_transaction(&transaction).await {
                                    tracing::error!("Failed to record auction transaction: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to update winner balance: {}", e);
                                return Err("Failed to process auction payment".to_string());
                            }
                        }
                    } else {
                        tracing::warn!("Winner {} has insufficient funds for auction win", winner_id);
                        return Err("Winner has insufficient funds to pay for auction".to_string());
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to get winner balance: {}", e);
                    return Err("Failed to process auction payment".to_string());
                }
            }
        }
        Ok(())
    }

    pub async fn cleanup_expired_auctions(&self) -> Vec<(serenity::ChannelId, Auction)> {
        let mut auctions = self.auctions.write().await;
        let mut expired = Vec::new();

        auctions.retain(|&channel_id, auction| {
            if auction.is_expired() {
                expired.push((channel_id, auction.clone()));
                false
            } else {
                true
            }
        });

        expired
    }
}

impl Default for AuctionManager {
    fn default() -> Self {
        Self::new()
    }
}
