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
    /// True if this spell can't be countered (e.g. Abrupt Decay).
    pub cant_be_countered: bool,
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
    /// Myr Retriever: return another artifact from graveyard to hand
    MyrRetrieverDeath,
    /// OrcishBowmasters: amass 1 and deal 1 damage
    OrcishBowmastersETB,
    /// Grief/Solitude evoke ETB
    GriefETB,
    SolitudeETB,
    /// Archon of Cruelty ETB/attack
    ArchonOfCrueltyTrigger,
    /// Orcish Bowmasters opponent draw trigger
    OrcishBowmastersOpponentDraw,
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
    /// Create N Treasure tokens for the given controller
    CreateTreasures { count: u8 },
    /// Ragavan deals combat damage: create a Treasure token
    RagavanCombatDamage,
    /// Gain control of target permanent (Agent of Treachery ETB, etc.)
    GainControlOfPermanent,
    /// Exchange control of this permanent and target creature (Gilded Drake ETB)
    GildedDrakeExchange { drake_id: ObjectId },
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
    /// Bazaar of Baghdad: draw 2, discard 3
    BazaarDraw,
    /// Sensei's Divining Top: look at top 3
    TopLook,
    /// Sensei's Divining Top: draw + put on top
    TopDraw,
    /// Voltaic Key / Manifold Key: untap artifact
    UntapArtifact,
    /// Karakas: bounce legendary creature
    KarakasBounce,
    /// Ghost Quarter: destroy land
    GhostQuarterDestroy,
    /// Narset -2: look at top 4
    NarsetMinus,
    /// Oko +2: create Food
    OkoFood,
    /// Oko +1: Elkify
    OkoElkify,
    /// Wrenn +1: return land from graveyard
    WrennReturn,
    /// Wrenn -1: deal 1 damage
    WrennPing,
    /// Karn +1: animate artifact
    KarnAnimate,
    /// Karn -2: wish for artifact
    KarnWish,
    /// Gideon 0: become creature
    GideonCreature,
    /// Gideon +1: prevent damage
    GideonPrevent,
    /// Kaya +1: exile from graveyard
    KayaExile,
    /// Kaya -1: exile permanent
    KayaMinus,
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
        self.push_with_flags(kind, controller, targets, false)
    }

    pub fn push_with_flags(
        &mut self,
        kind: StackItemKind,
        controller: PlayerId,
        targets: Vec<Target>,
        cant_be_countered: bool,
    ) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(StackItem {
            id,
            kind,
            controller,
            targets,
            cant_be_countered,
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
