/// Permanents on the battlefield: creatures, artifacts, enchantments, lands, planeswalkers.
/// Each permanent tracks its current state separate from its card definition.

use crate::card::CardName;
use crate::types::*;

/// A permanent on the battlefield.
#[derive(Debug, Clone)]
pub struct Permanent {
    pub id: ObjectId,
    pub card_name: CardName,
    pub controller: PlayerId,
    pub owner: PlayerId,
    pub tapped: bool,
    /// Base power (from card definition, before modifications)
    pub base_power: i16,
    /// Base toughness (from card definition, before modifications)
    pub base_toughness: i16,
    /// Power/toughness modifications from effects (counters, auras, etc.)
    pub power_mod: i16,
    pub toughness_mod: i16,
    /// Damage marked on this permanent this turn
    pub damage: i16,
    /// Keywords currently on this permanent
    pub keywords: Keywords,
    /// Counters on this permanent
    pub counters: Counters,
    /// Whether this permanent entered the battlefield this turn (summoning sickness)
    pub entered_this_turn: bool,
    /// Whether this permanent has attacked this turn
    pub attacked_this_turn: bool,
    /// Loyalty for planeswalkers
    pub loyalty: i8,
    /// Whether loyalty ability has been activated this turn
    pub loyalty_activated_this_turn: bool,
    /// Types (can be modified by effects)
    pub card_types: Vec<CardType>,
    /// For tokens
    pub is_token: bool,
}

/// Compact counter storage. Most permanents have 0-2 counter types.
#[derive(Debug, Clone, Default)]
pub struct Counters {
    entries: Vec<(CounterType, i16)>,
}

impl Counters {
    pub fn get(&self, ct: CounterType) -> i16 {
        self.entries
            .iter()
            .find(|(t, _)| *t == ct)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }

    pub fn add(&mut self, ct: CounterType, amount: i16) {
        if let Some(entry) = self.entries.iter_mut().find(|(t, _)| *t == ct) {
            entry.1 += amount;
            if entry.1 <= 0 {
                self.entries.retain(|(t, _)| *t != ct);
            }
        } else if amount > 0 {
            self.entries.push((ct, amount));
        }
    }

    pub fn remove(&mut self, ct: CounterType, amount: i16) {
        self.add(ct, -amount);
    }
}

impl Permanent {
    pub fn new(
        id: ObjectId,
        card_name: CardName,
        controller: PlayerId,
        owner: PlayerId,
        power: Option<i16>,
        toughness: Option<i16>,
        loyalty: Option<i8>,
        keywords: Keywords,
        card_types: &[CardType],
    ) -> Self {
        Permanent {
            id,
            card_name,
            controller,
            owner,
            tapped: false,
            base_power: power.unwrap_or(0),
            base_toughness: toughness.unwrap_or(0),
            power_mod: 0,
            toughness_mod: 0,
            damage: 0,
            keywords,
            counters: Counters::default(),
            entered_this_turn: true,
            attacked_this_turn: false,
            loyalty: loyalty.unwrap_or(0),
            loyalty_activated_this_turn: false,
            card_types: card_types.to_vec(),
            is_token: false,
        }
    }

    /// Current power after all modifications.
    pub fn power(&self) -> i16 {
        self.base_power
            + self.power_mod
            + self.counters.get(CounterType::PlusOnePlusOne)
            - self.counters.get(CounterType::MinusOneMinusOne)
    }

    /// Current toughness after all modifications.
    pub fn toughness(&self) -> i16 {
        self.base_toughness
            + self.toughness_mod
            + self.counters.get(CounterType::PlusOnePlusOne)
            - self.counters.get(CounterType::MinusOneMinusOne)
    }

    pub fn is_creature(&self) -> bool {
        self.card_types.contains(&CardType::Creature)
    }

    pub fn is_land(&self) -> bool {
        self.card_types.contains(&CardType::Land)
    }

    pub fn is_artifact(&self) -> bool {
        self.card_types.contains(&CardType::Artifact)
    }

    pub fn is_enchantment(&self) -> bool {
        self.card_types.contains(&CardType::Enchantment)
    }

    pub fn is_planeswalker(&self) -> bool {
        self.card_types.contains(&CardType::Planeswalker)
    }

    /// Can this creature attack? (not tapped, no summoning sickness unless haste, no defender)
    pub fn can_attack(&self) -> bool {
        self.is_creature()
            && !self.tapped
            && !self.attacked_this_turn
            && (!self.entered_this_turn || self.keywords.has(Keyword::Haste))
            && !self.keywords.has(Keyword::Defender)
    }

    /// Can this creature block?
    pub fn can_block(&self) -> bool {
        self.is_creature() && !self.tapped
    }

    /// Can this creature block an attacker with flying?
    pub fn can_block_flyer(&self) -> bool {
        self.can_block()
            && (self.keywords.has(Keyword::Flying) || self.keywords.has(Keyword::Reach))
    }

    /// Has lethal damage been dealt to this creature?
    pub fn has_lethal_damage(&self) -> bool {
        self.is_creature() && self.damage >= self.toughness() && !self.keywords.has(Keyword::Indestructible)
    }

    /// Clear damage and per-turn flags at end of turn.
    pub fn end_of_turn_cleanup(&mut self) {
        self.damage = 0;
        self.entered_this_turn = false;
        self.attacked_this_turn = false;
        self.loyalty_activated_this_turn = false;
    }
}
