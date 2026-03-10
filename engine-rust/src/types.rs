/// Core type definitions for the MTG engine.
/// All IDs are simple integers for fast cloning and cache-friendly access.

/// Unique identifier for game objects (cards, permanents, abilities on stack).
/// Uses u32 for compact state representation.
pub type ObjectId = u32;

/// Player identifier. Supports up to 256 players (2 for typical games).
pub type PlayerId = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl Color {
    pub const ALL: [Color; 5] = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Stack,
    Exile,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    Beginning,
    PreCombatMain,
    Combat,
    PostCombatMain,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Untap,
    Upkeep,
    Draw,
    // Main phase has no steps
    BeginCombat,
    DeclareAttackers,
    DeclareBlockers,
    FirstStrikeDamage,
    CombatDamage,
    EndOfCombat,
    End,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardType {
    Land,
    Creature,
    Artifact,
    Enchantment,
    Instant,
    Sorcery,
    Planeswalker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuperType {
    Basic,
    Legendary,
    Snow,
}

/// Keyword abilities that affect game rules directly.
/// Using a bitfield representation for fast checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Keyword {
    Flying = 1 << 0,
    FirstStrike = 1 << 1,
    DoubleStrike = 1 << 2,
    Deathtouch = 1 << 3,
    Lifelink = 1 << 4,
    Vigilance = 1 << 5,
    Trample = 1 << 6,
    Haste = 1 << 7,
    Hexproof = 1 << 8,
    Indestructible = 1 << 9,
    Flash = 1 << 10,
    Menace = 1 << 11,
    Reach = 1 << 12,
    Defender = 1 << 13,
    Protection = 1 << 14, // simplified - full protection needs color/type info
    Shroud = 1 << 15,
    Prowess = 1 << 16,
    Ward = 1 << 17,
    Convoke = 1 << 18,
    Storm = 1 << 19,
    Cascade = 1 << 20,
    Dredge = 1 << 21,
}

/// Compact bitfield for keyword abilities on a permanent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Keywords(pub u32);

impl Keywords {
    pub fn has(self, kw: Keyword) -> bool {
        self.0 & (kw as u32) != 0
    }

    pub fn add(&mut self, kw: Keyword) {
        self.0 |= kw as u32;
    }

    pub fn remove(&mut self, kw: Keyword) {
        self.0 &= !(kw as u32);
    }

    pub fn empty() -> Self {
        Keywords(0)
    }
}

/// Counter types that can be placed on permanents or players.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CounterType {
    PlusOnePlusOne,
    MinusOneMinusOne,
    Loyalty,
    Charge,
    Time,
    Fade,
    Poison,
}

/// Represents a target for a spell or ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Player(PlayerId),
    Object(ObjectId),
    None,
}

/// The result of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Win(PlayerId),
    Draw,
    InProgress,
}
