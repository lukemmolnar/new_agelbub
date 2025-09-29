pub mod admin;
pub mod user;
pub mod utility;

use std::env;

use crate::{Context, Error};

/// Check if user is an admin (bot owner, has admin role, or has ADMINISTRATOR permission)
pub async fn is_admin(ctx: Context<'_>) -> Result<bool, Error> {
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
pub async fn can_register_others(ctx: Context<'_>) -> Result<bool, Error> {
    // For now, same as admin check, but could be made more restrictive
    is_admin(ctx).await
}

// Re-export all commands
pub use admin::*;
pub use user::*;
pub use utility::*;
