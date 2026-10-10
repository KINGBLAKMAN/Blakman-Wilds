//! Game content as data: items, recipes, resource nodes, enemies, tunables and the island layout.
//!
//! The file is `data/content.ron`, embedded at build time so the sim stays self-contained and
//! deterministic. Ids are strings in the file and indices at runtime: `load` resolves every
//! reference once and fails with a message naming the bad id, so gameplay code never looks up a
//! string in the hot path.

use serde::Deserialize;

pub const EMBEDDED: &str = include_str!("../../data/content.ron");

pub type ItemId = usize;
pub type RecipeId = usize;
pub type NodeKindId = usize;
pub type EnemyKindId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ToolKind {
    Axe,
    Pickaxe,
    Weapon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Skill {
    Woodcutting,
    Mining,
    Crafting,
    Combat,
}

impl Skill {
    pub const ALL: [Skill; 4] = [Skill::Woodcutting, Skill::Mining, Skill::Crafting, Skill::Combat];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Skill::Woodcutting => "Woodcutting",
            Skill::Mining => "Mining",
            Skill::Crafting => "Crafting",
            Skill::Combat => "Combat",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum ItemKind {
    Material,
    Food { hunger: f32, heal: f32 },
    Tool { kind: ToolKind, tier: u32, power: f32 },
    /// Built in the world; `radius` is its solid footprint.
    Structure { model: String, radius: f32 },
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RecipeDef {
    pub output: String,
    pub count: u32,
    pub inputs: Vec<(String, u32)>,
    pub station: Option<String>,
    /// Crafting level needed.
    pub level: u32,
    pub xp: f32,
    #[serde(skip)]
    pub out: ItemId,
    #[serde(skip)]
    pub ins: Vec<(ItemId, u32)>,
    #[serde(skip)]
    pub station_item: Option<ItemId>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NodeDef {
    pub id: String,
    pub model: String,
    pub depleted_model: Option<String>,
    pub drop: String,
    pub base_yield: u32,
    pub charges: u32,
    pub respawn: f32,
    pub tool: Option<ToolKind>,
    /// Tool tier needed (0 = bare hands work).
    pub min_tier: u32,
    /// Skill level needed.
    pub level: u32,
    pub skill: Option<Skill>,
    pub xp: f32,
    pub radius: f32,
    pub height: f32,
    #[serde(skip)]
    pub drop_item: ItemId,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnemyDef {
    pub id: String,
    pub hp: f32,
    pub radius: f32,
    pub wander_speed: f32,
    pub chase_speed: f32,
    pub sight: f32,
    pub bite_range: f32,
    /// Telegraph: seconds the wolf crouches before the bite lands.
    pub windup: f32,
    pub bite_damage: f32,
    pub recover: f32,
    pub xp: f32,
    pub drops: Vec<(String, u32)>,
    #[serde(skip)]
    pub drop_items: Vec<(ItemId, u32)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Tunables {
    pub day_length: f32,
    pub start_time: f32,
    pub night_start: f32,
    pub night_end: f32,
    pub health_max: f32,
    pub hunger_max: f32,
    pub hunger_drain: f32,
    pub starve_damage: f32,
    pub regen_hunger_min: f32,
    pub health_regen: f32,
    pub respawn_delay: f32,
    pub respawn_hunger: f32,
    pub station_radius: f32,
    pub interact_range: f32,
    pub place_distance: f32,
    pub unarmed_damage: f32,
    pub combat_damage_per_level: f32,
    pub yield_levels_per_bonus: u32,
    pub xp_base: f32,
    pub max_level: u32,
    pub wolves_first_night: u32,
    pub wolves_per_night: u32,
    pub beacon_fuel: String,
    pub beacon: String,
    pub node_radius_scale: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Layout {
    pub half_size: f32,
    pub camp: (f32, f32),
    pub wolf_spawns: Vec<(f32, f32)>,
    pub nodes: Vec<(String, f32, f32)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Content {
    pub tunables: Tunables,
    pub items: Vec<ItemDef>,
    pub recipes: Vec<RecipeDef>,
    pub nodes: Vec<NodeDef>,
    pub enemies: Vec<EnemyDef>,
    pub layout: Layout,
    #[serde(skip)]
    pub fuel: ItemId,
    #[serde(skip)]
    pub beacon: ItemId,
}

impl Default for Content {
    fn default() -> Self {
        Content::load(EMBEDDED).unwrap_or_else(|e| panic!("data/content.ron: {e}"))
    }
}

impl Content {
    /// Parse and resolve every id reference. Errors name the file position or the bad id.
    pub fn load(text: &str) -> Result<Content, String> {
        let mut c: Content = ron::from_str(text).map_err(|e| e.to_string())?;
        for r in &mut c.recipes {
            r.out = find(&c.items, &r.output, "recipe output")?;
            r.ins = r.inputs.iter().map(|(id, n)| Ok((find(&c.items, id, "recipe input")?, *n))).collect::<Result<_, String>>()?;
            r.station_item = r.station.as_ref().map(|s| find(&c.items, s, "recipe station")).transpose()?;
        }
        for n in &mut c.nodes {
            n.drop_item = find(&c.items, &n.drop, "node drop")?;
        }
        for e in &mut c.enemies {
            e.drop_items = e.drops.iter().map(|(id, n)| Ok((find(&c.items, id, "enemy drop")?, *n))).collect::<Result<_, String>>()?;
        }
        for (id, _, _) in &c.layout.nodes {
            c.node_kind(id).ok_or_else(|| format!("layout: unknown node '{id}'"))?;
        }
        c.fuel = find(&c.items, &c.tunables.beacon_fuel, "beacon_fuel")?;
        c.beacon = find(&c.items, &c.tunables.beacon, "beacon")?;
        Ok(c)
    }

    pub fn item(&self, id: &str) -> Option<ItemId> {
        self.items.iter().position(|i| i.id == id)
    }

    pub fn recipe(&self, output: &str) -> Option<RecipeId> {
        self.recipes.iter().position(|r| r.output == output)
    }

    pub fn node_kind(&self, id: &str) -> Option<NodeKindId> {
        self.nodes.iter().position(|n| n.id == id)
    }

    pub fn enemy_kind(&self, id: &str) -> Option<EnemyKindId> {
        self.enemies.iter().position(|e| e.id == id)
    }

    /// Total XP needed to reach `level`.
    pub fn xp_for_level(&self, level: u32) -> f32 {
        self.tunables.xp_base * (level * level.saturating_sub(1)) as f32 / 2.0
    }

    pub fn level_for_xp(&self, xp: f32) -> u32 {
        let mut level = 1;
        while level < self.tunables.max_level && xp >= self.xp_for_level(level + 1) {
            level += 1;
        }
        level
    }
}

fn find(items: &[ItemDef], id: &str, what: &str) -> Result<ItemId, String> {
    items.iter().position(|i| i.id == id).ok_or_else(|| format!("{what}: unknown item '{id}'"))
}

impl Content {
    /// Footprint radius of a structure item (0 for anything else).
    pub fn footprint(&self, item: ItemId) -> f32 {
        match self.items[item].kind {
            ItemKind::Structure { radius, .. } => radius,
            _ => 0.0,
        }
    }
}
