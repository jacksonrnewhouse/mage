/// The stack: spells and abilities waiting to resolve.
/// LIFO order. Both players get priority before each resolution.

use crate::card::CardName;
use crate::types::*;

/// An item on the stack (spell or ability).
#[derive(Debug, Clone)]
pub struct StackItem {
    pub id: ObjectId,
    pub kind: StackItemKind,
    pub controller: PlayerId,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone)]
pub enum StackItemKind {
    /// A spell being cast from a card
    Spell {
        card_name: CardName,
        card_id: ObjectId,
    },
    /// A triggered ability
    TriggeredAbility {
        source_id: ObjectId,
        source_name: CardName,
        effect: TriggeredEffect,
    },
    /// An activated ability (non-mana)
    ActivatedAbility {
        source_id: ObjectId,
        source_name: CardName,
        effect: ActivatedEffect,
    },
}

/// Triggered effects that go on the stack.
#[derive(Debug, Clone)]
pub enum TriggeredEffect {
    ManaCryptUpkeep,
    GoblinGuideAttack,
    YoungPyromancerCast,
    MonasteryMentorCast,
    SheoldredDraw,
    SheoldredOpponentDraw,
    DarkConfidantUpkeep,
    WurmcoilDeath,
    SkullclampDeath,
    /// Generic: deal N damage to target
    DealDamage(u16),
    /// Generic: draw N cards
    DrawCards(u8),
    /// Generic: gain N life
    GainLife(i32),
    /// Generic: lose N life
    LoseLife(i32),
    /// Create N tokens
    CreateTokens { power: i16, toughness: i16, count: u8 },
}

/// Activated ability effects.
#[derive(Debug, Clone)]
pub enum ActivatedEffect {
    /// Sacrifice to add mana (Black Lotus, Lotus Petal)
    SacrificeForMana { amount: u8 },
    /// Planeswalker ability by index
    PlaneswalkerAbility { loyalty_cost: i8, index: u8 },
    /// Jace brainstorm (0 ability)
    JaceBrainstorm,
    /// Jace bounce (-1)
    JaceBounce,
    /// Jace fateseal (+2)
    JaceFateseal,
    /// Teferi bounce and draw (-3)
    TeferiBounce,
    /// Generic: draw cards
    DrawCards(u8),
}

/// The game stack.
#[derive(Debug, Clone, Default)]
pub struct GameStack {
    items: Vec<StackItem>,
    next_id: ObjectId,
}

impl GameStack {
    pub fn new(starting_id: ObjectId) -> Self {
        GameStack {
            items: Vec::with_capacity(8),
            next_id: starting_id,
        }
    }

    pub fn push(&mut self, kind: StackItemKind, controller: PlayerId, targets: Vec<Target>) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(StackItem {
            id,
            kind,
            controller,
            targets,
        });
        id
    }

    pub fn pop(&mut self) -> Option<StackItem> {
        self.items.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn top(&self) -> Option<&StackItem> {
        self.items.last()
    }

    pub fn items(&self) -> &[StackItem] {
        &self.items
    }

    pub fn next_id(&self) -> ObjectId {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: ObjectId) {
        self.next_id = id;
    }

    /// Remove a specific item from the stack (e.g., when countering a spell).
    pub fn remove(&mut self, id: ObjectId) -> Option<StackItem> {
        if let Some(pos) = self.items.iter().position(|item| item.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }
}
