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
    /// The chosen value of X for X spells (0 for non-X spells).
    pub x_value: u8,
    /// True if this spell was cast from the graveyard (flashback or Yawgmoth's Will).
    /// When true and the spell is an instant/sorcery, it is exiled instead of going to graveyard.
    pub cast_from_graveyard: bool,
    /// True if this spell is the adventure half of an adventure card.
    /// When true and the spell resolves, the card goes to exile (where the creature half can be cast).
    pub cast_as_adventure: bool,
    /// Chosen mode indices for modal spells (e.g., Kolaghan's Command choose 2 of 4).
    /// Empty for non-modal spells.
    pub modes: Vec<u8>,
    /// True if this item is a copy of another spell (e.g., from storm or Twincast).
    /// Copies are never cast, so they don't increment storm_count and they don't
    /// go to the graveyard when they finish resolving.
    pub is_copy: bool,
}

#[derive(Debug, Clone)]
pub enum StackItemKind {
    /// A spell being cast from a card
    Spell {
        card_name: CardName,
        card_id: ObjectId,
        /// True if this creature was cast via evoke (exile color card from hand).
        /// When the evoke creature enters the battlefield, it gets an evoke trigger
        /// that sacrifices it after the ETB effect resolves.
        cast_via_evoke: bool,
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
    /// Delver of Secrets upkeep trigger: reveal top card, transform if instant/sorcery
    DelverUpkeep { delver_id: ObjectId },
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
    /// Evoke sacrifice trigger: when a creature is cast via evoke, it's sacrificed
    /// after its ETB trigger resolves.
    EvokeSacrifice { permanent_id: ObjectId },
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
    /// ScrawlingCrawler deals combat damage to a player: draw a card
    ScrawlingCrawlerCombatDamage,
    /// PsychicFrog deals combat damage to a player: you may exile a card from your graveyard; if you do, draw a card
    PsychicFrogCombatDamage,
    /// Mai, Scornful Striker deals combat damage to a player: you may cast a creature card from a graveyard
    MaiCombatDamage,
    /// Gain control of target permanent (Agent of Treachery ETB, etc.)
    GainControlOfPermanent,
    /// Exchange control of this permanent and target creature (Gilded Drake ETB)
    GildedDrakeExchange { drake_id: ObjectId },
    /// Skyclave Apparition ETB: exile target nonland nontoken permanent with MV <= 4
    SkyclaveApparitionETB,
    /// Skyclave Apparition leaves the battlefield: create a token for the opponent
    /// The token's MV is stored in skyclave_token_mv on GameState, keyed by the apparition's id
    SkyclaveApparitionLeaves { apparition_id: ObjectId, token_mv: u32, opponent: PlayerId },
    /// Exile-until-leaves return trigger: return an exiled card to the battlefield
    ExileLinkedReturn { card_id: ObjectId, card_owner: PlayerId },
    /// Monarch end-step trigger: the monarch draws a card
    MonarchEndStep,
    /// Emrakul, the Aeons Torn cast trigger: take an extra turn after this one
    EmrakulCast,
    /// Dack Fayden emblem: gain control of a permanent (targets[0] is the permanent).
    DackEmblemControl,
    /// Tezzeret, Cruel Captain emblem: search library for an artifact and put it onto the battlefield.
    TezzeretEmblemArtifact,
    /// Delayed sacrifice: sacrifice a specific permanent (used by Sneak Attack and similar).
    SacrificeTarget { permanent_id: ObjectId },
    /// The One Ring ETB: controller gains protection from everything until their next turn.
    TheOneRingETB { ring_id: ObjectId },
    /// The One Ring upkeep trigger: lose 1 life per burden counter, then add a burden counter.
    TheOneRingUpkeep { ring_id: ObjectId },
    /// Chrome Mox ETB: imprint a nonartifact, nonland card from hand (exile it).
    ChromeMoxETB { mox_id: ObjectId },
    /// Isochron Scepter ETB: imprint an instant with MV <= 2 from hand (exile it).
    IsochronScepterETB { scepter_id: ObjectId },
    /// Hideaway ETB: look at top N cards, choose one to exile face-down, put the rest on bottom.
    /// land_id is the hideaway land's ObjectId so we can record the hideaway link.
    HideawayETB { land_id: ObjectId, n: u8 },
    /// Saga chapter trigger: a chapter ability fires when the saga reaches that lore count.
    /// `saga_id` is the saga permanent's ObjectId.
    /// `chapter` is the chapter number (1, 2, 3, …).
    SagaChapter { saga_id: ObjectId, chapter: u8 },
    /// Saga sacrifice: after the last chapter resolves, sacrifice the saga.
    /// `saga_id` is the saga permanent's ObjectId.
    SagaSacrifice { saga_id: ObjectId },
    /// Initiative upkeep trigger: the player with initiative ventures into the Undercity.
    InitiativeUpkeep,
    /// An Undercity dungeon room effect resolves.
    UndercityRoom(crate::types::UndercityRoom),
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
    /// Wrenn -7 ultimate: create Wrenn and Six emblem
    WrennUltimate,
    /// Karn +1: animate artifact
    KarnAnimate,
    /// Karn -2: wish for artifact
    KarnWish,
    /// Gideon 0: become creature
    GideonCreature,
    /// Gideon +1: prevent damage
    GideonPrevent,
    /// Gideon +0: create the Gideon of the Trials emblem
    GideonEmblem,
    /// Kaya +1: exile from graveyard
    KayaExile,
    /// Kaya -1: exile permanent
    KayaMinus,
    /// Equip: attach equipment to a creature (targets[0] = creature ObjectId)
    EquipCreature { equipment_id: ObjectId },
    /// Batterskull bounce: return Batterskull to owner's hand
    BatterskullBounce,
    /// Basic cycling: discard a card, draw a card (already discarded at activation).
    CyclingDraw,
    /// Shark Typhoon cycling: discard, create an X/X Shark token with flying (X chosen at activation).
    SharkTyphoonCycling { x_value: u8 },
    /// Boseiju channel: destroy target artifact, enchantment, or nonbasic land.
    BoseijuChannel,
    /// Otawara channel: return target artifact, creature, or planeswalker to owner's hand.
    OtawaraChannel,
    /// Dack Fayden +1: target player draws 2 cards, then discards 2.
    DackDraw,
    /// Dack Fayden -2: gain control of target artifact.
    DackSteal,
    /// Dack Fayden -6: create the Dack Fayden emblem.
    DackUltimate,
    /// Tezzeret, Cruel Captain +1: draw a card if you control an artifact.
    TezzeretDraw,
    /// Tezzeret, Cruel Captain -2: create a 1/1 Thopter artifact creature token with flying.
    TezzeretThopter,
    /// Tezzeret, Cruel Captain -7: create the Tezzeret emblem.
    TezzeretUltimate,
    /// The One Ring {T}: put a burden counter on The One Ring, draw cards equal to burden counters.
    TheOneRingDraw { ring_id: ObjectId },
    /// Isochron Scepter {2},{T}: copy and cast the imprinted instant without paying mana cost.
    IsochronScepterActivated { scepter_id: ObjectId },
    /// Hideaway land {T}: cast the hidden card for free (condition already checked in movegen).
    HideawayActivated { land_id: ObjectId },
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
        self.push_with_flags(kind, controller, targets, false, 0, false, vec![])
    }

