use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use poise::serenity_prelude as serenity;
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone)]
pub struct AuctionBid {
    pub user_id: serenity::UserId,
    pub game_name: String,
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

    pub fn add_or_update_bid(&mut self, user_id: serenity::UserId, game_name: String) {
        let now = Utc::now();
        
        // Extend the auction if this is a new bid or updated bid
        let should_extend = !self.bids.contains_key(&user_id) || 
                           self.bids.get(&user_id).map_or(false, |b| b.game_name != game_name);
        
        if should_extend {
            // Only extend if we're close to the end (within 30 seconds)
            let time_remaining = self.end_time.signed_duration_since(now).num_seconds();
            if time_remaining < 30 {
                self.end_time = now + Duration::seconds(self.extension_seconds);
            }
        }

        self.bids.insert(user_id, AuctionBid {
            user_id,
            game_name,
            timestamp: now,
        });
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.end_time
    }

    pub fn time_remaining(&self) -> i64 {
        self.end_time.signed_duration_since(Utc::now()).num_seconds().max(0)
    }

    pub fn get_winner(&self) -> Option<(String, Vec<serenity::UserId>)> {
        if self.bids.is_empty() {
            return None;
        }

        // Count votes for each game
        let mut vote_counts: HashMap<String, Vec<serenity::UserId>> = HashMap::new();
        for bid in self.bids.values() {
            vote_counts
                .entry(bid.game_name.clone())
                .or_insert_with(Vec::new)
                .push(bid.user_id);
        }

        // Find the game with the most votes
        vote_counts
            .into_iter()
            .max_by_key(|(_, voters)| voters.len())
            .map(|(game, voters)| (game, voters))
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
            return Err("An auction is already running in this voice channel!".to_string());
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
        game_name: String,
    ) -> Result<(), String> {
        let mut auctions = self.auctions.write().await;

        match auctions.get_mut(&voice_channel_id) {
            Some(auction) => {
                if auction.is_expired() {
                    return Err("This auction has already ended!".to_string());
                }
                auction.add_or_update_bid(user_id, game_name);
                Ok(())
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