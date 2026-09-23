pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("G8LCc1EkmWZApyYgdYbiUGY6KCETY4q4KwAYo31y7822");

#[program]
pub mod q3_26_core_staking {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, rewards_bps: u16, freeze_period: u16) -> Result<()> {
        ctx.accounts.handle(rewards_bps, freeze_period, &ctx.bumps)
    }
}
