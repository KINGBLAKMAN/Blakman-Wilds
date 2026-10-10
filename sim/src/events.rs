//! What happened this tick. Gameplay pushes; HUD, audio, VFX and tests read.

use crate::content::{EnemyKindId, ItemId, RecipeId, Skill};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftError {
    /// Crafting level too low.
    Level,
    /// The station isn't built nearby.
    Station,
    /// Missing ingredients.
    Items,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Jumped,
    Landed { fall_speed: f32 },
    ActionStarted(&'static str),
    ActionEnded(&'static str),
    Hit { target: usize, damage: f32, hp_left: f32 },
    Killed { target: usize },

    // Survival
    ItemGathered { item: ItemId, count: u32 },
    /// The swing bounced off: tool tier or skill level too low (read the node's def for which).
    GatherBlocked { node: usize },
    NodeDepleted { node: usize },
    NodeRespawned { node: usize },
    ItemCrafted { item: ItemId, count: u32 },
    CraftFailed { recipe: RecipeId, reason: CraftError },
    Built { structure: usize, item: ItemId },
    XpGained { skill: Skill, amount: f32 },
    LevelUp { skill: Skill, level: u32 },
    Ate { item: ItemId },
    Starving,
    PlayerHurt { damage: f32, hp_left: f32 },
    PlayerDied,
    PlayerRespawned,
    WolfSpawned { wolf: u32 },
    /// A wolf crouched: its bite lands after the windup.
    WolfTelegraph { wolf: u32 },
    BiteDodged { wolf: u32 },
    WolfHit { wolf: u32, damage: f32, hp_left: f32 },
    EnemyKilled { kind: EnemyKindId },
    NightFell { night: u32 },
    DayBroke { day: u32 },
    NeedFuel,
    BeaconLit,
}