    pub fn push_with_flags(
        &mut self,
        kind: StackItemKind,
        controller: PlayerId,
        targets: Vec<Target>,
        cant_be_countered: bool,
        x_value: u8,
        cast_from_graveyard: bool,
        modes: Vec<u8>,
    ) -> ObjectId {
        self.push_with_all_flags(kind, controller, targets, cant_be_countered, x_value, cast_from_graveyard, false, modes)
    }

    pub fn push_with_all_flags(
        &mut self,
        kind: StackItemKind,
        controller: PlayerId,
        targets: Vec<Target>,
        cant_be_countered: bool,
        x_value: u8,
        cast_from_graveyard: bool,
        cast_as_adventure: bool,
        modes: Vec<u8>,
    ) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(StackItem {
            id,
            kind,
            controller,
            targets,
            cant_be_countered,
            x_value,
            cast_from_graveyard,
            cast_as_adventure,
            modes,
            is_copy: false,
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

    /// Create a copy of the given stack item, push it on the stack, and return the new item's id.
    /// The copy is a new object with a fresh id but inherits the same kind, controller, targets,
    /// x_value, and modes. Copies are never "cast from graveyard", can always be countered,
    /// and are marked as copies (is_copy = true) so they don't re-trigger storm.
    pub fn copy_spell(&mut self, source_id: ObjectId) -> Option<ObjectId> {
        let source = self.items.iter().find(|item| item.id == source_id)?.clone();
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(StackItem {
            id,
            kind: source.kind.clone(),
            controller: source.controller,
            targets: source.targets.clone(),
            cant_be_countered: false,
            x_value: source.x_value,
            cast_from_graveyard: false,
            cast_as_adventure: false,
            modes: source.modes.clone(),
            is_copy: true,
        });
        Some(id)
    }

    /// Push a spell copy using an explicit StackItem template (for storm copies created
    /// after the original has already been popped off the stack).
    pub fn push_copy(&mut self, template: &StackItem) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(StackItem {
            id,
            kind: template.kind.clone(),
            controller: template.controller,
            targets: template.targets.clone(),
            cant_be_countered: false,
            x_value: template.x_value,
            cast_from_graveyard: false,
            cast_as_adventure: false,
            modes: template.modes.clone(),
            is_copy: true,
        });
        id
    }
}
