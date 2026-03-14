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
    /// Razorkin Needlehead: whenever an opponent draws a card, deal 1 damage to them
    RazorkinNeedleheadOpponentDraw,
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
    /// PsychicFrog deals combat damage to a player or planeswalker: draw a card
    PsychicFrogCombatDamage,
    /// Mai, Scornful Striker: whenever a player casts a noncreature spell, they lose 2 life
    MaiNoncreatureSpellCast { target_player: PlayerId },
    /// Barrowgoyf deals combat damage to a player: mill that many cards, may put a creature card among them into hand
    BarrowgoyfCombatDamage { damage: i16 },
    /// Vessel of the All-Consuming deals damage: put a +1/+1 counter on it.
    VesselDealsDamage { vessel_id: ObjectId },
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
    /// Tezzeret, Cruel Captain emblem: at beginning of combat, put three +1/+1 counters on target artifact.
    /// If it's not a creature, it becomes a 0/0 Robot artifact creature.
    TezzeretEmblemCombat,
    /// Tezzeret, Cruel Captain: whenever an artifact you control enters, put a loyalty counter on Tezzeret.
    TezzeretArtifactEnters { tezzeret_id: ObjectId },
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
    /// Thassa's Oracle ETB: if cards in library <= devotion to blue, you win.
    ThassasOracleETB,
    /// Coveted Jewel ETB: draw 3 cards.
    CovetedJewelETB,
    /// Portable Hole ETB: exile target nonland permanent an opponent controls with MV <= 2.
    PortableHoleETB { hole_id: ObjectId },
    /// Argentum Masticore upkeep: sacrifice unless you discard a card.
    ArgentumMasticoreUpkeep { masticore_id: ObjectId },
    /// Cindervines: whenever opponent casts a noncreature spell, deal 1 damage to them.
    CindervinesDamage { target_player: PlayerId },
    /// Lavinia: whenever an opponent casts a spell with no mana spent, counter that spell.
    LaviniaCounter { spell_id: ObjectId },
    /// Oko -5: exchange control of target artifact/creature you control and target creature
    /// opponent controls with power 3 or less.
    OkoExchange,
    /// Chalice of the Void: whenever a player casts a spell with mana value equal to charge counters, counter it.
    ChaliceCounter { spell_id: ObjectId },
    /// Eidolon of the Great Revel: whenever a player casts a spell with MV 3 or less, deal 2 damage to that player.
    EidolonDamage { target_player: PlayerId },
    /// Harsh Mentor: whenever an opponent activates an ability of an artifact, creature, or land
    /// that isn't a mana ability, deal 2 damage to that player.
    HarshMentorDamage { target_player: PlayerId },
    /// Avalanche of Sector 7: whenever an opponent activates an ability of an artifact they
    /// control, deal 1 damage to that player.
    AvalancheDamage { target_player: PlayerId },
    /// Magebane Lizard: whenever a player casts a noncreature spell, deal damage to that player
    /// equal to the number of noncreature spells they've cast this turn.
    MagebaneLizardDamage { target_player: PlayerId, damage: u16 },
    /// Animate Dead ETB: return target creature from any graveyard to the battlefield under your control with -1/-0.
    AnimateDeadETB,
    /// Mystic Remora: whenever an opponent casts a noncreature spell, draw a card.
    MysticRemoraOpponentCast,
    /// Dress Down ETB: draw a card.
    DressDownETB,
    /// Dress Down: sacrifice at the beginning of the next end step.
    DressDownSacrifice { permanent_id: ObjectId },
    /// Underworld Breach: sacrifice at the beginning of the end step.
    UnderworldBreachSacrifice { permanent_id: ObjectId },
    /// Oath of Druids upkeep: if opponent controls more creatures, reveal cards from library
    /// until finding a creature, put it onto the battlefield, rest to graveyard.
    OathOfDruidsReveal,
    /// Roiling Vortex upkeep: deal 1 damage to each player.
    RoilingVortexUpkeep,
    /// Roiling Vortex: whenever a player casts a spell without paying its mana cost, deal 5 damage to that player.
    RoilingVortexFreeCast { target_player: PlayerId },
    /// Patchwork Automaton: whenever you cast an artifact spell, put a +1/+1 counter on it.
    PatchworkAutomatonCast { automaton_id: ObjectId },
    /// Nadu, Winged Wisdom: whenever a creature you control becomes the target of a spell
    /// or ability, reveal the top card of your library. If it's a land, put it onto the
    /// battlefield. Otherwise, put it into your hand.
    NaduTrigger,
    /// Displacer Kitten: whenever you cast a noncreature spell, exile up to one target
    /// nonland permanent you control, then return it to the battlefield under its owner's control.
    DisplacerKittenBlink,
    /// Kappa Cannoneer: whenever this or another artifact enters, put a +1/+1 counter on it.
    KappaCannoneerTrigger { cannoneer_id: ObjectId },
    /// Pinnacle Emissary: whenever you cast an artifact spell, create a 1/1 Drone token with flying.
    PinnacleEmissaryCast { emissary_controller: PlayerId },
    /// Emry ETB: mill four cards.
    EmryETB,
    /// Chromatic Star: when put into graveyard from the battlefield, draw a card.
    ChromaticStarDeath,
    /// Scrap Trawler: when this or another artifact dies, return a lesser-MV artifact from graveyard.
    ScrapTrawlerDeath,
    /// The Mightstone and Weakstone ETB: choose draw 2 or -5/-5.
    MightstoneWeakstoneETB { permanent_id: ObjectId },
    /// Golos ETB: search library for a land card and put it onto the battlefield tapped.
    GolosETB,
    /// Soul-Guide Lantern ETB: exile target card from a graveyard.
    SoulGuideLanternETB,
    /// Satyr Wayfinder ETB: reveal top 4, may put a land into hand, rest to graveyard.
    SatyrWayfinderETB,
    /// Haywire Mite dies: gain 2 life.
    HaywireMiteDeath,
    /// Emperor of Bones: at the beginning of combat, exile up to one target card from a graveyard.
    /// The exiled card is tracked so the +1/+1 counter trigger can reanimate it.
    EmperorOfBonesExile { emperor_id: ObjectId },
    /// Emperor of Bones: whenever +1/+1 counters are put on it, put a creature card exiled
    /// with it onto the battlefield with haste. Sacrifice it at beginning of next end step.
    EmperorOfBonesReanimate { emperor_id: ObjectId },
    /// Master of Death upkeep: if in graveyard, pay 1 life and return to hand.
    MasterOfDeathUpkeep { owner: PlayerId },
    /// Kishla Skimmer: whenever a card leaves your graveyard during your turn, draw a card (once per turn).
    KishlaSkimmerLeavesGraveyard,
    /// Phelia, Exuberant Shepherd: attack trigger — exile target nonland permanent.
    /// At the beginning of the next end step, return it under its owner's control.
    /// If it entered under Phelia's controller, put a +1/+1 counter on Phelia.
    PheliaAttackExile { phelia_id: ObjectId },
    /// Phelia delayed end-step return: return the exiled card to the battlefield.
    PheliaEndStepReturn { exiled_card_id: ObjectId, card_owner: PlayerId, phelia_controller: PlayerId, phelia_id: ObjectId },
    /// Seasoned Dungeoneer: attack trigger — target creature explores and gains
    /// protection from creatures until end of turn.
    SeasonedDungeoneerAttack,
    /// White Plume Adventurer: at the beginning of each opponent's upkeep, untap a creature
    /// you control. If you've completed a dungeon, untap all creatures you control instead.
    WhitePlumeAdventurerUntap,
    /// Seasoned Pyromancer ETB: discard 2, draw 2, create 1/1 tokens for nonland discards.
    SeasonedPyromancerETB,
    /// Broadside Bombardiers: deals 3 damage to any target (ETB or dies).
    BroadsideBombardiersDamage,
    /// Pyrogoyf dies: deals damage equal to its power to any target.
    PyrogoyfDeath { power: i16 },
    /// Gut, True Soul Zealot: attack trigger — sacrifice creature/artifact, create 4/1 skeleton token.
    GutAttackToken,
    /// Caves of Chaos Adventurer: attack trigger — exile top card, may play it this turn.
    CavesOfChaosAttackExile,
    /// Zhao, the Moon Slayer: attack trigger — exile top card, may play it this turn.
    ZhaoAttackExile,
    /// Bonecrusher Giant: whenever this creature becomes the target of a spell,
    /// deal 2 damage to that spell's controller.
    BonecrusherGiantTargeted { target_player: PlayerId },
    /// Leovold, Emissary of Trest: whenever you or a permanent you control becomes
    /// the target of a spell or ability an opponent controls, you may draw a card.
    LeovoldTargetDraw,
    /// Vengevine: when a player casts their second creature spell this turn,
    /// return Vengevine from their graveyard to the battlefield.
    VengevineReturn { vengevine_id: ObjectId, owner: PlayerId },
    /// Mana Vault draw-step trigger: if Mana Vault is tapped, deal 1 damage to its controller.
    ManaVaultDrawStep { vault_id: ObjectId },
    /// Minsc & Boo ETB/upkeep: you may create Boo, a legendary 1/1 red Hamster creature token
    /// with trample and haste.
    MinscCreateBoo { minsc_id: ObjectId },
    /// Portal to Phyrexia upkeep: put target creature card from a graveyard onto the battlefield
    /// under your control.
    PortalToPhyrexiaUpkeep,
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
    /// Oko -5: exchange control
    OkoExchange,
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
    /// Kaya -5: deal damage to target player equal to cards they own in exile, gain that much life
    KayaUltimate,
    /// Equip: attach equipment to a creature (targets[0] = creature ObjectId)
    EquipCreature { equipment_id: ObjectId },
    /// Batterskull bounce: return Batterskull to owner's hand
    BatterskullBounce,
    /// Basic cycling: discard a card, draw a card (already discarded at activation).
    CyclingDraw,
    /// Swampcycling: discard, search library for a Swamp card, put it into hand (already discarded).
    CyclingSearchSwamp,
    /// Islandcycling: discard, search library for an Island card, put it into hand (already discarded).
    CyclingSearchIsland,
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
    /// Tezzeret, Cruel Captain 0: untap target artifact or creature, +1/+1 counter if artifact creature.
    TezzeretUntap { target_id: ObjectId },
    /// Tezzeret, Cruel Captain -3: search library for artifact with MV <= 1, put into hand.
    TezzeretSearch,
    /// Tezzeret, Cruel Captain -7: create the Tezzeret emblem.
    TezzeretUltimate,
    /// The One Ring {T}: put a burden counter on The One Ring, draw cards equal to burden counters.
    TheOneRingDraw { ring_id: ObjectId },
    /// Isochron Scepter {2},{T}: copy and cast the imprinted instant without paying mana cost.
    IsochronScepterActivated { scepter_id: ObjectId },
    /// Hideaway land {T}: cast the hidden card for free (condition already checked in movegen).
    HideawayActivated { land_id: ObjectId },
    /// Griselbrand: pay 7 life, draw 7 cards.
    GriselbrandDraw,
    /// Walking Ballista: {4}: Put a +1/+1 counter on Walking Ballista.
    WalkingBallistaAddCounter { ballista_id: ObjectId },
    /// Walking Ballista: Remove a +1/+1 counter: deal 1 damage to any target.
    WalkingBallistaPing { ballista_id: ObjectId },
    /// Time Vault: {T}: Take an extra turn after this one.
    TimeVaultExtraTurn,
    /// Time Vault: Skip your next turn: Untap Time Vault.
    TimeVaultUntap { vault_id: ObjectId },
    /// Krark-Clan Ironworks: Sacrifice an artifact: Add {C}{C}. (mana ability, resolved at activation)
    KrarkClanIronworksSacrifice,
    /// Engineered Explosives: {2}, Sacrifice: Destroy each nonland permanent with MV equal to charge counters.
    EngineeredExplosivesDestroy { charge_counters: u32 },
    /// Minsc & Boo +1: Put three +1/+1 counters on up to one target creature with trample or haste.
    MinscCounters,
    /// Minsc & Boo -2: Sacrifice a creature. Deal X damage to any target (X = sacrificed creature's power).
    /// If the sacrificed creature was a Hamster, draw X cards.
    MinscSacDamage,
    /// Comet, Stellar Pup 0: Simplified — create two 1/1 tokens.
    CometCreateTokens,
    /// Dovin, Hand of Control -1: Prevent damage from/to target permanent (simplified: no-op).
    DovinPrevent,
    /// Necropotence: pay 1 life, draw a card (simplified approximation).
    NecropotencePayLife,
    /// Aphetto Alchemist: untap target artifact or creature.
    UntapArtifactOrCreature,
    /// Emry, Lurker of the Loch: choose target artifact in graveyard, may cast it this turn.
    EmryCastArtifact,
    /// Aether Spellbomb: {U}, Sacrifice: Return target creature to its owner's hand.
    AetherSpellbombBounce,
    /// Aether Spellbomb: {1}, Sacrifice: Draw a card.
    AetherSpellbombDraw,
    /// Cryogen Relic: {1}{U}, Sacrifice: Put a stun counter on target tapped creature.
    CryogenRelicStun,
    /// Tormod's Crypt: {T}, Sacrifice: Exile target player's graveyard.
    TormodsCryptExile,
    /// Soul-Guide Lantern: {T}, Sacrifice: Exile each opponent's graveyard.
    SoulGuideLanternExile,
    /// Soul-Guide Lantern: {1}, {T}, Sacrifice: Draw a card.
    SoulGuideLanternDraw,
    /// Manifold Key: {3}, {T}: Target creature can't be blocked this turn.
    ManifoldKeyUnblockable,
    /// Haywire Mite: {G}, Sacrifice: Exile target noncreature artifact or noncreature enchantment.
    HaywireMiteExile,
    /// Outland Liberator: {1}, Sacrifice: Destroy target artifact or enchantment.
    OutlandLiberatorDestroy,
    /// Cathar Commando: {1}, Sacrifice: Destroy target artifact or enchantment.
    CatharCommandoDestroy,
    /// Seal of Cleansing: Sacrifice: Destroy target artifact or enchantment.
    SealOfCleansingDestroy,
    /// Hermit Druid: {G}, {T}: Reveal cards until basic land, put it in hand, rest to graveyard.
    HermitDruidReveal,
    /// Sylvan Safekeeper: Sacrifice a land: Target creature you control gains shroud until end of turn.
    SylvanSafekeeperShroud,
    /// Emperor of Bones: {1}{B}: Adapt 2. Put two +1/+1 counters on it if it has none.
    EmperorOfBonesAdapt { emperor_id: ObjectId },
    /// Mystic Forge: {T}, Pay 1 life: Exile the top card of your library.
    MysticForgeExile,
    /// Boromir, Warden of the Tower: Sacrifice — creatures you control gain indestructible until end of turn.
    BoromirSacrifice,
    /// Gorilla Shaman: {X}{X}{1}: Destroy target noncreature artifact with mana value X.
    GorillaShamanDestroy { target_mv: u8 },
    /// Mana Vault / Grim Monolith: {4}: Untap this artifact.
    UntapSelf { permanent_id: ObjectId },
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
