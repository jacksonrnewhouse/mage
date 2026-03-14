/// Card definitions: static card data (templates) separated from runtime state.
/// Card behaviors are dispatched by CardName enum for branch-prediction-friendly code.

use crate::mana::ManaCost;
use crate::types::*;

/// Creature type data for a card. Static slices for zero-overhead lookup.
pub type CreatureTypes = &'static [CreatureType];

/// Every card known to the engine. Using an enum for fast dispatch.
/// Compiler can optimize match statements into jump tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum CardName {
    // === Basic Lands ===
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,

    // === Dual Lands ===
    UndergroundSea,
    VolcanicIsland,
    Tundra,
    TropicalIsland,
    Badlands,
    Bayou,
    Plateau,
    Savannah,
    Scrubland,
    Taiga,

    // === Fetch Lands ===
    FloodedStrand,
    PollutedDelta,
    BloodstainedMire,
    WoodedFoothills,
    WindsweptHeath,
    MistyRainforest,
    ScaldingTarn,
    VerdantCatacombs,
    AridMesa,
    MarshFlats,

    // === Shock Lands ===
    HallowedFountain,
    WateryGrave,
    BloodCrypt,
    StompingGround,
    TempleGarden,
    GodlessShrine,
    SteamVents,
    OvergrownTomb,
    SacredFoundry,
    BreedingPool,

    // === Survey/Misc Dual Lands ===
    MeticulousArchive,
    UndercitySewers,
    ThunderingFalls,
    HedgeMaze,

    // === Other Lands ===
    LibraryOfAlexandria,
    StripMine,
    Wasteland,
    TolarianAcademy,
    AncientTomb,
    MishrasWorkshop,
    Karakas,
    UrborgTombOfYawgmoth,
    OtawaraSoaringCity,
    BoseijuWhoEndures,
    GaeasCradle,
    YavimayaCradleOfGrowth,
    CityOfTraitors,
    ForbiddenOrchard,
    GhostQuarter,
    SpireOfIndustry,
    StartingTown,
    TalonGatesOfMadara,
    ShelldockIsle,
    MosswortBridge,
    TheMycoSynthGardens,
    UrzasSaga,
    BazaarOfBaghdad,
    DryadArbor,
    CavernOfSouls,

    // === Power 9 ===
    BlackLotus,
    AncestralRecall,
    TimeWalk,
    Timetwister,
    TemporalMastery,
    MoxPearl,
    MoxSapphire,
    MoxJet,
    MoxRuby,
    MoxEmerald,

    // === Fast Mana ===
    SolRing,
    ManaCrypt,
    ManaVault,
    LotusPetal,
    LionEyeDiamond,
    GrimMonolith,
    ChromeMox,
    MoxDiamond,
    MoxOpal,

    // === White Creatures ===
    NomadsEnKor,
    AjaniNacatlPariah,
    CatharCommando,
    ContainmentPriest,
    DauntlessDismantler,
    DoorkeeperThrull,
    DrannithMagistrate,
    EtherswornCanonist,
    KatakiWarsWage,
    LeoninArbiter,
    OswaldFiddlebender,
    PheliaExuberantShepherd,
    SamwiseTheStouthearted,
    SpiritOfTheLabyrinth,
    ThaliaGuardianOfThraben,
    VoiceOfVictory,
    WhiteOrchidPhantom,
    ArchonOfEmeria,
    BoromirWardenOfTheTower,
    ClarionConqueror,
    LoranOfTheThirdPath,
    MonasteryMentor,
    WhitePlumeAdventurer,
    SeasonedDungeoneer,
    Solitude,
    SkyclaveApparition,
    StoneforgeMystic,
    PalaceJailer,
    AuriokChampion,
    KorFirewalker,

    // === White Spells ===
    SwordsToPlowshares,
    Balance,
    CouncilsJudgment,
    PathToExile,
    Armageddon,
    Disenchant,
    EnlightenedTutor,
    MarchOfOtherworldlyLight,
    Fragmentize,
    PrismaticEnding,
    WrathOfTheSkies,

    // === White Enchantments/Artifacts ===
    PortableHole,
    DeafeningSilence,
    HighNoon,
    RestInPeace,
    SealOfCleansing,
    StonySilence,
    WitchEnchanter,
    /// Back face of Witch Enchanter — MDFC land
    WitchBlessedMeadow,

    // === White Planeswalkers ===
    GideonOfTheTrials,

    // === Double-Faced Cards (DFCs) ===
    /// Delver of Secrets — front face (1/1 Human Wizard, {U})
    DelverOfSecrets,
    /// Insectile Aberration — back face of Delver of Secrets (3/2 Insect with flying)
    InsectileAberration,

    // === Blue Creatures ===
    TamiyoInquisitiveStudent,
    AphettoAlchemist,
    MercurialSpelldancer,
    SnapcasterMage,
    TrueNameNemesis,
    ThassasOracle,
    ThievingSkydiver,
    ThundertrapTrainer,
    BrazenBorrower,
    EmryLurkerOfTheLoch,
    Hullbreacher,
    PlagonLordOfTheBeach,
    DisplacerKitten,
    KappaCannoneer,
    ThoughtMonitor,

    // === Blue Planeswalkers ===
    NarsetParterOfVeils,
    JaceTheMindSculptor,

    // === Blue Spells ===
    Counterspell,
    ForceOfWill,
    Brainstorm,
    Ponder,
    Preordain,
    ManaDrain,
    MysticalTutor,
    TreasureCruise,
    DigThroughTime,
    MentalMisstep,
    SpellPierce,
    ChainOfVapor,
    ConsignToMemory,
    Flusterstorm,
    IntoTheFloodMaw,
    Stifle,
    BrainFreeze,
    Daze,
    Flash,
    HurkylsRecall,
    ManaLeak,
    MemoryLapse,
    Remand,
    ForceOfNegation,
    MysticalDispute,
    GiftsUngiven,
    ParadoxicalOutcome,
    Gush,
    Misdirection,
    Commandeer,
    CarefulStudy,
    Consider,
    MerchantScroll,
    TransmuteArtifact,
    ShowAndTell,
    StockUp,
    Windfall,
    LorienRevealed,
    StepThrough,
    Thoughtcast,
    EchoOfEons,
    MindsDesire,
    Tinker,

    // === Blue Enchantments/Artifacts ===
    AetherSpellbomb,
    CryogenRelic,
    MysticRemora,
    UnableToScream,
    DressDown,
    EnergyFlux,
    SinkIntoStupor,
    SharkTyphoon,
    SharkToken,
    FutureSight,

    // === Black Creatures ===
    Nethergoyf,
    DarkConfidant,
    DauthiVoidwalker,
    EmperorOfBones,
    MaiScornfulStriker,
    OrcishBowmasters,
    Barrowgoyf,
    OppositionAgent,
    Grief,
    SheoldredTheApocalypse,
    StreetWraith,
    TrollOfKhazadDum,
    ArchonOfCruelty,
    Griselbrand,

    // === Black Spells ===
    VillageRites,
    DeadlyDispute,
    DarkRitual,
    DemonicConsultation,
    DemonicTutor,
    Thoughtseize,
    HymnToTourach,
    ToxicDeluge,
    Reanimate,
    Entomb,
    FatalPush,
    VampiricTutor,
    YawgmothsWill,
    TendrilsOfAgony,
    BitterTriumph,
    CabalRitual,
    SheoldredsEdict,
    SnuffOut,
    Duress,
    ImperialSeal,
    InquisitionOfKozilek,
    MindTwist,
    Exhume,
    Doomsday,
    BeseechTheMirror,
    Unmask,

    // === Black Enchantments/Artifacts ===
    BolassCitadel,
    AnimateDead,
    ChainsOfMephistopheles,
    Necropotence,

    // === Red Creatures ===
    GorillaShaman,
    RagavanNimblePilferer,
    AshZealot,
    EidolonOfTheGreatRevel,
    GenerousPlunderer,
    HarshMentor,
    MagebaneLizard,
    RazorkinNeedlehead,
    ZhaoTheMoonSlayer,
    NameStickerGoblin,
    AvalancheOfSector7,
    BonecrusherGiant,
    BroadsideBombardiers,
    GutTrueSoulZealot,
    MagusOfTheMoon,
    SeasonedPyromancer,
    SimianSpiritGuide,
    CavesOfChaosAdventurer,
    Pyrogoyf,
    Fury,
    GoblinGuide,
    MonasterySwiftspear,
    YoungPyromancer,

    // === Red Spells ===
    LightningBolt,
    WheelOfFortune,
    Pyroblast,
    RedElementalBlast,
    ChainLightning,
    RedirectLightning,
    Abrade,
    ShrapnelBlast,
    Crash,
    UntimelyMalfunction,
    Meltdown,
    ShatteringSpree,
    Vandalblast,
    Suplex,
    BrotherhoodsEnd,

    // === Red Enchantments ===
    RoilingVortex,
    UnderworldBreach,
    BloodMoon,
    FableOfTheMirrorBreaker,
    /// Back face of Fable of the Mirror-Breaker
    ReflectionOfKikiJiki,
    /// Token created by Fable of the Mirror-Breaker Chapter I
    FableGoblinToken,
    ShatterskullSmashing,
    /// Back face of Shatterskull Smashing — MDFC land
    ShatterskullTheHammerPass,
    SunderingEruption,
    /// Back face of Sundering Eruption — MDFC land
    VolcanicFissure,
    ExperimentalFrenzy,

    // === Red/Green extra creatures (madness/pitch) ===
    BaskingRootwalla,
    BlazingRootwalla,
    SqueeGoblinNabob,

    // === Green Creatures ===
    DelightedHalfling,
    HaywireMite,
    Hexdrinker,
    SylvanSafekeeper,
    CollectorOuphe,
    HermitDruid,
    OutlandLiberator,
    SatyrWayfinder,
    Tarmogoyf,
    TownGreeter,
    ElvishSpiritGuide,
    Endurance,
    Manglehorn,
    IcetillExplorer,
    UndermountainAdventurer,
    Vengevine,
    HollowOne,
    BirdsOfParadise,
    QuirionRanger,

    // === Green Spells ===
    CropRotation,
    NaturesClaim,
    VeilOfSummer,
    OnceUponATime,
    ForceOfVigor,
    GreenSunsZenith,
    Channel,
    LifeFromTheLoam,
    SeedsOfInnocence,
    NaturalOrder,
    Regrowth,

    // === Green Enchantments ===
    OathOfDruids,

    // === Green/Black Creatures ===
    MasterOfDeath,
    HogaakArisenNecropolis,
    KishlaSkimmer,
    GolgariGraveTroll,
    StinkweedImp,

    // === Colorless Creatures ===
    StonecoilSerpent,
    WalkingBallista,
    PhyrexianDreadnought,
    MyrRetriever,
    PatchworkAutomaton,
    PhyrexianRevoker,
    FoundryInspector,
    GlaringFleshraker,
    PhyrexianMetamorph,
    ScrapTrawler,
    ScrawlingCrawler,
    LodestoneGolem,
    ArgentumMasticore,
    GolosTirelessPilgrim,
    KarnSilverGolem,
    WurmcoilEngine,
    EmrakulTheAeonsTorn,

    // === Colorless Planeswalkers ===
    TezzeretCruelCaptain,
    KarnTheGreatCreator,

    // === Colorless Spells ===
    MindbreakTrap,
    NoxiousRevival,
    Dismember,
    KozileksCommand,
    GitaxianProbe,
    SurgicalExtraction,

    // === Colorless Artifacts ===
    ChaliceOfTheVoid,
    ClownCar,
    EngineeredExplosives,
    Gleemox,
    TormodsCrypt,
    UrzasBauble,
    MishrasBauble,
    ChromaticStar,
    GrafdiggersCage,
    LavaspurBoots,
    ManifoldKey,
    PithingNeedle,
    SenseisDiviningTop,
    Shadowspear,
    Shuko,
    SoulGuideLantern,
    VexingBauble,
    VoltaicKey,
    DampingSphere,
    DefenseGrid,
    DisruptorFlute,
    IchorWellspring,
    NullRod,
    SphereOfResistance,
    ThornOfAmethyst,
    TimeVault,
    TorporOrb,
    VoidMirror,
    CrucibleOfWorlds,
    Nettlecyst,
    Trinisphere,
    KrarkClanIronworks,
    MysticForge,
    TheOneRing,
    MemoryJar,
    TheMightstoneAndWeakstone,
    CovetedJewel,
    PortalToPhyrexia,
    Batterskull,
    SkullClamp,
    IsochronScepter,

    // === Azorius (WU) ===
    LaviniaAzoriusRenegade,
    MakdeeAndItlaSkysnarers,
    DovinHandOfControl,
    TeferiTimeRaveler,

    // === Dimir (UB) ===
    PsychicFrog,

    // === Rakdos (BR) ===
    MoltenCollapse,
    HidetsuguConsumesAll,
    /// Back face of Hidetsugu Consumes All
    VesselOfTheAllConsuming,

    // === Gruul (RG) ===
    AncientGrudge,
    Cindervines,
    WrennAndSix,
    MinscAndBooTimelessHeroes,

    // === Selesnya (GW) ===
    DryadMilitant,

    // === Orzhov (WB) ===
    PestControl,
    KayaOrzhovUsurper,
    LurrusOfTheDreamDen,

    // === Izzet (UR) ===
    ExpressiveIteration,
    FlameOfAnor,
    PinnacleEmissary,
    DackFayden,
    /// Twincast: copy target instant or sorcery spell. You may choose new targets.
    Twincast,
    /// Galvanic Relay: exile the top card of your library. Until end of turn, you may play it. Storm.
    GalvanicRelay,

    // === Golgari (BG) ===
    DeathriteShaman,
    AbruptDecay,

    // === Boros (RW) ===
    ForthEorlingas,
    CometStellarPup,

    // === Simic (GU) ===
    GildedDrake,
    AgentOfTreachery,
    MemorysJourney,
    NaduWingedWisdom,
    OkoThiefOfCrowns,

    // === Multicolor (3+) ===
    LeovoldEmissaryOfTrest,
    AtraxaGrandUnifier,
    KolaghanCommand,

    // === Tokens ===
    /// Represents a Treasure token (artifact: "Sacrifice: Add one mana of any color.")
    TreasureToken,
    /// Represents a Thopter token (1/1 colorless Thopter artifact creature with flying).
    ThopterToken,
    /// Represents an Eldrazi Spawn token (0/1 colorless creature: "Sacrifice: Add {C}.")
    EldraziSpawnToken,
    /// Represents a Pest token (1/1 black and green creature: "When this creature dies, you gain 1 life.")
    PestToken,

    // Sentinel value for array sizing
    _Count,
}

/// Static card definition. Immutable, shared across all game states.
#[derive(Debug, Clone)]
pub struct CardDef {
    pub name: CardName,
    pub display_name: &'static str,
    pub mana_cost: ManaCost,
    /// True if this card has X in its mana cost (e.g., Walking Ballista {X}{X},
    /// Chalice of the Void {X}{X}, Stonecoil Serpent {X}).
    /// When casting, the player chooses X and pays mana_cost + (x_multiplier * X).
    pub has_x_cost: bool,
    /// How many times X appears in the mana cost symbol (1 for {X}, 2 for {X}{X}).
    pub x_multiplier: u8,
    pub card_types: &'static [CardType],
    pub supertypes: &'static [SuperType],
    pub power: Option<i16>,
    pub toughness: Option<i16>,
    pub loyalty: Option<i8>,
    pub keywords: Keywords,
    pub color_identity: &'static [Color],
    pub oracle_text: &'static str,
    /// Flashback cost: if Some, this card can be cast from the graveyard for this alternate cost.
    /// When cast via flashback (or countered), the card is exiled instead of going to graveyard.
    pub flashback_cost: Option<ManaCost>,
    /// Madness cost: if Some, when this card is discarded it goes to exile instead, and the
    /// controller may cast it for this alternate cost. If declined, it moves to the graveyard.
    pub madness_cost: Option<ManaCost>,
    /// Creature subtypes for tribal interactions. Empty for non-creatures.
    pub creature_types: &'static [CreatureType],
    /// True if this card is a changeling (has all creature types).
    pub is_changeling: bool,
    /// Adventure: if Some, this creature card has an adventure half.
    /// The adventure is an instant or sorcery spell with its own cost, types, and name.
    /// When you cast the adventure, it goes to exile; then you may cast the creature from exile.
    pub adventure: Option<AdventureDef>,
    /// Double-faced card: if Some, this is the front face and the value is the back-face CardName.
    /// Transform flips the permanent to show its back face.
    pub back_face: Option<CardName>,
}

/// The adventure half of an adventure card (e.g., "Stomp" on Bonecrusher Giant).
#[derive(Debug, Clone, Copy)]
pub struct AdventureDef {
    /// The display name of the adventure spell (e.g., "Stomp").
    pub display_name: &'static str,
    /// The mana cost of the adventure spell.
    pub cost: ManaCost,
    /// The card types of the adventure spell (always Instant or Sorcery).
    pub card_types: &'static [CardType],
}

/// Equipment bonus: P/T modification and keyword grants applied when equipped.
#[derive(Debug, Clone, Copy)]
pub struct EquipmentBonus {
    pub power_mod: i16,
    pub toughness_mod: i16,
    pub keywords: Keywords,
}

/// Returns the equip cost (generic mana) for a known equipment card, or None if not equipment.
pub fn equip_cost(card_name: CardName) -> Option<u8> {
    match card_name {
        CardName::SkullClamp => Some(1),
        CardName::Batterskull => Some(5),
        CardName::Shadowspear => Some(2),
        CardName::Shuko => Some(0),
        CardName::LavaspurBoots => Some(1),
        CardName::Nettlecyst => Some(2),
        _ => None,
    }
}

/// Returns the stat bonus granted by an equipment when attached to a creature.
pub fn equipment_bonus(card_name: CardName) -> Option<EquipmentBonus> {
    let mut kw = Keywords::empty();
    match card_name {
        CardName::SkullClamp => Some(EquipmentBonus {
            power_mod: 1,
            toughness_mod: -1,
            keywords: kw,
        }),
        CardName::Batterskull => {
            kw.add(Keyword::Vigilance);
            kw.add(Keyword::Lifelink);
            Some(EquipmentBonus {
                power_mod: 4,
                toughness_mod: 4,
                keywords: kw,
            })
        }
        CardName::Shadowspear => {
            kw.add(Keyword::Trample);
            kw.add(Keyword::Lifelink);
            Some(EquipmentBonus {
                power_mod: 1,
                toughness_mod: 1,
                keywords: kw,
            })
        }
        CardName::Shuko => Some(EquipmentBonus {
            power_mod: 1,
            toughness_mod: 0,
            keywords: kw,
        }),
        CardName::LavaspurBoots => {
            kw.add(Keyword::Haste);
            kw.add(Keyword::Ward);
            Some(EquipmentBonus {
                power_mod: 1,
                toughness_mod: 0,
                keywords: kw,
            })
        }
        CardName::Nettlecyst => {
            // Nettlecyst gives +1/+1 for each artifact and/or enchantment you control.
            // The dynamic bonus is computed in effective_power/effective_toughness;
            // return a zero-bonus entry so it's recognized as equipment.
            Some(EquipmentBonus {
                power_mod: 0,
                toughness_mod: 0,
                keywords: kw,
            })
        }
        _ => None,
    }
}

/// Returns the annihilator N value for a creature, or 0 if it has no annihilator.
/// Annihilator N means: when this creature attacks, the defending player sacrifices N permanents.
pub fn annihilator_value(card_name: CardName) -> u8 {
    match card_name {
        CardName::EmrakulTheAeonsTorn => 6,
        _ => 0,
    }
}

/// Cycling ability info: cost and whether it's a basic cycling (draw a card) or special.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclingKind {
    /// Pay cost, discard this card, draw a card.
    Basic,
    /// Shark Typhoon: pay {X}{U}, discard, create an X/X Shark with flying.
    SharkTyphoon,
    /// Swampcycling: discard, search library for a Swamp card, put it into hand.
    Swampcycling,
    /// Islandcycling: discard, search library for an Island card, put it into hand.
    Islandcycling,
}

/// Returns the cycling cost and kind for a card in hand, or None if it can't cycle.
/// Returns (cost, kind). Cost is the colored/generic part (before X for SharkTyphoon).
pub fn cycling_ability(card_name: CardName) -> Option<(ManaCost, CyclingKind)> {
    use crate::mana::ManaCost;
    match card_name {
        // Street Wraith: cycling costs 2 life (life payment handled separately; cost is zero mana)
        CardName::StreetWraith => Some((ManaCost::ZERO, CyclingKind::Basic)),
        // Troll of Khazad-dum: Swamp cycling {1}
        CardName::TrollOfKhazadDum => Some((ManaCost::generic(1), CyclingKind::Swampcycling)),
        // Lorien Revealed: Island cycling {1}
        CardName::LorienRevealed => Some((ManaCost::generic(1), CyclingKind::Islandcycling)),
        // Step Through: Wizardcycling {2}
        CardName::StepThrough => Some((ManaCost::generic(2), CyclingKind::Basic)),
        // Shark Typhoon: cycling {X}{U}
        CardName::SharkTyphoon => Some((ManaCost::u(1), CyclingKind::SharkTyphoon)),
        // Hollow One: cycling {2}
        CardName::HollowOne => Some((ManaCost::generic(2), CyclingKind::Basic)),
        // Pest Control: cycling {2}
        CardName::PestControl => Some((ManaCost::generic(2), CyclingKind::Basic)),
        _ => None,
    }
}

/// Channel ability info: whether a card has a channel ability and what its cost is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    /// Boseiju: {1}{G}, discard → destroy target artifact, enchantment, or nonbasic land.
    Boseiju,
    /// Otawara: {3}{U}, discard → bounce target artifact, creature, or planeswalker.
    Otawara,
}

/// Returns the channel cost and kind for a card in hand, or None if it has no channel.
pub fn channel_ability(card_name: CardName) -> Option<(ManaCost, ChannelKind)> {
    use crate::mana::ManaCost;
    match card_name {
        CardName::BoseijuWhoEndures => Some((ManaCost { green: 1, generic: 1, ..ManaCost::ZERO }, ChannelKind::Boseiju)),
        CardName::OtawaraSoaringCity => Some((ManaCost { blue: 1, generic: 3, ..ManaCost::ZERO }, ChannelKind::Otawara)),
        _ => None,
    }
}

/// Build the complete card database. Called once at startup.
pub fn build_card_db() -> Vec<CardDef> {
    use CardName::*;
    use CardType::*;
    use Color::*;
    use SuperType::*;

    let mut db = Vec::with_capacity(CardName::_Count as usize);

    macro_rules! card {
        ($name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: None,
                creature_types: &[],
                is_changeling: false,
                adventure: None,
                back_face: None,
            });
        };
        // Variant with creature types
        (CT($ct:expr) $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: None,
                creature_types: $ct,
                is_changeling: false,
                adventure: None,
                back_face: None,
            });
        };
        // Variant with changeling (all creature types)
        (CHANGELING $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: None,
                creature_types: &[],
                is_changeling: true,
                adventure: None,
                back_face: None,
            });
        };
        // Variant with X cost: x_mult is how many times X appears (1 or 2)
        (X($x_mult:expr) $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: true,
                x_multiplier: $x_mult,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: None,
                creature_types: &[],
                is_changeling: false,
                adventure: None,
                back_face: None,
            });
        };
        // Variant with flashback cost
        (FB($fb:expr) $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: Some($fb),
                madness_cost: None,
                creature_types: &[],
                is_changeling: false,
                adventure: None,
                back_face: None,
            });
        };
        // Variant with madness cost
        (MADNESS($mc:expr) $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: Some($mc),
                creature_types: &[],
                is_changeling: false,
                adventure: None,
                back_face: None,
            });
        };
        // Variant with adventure half
        (ADVENTURE($adv:expr) $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: None,
                creature_types: &[],
                is_changeling: false,
                adventure: Some($adv),
                back_face: None,
            });
        };
        // Variant with adventure half and creature types
        (ADVENTURE($adv:expr) CT($ct:expr) $name:expr, $display:expr, $cost:expr, $types:expr, $supers:expr,
         $pow:expr, $tou:expr, $loy:expr, $kw:expr, $colors:expr, $text:expr) => {
            db.push(CardDef {
                name: $name,
                display_name: $display,
                mana_cost: $cost,
                has_x_cost: false,
                x_multiplier: 0,
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
                flashback_cost: None,
                madness_cost: None,
                creature_types: $ct,
                is_changeling: false,
                adventure: Some($adv),
                back_face: None,
            });
        };
    }

    let kw = Keywords::empty;
    let flying = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flying);
        k
    };
    let haste = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Haste);
        k
    };
    let flash = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flash);
        k
    };
    let flash_flying = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flash);
        k.add(Keyword::Flying);
        k
    };
    let prowess_haste = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Prowess);
        k.add(Keyword::Haste);
        k
    };
    #[allow(unused)]
    let flying_lifelink = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flying);
        k.add(Keyword::Lifelink);
        k
    };
    #[allow(unused)]
    let first_strike = || {
        let mut k = Keywords::empty();
        k.add(Keyword::FirstStrike);
        k
    };
    #[allow(unused)]
    let deathtouch = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Deathtouch);
        k
    };
    #[allow(unused)]
    let lifelink = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Lifelink);
        k
    };
    #[allow(unused)]
    let trample = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Trample);
        k
    };
    #[allow(unused)]
    let hexproof = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Hexproof);
        k
    };
    #[allow(unused)]
    let menace = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Menace);
        k
    };
    #[allow(unused)]
    let menace_haste = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Menace);
        k.add(Keyword::Haste);
        k
    };
    #[allow(unused)]
    let storm = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Storm);
        k
    };
    #[allow(unused)]
    let vigilance = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Vigilance);
        k
    };
    #[allow(unused)]
    let reach = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Reach);
        k
    };
    #[allow(unused)]
    let flash_lifelink = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flash);
        k.add(Keyword::Lifelink);
        k
    };
    #[allow(unused)]
    let deathtouch_lifelink = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Deathtouch);
        k.add(Keyword::Lifelink);
        k
    };
    #[allow(unused)]
    let flying_trample = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flying);
        k.add(Keyword::Trample);
        k
    };
    #[allow(unused)]
    let flying_vigilance_deathtouch_lifelink = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flying);
        k.add(Keyword::Vigilance);
        k.add(Keyword::Deathtouch);
        k.add(Keyword::Lifelink);
        k
    };
    #[allow(unused)]
    let flash_reach = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Flash);
        k.add(Keyword::Reach);
        k
    };
    #[allow(unused)]
    let shadow = || {
        let mut k = Keywords::empty();
        k.add(Keyword::Shadow);
        k
    };
    let c = ManaCost::ZERO;

    // === Basic Lands ===
    card!(Plains, "Plains", c, &[Land], &[Basic], None, None, None, kw(), &[White],
        "{T}: Add {W}.");
    card!(Island, "Island", c, &[Land], &[Basic], None, None, None, kw(), &[Blue],
        "{T}: Add {U}.");
    card!(Swamp, "Swamp", c, &[Land], &[Basic], None, None, None, kw(), &[Black],
        "{T}: Add {B}.");
    card!(Mountain, "Mountain", c, &[Land], &[Basic], None, None, None, kw(), &[Red],
        "{T}: Add {R}.");
    card!(Forest, "Forest", c, &[Land], &[Basic], None, None, None, kw(), &[Green],
        "{T}: Add {G}.");

    // === Dual Lands ===
    card!(UndergroundSea, "Underground Sea", c, &[Land], &[], None, None, None, kw(), &[Blue, Black],
        "{T}: Add {U} or {B}.");
    card!(VolcanicIsland, "Volcanic Island", c, &[Land], &[], None, None, None, kw(), &[Blue, Red],
        "{T}: Add {U} or {R}.");
    card!(Tundra, "Tundra", c, &[Land], &[], None, None, None, kw(), &[White, Blue],
        "{T}: Add {W} or {U}.");
    card!(TropicalIsland, "Tropical Island", c, &[Land], &[], None, None, None, kw(), &[Blue, Green],
        "{T}: Add {U} or {G}.");
    card!(Badlands, "Badlands", c, &[Land], &[], None, None, None, kw(), &[Black, Red],
        "{T}: Add {B} or {R}.");
    card!(Bayou, "Bayou", c, &[Land], &[], None, None, None, kw(), &[Black, Green],
        "{T}: Add {B} or {G}.");
    card!(Plateau, "Plateau", c, &[Land], &[], None, None, None, kw(), &[Red, White],
        "{T}: Add {R} or {W}.");
    card!(Savannah, "Savannah", c, &[Land], &[], None, None, None, kw(), &[Green, White],
        "{T}: Add {G} or {W}.");
    card!(Scrubland, "Scrubland", c, &[Land], &[], None, None, None, kw(), &[White, Black],
        "{T}: Add {W} or {B}.");
    card!(Taiga, "Taiga", c, &[Land], &[], None, None, None, kw(), &[Red, Green],
        "{T}: Add {R} or {G}.");

    // === Power 9 ===
    card!(BlackLotus, "Black Lotus", c, &[Artifact], &[], None, None, None, kw(), &[],
        "{T}, Sacrifice Black Lotus: Add three mana of any one color.");
    card!(AncestralRecall, "Ancestral Recall", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Target player draws three cards.");
    card!(TimeWalk, "Time Walk", ManaCost { blue: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Take an extra turn after this one.");
    card!(TemporalMastery, "Temporal Mastery", ManaCost { blue: 2, generic: 5, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Take an extra turn after this one. Miracle {1}{U}.");
    card!(Timetwister, "Timetwister", ManaCost { blue: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Each player shuffles their hand and graveyard into their library, then draws seven cards.");
    card!(MoxPearl, "Mox Pearl", c, &[Artifact], &[], None, None, None, kw(), &[White],
        "{T}: Add {W}.");
    card!(MoxSapphire, "Mox Sapphire", c, &[Artifact], &[], None, None, None, kw(), &[Blue],
        "{T}: Add {U}.");
    card!(MoxJet, "Mox Jet", c, &[Artifact], &[], None, None, None, kw(), &[Black],
        "{T}: Add {B}.");
    card!(MoxRuby, "Mox Ruby", c, &[Artifact], &[], None, None, None, kw(), &[Red],
        "{T}: Add {R}.");
    card!(MoxEmerald, "Mox Emerald", c, &[Artifact], &[], None, None, None, kw(), &[Green],
        "{T}: Add {G}.");

    // === Fast Mana ===
    card!(SolRing, "Sol Ring", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "{T}: Add {C}{C}.");
    card!(ManaCrypt, "Mana Crypt", c, &[Artifact], &[], None, None, None, kw(), &[],
        "At the beginning of your upkeep, flip a coin. If you lose the flip, Mana Crypt deals 3 damage to you. {T}: Add {C}{C}.");
    card!(ManaVault, "Mana Vault", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "{T}: Add {C}{C}{C}. Mana Vault doesn't untap during your untap step.");
    card!(LotusPetal, "Lotus Petal", c, &[Artifact], &[], None, None, None, kw(), &[],
        "{T}, Sacrifice Lotus Petal: Add one mana of any color.");
    card!(LionEyeDiamond, "Lion's Eye Diamond", c, &[Artifact], &[], None, None, None, kw(), &[],
        "Discard your hand, {T}, Sacrifice: Add three mana of any one color. Activate only as an instant.");
    card!(GrimMonolith, "Grim Monolith", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "{T}: Add {C}{C}{C}. Grim Monolith doesn't untap during your untap step. {4}: Untap Grim Monolith.");
    card!(ChromeMox, "Chrome Mox", c, &[Artifact], &[], None, None, None, kw(), &[],
        "Imprint - When Chrome Mox enters, you may exile a nonartifact, nonland card from your hand. {T}: Add one mana of any of the exiled card's colors.");
    card!(MoxDiamond, "Mox Diamond", c, &[Artifact], &[], None, None, None, kw(), &[],
        "If Mox Diamond would enter, you may discard a land card instead. If you do, put Mox Diamond onto the battlefield. {T}: Add one mana of any color.");
    card!(MoxOpal, "Mox Opal", c, &[Artifact], &[Legendary], None, None, None, kw(), &[],
        "Metalcraft - {T}: Add one mana of any color. Activate only if you control three or more artifacts.");

    // === Blue Spells ===
    card!(Counterspell, "Counterspell", ManaCost { blue: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target spell.");
    card!(ForceOfWill, "Force of Will", ManaCost { blue: 1, generic: 3, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "You may pay 1 life and exile a blue card from your hand rather than pay this spell's mana cost. Counter target spell.");
    card!(Brainstorm, "Brainstorm", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Draw three cards, then put two cards from your hand on top of your library in any order.");
    card!(Ponder, "Ponder", ManaCost::u(1), &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Look at the top three cards of your library, then put them back in any order. You may shuffle. Draw a card.");
    card!(Preordain, "Preordain", ManaCost::u(1), &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Scry 2, then draw a card.");
    card!(ManaDrain, "Mana Drain", ManaCost { blue: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell's mana value.");
    card!(MysticalTutor, "Mystical Tutor", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Search your library for an instant or sorcery card, reveal it, then shuffle and put it on top.");
    card!(TreasureCruise, "Treasure Cruise", ManaCost { blue: 1, generic: 7, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Delve. Draw three cards.");
    card!(DigThroughTime, "Dig Through Time", ManaCost { blue: 2, generic: 6, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Delve. Look at the top seven cards of your library. Put two into your hand and the rest on the bottom in any order.");
    card!(MentalMisstep, "Mental Misstep", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target spell with mana value 1.");
    card!(SpellPierce, "Spell Pierce", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target noncreature spell unless its controller pays {2}.");

    // === Blue Creatures ===
    card!(CT(&[CreatureType::Human, CreatureType::Wizard]) SnapcasterMage, "Snapcaster Mage", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, flash(), &[Blue],
        "Flash. When Snapcaster Mage enters, target instant or sorcery card in your graveyard gains flashback until end of turn.");
    card!(CT(&[CreatureType::Merfolk, CreatureType::Rogue]) TrueNameNemesis, "True-Name Nemesis", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(1), None, kw(), &[Blue],
        "As True-Name Nemesis enters, choose a player. True-Name Nemesis has protection from the chosen player.");
    card!(CT(&[CreatureType::Merfolk, CreatureType::Pirate]) Hullbreacher, "Hullbreacher", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(2), None, flash(), &[Blue],
        "Flash. If an opponent would draw a card except the first one they draw in each of their draw steps, instead you create a Treasure token.");
    card!(CT(&[CreatureType::Human, CreatureType::Rogue]) OppositionAgent, "Opposition Agent", ManaCost { black: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(2), None, flash(), &[Black],
        "Flash. You control your opponents while they're searching their libraries.");

    // === Blue Planeswalkers ===
    card!(JaceTheMindSculptor, "Jace, the Mind Sculptor", ManaCost { blue: 2, generic: 2, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(3), kw(), &[Blue],
        "+2: Look at the top card of target player's library. You may put that card on the bottom. 0: Draw three cards, then put two cards from your hand on top in any order. -1: Return target creature to its owner's hand. -12: Exile all cards from target player's library, then that player shuffles their hand into their library.");

    // === Black Spells ===
    card!(DarkRitual, "Dark Ritual", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "Add {B}{B}{B}.");
    card!(DemonicTutor, "Demonic Tutor", ManaCost { black: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Search your library for a card, put it into your hand, then shuffle.");
    card!(Thoughtseize, "Thoughtseize", ManaCost::b(1), &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Target player reveals their hand. You choose a nonland card from it. That player discards that card. You lose 2 life.");
    card!(HymnToTourach, "Hymn to Tourach", ManaCost { black: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Target player discards two cards at random.");
    card!(ToxicDeluge, "Toxic Deluge", ManaCost { black: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "As an additional cost to cast this spell, pay X life. All creatures get -X/-X until end of turn.");
    card!(Reanimate, "Reanimate", ManaCost::b(1), &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to its mana value.");
    card!(Entomb, "Entomb", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "Search your library for a card, put it into your graveyard, then shuffle.");
    card!(VampiricTutor, "Vampiric Tutor", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "Search your library for a card, then shuffle and put it on top. You lose 2 life.");
    card!(YawgmothsWill, "Yawgmoth's Will", ManaCost { black: 1, generic: 2, ..c }, &[Sorcery], &[Legendary], None, None, None, kw(), &[Black],
        "Until end of turn, you may play lands and cast spells from your graveyard. If a card would be put into your graveyard from anywhere this turn, exile it instead.");
    card!(TendrilsOfAgony, "Tendrils of Agony", ManaCost { black: 2, generic: 2, ..c }, &[Sorcery], &[], None, None, None, storm(), &[Black],
        "Target player loses 2 life and you gain 2 life. Storm.");

    // === Black Creatures ===
    card!(CT(&[CreatureType::Phyrexian, CreatureType::Praetor]) SheoldredTheApocalypse, "Sheoldred, the Apocalypse", ManaCost { black: 2, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(4), Some(5), None, deathtouch(), &[Black],
        "Deathtouch. Whenever you draw a card, you gain 2 life. Whenever an opponent draws a card, they lose 2 life.");
    card!(CT(&[CreatureType::Human, CreatureType::Wizard]) DarkConfidant, "Dark Confidant", ManaCost { black: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, kw(), &[Black],
        "At the beginning of your upkeep, reveal the top card of your library and put it into your hand. You lose life equal to its mana value.");

    // === Red Spells ===
    card!(LightningBolt, "Lightning Bolt", ManaCost::r(1), &[Instant], &[], None, None, None, kw(), &[Red],
        "Lightning Bolt deals 3 damage to any target.");
    card!(WheelOfFortune, "Wheel of Fortune", ManaCost { red: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Each player discards their hand, then draws seven cards.");
    card!(Pyroblast, "Pyroblast", ManaCost::r(1), &[Instant], &[], None, None, None, kw(), &[Red],
        "Choose one: Counter target spell if it's blue. Destroy target permanent if it's blue.");
    card!(RedElementalBlast, "Red Elemental Blast", ManaCost::r(1), &[Instant], &[], None, None, None, kw(), &[Red],
        "Choose one: Counter target blue spell. Destroy target blue permanent.");
    card!(ChainLightning, "Chain Lightning", ManaCost::r(1), &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Chain Lightning deals 3 damage to any target.");

    // === Red Creatures ===
    card!(CT(&[CreatureType::Goblin]) GoblinGuide, "Goblin Guide", ManaCost::r(1), &[Creature], &[],
        Some(2), Some(2), None, haste(), &[Red],
        "Haste. Whenever Goblin Guide attacks, defending player reveals the top card of their library. If it's a land card, that player puts it into their hand.");
    card!(CT(&[CreatureType::Human, CreatureType::Monk]) MonasterySwiftspear, "Monastery Swiftspear", ManaCost::r(1), &[Creature], &[],
        Some(1), Some(2), None, prowess_haste(), &[Red],
        "Haste. Prowess.");
    card!(CT(&[CreatureType::Monkey, CreatureType::Pirate]) RagavanNimblePilferer, "Ragavan, Nimble Pilferer", ManaCost::r(1), &[Creature], &[Legendary],
        Some(2), Some(1), None, haste(), &[Red],
        "Whenever Ragavan deals combat damage to a player, create a Treasure token and exile the top card of that player's library. You may cast that card this turn. Dash {1}{R}.");
    card!(CT(&[CreatureType::Human, CreatureType::Shaman]) YoungPyromancer, "Young Pyromancer", ManaCost { red: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, kw(), &[Red],
        "Whenever you cast an instant or sorcery spell, create a 1/1 red Elemental creature token.");

    // === White Spells ===
    card!(SwordsToPlowshares, "Swords to Plowshares", ManaCost::w(1), &[Instant], &[], None, None, None, kw(), &[White],
        "Exile target creature. Its controller gains life equal to its power.");
    card!(Balance, "Balance", ManaCost { white: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[White],
        "Each player chooses a number of lands they control equal to the number of lands controlled by the player who controls the fewest, then sacrifices the rest. Players discard and sacrifice creatures the same way.");
    card!(CouncilsJudgment, "Council's Judgment", ManaCost { white: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[White],
        "Will of the council - Starting with you, each player votes for a nonland permanent you don't control. Exile each permanent with the most votes or tied for most votes.");
    card!(PathToExile, "Path to Exile", ManaCost::w(1), &[Instant], &[], None, None, None, kw(), &[White],
        "Exile target creature. Its controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle.");
    card!(Armageddon, "Armageddon", ManaCost { white: 1, generic: 3, ..c }, &[Sorcery], &[], None, None, None, kw(), &[White],
        "Destroy all lands.");
    card!(Disenchant, "Disenchant", ManaCost { white: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[White],
        "Destroy target artifact or enchantment.");

    // === White Creatures ===
    card!(CT(&[CreatureType::Human, CreatureType::Soldier]) ThaliaGuardianOfThraben, "Thalia, Guardian of Thraben", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(1), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::FirstStrike);
            k
        }, &[White],
        "First strike. Noncreature spells cost {1} more to cast.");
    card!(CT(&[CreatureType::Human, CreatureType::Monk]) MonasteryMentor, "Monastery Mentor", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Prowess);
            k
        }, &[White],
        "Prowess. Whenever you cast a noncreature spell, create a 1/1 white Monk creature token with prowess.");
    card!(CT(&[CreatureType::Elemental]) Solitude, "Solitude", ManaCost { white: 2, generic: 3, ..c }, &[Creature], &[],
        Some(3), Some(2), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Flash);
            k.add(Keyword::Flying);
            k.add(Keyword::Lifelink);
            k
        }, &[White],
        "Flash. Flying. Lifelink. When Solitude enters, exile up to one other target creature. That creature's controller gains life equal to its power. Evoke - Exile a white card from your hand.");
    card!(CT(&[CreatureType::Spirit]) SkyclaveApparition, "Skyclave Apparition", ManaCost { white: 2, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, flying(), &[White],
        "Flying. When Skyclave Apparition enters, exile up to one target nonland, nontoken permanent you don't control with mana value 4 or less. When Skyclave Apparition leaves the battlefield, that permanent's controller creates an X/X blue Illusion creature token, where X is the exiled card's mana value.");
    card!(CT(&[CreatureType::Kor, CreatureType::Artificer]) StoneforgeMystic, "Stoneforge Mystic", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, kw(), &[White],
        "When Stoneforge Mystic enters, you may search your library for an Equipment card, reveal it, put it into your hand, then shuffle. {1}{W}, {T}: You may put an Equipment card from your hand onto the battlefield.");
    card!(CT(&[CreatureType::Human, CreatureType::Soldier]) PalaceJailer, "Palace Jailer", ManaCost { white: 2, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[White],
        "When Palace Jailer enters, you become the monarch. When Palace Jailer enters, exile target creature an opponent controls until an opponent becomes the monarch.");
    card!(CT(&[CreatureType::Human, CreatureType::Cleric]) AuriokChampion, "Auriok Champion", ManaCost { white: 2, ..c }, &[Creature], &[],
        Some(1), Some(1), None, kw(), &[White],
        "Protection from black and from red. Whenever another creature enters the battlefield, you may gain 1 life.");
    card!(CT(&[CreatureType::Kor, CreatureType::Soldier]) KorFirewalker, "Kor Firewalker", ManaCost { white: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[White],
        "Protection from red. Whenever a player casts a red spell, you may gain 1 life.");

    // === Green Spells ===
    card!(Channel, "Channel", ManaCost { green: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Green],
        "Until end of turn, any time you could activate a mana ability, you may pay 1 life. If you do, add {C}.");
    card!(GreenSunsZenith, "Green Sun's Zenith", ManaCost { green: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Green],
        "Search your library for a green creature card with mana value X or less, put it onto the battlefield, then shuffle. Shuffle Green Sun's Zenith into its owner's library.");
    card!(NaturalOrder, "Natural Order", ManaCost { green: 2, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Green],
        "As an additional cost, sacrifice a green creature. Search your library for a green creature card, put it onto the battlefield, then shuffle.");
    card!(Regrowth, "Regrowth", ManaCost { green: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Green],
        "Return target card from your graveyard to your hand.");

    // === Green Creatures ===
    card!(CT(&[CreatureType::Bird]) BirdsOfParadise, "Birds of Paradise", ManaCost::g(1), &[Creature], &[],
        Some(0), Some(1), None, flying(), &[Green],
        "Flying. {T}: Add one mana of any color.");
    card!(CT(&[CreatureType::Ouphe]) CollectorOuphe, "Collector Ouphe", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Green],
        "Activated abilities of artifacts can't be activated.");
    card!(CT(&[CreatureType::Elemental, CreatureType::Incarnation]) Endurance, "Endurance", ManaCost { green: 2, generic: 1, ..c }, &[Creature], &[],
        Some(3), Some(4), None, flash_reach(), &[Green],
        "Flash. Reach. When Endurance enters, up to one target player puts all the cards from their graveyard on the bottom of their library in a random order.");
    card!(QuirionRanger, "Quirion Ranger", ManaCost::g(1), &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "Return a Forest you control to its owner's hand: Untap target creature. Activate only once each turn.");

    // === Multicolor ===
    card!(TeferiTimeRaveler, "Teferi, Time Raveler", ManaCost { white: 1, blue: 1, generic: 1, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(4), kw(), &[White, Blue],
        "Each opponent can cast spells only any time they could cast a sorcery. +1: Until your next turn, you may cast sorcery spells as though they had flash. -3: Return up to one target artifact, creature, or enchantment to its owner's hand. Draw a card.");
    card!(LeovoldEmissaryOfTrest, "Leovold, Emissary of Trest", ManaCost { black: 1, blue: 1, green: 1, ..c }, &[Creature], &[Legendary],
        Some(3), Some(3), None, kw(), &[Black, Blue, Green],
        "Each opponent can't draw more than one card each turn. Whenever you or a permanent you control becomes the target of a spell or ability an opponent controls, you may draw a card.");
    card!(KolaghanCommand, "Kolaghan's Command", ManaCost { black: 1, red: 1, generic: 1, ..c }, &[Instant], &[],
        None, None, None, kw(), &[Black, Red],
        "Choose two: Return target creature card from your graveyard to your hand. Target player discards a card. Destroy target artifact. Kolaghan's Command deals 2 damage to any target.");
    card!(DackFayden, "Dack Fayden", ManaCost { blue: 1, red: 1, generic: 1, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(3), kw(), &[Blue, Red],
        "+1: Target player draws two cards, then discards two cards. -2: Gain control of target artifact. -6: You get an emblem with \"Whenever you cast a spell that targets one or more permanents, gain control of those permanents.\"");

    // === Colorless Artifacts ===
    card!(Batterskull, "Batterskull", ManaCost::generic(5), &[Artifact], &[],
        None, None, None, kw(), &[],
        "Living weapon. Equipped creature gets +4/+4 and has vigilance and lifelink. {3}: Return Batterskull to its owner's hand. Equip {5}.");
    card!(SkullClamp, "Skullclamp", ManaCost::generic(1), &[Artifact], &[],
        None, None, None, kw(), &[],
        "Equipped creature gets +1/-1. When equipped creature dies, draw two cards. Equip {1}.");
    card!(IsochronScepter, "Isochron Scepter", ManaCost::generic(2), &[Artifact], &[Legendary],
        None, None, None, kw(), &[],
        "Imprint - When Isochron Scepter enters, you may exile an instant card with mana value 2 or less from your hand. {2}, {T}: You may copy the exiled card. If you do, you may cast the copy without paying its mana cost.");

    // === Fetch Lands (simplified - they search for land types) ===
    card!(FloodedStrand, "Flooded Strand", c, &[Land], &[], None, None, None, kw(), &[White, Blue],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Plains or Island card, put it onto the battlefield, then shuffle.");
    card!(PollutedDelta, "Polluted Delta", c, &[Land], &[], None, None, None, kw(), &[Blue, Black],
        "{T}, Pay 1 life, Sacrifice: Search your library for an Island or Swamp card, put it onto the battlefield, then shuffle.");
    card!(BloodstainedMire, "Bloodstained Mire", c, &[Land], &[], None, None, None, kw(), &[Black, Red],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Swamp or Mountain card, put it onto the battlefield, then shuffle.");
    card!(WoodedFoothills, "Wooded Foothills", c, &[Land], &[], None, None, None, kw(), &[Red, Green],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Mountain or Forest card, put it onto the battlefield, then shuffle.");
    card!(WindsweptHeath, "Windswept Heath", c, &[Land], &[], None, None, None, kw(), &[Green, White],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Forest or Plains card, put it onto the battlefield, then shuffle.");
    card!(MistyRainforest, "Misty Rainforest", c, &[Land], &[], None, None, None, kw(), &[Blue, Green],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Forest or Island card, put it onto the battlefield, then shuffle.");
    card!(ScaldingTarn, "Scalding Tarn", c, &[Land], &[], None, None, None, kw(), &[Blue, Red],
        "{T}, Pay 1 life, Sacrifice: Search your library for an Island or Mountain card, put it onto the battlefield, then shuffle.");
    card!(VerdantCatacombs, "Verdant Catacombs", c, &[Land], &[], None, None, None, kw(), &[Black, Green],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Swamp or Forest card, put it onto the battlefield, then shuffle.");
    card!(AridMesa, "Arid Mesa", c, &[Land], &[], None, None, None, kw(), &[Red, White],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Mountain or Plains card, put it onto the battlefield, then shuffle.");
    card!(MarshFlats, "Marsh Flats", c, &[Land], &[], None, None, None, kw(), &[White, Black],
        "{T}, Pay 1 life, Sacrifice: Search your library for a Plains or Swamp card, put it onto the battlefield, then shuffle.");

    // === Other Lands ===
    card!(LibraryOfAlexandria, "Library of Alexandria", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}. {T}: Draw a card. Activate only if you have exactly seven cards in hand.");
    card!(StripMine, "Strip Mine", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}. {T}, Sacrifice Strip Mine: Destroy target land.");
    card!(Wasteland, "Wasteland", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}. {T}, Sacrifice Wasteland: Destroy target nonbasic land.");
    card!(TolarianAcademy, "Tolarian Academy", c, &[Land], &[Legendary], None, None, None, kw(), &[Blue],
        "{T}: Add {U} for each artifact you control.");
    card!(AncientTomb, "Ancient Tomb", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}{C}. Ancient Tomb deals 2 damage to you.");
    card!(MishrasWorkshop, "Mishra's Workshop", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}{C}{C}. Spend this mana only to cast artifact spells.");

    // === Shock Lands ===
    card!(HallowedFountain, "Hallowed Fountain", c, &[Land], &[], None, None, None, kw(), &[White, Blue],
        "As Hallowed Fountain enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {W} or {U}.");
    card!(WateryGrave, "Watery Grave", c, &[Land], &[], None, None, None, kw(), &[Blue, Black],
        "As Watery Grave enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {U} or {B}.");
    card!(BloodCrypt, "Blood Crypt", c, &[Land], &[], None, None, None, kw(), &[Black, Red],
        "As Blood Crypt enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {B} or {R}.");
    card!(StompingGround, "Stomping Ground", c, &[Land], &[], None, None, None, kw(), &[Red, Green],
        "As Stomping Ground enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {R} or {G}.");
    card!(TempleGarden, "Temple Garden", c, &[Land], &[], None, None, None, kw(), &[Green, White],
        "As Temple Garden enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {G} or {W}.");
    card!(GodlessShrine, "Godless Shrine", c, &[Land], &[], None, None, None, kw(), &[White, Black],
        "As Godless Shrine enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {W} or {B}.");
    card!(SteamVents, "Steam Vents", c, &[Land], &[], None, None, None, kw(), &[Blue, Red],
        "As Steam Vents enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {U} or {R}.");
    card!(OvergrownTomb, "Overgrown Tomb", c, &[Land], &[], None, None, None, kw(), &[Black, Green],
        "As Overgrown Tomb enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {B} or {G}.");
    card!(SacredFoundry, "Sacred Foundry", c, &[Land], &[], None, None, None, kw(), &[Red, White],
        "As Sacred Foundry enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {R} or {W}.");
    card!(BreedingPool, "Breeding Pool", c, &[Land], &[], None, None, None, kw(), &[Green, Blue],
        "As Breeding Pool enters, you may pay 2 life. If you don't, it enters tapped. {T}: Add {G} or {U}.");

    // === Survey/Misc Dual Lands ===
    card!(MeticulousArchive, "Meticulous Archive", c, &[Land], &[], None, None, None, kw(), &[White, Blue],
        "Meticulous Archive enters tapped. When Meticulous Archive enters, surveil 1. {T}: Add {W} or {U}.");
    card!(UndercitySewers, "Undercity Sewers", c, &[Land], &[], None, None, None, kw(), &[Blue, Black],
        "Undercity Sewers enters tapped. When Undercity Sewers enters, surveil 1. {T}: Add {U} or {B}.");
    card!(ThunderingFalls, "Thundering Falls", c, &[Land], &[], None, None, None, kw(), &[Blue, Red],
        "Thundering Falls enters tapped. When Thundering Falls enters, surveil 1. {T}: Add {U} or {R}.");
    card!(HedgeMaze, "Hedge Maze", c, &[Land], &[], None, None, None, kw(), &[Green, Blue],
        "Hedge Maze enters tapped. When Hedge Maze enters, surveil 1. {T}: Add {G} or {U}.");

    // === Other Lands ===
    card!(Karakas, "Karakas", c, &[Land], &[Legendary], None, None, None, kw(), &[White],
        "{T}: Add {W}. {T}: Return target legendary creature to its owner's hand.");
    card!(UrborgTombOfYawgmoth, "Urborg, Tomb of Yawgmoth", c, &[Land], &[Legendary], None, None, None, kw(), &[Black],
        "Each land is a Swamp in addition to its other land types.");
    card!(OtawaraSoaringCity, "Otawara, Soaring City", c, &[Land], &[Legendary], None, None, None, kw(), &[Blue],
        "{T}: Add {U}. Channel - {3}{U}, Discard Otawara: Return target artifact, creature, enchantment, or planeswalker to its owner's hand.");
    card!(BoseijuWhoEndures, "Boseiju, Who Endures", c, &[Land], &[Legendary], None, None, None, kw(), &[Green],
        "{T}: Add {G}. Channel - {1}{G}, Discard Boseiju: Destroy target artifact, enchantment, or nonbasic land an opponent controls. That player may search for a land with a basic land type and put it tapped.");
    card!(GaeasCradle, "Gaea's Cradle", c, &[Land], &[Legendary], None, None, None, kw(), &[Green],
        "{T}: Add {G} for each creature you control.");
    card!(YavimayaCradleOfGrowth, "Yavimaya, Cradle of Growth", c, &[Land], &[Legendary], None, None, None, kw(), &[Green],
        "Each land is a Forest in addition to its other land types.");
    card!(CityOfTraitors, "City of Traitors", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}{C}. When you play another land, sacrifice City of Traitors.");
    card!(ForbiddenOrchard, "Forbidden Orchard", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add one mana of any color. Whenever you tap Forbidden Orchard for mana, target opponent creates a 1/1 colorless Spirit creature token.");
    card!(GhostQuarter, "Ghost Quarter", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}. {T}, Sacrifice Ghost Quarter: Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle.");
    card!(SpireOfIndustry, "Spire of Industry", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}. {T}, Pay 1 life: Add one mana of any color. Activate only if you control an artifact.");
    card!(StartingTown, "Starting Town", c, &[Land], &[], None, None, None, kw(), &[],
        "Starting Town enters tapped unless it's turn 1, 2, or 3. {T}: Add {C}. {T}, Pay 1 life: Add one mana of any color.");
    card!(TalonGatesOfMadara, "Talon Gates of Madara", c, &[Land], &[Legendary], None, None, None, kw(), &[],
        "When this land enters, up to one target creature phases out. {T}: Add {C}. {1}, {T}: Add one mana of any color. {4}: Put this card from your hand onto the battlefield.");
    card!(ShelldockIsle, "Shelldock Isle", c, &[Land], &[], None, None, None, kw(), &[],
        "Hideaway 4. {T}: Add {U}. {T}: You may play the exiled card without paying its mana cost if there are twenty or fewer cards in your library.");
    card!(MosswortBridge, "Mosswort Bridge", c, &[Land], &[], None, None, None, kw(), &[],
        "Hideaway 4. {T}: Add {G}. {T}: You may play the exiled card without paying its mana cost if you control a creature with power 10 or greater.");
    card!(TheMycoSynthGardens, "The Mycosynth Gardens", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Add {C}. {1}, {T}: Add one mana of any color. {X}: The Mycosynth Gardens becomes a copy of target artifact you control with mana value X.");
    card!(UrzasSaga, "Urza's Saga", c, &[Land, Enchantment], &[], None, None, None, kw(), &[],
        "I - Urza's Saga gains '{T}: Add {C}.' II - Urza's Saga gains '{2}, {T}: Create a 0/0 colorless Construct artifact creature token with \"This creature gets +1/+1 for each artifact you control.\"' III - Search your library for an artifact card with mana cost {0} or {1}, put it onto the battlefield, then shuffle.");
    card!(BazaarOfBaghdad, "Bazaar of Baghdad", c, &[Land], &[], None, None, None, kw(), &[],
        "{T}: Draw two cards, then discard three cards.");
    card!(DryadArbor, "Dryad Arbor", c, &[Land, Creature], &[], Some(1), Some(1), None, kw(), &[Green],
        "Dryad Arbor is green.");
    // Cavern of Souls: named creature type produces colored mana and makes spells uncounterable
    card!(CavernOfSouls, "Cavern of Souls", c, &[Land], &[], None, None, None, kw(), &[],
        "As Cavern of Souls enters, choose a creature type. {T}: Add {C}. {T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type. That spell can't be countered.");

    // === White Creatures ===
    card!(CT(&[CreatureType::Kor, CreatureType::Soldier]) NomadsEnKor, "Nomads en-Kor", ManaCost::w(1), &[Creature], &[],
        Some(1), Some(1), None, kw(), &[White],
        "{0}: The next 1 damage that would be dealt to Nomads en-Kor this turn is dealt to target creature you control instead.");
    card!(CT(&[CreatureType::Cat, CreatureType::Warrior]) AjaniNacatlPariah, "Ajani, Nacatl Pariah", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(1), Some(2), None, kw(), &[White],
        "When Ajani enters, create a 2/1 white Cat Warrior creature token. Whenever one or more other Cats you control die, exile Ajani, then return him transformed.");
    card!(CT(&[CreatureType::Human, CreatureType::Soldier]) CatharCommando, "Cathar Commando", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(3), Some(1), None, flash(), &[White],
        "Flash. {1}, Sacrifice Cathar Commando: Destroy target artifact or enchantment.");
    card!(CT(&[CreatureType::Human, CreatureType::Cleric]) ContainmentPriest, "Containment Priest", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, flash(), &[White],
        "Flash. If a nontoken creature would enter the battlefield and it wasn't cast, exile it instead.");
    card!(CT(&[CreatureType::Human, CreatureType::Artificer]) DauntlessDismantler, "Dauntless Dismantler", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(4), None, kw(), &[White],
        "Artifacts your opponents control enter tapped. {X}{X}{W}, Sacrifice this creature: Destroy each artifact with mana value X.");
    card!(CT(&[CreatureType::Thrull]) DoorkeeperThrull, "Doorkeeper Thrull", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, flash_flying(), &[White],
        "Flash. Flying. Artifacts and creatures entering don't cause abilities to trigger.");
    card!(CT(&[CreatureType::Human, CreatureType::Wizard]) DrannithMagistrate, "Drannith Magistrate", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(3), None, kw(), &[White],
        "Your opponents can't cast spells from anywhere other than their hands.");
    card!(CT(&[CreatureType::Human, CreatureType::Artificer]) EtherswornCanonist, "Ethersworn Canonist", ManaCost { white: 1, generic: 1, ..c }, &[Artifact, Creature], &[],
        Some(2), Some(2), None, kw(), &[White],
        "Each player who has cast a nonartifact spell this turn can't cast additional nonartifact spells.");
    card!(CT(&[CreatureType::Spirit]) KatakiWarsWage, "Kataki, War's Wage", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(1), None, kw(), &[White],
        "All artifacts have \"At the beginning of your upkeep, sacrifice this artifact unless you pay {1}.\"");
    card!(CT(&[CreatureType::Cat, CreatureType::Cleric]) LeoninArbiter, "Leonin Arbiter", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[White],
        "Players can't search libraries. Any player may pay {2} for that player to ignore this effect until end of turn.");
    card!(CT(&[CreatureType::Human, CreatureType::Artificer]) OswaldFiddlebender, "Oswald Fiddlebender", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, kw(), &[White],
        "{W}, {T}, Sacrifice an artifact: Search your library for an artifact card with mana value equal to 1 plus the sacrificed artifact's mana value, put it onto the battlefield, then shuffle.");
    card!(CT(&[CreatureType::Dog]) PheliaExuberantShepherd, "Phelia, Exuberant Shepherd", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, flash(), &[White],
        "Flash. Whenever Phelia attacks, exile up to one target nonland permanent. If it was a token, it won't return. Otherwise, return it at the beginning of the next end step with a +1/+1 counter on it if it's a creature.");
    card!(CT(&[CreatureType::Halfling]) SamwiseTheStouthearted, "Samwise the Stouthearted", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(1), None, flash(), &[White],
        "Flash. When Samwise enters, choose up to one target permanent card in your graveyard that was put there from the battlefield this turn. Return it to your hand.");
    card!(CT(&[CreatureType::Spirit]) SpiritOfTheLabyrinth, "Spirit of the Labyrinth", ManaCost { white: 1, generic: 1, ..c }, &[Creature, Enchantment], &[],
        Some(3), Some(1), None, kw(), &[White],
        "Each player can't draw more than one card each turn.");
    card!(CT(&[CreatureType::Human]) VoiceOfVictory, "Voice of Victory", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(3), None, kw(), &[White],
        "Mobilize 2 (Whenever this creature attacks, create two tapped and attacking 1/1 red Warrior creature tokens. Sacrifice them at the beginning of the next end step.) Your opponents can't cast spells during your turn.");
    card!(CT(&[CreatureType::Spirit, CreatureType::Knight]) WhiteOrchidPhantom, "White Orchid Phantom", ManaCost { white: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Flying);
            k.add(Keyword::FirstStrike);
            k
        }, &[White],
        "Flying, first strike. When this creature enters, destroy up to one target nonbasic land. Its controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle.");
    card!(CT(&[CreatureType::Archon]) ArchonOfEmeria, "Archon of Emeria", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(3), None, flying(), &[White],
        "Flying. Each player can't cast more than one spell each turn. Nonbasic lands enter tapped.");
    card!(CT(&[CreatureType::Human, CreatureType::Soldier]) BoromirWardenOfTheTower, "Boromir, Warden of the Tower", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(3), Some(3), None, vigilance(), &[White],
        "Vigilance. Whenever an opponent casts a spell, if no mana was spent to cast it, counter that spell. Sacrifice Boromir: Creatures you control gain indestructible until end of turn. The Ring tempts you.");
    card!(CT(&[CreatureType::Dragon]) ClarionConqueror, "Clarion Conqueror", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(3), None, flying(), &[White],
        "Flying. Activated abilities of artifacts, creatures, and planeswalkers can't be activated.");
    card!(CT(&[CreatureType::Human, CreatureType::Artificer]) LoranOfTheThirdPath, "Loran of the Third Path", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(2), Some(1), None, vigilance(), &[White],
        "Vigilance. When Loran enters, destroy target artifact or enchantment an opponent controls. {T}: You and target opponent each draw a card.");
    card!(CT(&[CreatureType::Orc, CreatureType::Cleric]) WhitePlumeAdventurer, "White Plume Adventurer", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(3), None, kw(), &[White],
        "When White Plume Adventurer enters, you take the initiative. At the beginning of each opponent's upkeep, untap a creature you control. If you've completed a dungeon, untap all creatures you control instead.");
    card!(CT(&[CreatureType::Human, CreatureType::Warrior]) SeasonedDungeoneer, "Seasoned Dungeoneer", ManaCost { white: 1, generic: 3, ..c }, &[Creature], &[],
        Some(3), Some(4), None, kw(), &[White],
        "Ward—Discard a card. When Seasoned Dungeoneer enters, you take the initiative. Whenever Seasoned Dungeoneer attacks, target creature you control explores. It gains protection from creatures until end of turn.");

    // === White Spells ===
    card!(EnlightenedTutor, "Enlightened Tutor", ManaCost::w(1), &[Instant], &[], None, None, None, kw(), &[White],
        "Search your library for an artifact or enchantment card, reveal it, then shuffle and put it on top.");
    card!(MarchOfOtherworldlyLight, "March of Otherworldly Light", ManaCost { white: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[White],
        "As an additional cost, you may exile any number of white cards from your hand. Exile target artifact, creature, or enchantment with mana value X or less, where X is the amount of mana paid plus twice the number of cards exiled.");
    card!(Fragmentize, "Fragmentize", ManaCost::w(1), &[Sorcery], &[], None, None, None, kw(), &[White],
        "Destroy target artifact or enchantment with mana value 4 or less.");
    card!(PrismaticEnding, "Prismatic Ending", ManaCost { white: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[White],
        "Converge - Exile target nonland permanent if its mana value is less than or equal to the number of colors of mana spent to cast this spell.");
    card!(WrathOfTheSkies, "Wrath of the Skies", ManaCost { white: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[White],
        "As an additional cost to cast this spell, pay X energy. Destroy each creature and each non-Aura enchantment with mana value X or less.");

    // === White Enchantments/Artifacts ===
    card!(PortableHole, "Portable Hole", ManaCost::w(1), &[Artifact], &[], None, None, None, kw(), &[White],
        "When Portable Hole enters, exile target nonland permanent an opponent controls with mana value 2 or less until Portable Hole leaves the battlefield.");
    card!(DeafeningSilence, "Deafening Silence", ManaCost::w(1), &[Enchantment], &[], None, None, None, kw(), &[White],
        "Each player can't cast more than one noncreature spell each turn.");
    card!(HighNoon, "High Noon", ManaCost { white: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[White],
        "Each player can't cast more than one spell each turn. {4}{R}, Sacrifice this enchantment: It deals 5 damage to any target.");
    card!(RestInPeace, "Rest in Peace", ManaCost { white: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[White],
        "When Rest in Peace enters, exile all graveyards. If a card or token would be put into a graveyard from anywhere, exile it instead.");
    card!(SealOfCleansing, "Seal of Cleansing", ManaCost { white: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[White],
        "Sacrifice Seal of Cleansing: Destroy target artifact or enchantment.");
    card!(StonySilence, "Stony Silence", ManaCost { white: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[White],
        "Activated abilities of artifacts can't be activated.");
    db.push(CardDef {
        name: CardName::WitchEnchanter,
        display_name: "Witch Enchanter",
        mana_cost: ManaCost { white: 1, generic: 3, ..c },
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[Creature],
        supertypes: &[],
        power: Some(2),
        toughness: Some(2),
        loyalty: None,
        keywords: kw(),
        color_identity: &[White],
        oracle_text: "When this creature enters, destroy target artifact or enchantment an opponent controls.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[CreatureType::Human, CreatureType::Warlock],
        is_changeling: false,
        adventure: None,
        back_face: Some(CardName::WitchBlessedMeadow),
    });
    db.push(CardDef {
        name: CardName::WitchBlessedMeadow,
        display_name: "Witch-Blessed Meadow",
        mana_cost: ManaCost::ZERO,
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[Land],
        supertypes: &[],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[White],
        oracle_text: "As this land enters, you may pay 3 life. If you don't, it enters tapped. {T}: Add {W}.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: None,
    });

    // === White Planeswalkers ===
    card!(GideonOfTheTrials, "Gideon of the Trials", ManaCost { white: 2, generic: 1, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(3), kw(), &[White],
        "+1: Until your next turn, prevent all damage target permanent would deal. 0: Until end of turn, Gideon becomes a 4/4 Human Soldier creature with indestructible. +0: You get an emblem with \"As long as you control a Gideon planeswalker, you can't lose the game and your opponents can't win the game.\"");

    // === Double-Faced Cards (DFCs) ===
    // Front face: Delver of Secrets — {U} 1/1 Human Wizard
    // Back face: Insectile Aberration — 3/2 Insect with Flying (no mana cost)
    db.push(CardDef {
        name: CardName::DelverOfSecrets,
        display_name: "Delver of Secrets",
        mana_cost: ManaCost::u(1),
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[CardType::Creature],
        supertypes: &[],
        power: Some(1),
        toughness: Some(1),
        loyalty: None,
        keywords: kw(),
        color_identity: &[Color::Blue],
        oracle_text: "At the beginning of your upkeep, look at the top card of your library. You may reveal it. If it's an instant or sorcery card, transform Delver of Secrets.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[CreatureType::Human, CreatureType::Wizard],
        is_changeling: false,
        adventure: None,
        back_face: Some(CardName::InsectileAberration),
    });
    // Back face of Delver of Secrets — 3/2 Insect with Flying (can't be cast directly)
    db.push(CardDef {
        name: CardName::InsectileAberration,
        display_name: "Insectile Aberration",
        mana_cost: ManaCost::ZERO,
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[CardType::Creature],
        supertypes: &[],
        power: Some(3),
        toughness: Some(2),
        loyalty: None,
        keywords: flying(),
        color_identity: &[Color::Blue],
        oracle_text: "Flying",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[CreatureType::Human, CreatureType::Insect],
        is_changeling: false,
        adventure: None,
        back_face: None,
    });

    // === Blue Creatures ===
    card!(CT(&[CreatureType::Wizard]) TamiyoInquisitiveStudent, "Tamiyo, Inquisitive Student", ManaCost::u(1), &[Creature], &[Legendary],
        Some(0), Some(3), None, flying(), &[Blue],
        "Flying. Whenever Tamiyo attacks, investigate. When you draw your third card in a turn, exile Tamiyo, then return her to the battlefield transformed under her owner's control.");
    card!(AphettoAlchemist, "Aphetto Alchemist", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, kw(), &[Blue],
        "{T}: Untap target artifact or creature. Morph {U}.");
    card!(CT(&[CreatureType::Phyrexian, CreatureType::Rogue]) MercurialSpelldancer, "Mercurial Spelldancer", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::CantBeBlocked);
            k
        }, &[Blue],
        "Mercurial Spelldancer can't be blocked. Whenever you cast a noncreature spell, put an oil counter on Mercurial Spelldancer. Whenever Mercurial Spelldancer deals combat damage to a player, you may remove two oil counters from it. If you do, when you next cast an instant or sorcery spell this turn, copy that spell. You may choose new targets for the copy.");
    card!(ThassasOracle, "Thassa's Oracle", ManaCost { blue: 2, ..c }, &[Creature], &[],
        Some(1), Some(3), None, kw(), &[Blue],
        "When Thassa's Oracle enters, look at the top X cards of your library, where X is your devotion to blue. Put up to one of them on top and the rest on the bottom. If X is greater than or equal to the number of cards in your library, you win the game.");
    card!(CT(&[CreatureType::Merfolk, CreatureType::Rogue]) ThievingSkydiver, "Thieving Skydiver", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, flying(), &[Blue],
        "Kicker {X}. X can't be 0. Flying. When Thieving Skydiver enters, if it was kicked, gain control of target artifact with mana value X or less. If that artifact is an Equipment, attach it to Thieving Skydiver.");
    card!(ThundertrapTrainer, "Thundertrap Trainer", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, kw(), &[Blue],
        "Offspring {4}. When this creature enters, look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.");
    card!(ADVENTURE(AdventureDef {
        display_name: "Petty Theft",
        cost: ManaCost { blue: 1, generic: 1, ..c },
        card_types: &[CardType::Instant],
    }) CT(&[CreatureType::Faerie, CreatureType::Rogue]) BrazenBorrower, "Brazen Borrower", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(1), None, flash_flying(), &[Blue],
        "Flash. Flying. Brazen Borrower can block only creatures with flying. Adventure - Petty Theft {1}{U}: Return target nonland permanent an opponent controls to its owner's hand.");
    card!(EmryLurkerOfTheLoch, "Emry, Lurker of the Loch", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(1), Some(2), None, kw(), &[Blue],
        "This spell costs {1} less for each artifact you control. When Emry enters, mill four cards. {T}: Choose target artifact in your graveyard. You may cast it this turn.");
    card!(PlagonLordOfTheBeach, "Plagon, Lord of the Beach", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(0), Some(3), None, kw(), &[Blue],
        "When Plagon enters, draw a card for each creature you control with toughness greater than its power. {W/U}: Target creature you control assigns combat damage equal to its toughness rather than its power this turn.");
    card!(DisplacerKitten, "Displacer Kitten", ManaCost { blue: 1, generic: 3, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Blue],
        "Avoidance. Whenever you cast a noncreature spell, exile up to one target nonland permanent you control, then return it to the battlefield under its owner's control.");
    card!(KappaCannoneer, "Kappa Cannoneer", ManaCost { blue: 2, generic: 4, ..c }, &[Artifact, Creature], &[],
        Some(4), Some(4), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Ward);
            k
        }, &[Blue],
        "Improvise. Ward {4}. Whenever Kappa Cannoneer enters or whenever you cast an artifact spell, put a +1/+1 counter on Kappa Cannoneer and it can't be blocked this turn.");
    card!(ThoughtMonitor, "Thought Monitor", ManaCost { blue: 1, generic: 6, ..c }, &[Artifact, Creature], &[],
        Some(2), Some(2), None, flying(), &[Blue],
        "Affinity for artifacts. Flying. When Thought Monitor enters, draw two cards.");

    // === Blue Planeswalkers ===
    card!(NarsetParterOfVeils, "Narset, Parter of Veils", ManaCost { blue: 1, generic: 2, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(5), kw(), &[Blue],
        "Each opponent can't draw more than one card each turn. -2: Look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom in a random order.");

    // === Blue Spells (remaining) ===
    card!(ChainOfVapor, "Chain of Vapor", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Return target nonland permanent to its owner's hand. Then that permanent's controller may sacrifice a land. If they do, they may copy this spell and choose a new target.");
    card!(ConsignToMemory, "Consign to Memory", ManaCost::u(1), &[Instant], &[], None, None, None, storm(), &[Blue],
        "Counter target artifact or enchantment spell. Storm.");
    card!(Flusterstorm, "Flusterstorm", ManaCost::u(1), &[Instant], &[], None, None, None, storm(), &[Blue],
        "Counter target instant or sorcery spell unless its controller pays {1}. Storm.");
    card!(IntoTheFloodMaw, "Into the Flood Maw", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Return target nonland permanent to its owner's hand.");
    card!(Stifle, "Stifle", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target activated or triggered ability.");
    card!(BrainFreeze, "Brain Freeze", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, storm(), &[Blue],
        "Target player mills three cards. Storm.");
    card!(Daze, "Daze", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "You may return an Island you control to its owner's hand rather than pay this spell's mana cost. Counter target spell unless its controller pays {1}.");
    card!(Flash, "Flash", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "You may put a creature card from your hand onto the battlefield. If you do, sacrifice it unless you pay its mana cost reduced by up to {2}.");
    card!(HurkylsRecall, "Hurkyl's Recall", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Return all artifacts target player owns to their hand.");
    card!(ManaLeak, "Mana Leak", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target spell unless its controller pays {3}.");
    card!(MemoryLapse, "Memory Lapse", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target spell. If that spell is countered this way, put it on top of its owner's library instead of into that player's graveyard.");
    card!(Remand, "Remand", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Counter target spell. If that spell is countered this way, put it into its owner's hand instead of into that player's graveyard. Draw a card.");
    card!(ForceOfNegation, "Force of Negation", ManaCost { blue: 1, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "If it's not your turn, you may exile a blue card from your hand rather than pay this spell's mana cost. Counter target noncreature spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.");
    card!(MysticalDispute, "Mystical Dispute", ManaCost { blue: 1, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "This spell costs {2} less to cast if it targets a blue spell. Counter target spell unless its controller pays {3}.");
    card!(GiftsUngiven, "Gifts Ungiven", ManaCost { blue: 1, generic: 3, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Search your library for up to four cards with different names and reveal them. Target opponent chooses two of those cards. Put the chosen cards into your graveyard and the rest into your hand. Then shuffle.");
    card!(ParadoxicalOutcome, "Paradoxical Outcome", ManaCost { blue: 1, generic: 3, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Return any number of target nonland, nontoken permanents you control to their owners' hands. Draw a card for each card returned to your hand this way.");
    card!(Gush, "Gush", ManaCost { blue: 1, generic: 4, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "You may return two Islands you control to their owner's hand rather than pay this spell's mana cost. Draw two cards.");
    card!(Misdirection, "Misdirection", ManaCost { blue: 1, generic: 4, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "You may exile a blue card from your hand rather than pay this spell's mana cost. Change the target of target spell with a single target.");
    card!(Commandeer, "Commandeer", ManaCost { blue: 2, generic: 5, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "You may exile two blue cards from your hand rather than pay this spell's mana cost. Gain control of target noncreature spell. You may choose new targets for it.");

    // === Blue Sorceries ===
    card!(CarefulStudy, "Careful Study", ManaCost::u(1), &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Draw two cards, then discard two cards.");
    card!(Consider, "Consider", ManaCost::u(1), &[Instant], &[], None, None, None, kw(), &[Blue],
        "Surveil 1, then draw a card.");
    card!(MerchantScroll, "Merchant Scroll", ManaCost { blue: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Search your library for a blue instant card, reveal it, put it into your hand, then shuffle.");
    card!(TransmuteArtifact, "Transmute Artifact", ManaCost { blue: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Sacrifice an artifact. If you do, search your library for an artifact card. If that card's mana cost is less than or equal to the sacrificed artifact's, put it onto the battlefield. Otherwise, pay the difference in mana. Then shuffle.");
    card!(ShowAndTell, "Show and Tell", ManaCost { blue: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Each player may put an artifact, creature, enchantment, or land card from their hand onto the battlefield.");
    card!(StockUp, "Stock Up", ManaCost { blue: 1, generic: 3, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Draw three cards. If you control a token, draw four cards instead.");
    card!(Windfall, "Windfall", ManaCost { blue: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Each player discards their hand, then draws cards equal to the greatest number of cards a player discarded this way.");
    card!(LorienRevealed, "Lorien Revealed", ManaCost { blue: 1, generic: 4, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Draw three cards. Island cycling {1}.");
    card!(StepThrough, "Step Through", ManaCost { blue: 1, generic: 4, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Return two target creatures to their owners' hands. Wizardcycling {2}.");
    card!(Thoughtcast, "Thoughtcast", ManaCost { blue: 1, generic: 4, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Affinity for artifacts. Draw two cards.");
    card!(FB(ManaCost { blue: 1, generic: 2, ..c }) EchoOfEons, "Echo of Eons", ManaCost { blue: 2, generic: 4, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "Each player shuffles their hand and graveyard into their library, then draws seven cards. Flashback {2}{U}.");
    card!(MindsDesire, "Mind's Desire", ManaCost { blue: 2, generic: 4, ..c }, &[Sorcery], &[], None, None, None, storm(), &[Blue],
        "Shuffle your library. Then exile the top card of your library. Until end of turn, you may play that card without paying its mana cost. Storm.");
    card!(Tinker, "Tinker", ManaCost { blue: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "As an additional cost to cast this spell, sacrifice an artifact. Search your library for an artifact card and put it onto the battlefield. Then shuffle.");

    // === Blue Enchantments/Artifacts ===
    card!(AetherSpellbomb, "Aether Spellbomb", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[Blue],
        "{U}, Sacrifice Aether Spellbomb: Return target creature to its owner's hand. {1}, Sacrifice Aether Spellbomb: Draw a card.");
    card!(CryogenRelic, "Cryogen Relic", ManaCost { blue: 1, generic: 1, ..ManaCost::ZERO }, &[Artifact], &[], None, None, None, kw(), &[Blue],
        "When Cryogen Relic enters or leaves the battlefield, draw a card. {1}{U}, Sacrifice Cryogen Relic: Put a stun counter on up to one target tapped creature.");
    card!(MysticRemora, "Mystic Remora", ManaCost::u(1), &[Enchantment], &[], None, None, None, kw(), &[Blue],
        "Cumulative upkeep {1}. Whenever an opponent casts a noncreature spell, you may draw a card unless that player pays {4}.");
    card!(UnableToScream, "Unable to Scream", ManaCost::u(1), &[Enchantment], &[], None, None, None, kw(), &[Blue],
        "Enchant creature. Enchanted creature loses all abilities and is a 0/2.");
    card!(DressDown, "Dress Down", ManaCost { blue: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, flash(), &[Blue],
        "Flash. When Dress Down enters, draw a card. Creatures lose all abilities. At the beginning of the end step, sacrifice Dress Down.");
    card!(EnergyFlux, "Energy Flux", ManaCost { blue: 1, generic: 2, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Blue],
        "All artifacts have \"At the beginning of your upkeep, sacrifice this artifact unless you pay {2}.\"");
    card!(SinkIntoStupor, "Sink into Stupor", ManaCost { blue: 1, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "Return target spell or nonland permanent to its owner's hand.");
    card!(SharkTyphoon, "Shark Typhoon", ManaCost { blue: 1, generic: 5, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Blue],
        "Whenever you cast a noncreature spell, create an X/X blue Shark creature token with flying, where X is that spell's mana value. Cycling {X}{U}.");
    card!(SharkToken, "Shark Token", ManaCost::ZERO, &[Creature], &[], Some(0), Some(0), None, flying(), &[Blue],
        "Flying. (This is a token created by Shark Typhoon.)");
    card!(FutureSight, "Future Sight", ManaCost { blue: 1, generic: 4, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Blue],
        "Play with the top card of your library revealed. You may play the top card of your library.");

    // === Black Creatures ===
    card!(Nethergoyf, "Nethergoyf", ManaCost { black: 1, ..c }, &[Creature], &[],
        Some(0), None, None, kw(), &[Black],
        "Nethergoyf's power is equal to the number of card types among cards in your graveyard and its toughness is equal to that number plus 1. Escape - {2}{B}, exile any number of other cards from your graveyard with four or more card types among them.");
    card!(DauthiVoidwalker, "Dauthi Voidwalker", ManaCost { black: 2, ..c }, &[Creature], &[],
        Some(3), Some(2), None, shadow(), &[Black],
        "Shadow. If a card would be put into an opponent's graveyard from anywhere, instead exile it with a void counter on it. {T}, Sacrifice Dauthi Voidwalker: Choose an exiled card an opponent owns with a void counter on it. You may play it this turn without paying its mana cost.");
    card!(EmperorOfBones, "Emperor of Bones", ManaCost { black: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Black],
        "At the beginning of combat on your turn, exile up to one target card from a graveyard. {1}{B}: Adapt 2. Whenever one or more +1/+1 counters are put on this creature, put a creature card exiled with this creature onto the battlefield under your control with a finality counter on it. It gains haste. Sacrifice it at the beginning of the next end step.");
    card!(MaiScornfulStriker, "Mai, Scornful Striker", ManaCost { black: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, first_strike(), &[Black],
        "First strike. Whenever a player casts a noncreature spell, they lose 2 life.");
    card!(OrcishBowmasters, "Orcish Bowmasters", ManaCost { black: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(1), None, flash(), &[Black],
        "Flash. When Orcish Bowmasters enters and whenever an opponent draws a card except the first one they draw in each of their draw steps, amass Orcs 1 and Orcish Bowmasters deals 1 damage to any target.");
    card!(Barrowgoyf, "Barrowgoyf", ManaCost { black: 1, generic: 2, ..c }, &[Creature], &[],
        Some(0), None, None, deathtouch_lifelink(), &[Black],
        "Deathtouch, lifelink. Barrowgoyf's power is equal to the number of card types among cards in all graveyards and its toughness is equal to that number plus 1. Whenever this creature deals combat damage to a player, you may mill that many cards. If you do, you may put a creature card from among them into your hand.");
    card!(Grief, "Grief", ManaCost { black: 2, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(2), None, menace(), &[Black],
        "Menace. When Grief enters, target opponent reveals their hand. You choose a nonland card from it. That player discards that card. Evoke - Exile a black card from your hand.");
    card!(StreetWraith, "Street Wraith", ManaCost { black: 2, generic: 3, ..c }, &[Creature], &[],
        Some(3), Some(4), None, kw(), &[Black],
        "Swampwalk. Cycling - Pay 2 life.");
    card!(TrollOfKhazadDum, "Troll of Khazad-dum", ManaCost { black: 1, generic: 5, ..c }, &[Creature], &[],
        Some(6), Some(5), None, kw(), &[Black],
        "This creature can't be blocked except by three or more creatures. Swampcycling {1}.");
    card!(CT(&[CreatureType::Archon]) ArchonOfCruelty, "Archon of Cruelty", ManaCost { black: 2, generic: 6, ..c }, &[Creature], &[],
        Some(6), Some(6), None, flying(), &[Black],
        "Flying. Whenever Archon of Cruelty enters or attacks, target opponent sacrifices a creature, discards a card, and loses 3 life. You draw a card and gain 3 life.");
    card!(Griselbrand, "Griselbrand", ManaCost { black: 4, generic: 4, ..c }, &[Creature], &[Legendary],
        Some(7), Some(7), None, flying_lifelink(), &[Black],
        "Flying, lifelink. Pay 7 life: Draw seven cards.");

    // === Black Spells ===
    card!(VillageRites, "Village Rites", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "As an additional cost to cast this spell, sacrifice a creature. Draw 2 cards.");
    card!(DeadlyDispute, "Deadly Dispute", ManaCost { black: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Black],
        "As an additional cost to cast this spell, sacrifice an artifact or creature. Draw 2 cards and create a Treasure token.");
    card!(DemonicConsultation, "Demonic Consultation", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "Choose a card name. Exile the top six cards of your library, then reveal cards from the top of your library until you reveal a card with the chosen name. Put that card into your hand and exile all other cards revealed this way.");
    card!(FatalPush, "Fatal Push", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "Destroy target creature if it has mana value 2 or less. Revolt - Destroy that creature if it has mana value 4 or less instead if a permanent you controlled left the battlefield this turn.");
    card!(BitterTriumph, "Bitter Triumph", ManaCost { black: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Black],
        "As an additional cost, discard a card or pay 3 life. Destroy target creature or planeswalker.");
    card!(CabalRitual, "Cabal Ritual", ManaCost { black: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Black],
        "Add {B}{B}{B}. Threshold - Add {B}{B}{B}{B}{B} instead if seven or more cards are in your graveyard.");
    card!(SheoldredsEdict, "Sheoldred's Edict", ManaCost { black: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Black],
        "Choose one: Each opponent sacrifices a nontoken creature. Each opponent sacrifices a creature token. Each opponent sacrifices a planeswalker.");
    card!(SnuffOut, "Snuff Out", ManaCost { black: 1, generic: 3, ..c }, &[Instant], &[], None, None, None, kw(), &[Black],
        "If you control a Swamp, you may pay 4 life rather than pay this spell's mana cost. Destroy target nonblack creature. It can't be regenerated.");
    card!(Duress, "Duress", ManaCost::b(1), &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Target opponent reveals their hand. You choose a noncreature, nonland card from it. That player discards that card.");
    card!(ImperialSeal, "Imperial Seal", ManaCost::b(1), &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Search your library for a card, then shuffle and put that card on top. You lose 2 life.");
    card!(InquisitionOfKozilek, "Inquisition of Kozilek", ManaCost::b(1), &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Target player reveals their hand. You choose a nonland card from it with mana value 3 or less. That player discards that card.");
    card!(X(1) MindTwist, "Mind Twist", ManaCost { black: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Target player discards X cards at random.");
    card!(Exhume, "Exhume", ManaCost { black: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Each player puts a creature card from their graveyard onto the battlefield.");
    card!(Doomsday, "Doomsday", ManaCost { black: 3, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Search your library and graveyard for five cards and exile the rest. Put the chosen cards on top of your library in any order. You lose half your life, rounded up.");
    card!(BeseechTheMirror, "Beseech the Mirror", ManaCost { black: 3, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Bargain. Search your library for a card, put it into your hand, then shuffle. If this spell was bargained, you may cast the found card with mana value 4 or less without paying its mana cost.");
    card!(Unmask, "Unmask", ManaCost { black: 1, generic: 3, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "You may exile a black card from your hand rather than pay this spell's mana cost. Target player reveals their hand. You choose a nonland card from it. That player discards that card.");

    // === Black Enchantments/Artifacts ===
    card!(BolassCitadel, "Bolas's Citadel", ManaCost { black: 3, generic: 3, ..c }, &[Artifact], &[Legendary], None, None, None, kw(), &[Black],
        "You may look at the top card of your library any time. You may play lands and cast spells from the top of your library. If you do, pay life equal to that spell's mana cost rather than pay its mana cost. {T}, Sacrifice ten nonland permanents: Each opponent loses 10 life.");
    card!(AnimateDead, "Animate Dead", ManaCost { black: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Black],
        "Enchant creature card in a graveyard. When Animate Dead enters, return enchanted creature card to the battlefield. It gets -1/-0. When Animate Dead leaves the battlefield, sacrifice the returned creature.");
    card!(ChainsOfMephistopheles, "Chains of Mephistopheles", ManaCost { black: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Black],
        "If a player would draw a card except the first one they draw in their draw step each turn, that player discards a card instead. If they discard a card this way, they draw a card. If they can't, they mill a card.");
    card!(Necropotence, "Necropotence", ManaCost { black: 3, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Black],
        "Skip your draw step. Whenever you discard a card, exile that card from your graveyard. Pay 1 life: Exile the top card of your library face down. Put that card into your hand at the beginning of your next end step.");

    // === Red Creatures ===
    card!(GorillaShaman, "Gorilla Shaman", ManaCost::r(1), &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Red],
        "{X}{X}{1}: Destroy target noncreature artifact with mana value X.");
    card!(AshZealot, "Ash Zealot", ManaCost { red: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::FirstStrike);
            k.add(Keyword::Haste);
            k
        }, &[Red],
        "First strike, haste. Whenever a player casts a spell from a graveyard, Ash Zealot deals 3 damage to that player.");
    card!(EidolonOfTheGreatRevel, "Eidolon of the Great Revel", ManaCost { red: 2, ..c }, &[Creature, Enchantment], &[],
        Some(2), Some(2), None, kw(), &[Red],
        "Whenever a player casts a spell with mana value 3 or less, Eidolon of the Great Revel deals 2 damage to that player.");
    card!(GenerousPlunderer, "Generous Plunderer", ManaCost { red: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Menace);
            k
        }, &[Red],
        "Menace. At the beginning of your upkeep, you may create a Treasure token. When you do, target opponent creates a tapped Treasure token. Whenever this creature attacks, it deals damage to defending player equal to the number of artifacts they control.");
    card!(HarshMentor, "Harsh Mentor", ManaCost { red: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Red],
        "Whenever an opponent activates an ability of an artifact, creature, or land on the battlefield, if it's not a mana ability, Harsh Mentor deals 2 damage to that player.");
    card!(MagebaneLizard, "Magebane Lizard", ManaCost { red: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(4), None, kw(), &[Red],
        "Whenever a player casts a noncreature spell, Magebane Lizard deals damage to that player equal to the number of noncreature spells they've cast this turn.");
    card!(CT(&[CreatureType::Human]) RazorkinNeedlehead, "Razorkin Needlehead", ManaCost { red: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, first_strike(), &[Red],
        "This creature has first strike during your turn. Whenever an opponent draws a card, this creature deals 1 damage to them.");
    card!(ZhaoTheMoonSlayer, "Zhao, the Moon Slayer", ManaCost { red: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, menace(), &[Red],
        "Menace. Nonbasic lands enter tapped. {7}: Put a conqueror counter on Zhao. As long as Zhao has a conqueror counter on him, nonbasic lands are Mountains.");
    card!(CT(&[CreatureType::Goblin]) NameStickerGoblin, "Name Sticker Goblin", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Red],
        "When this creature enters from anywhere other than a graveyard or exile, roll a 20-sided die. 1-6: Add RRRR. 7-14: Add RRRRR. 15-20: Add RRRRRR.");
    card!(CT(&[CreatureType::Human, CreatureType::Rebel]) AvalancheOfSector7, "Avalanche of Sector 7", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(0), Some(3), None, menace(), &[Red],
        "Menace. Avalanche of Sector 7's power is equal to the number of artifacts your opponents control. Whenever an opponent activates an ability of an artifact they control, Avalanche of Sector 7 deals 1 damage to that player.");
    card!(ADVENTURE(AdventureDef {
        display_name: "Stomp",
        cost: ManaCost { red: 1, generic: 1, ..c },
        card_types: &[CardType::Instant],
    }) BonecrusherGiant, "Bonecrusher Giant", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[],
        Some(4), Some(3), None, kw(), &[Red],
        "Whenever Bonecrusher Giant becomes the target of a spell, Bonecrusher Giant deals 2 damage to that spell's controller. Adventure - Stomp {1}{R}: Deal 2 damage to any target. Damage can't be prevented this turn.");
    card!(BroadsideBombardiers, "Broadside Bombardiers", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, menace_haste(), &[Red],
        "Menace, haste. Boast — Sacrifice another creature or artifact: Broadside Bombardiers deals damage equal to 2 plus the sacrificed permanent's mana value to any target.");
    card!(CT(&[CreatureType::Goblin, CreatureType::Shaman]) GutTrueSoulZealot, "Gut, True Soul Zealot", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, kw(), &[Red],
        "Whenever you attack, you may sacrifice another creature or an artifact. If you do, create a 4/1 black Skeleton creature token with menace that's tapped and attacking. Choose a Background.");
    card!(MagusOfTheMoon, "Magus of the Moon", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Red],
        "Nonbasic lands are Mountains.");
    card!(SeasonedPyromancer, "Seasoned Pyromancer", ManaCost { red: 2, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Red],
        "When Seasoned Pyromancer enters, discard two cards, then draw two cards. For each nonland card discarded this way, create a 1/1 red Elemental creature token. {3}{R}{R}, Exile Seasoned Pyromancer from your graveyard: Create two 1/1 red Elemental creature tokens.");
    card!(SimianSpiritGuide, "Simian Spirit Guide", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Red],
        "Exile Simian Spirit Guide from your hand: Add {R}.");
    card!(CavesOfChaosAdventurer, "Caves of Chaos Adventurer", ManaCost { red: 1, generic: 3, ..c }, &[Creature], &[],
        Some(5), Some(3), None, trample(), &[Red],
        "When Caves of Chaos Adventurer enters, you take the initiative. Whenever Caves of Chaos Adventurer attacks, exile the top card of your library. You may play it this turn.");
    card!(Pyrogoyf, "Pyrogoyf", ManaCost { red: 1, generic: 3, ..c }, &[Creature], &[],
        Some(0), Some(1), None, kw(), &[Red],
        "*/*+1. Pyrogoyf's power is equal to the number of card types among cards in all graveyards and its toughness is equal to that number plus 1. When Pyrogoyf dies, it deals damage equal to its power to any target.");
    card!(Fury, "Fury", ManaCost { red: 2, generic: 3, ..c }, &[Creature], &[],
        Some(3), Some(3), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::DoubleStrike);
            k
        }, &[Red],
        "Double strike. When Fury enters, it deals 4 damage divided as you choose among any number of target creatures and/or planeswalkers. Evoke - Exile a red card from your hand.");

    // === Red Spells ===
    card!(RedirectLightning, "Redirect Lightning", ManaCost { red: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Red],
        "As an additional cost to cast this spell, pay 5 life or pay {2}. Change the target of target spell or ability with a single target.");
    card!(Abrade, "Abrade", ManaCost { red: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Red],
        "Choose one: Abrade deals 3 damage to target creature. Destroy target artifact.");
    card!(ShrapnelBlast, "Shrapnel Blast", ManaCost { red: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Red],
        "As an additional cost, sacrifice an artifact. Shrapnel Blast deals 5 damage to any target.");
    card!(UntimelyMalfunction, "Untimely Malfunction", ManaCost { red: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Red],
        "Choose one — Destroy target artifact. Change the target of target spell or ability with a single target. One or two target creatures can't block this turn.");
    card!(Crash, "Crash", ManaCost { red: 1, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Red],
        "You may sacrifice a Mountain rather than pay this spell's mana cost. Destroy target artifact.");
    card!(X(1) Meltdown, "Meltdown", ManaCost { red: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Destroy each artifact with mana value X or less.");
    card!(ShatteringSpree, "Shattering Spree", ManaCost::r(1), &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Destroy target artifact. Replicate {R}.");
    card!(Vandalblast, "Vandalblast", ManaCost::r(1), &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Destroy target artifact you don't control. Overload {4}{R}.");
    card!(Suplex, "Suplex", ManaCost { red: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Choose one — Suplex deals 3 damage to target creature. If that creature would die this turn, exile it instead. / Exile target artifact.");
    card!(BrotherhoodsEnd, "Brotherhood's End", ManaCost { red: 2, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Red],
        "Choose one: Brotherhood's End deals 3 damage to each creature and each planeswalker. Destroy all artifacts with mana value 3 or less.");

    // === Red Enchantments ===
    card!(RoilingVortex, "Roiling Vortex", ManaCost { red: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Red],
        "At the beginning of each player's upkeep, Roiling Vortex deals 1 damage to them. Whenever a player casts a spell, if no mana was spent to cast that spell, Roiling Vortex deals 5 damage to that player.");
    card!(UnderworldBreach, "Underworld Breach", ManaCost { red: 1, generic: 1, ..c }, &[Enchantment], &[Legendary], None, None, None, kw(), &[Red],
        "Each nonland card in your graveyard has escape. The escape cost is equal to the card's mana cost plus exile three other cards from your graveyard. At the beginning of the end step, sacrifice Underworld Breach.");
    card!(BloodMoon, "Blood Moon", ManaCost { red: 1, generic: 2, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Red],
        "Nonbasic lands are Mountains.");
    db.push(CardDef {
        name: CardName::FableOfTheMirrorBreaker,
        display_name: "Fable of the Mirror-Breaker",
        mana_cost: ManaCost { red: 1, generic: 2, ..c },
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[Enchantment],
        supertypes: &[Legendary],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[Red],
        oracle_text: "(As this Saga enters and after your draw step, add a lore counter.) I — Create a 2/2 red Goblin Shaman creature token with \"Whenever this token attacks, create a Treasure token.\" II — You may discard up to two cards. If you do, draw that many cards. III — Exile this Saga, then return it to the battlefield transformed under your control.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: Some(CardName::ReflectionOfKikiJiki),
    });
    // Back face of Fable of the Mirror-Breaker — 2/2 Goblin Shaman enchantment creature
    db.push(CardDef {
        name: CardName::ReflectionOfKikiJiki,
        display_name: "Reflection of Kiki-Jiki",
        mana_cost: ManaCost::ZERO,
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[Enchantment, Creature],
        supertypes: &[Legendary],
        power: Some(2),
        toughness: Some(2),
        loyalty: None,
        keywords: kw(),
        color_identity: &[Red],
        oracle_text: "{1}, {T}: Create a token that's a copy of another target nonlegendary creature you control, except it has haste. Sacrifice it at the beginning of the next end step.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[CreatureType::Goblin, CreatureType::Shaman],
        is_changeling: false,
        adventure: None,
        back_face: None,
    });
    db.push(CardDef {
        name: CardName::ShatterskullSmashing,
        display_name: "Shatterskull Smashing",
        mana_cost: ManaCost { red: 2, ..c },
        has_x_cost: true,
        x_multiplier: 1,
        card_types: &[CardType::Sorcery],
        supertypes: &[],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[Color::Red],
        oracle_text: "Shatterskull Smashing deals X damage divided as you choose among up to two target creatures and/or planeswalkers. If X is 6 or more, Shatterskull Smashing deals twice X damage divided as you choose instead.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: Some(CardName::ShatterskullTheHammerPass),
    });
    db.push(CardDef {
        name: CardName::ShatterskullTheHammerPass,
        display_name: "Shatterskull, the Hammer Pass",
        mana_cost: ManaCost::ZERO,
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[CardType::Land],
        supertypes: &[],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[Color::Red],
        oracle_text: "Shatterskull, the Hammer Pass enters tapped unless you pay 3 life. {T}: Add {R}.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: None,
    });
    db.push(CardDef {
        name: CardName::SunderingEruption,
        display_name: "Sundering Eruption",
        mana_cost: ManaCost { red: 1, generic: 2, ..c },
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[CardType::Sorcery],
        supertypes: &[],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[Color::Red],
        oracle_text: "Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle. Creatures without flying can't block this turn.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: Some(CardName::VolcanicFissure),
    });
    db.push(CardDef {
        name: CardName::VolcanicFissure,
        display_name: "Volcanic Fissure",
        mana_cost: ManaCost::ZERO,
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[CardType::Land],
        supertypes: &[],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[Color::Red],
        oracle_text: "As this land enters, you may pay 3 life. If you don't, it enters tapped.\n{T}: Add {R}.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: None,
    });
    card!(ExperimentalFrenzy, "Experimental Frenzy", ManaCost { red: 1, generic: 3, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Red],
        "You may look at the top card of your library any time. You may cast spells from the top of your library. You can't cast spells from your hand. {3}{R}: Destroy Experimental Frenzy.");

    // === Red/Green Madness/Pitch Creatures ===
    card!(MADNESS(ManaCost::ZERO) BaskingRootwalla, "Basking Rootwalla", ManaCost::g(1), &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "{1}{G}: Basking Rootwalla gets +2/+2 until end of turn. Activate only once each turn. Madness {0}.");
    card!(MADNESS(ManaCost::ZERO) BlazingRootwalla, "Blazing Rootwalla", ManaCost::r(1), &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Red],
        "{R}{R}: Blazing Rootwalla gets +2/+0 until end of turn. Activate only once each turn. Madness {0}.");
    card!(CT(&[CreatureType::Goblin]) SqueeGoblinNabob, "Squee, Goblin Nabob", ManaCost { red: 1, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(1), Some(1), None, kw(), &[Red],
        "At the beginning of your upkeep, you may return Squee, Goblin Nabob from your graveyard to your hand.");

    // === Green Creatures ===
    card!(CT(&[CreatureType::Halfling]) DelightedHalfling, "Delighted Halfling", ManaCost::g(1), &[Creature], &[],
        Some(1), Some(2), None, kw(), &[Green],
        "{T}: Add {C}. {T}: Add one mana of any color. Spend this mana only to cast a legendary spell, and that spell can't be countered.");
    card!(CT(&[CreatureType::Insect]) HaywireMite, "Haywire Mite", ManaCost::generic(1), &[Artifact, Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "When Haywire Mite dies, you gain 2 life. {G}, Sacrifice Haywire Mite: Exile target noncreature artifact or noncreature enchantment.");
    card!(CT(&[CreatureType::Snake]) Hexdrinker, "Hexdrinker", ManaCost::g(1), &[Creature], &[],
        Some(2), Some(1), None, kw(), &[Green],
        "Level up {1}. Level 3-7: 4/4 with protection from instants. Level 8+: 6/6 with protection from everything.");
    card!(SylvanSafekeeper, "Sylvan Safekeeper", ManaCost::g(1), &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "Sacrifice a land: Target creature you control gains shroud until end of turn.");
    card!(CT(&[CreatureType::Human, CreatureType::Druid]) HermitDruid, "Hermit Druid", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "{G}, {T}: Reveal cards from the top of your library until you reveal a basic land card. Put that card into your hand and all other cards revealed this way into your graveyard.");
    card!(OutlandLiberator, "Outland Liberator", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Green],
        "{1}, Sacrifice Outland Liberator: Destroy target artifact or enchantment. Daybound.");
    card!(SatyrWayfinder, "Satyr Wayfinder", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "When Satyr Wayfinder enters, reveal the top four cards of your library. You may put a land card from among them into your hand. Put the rest into your graveyard.");
    card!(CT(&[CreatureType::Lhurgoyf]) Tarmogoyf, "Tarmogoyf", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(0), Some(1), None, kw(), &[Green],
        "Tarmogoyf's power is equal to the number of card types among cards in all graveyards and its toughness is that number plus 1.");
    card!(CT(&[CreatureType::Human]) TownGreeter, "Town Greeter", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(1), None, kw(), &[Green],
        "When this creature enters, mill four cards. You may put a land card from among them into your hand.");
    card!(CT(&[CreatureType::Elf, CreatureType::Spirit]) ElvishSpiritGuide, "Elvish Spirit Guide", ManaCost { green: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Green],
        "Exile Elvish Spirit Guide from your hand: Add {G}.");
    card!(CT(&[CreatureType::Beast]) Manglehorn, "Manglehorn", ManaCost { green: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Green],
        "When Manglehorn enters, you may destroy target artifact. Artifacts your opponents control enter tapped.");
    card!(CT(&[CreatureType::Insect, CreatureType::Scout]) IcetillExplorer, "Icetill Explorer", ManaCost { green: 2, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(4), None, kw(), &[Green],
        "You may play an additional land on each of your turns. You may play lands from your graveyard. Landfall — Whenever a land you control enters, mill a card.");
    card!(UndermountainAdventurer, "Undermountain Adventurer", ManaCost { green: 1, generic: 3, ..c }, &[Creature], &[],
        Some(3), Some(4), None, vigilance(), &[Green],
        "Vigilance. When Undermountain Adventurer enters, you take the initiative. {T}: Add {G}{G}.");
    card!(CT(&[CreatureType::Elemental]) Vengevine, "Vengevine", ManaCost { green: 2, generic: 2, ..c }, &[Creature], &[],
        Some(4), Some(3), None, haste(), &[Green],
        "Haste. Whenever you cast two creature spells in a turn, you may return Vengevine from your graveyard to the battlefield.");
    card!(CT(&[CreatureType::Golem]) HollowOne, "Hollow One", ManaCost::generic(5), &[Artifact, Creature], &[],
        Some(4), Some(4), None, kw(), &[],
        "This spell costs {2} less to cast for each card you've cycled or discarded this turn. Cycling {2}.");

    // === Green Spells ===
    card!(CropRotation, "Crop Rotation", ManaCost::g(1), &[Instant], &[], None, None, None, kw(), &[Green],
        "As an additional cost, sacrifice a land. Search your library for a land card, put it onto the battlefield, then shuffle.");
    card!(NaturesClaim, "Nature's Claim", ManaCost::g(1), &[Instant], &[], None, None, None, kw(), &[Green],
        "Destroy target artifact or enchantment. Its controller gains 4 life.");
    card!(VeilOfSummer, "Veil of Summer", ManaCost::g(1), &[Instant], &[], None, None, None, kw(), &[Green],
        "Draw a card if an opponent has cast a blue or black spell this turn. Spells you control can't be countered this turn. You and permanents you control gain hexproof from blue and from black until end of turn.");
    card!(OnceUponATime, "Once Upon a Time", ManaCost { green: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Green],
        "If this spell is the first spell you've cast this game, you may cast it without paying its mana cost. Look at the top five cards of your library. You may reveal a creature or land card from among them and put it into your hand. Put the rest on the bottom in a random order.");
    card!(ForceOfVigor, "Force of Vigor", ManaCost { green: 2, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Green],
        "If it's not your turn, you may exile a green card from your hand rather than pay this spell's mana cost. Destroy up to two target artifacts and/or enchantments.");
    card!(LifeFromTheLoam, "Life from the Loam", ManaCost { green: 1, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Green],
        "Return up to three target land cards from your graveyard to your hand. Dredge 3.");
    card!(SeedsOfInnocence, "Seeds of Innocence", ManaCost { green: 2, generic: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Green],
        "Destroy all artifacts. For each artifact destroyed this way, its controller gains life equal to its mana value.");

    // === Green Enchantments ===
    card!(OathOfDruids, "Oath of Druids", ManaCost { green: 1, generic: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Green],
        "At the beginning of each player's upkeep, that player chooses target player who controls more creatures than they do and is their opponent. The first player may reveal cards from the top of their library until they reveal a creature card. If they do, that player puts that card onto the battlefield and puts all other cards revealed this way into their graveyard.");

    // === Green/Black Creatures ===
    card!(MasterOfDeath, "Master of Death", ManaCost { blue: 1, black: 1, generic: 1, ..c }, &[Creature], &[],
        Some(3), Some(1), None, kw(), &[Blue, Black],
        "When this creature enters, surveil 2. At the beginning of your upkeep, if this card is in your graveyard, you may pay 1 life. If you do, return it to your hand.");
    card!(HogaakArisenNecropolis, "Hogaak, Arisen Necropolis", ManaCost { black: 1, green: 1, generic: 5, ..c }, &[Creature], &[Legendary],
        Some(8), Some(8), None, trample(), &[Black, Green],
        "You can't spend mana to cast this spell. Convoke, delve. Trample. You may cast Hogaak from your graveyard.");
    card!(KishlaSkimmer, "Kishla Skimmer", ManaCost { green: 1, blue: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, flying(), &[Green, Blue],
        "Flying. Whenever a card leaves your graveyard during your turn, draw a card. This ability triggers only once each turn.");
    card!(GolgariGraveTroll, "Golgari Grave-Troll", ManaCost { green: 1, generic: 5, ..c }, &[Creature], &[],
        Some(0), Some(0), None, trample(), &[Black, Green],
        "Golgari Grave-Troll enters with a +1/+1 counter on it for each creature card in your graveyard. {1}, Remove a +1/+1 counter from Golgari Grave-Troll: Regenerate Golgari Grave-Troll. Dredge 6.");
    card!(StinkweedImp, "Stinkweed Imp", ManaCost { black: 1, generic: 2, ..c }, &[Creature], &[],
        Some(1), Some(2), None, flying(), &[Black],
        "Flying. Whenever Stinkweed Imp deals combat damage to a creature, destroy that creature. Dredge 5.");

    // === Colorless Creatures ===
    // Stonecoil Serpent: {X} — X appears once, pays 1 per X
    card!(X(1) StonecoilSerpent, "Stonecoil Serpent", c, &[Artifact, Creature], &[],
        Some(0), Some(0), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Reach);
            k.add(Keyword::Trample);
            k
        }, &[],
        "Reach, trample, protection from multicolored. Stonecoil Serpent enters with X +1/+1 counters on it.");
    // Walking Ballista: {X}{X} — X appears twice, pays 2 per X
    card!(X(2) WalkingBallista, "Walking Ballista", c, &[Artifact, Creature], &[],
        Some(0), Some(0), None, kw(), &[],
        "Walking Ballista enters with X +1/+1 counters on it. {4}: Put a +1/+1 counter on Walking Ballista. Remove a +1/+1 counter from Walking Ballista: It deals 1 damage to any target.");
    card!(PhyrexianDreadnought, "Phyrexian Dreadnought", ManaCost::generic(1), &[Artifact, Creature], &[],
        Some(12), Some(12), None, trample(), &[],
        "Trample. When Phyrexian Dreadnought enters, unless you sacrifice any number of creatures with total power 12 or greater, sacrifice it.");
    card!(CT(&[CreatureType::Myr]) MyrRetriever, "Myr Retriever", ManaCost::generic(2), &[Artifact, Creature], &[],
        Some(1), Some(1), None, kw(), &[],
        "When Myr Retriever dies, return another target artifact card from your graveyard to your hand.");
    card!(PatchworkAutomaton, "Patchwork Automaton", ManaCost::generic(2), &[Artifact, Creature], &[],
        Some(1), Some(1), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Ward);
            k
        }, &[],
        "Ward {2}. Whenever you cast an artifact spell, put a +1/+1 counter on Patchwork Automaton.");
    card!(PhyrexianRevoker, "Phyrexian Revoker", ManaCost::generic(2), &[Artifact, Creature], &[],
        Some(2), Some(1), None, kw(), &[],
        "As Phyrexian Revoker enters, choose a nonland card name. Activated abilities of sources with the chosen name can't be activated.");
    card!(FoundryInspector, "Foundry Inspector", ManaCost::generic(3), &[Artifact, Creature], &[],
        Some(3), Some(2), None, kw(), &[],
        "Artifact spells you cast cost {1} less to cast.");
    card!(GlaringFleshraker, "Glaring Fleshraker", ManaCost { colorless: 1, generic: 2, ..c }, &[Artifact, Creature], &[],
        Some(2), Some(2), None, kw(), &[],
        "Whenever you cast a colorless spell, create a 0/1 colorless Eldrazi Spawn creature token with \"Sacrifice this creature: Add {C}.\" Whenever another colorless creature you control enters, Glaring Fleshraker deals 1 damage to each opponent.");
    card!(PhyrexianMetamorph, "Phyrexian Metamorph", ManaCost { blue: 1, generic: 3, ..c }, &[Artifact, Creature], &[],
        Some(0), Some(0), None, kw(), &[Blue],
        "You may pay 2 life and {1} instead of {U}. You may have Phyrexian Metamorph enter as a copy of any artifact or creature on the battlefield, except it's an artifact in addition to its other types.");
    card!(ScrapTrawler, "Scrap Trawler", ManaCost::generic(3), &[Artifact, Creature], &[],
        Some(3), Some(2), None, kw(), &[],
        "Whenever Scrap Trawler or another artifact you control is put into a graveyard from the battlefield, return target artifact card in your graveyard with lesser mana value to your hand.");
    card!(ScrawlingCrawler, "Scrawling Crawler", ManaCost::generic(3), &[Artifact, Creature], &[],
        Some(3), Some(3), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::CantBeBlocked);
            k
        }, &[],
        "Scrawling Crawler can't be blocked. Whenever Scrawling Crawler deals combat damage to a player, draw a card.");
    card!(CT(&[CreatureType::Golem]) LodestoneGolem, "Lodestone Golem", ManaCost::generic(4), &[Artifact, Creature], &[],
        Some(5), Some(3), None, kw(), &[],
        "Nonartifact spells cost {1} more to cast.");
    card!(ArgentumMasticore, "Argentum Masticore", ManaCost::generic(5), &[Artifact, Creature], &[],
        Some(5), Some(5), None, first_strike(), &[],
        "First strike, protection from multicolored. At the beginning of your upkeep, discard a card. When you discard a card this way, destroy target nonland permanent an opponent controls.");
    card!(GolosTirelessPilgrim, "Golos, Tireless Pilgrim", ManaCost::generic(5), &[Artifact, Creature], &[Legendary],
        Some(3), Some(5), None, kw(), &[],
        "When Golos enters, you may search your library for a land card, put it tapped, then shuffle. {2}{W}{U}{B}{R}{G}: Exile the top three cards of your library. You may play them this turn without paying their mana costs.");
    card!(KarnSilverGolem, "Karn, Silver Golem", ManaCost::generic(5), &[Artifact, Creature], &[Legendary],
        Some(4), Some(4), None, kw(), &[],
        "Whenever Karn blocks or becomes blocked, it gets -4/+4 until end of turn. {1}: Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn.");
    card!(CT(&[CreatureType::Wurm]) WurmcoilEngine, "Wurmcoil Engine", ManaCost::generic(6), &[Artifact, Creature], &[],
        Some(6), Some(6), None, deathtouch_lifelink(), &[],
        "Deathtouch, lifelink. When Wurmcoil Engine dies, create a 3/3 colorless Wurm artifact creature token with deathtouch and a 3/3 colorless Wurm artifact creature token with lifelink.");
    card!(CT(&[CreatureType::Eldrazi]) EmrakulTheAeonsTorn, "Emrakul, the Aeons Torn", ManaCost::generic(15), &[Creature], &[Legendary],
        Some(15), Some(15), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Flying);
            k
        }, &[],
        "This spell can't be countered. When you cast this spell, take an extra turn after this one. Flying, protection from spells that are one or more colors, annihilator 6. When Emrakul is put into a graveyard from anywhere, its owner shuffles their graveyard into their library.");

    // === Colorless Planeswalkers ===
    card!(TezzeretCruelCaptain, "Tezzeret, Cruel Captain", ManaCost { blue: 1, black: 1, generic: 2, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(4), kw(), &[Blue, Black],
        "+1: Draw a card if you control an artifact. -2: Create a 1/1 colorless Thopter artifact creature token with flying. -7: You get an emblem with \"Whenever you cast an artifact spell, search your library for an artifact card, put it onto the battlefield, then shuffle.\"");
    card!(KarnTheGreatCreator, "Karn, the Great Creator", ManaCost::generic(4), &[Planeswalker], &[Legendary],
        None, None, Some(5), kw(), &[],
        "Activated abilities of artifacts your opponents control can't be activated. +1: Until your next turn, up to one target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value. -2: You may reveal an artifact card you own from outside the game or in exile and put it into your hand.");

    // === Colorless Spells ===
    card!(MindbreakTrap, "Mindbreak Trap", ManaCost { blue: 2, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue],
        "If an opponent cast three or more spells this turn, you may cast this spell without paying its mana cost. Exile any number of target spells.");
    card!(NoxiousRevival, "Noxious Revival", ManaCost::g(1), &[Instant], &[], None, None, None, kw(), &[Green],
        "You may pay 2 life instead of {G}. Put target card from a graveyard on top of its owner's library.");
    card!(Dismember, "Dismember", ManaCost { black: 1, generic: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Black],
        "You may pay 4 life instead of {B}{B}. Target creature gets -5/-5 until end of turn.");
    card!(X(1) KozileksCommand, "Kozilek's Command", ManaCost { colorless: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[],
        "Choose two: Target player creates X 0/1 Eldrazi Spawn creature tokens with \"Sacrifice: Add {C}.\" Target player scries X, then draws a card. Exile target creature with mana value X or less. Exile up to X target cards from graveyards.");
    card!(GitaxianProbe, "Gitaxian Probe", ManaCost::u(1), &[Sorcery], &[], None, None, None, kw(), &[Blue],
        "You may pay 2 life instead of {U}. Look at target player's hand. Draw a card.");
    card!(SurgicalExtraction, "Surgical Extraction", ManaCost::b(1), &[Instant], &[], None, None, None, kw(), &[Black],
        "You may pay 2 life instead of {B}. Choose target card in a graveyard other than a basic land. Search its owner's graveyard, hand, and library for any number of cards with the same name as that card and exile them. Then that player shuffles.");

    // === Colorless Artifacts ===
    // Chalice of the Void: {X}{X} — X appears twice
    card!(X(2) ChaliceOfTheVoid, "Chalice of the Void", c, &[Artifact], &[], None, None, None, kw(), &[],
        "Chalice of the Void enters with X charge counters on it. Whenever a player casts a spell with mana value equal to the number of charge counters on Chalice of the Void, counter that spell.");
    // Clown Car: {X} — X appears once
    card!(X(1) ClownCar, "Clown Car", c, &[Artifact], &[], None, None, None, kw(), &[],
        "When Clown Car enters, roll X six-sided dice. For each odd result, create a 1/1 white Clown Robot artifact creature token. Crew 2.");
    // Engineered Explosives: {X} — X appears once (sunburst adds charge counters based on colors)
    card!(X(1) EngineeredExplosives, "Engineered Explosives", c, &[Artifact], &[], None, None, None, kw(), &[],
        "Sunburst. {2}, Sacrifice Engineered Explosives: Destroy each nonland permanent with mana value equal to the number of charge counters on Engineered Explosives.");
    card!(Gleemox, "Gleemox", c, &[Artifact], &[], None, None, None, kw(), &[],
        "{T}: Add one mana of any color.");
    card!(TormodsCrypt, "Tormod's Crypt", c, &[Artifact], &[], None, None, None, kw(), &[],
        "{T}, Sacrifice Tormod's Crypt: Exile target player's graveyard.");
    card!(UrzasBauble, "Urza's Bauble", c, &[Artifact], &[], None, None, None, kw(), &[],
        "{T}, Sacrifice Urza's Bauble: Look at a card at random in target player's hand. You draw a card at the beginning of the next turn's upkeep.");
    card!(MishrasBauble, "Mishra's Bauble", c, &[Artifact], &[], None, None, None, kw(), &[],
        "{T}, Sacrifice Mishra's Bauble: Look at the top card of target player's library. You draw a card at the beginning of the next turn's upkeep.");
    card!(ChromaticStar, "Chromatic Star", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "{1}, {T}, Sacrifice Chromatic Star: Add one mana of any color. When Chromatic Star is put into a graveyard from the battlefield, draw a card.");
    card!(GrafdiggersCage, "Grafdigger's Cage", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "Creature cards in graveyards and libraries can't enter the battlefield. Players can't cast spells from graveyards or libraries.");
    card!(LavaspurBoots, "Lavaspur Boots", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "Equipped creature gets +1/+0 and has ward {1} and haste. Equip {1}.");
    card!(ManifoldKey, "Manifold Key", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "{1}, {T}: Untap another target artifact. {3}, {T}: Target creature can't be blocked this turn.");
    card!(PithingNeedle, "Pithing Needle", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "As Pithing Needle enters, choose a card name. Activated abilities of sources with the chosen name can't be activated unless they're mana abilities.");
    card!(SenseisDiviningTop, "Sensei's Divining Top", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "{1}: Look at the top three cards of your library, then put them back in any order. {T}: Draw a card, then put Sensei's Divining Top on top of its owner's library.");
    card!(Shadowspear, "Shadowspear", ManaCost::generic(1), &[Artifact], &[Legendary], None, None, None, kw(), &[],
        "Equipped creature gets +1/+1 and has trample and lifelink. {1}: Permanents your opponents control lose hexproof and indestructible until end of turn. Equip {2}.");
    card!(Shuko, "Shuko", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "Equipped creature gets +1/+0. Equip {0}.");
    card!(SoulGuideLantern, "Soul-Guide Lantern", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "When Soul-Guide Lantern enters, exile target card from a graveyard. {T}, Sacrifice Soul-Guide Lantern: Draw a card. {1}, {T}, Sacrifice Soul-Guide Lantern: Exile each opponent's graveyard.");
    card!(VexingBauble, "Vexing Bauble", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "Whenever a player casts a spell, if no mana was spent to cast it, counter that spell. {1}, {T}, Sacrifice Vexing Bauble: Draw a card.");
    card!(VoltaicKey, "Voltaic Key", ManaCost::generic(1), &[Artifact], &[], None, None, None, kw(), &[],
        "{1}, {T}: Untap target artifact.");
    card!(DampingSphere, "Damping Sphere", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "If a land is tapped for two or more mana, it produces {C} instead of any other type and amount. Each spell a player casts costs {1} more to cast for each other spell that player has cast this turn.");
    card!(DefenseGrid, "Defense Grid", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Each spell that isn't cast during its controller's turn costs {3} more to cast.");
    card!(DisruptorFlute, "Disruptor Flute", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "As Disruptor Flute enters, choose a card name. Activated abilities of sources with the chosen name can't be activated. Spells with the chosen name cost {3} more to cast.");
    card!(IchorWellspring, "Ichor Wellspring", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "When Ichor Wellspring enters or is put into a graveyard from the battlefield, draw a card.");
    card!(NullRod, "Null Rod", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Activated abilities of artifacts can't be activated.");
    card!(SphereOfResistance, "Sphere of Resistance", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Each spell costs {1} more to cast.");
    card!(ThornOfAmethyst, "Thorn of Amethyst", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Noncreature spells cost {1} more to cast.");
    card!(TimeVault, "Time Vault", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Time Vault enters tapped. Time Vault doesn't untap during your untap step. If you would begin an extra turn, you may skip that turn instead. If you do, untap Time Vault. {T}: Take an extra turn after this one.");
    card!(TorporOrb, "Torpor Orb", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Creatures entering the battlefield don't cause abilities to trigger.");
    card!(VoidMirror, "Void Mirror", ManaCost::generic(2), &[Artifact], &[], None, None, None, kw(), &[],
        "Whenever a player casts a spell, if no colored mana was spent to cast it, counter that spell.");
    card!(CrucibleOfWorlds, "Crucible of Worlds", ManaCost::generic(3), &[Artifact], &[], None, None, None, kw(), &[],
        "You may play lands from your graveyard.");
    card!(Nettlecyst, "Nettlecyst", ManaCost::generic(3), &[Artifact], &[], None, None, None, kw(), &[],
        "Living weapon. Equipped creature gets +1/+1 for each artifact and/or enchantment you control. Equip {2}.");
    card!(Trinisphere, "Trinisphere", ManaCost::generic(3), &[Artifact], &[], None, None, None, kw(), &[],
        "As long as Trinisphere is untapped, each spell that would cost less than {3} costs {3} to cast.");
    card!(KrarkClanIronworks, "Krark-Clan Ironworks", ManaCost::generic(4), &[Artifact], &[], None, None, None, kw(), &[],
        "Sacrifice an artifact: Add {C}{C}.");
    card!(MysticForge, "Mystic Forge", ManaCost::generic(4), &[Artifact], &[], None, None, None, kw(), &[],
        "You may look at the top card of your library any time. You may cast artifact spells and colorless spells from the top of your library. {T}, Pay 1 life: Exile the top card of your library.");
    card!(TheOneRing, "The One Ring", ManaCost::generic(4), &[Artifact], &[Legendary], None, None, None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Indestructible);
            k
        }, &[],
        "Indestructible. When The One Ring enters, if you cast it, you gain protection from everything until your next turn. At the beginning of your upkeep, you lose 1 life for each burden counter on The One Ring. {T}: Put a burden counter on The One Ring, then draw a card for each burden counter on it.");
    card!(MemoryJar, "Memory Jar", ManaCost::generic(5), &[Artifact], &[], None, None, None, kw(), &[],
        "{T}, Sacrifice Memory Jar: Each player exiles their hand face down and draws seven cards. At the beginning of the next end step, each player discards their hand and returns cards exiled this way to their hand.");
    card!(TheMightstoneAndWeakstone, "The Mightstone and Weakstone", ManaCost::generic(5), &[Artifact], &[Legendary], None, None, None, kw(), &[],
        "When The Mightstone and Weakstone enters, choose one: Draw two cards. Target creature gets -5/-5 until end of turn. {T}: Add {C}{C}. (This mana can't be spent to cast nonartifact spells. Powerstone token equivalent.)");
    card!(CovetedJewel, "Coveted Jewel", ManaCost::generic(6), &[Artifact], &[], None, None, None, kw(), &[],
        "When Coveted Jewel enters, draw three cards. {T}: Add three mana of any one color. Whenever one or more creatures an opponent controls deal combat damage to you, that player draws three cards and gains control of Coveted Jewel.");
    card!(PortalToPhyrexia, "Portal to Phyrexia", ManaCost::generic(9), &[Artifact], &[], None, None, None, kw(), &[],
        "When Portal to Phyrexia enters, each opponent sacrifices three creatures. At the beginning of your upkeep, put target creature card from a graveyard onto the battlefield under your control. It's a Phyrexian in addition to its other types.");

    // === Azorius (WU) ===
    card!(CT(&[CreatureType::Human, CreatureType::Soldier]) LaviniaAzoriusRenegade, "Lavinia, Azorius Renegade", ManaCost { white: 1, blue: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, kw(), &[White, Blue],
        "Each opponent can't cast noncreature spells with mana value greater than the number of lands that player controls. Whenever an opponent casts a spell, if no mana was spent to cast it, counter that spell.");
    card!(MakdeeAndItlaSkysnarers, "Makdee and Itla, Skysnarers", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(2), None, flying(), &[White, Blue],
        "Flying. Venom Blast — Artifacts and creatures your opponents control enter tapped.");
    // Actual cost {2}{W/U} — hybrid {W/U} approximated as {1} generic since ManaCost lacks hybrid support
    card!(DovinHandOfControl, "Dovin, Hand of Control", ManaCost { generic: 3, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(5), kw(), &[White, Blue],
        "Artifact, instant, and sorcery spells your opponents cast cost {1} more to cast. -1: Until your next turn, prevent all damage that would be dealt to and dealt by target permanent.");

    // === Dimir (UB) ===
    card!(PsychicFrog, "Psychic Frog", ManaCost { blue: 1, black: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, kw(), &[Blue, Black],
        "Whenever this creature deals combat damage to a player or planeswalker, draw a card. Discard a card: Put a +1/+1 counter on this creature. Exile three cards from your graveyard: This creature gains flying until end of turn.");

    // === Rakdos (BR) ===
    card!(MoltenCollapse, "Molten Collapse", ManaCost { black: 1, red: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black, Red],
        "Choose one. If you descended this turn, you may choose both instead. Destroy target creature or planeswalker. Destroy target noncreature, nonland permanent with mana value 1 or less.");
    db.push(CardDef {
        name: CardName::HidetsuguConsumesAll,
        display_name: "Hidetsugu Consumes All",
        mana_cost: ManaCost { black: 1, red: 1, generic: 1, ..c },
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[Enchantment],
        supertypes: &[Legendary],
        power: None,
        toughness: None,
        loyalty: None,
        keywords: kw(),
        color_identity: &[Black, Red],
        oracle_text: "(As this Saga enters and after your draw step, add a lore counter.) I — Destroy each nonland permanent with mana value 1 or less. II — Exile all graveyards. III — Exile this Saga, then return it to the battlefield transformed under your control.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[],
        is_changeling: false,
        adventure: None,
        back_face: Some(CardName::VesselOfTheAllConsuming),
    });
    // Back face of Hidetsugu Consumes All — 3/3 Ogre Shaman with Trample
    db.push(CardDef {
        name: CardName::VesselOfTheAllConsuming,
        display_name: "Vessel of the All-Consuming",
        mana_cost: ManaCost::ZERO,
        has_x_cost: false,
        x_multiplier: 0,
        card_types: &[Enchantment, Creature],
        supertypes: &[Legendary],
        power: Some(3),
        toughness: Some(3),
        loyalty: None,
        keywords: {
            let mut k = Keywords::empty();
            k.add(Keyword::Trample);
            k
        },
        color_identity: &[Black, Red],
        oracle_text: "Trample\nWhenever this creature deals damage, put a +1/+1 counter on it.\nWhenever this creature deals damage to a player, if it has dealt 10 or more damage to that player this turn, they lose the game.",
        flashback_cost: None,
        madness_cost: None,
        creature_types: &[CreatureType::Ogre, CreatureType::Shaman],
        is_changeling: false,
        adventure: None,
        back_face: None,
    });

    // === Gruul (RG) ===
    card!(FB(ManaCost { green: 1, ..c }) AncientGrudge, "Ancient Grudge", ManaCost { red: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Red, Green],
        "Destroy target artifact. Flashback {G}.");
    card!(Cindervines, "Cindervines", ManaCost { red: 1, green: 1, ..c }, &[Enchantment], &[], None, None, None, kw(), &[Red, Green],
        "Whenever an opponent casts a noncreature spell, Cindervines deals 1 damage to that player. {1}, Sacrifice Cindervines: Destroy target artifact or enchantment. Cindervines deals 2 damage to that permanent's controller.");
    card!(WrennAndSix, "Wrenn and Six", ManaCost { red: 1, green: 1, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(3), kw(), &[Red, Green],
        "+1: Return up to one target land card from your graveyard to your hand. -1: Wrenn and Six deals 1 damage to any target. -7: You get an emblem with \"Instant and sorcery cards in your graveyard have retrace.\"");
    card!(MinscAndBooTimelessHeroes, "Minsc & Boo, Timeless Heroes", ManaCost { red: 1, green: 1, generic: 2, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(3), kw(), &[Red, Green],
        "+1: Create Boo, a legendary 1/1 red Hamster creature token with trample and haste. -2: Target creature you control gets +X/+0 and gains trample and haste until end of turn, where X is its power. -6: You may sacrifice any number of creatures. When you sacrifice one or more creatures this way, Minsc & Boo deals X damage to any target, where X is the total power of those creatures, and you draw X cards.");

    // === Selesnya (GW) ===
    // Actual cost {G/W} — hybrid approximated as {1} generic since ManaCost lacks hybrid support
    card!(CT(&[CreatureType::Dryad, CreatureType::Soldier]) DryadMilitant, "Dryad Militant", ManaCost { generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, kw(), &[Green, White],
        "If an instant or sorcery card would be put into a graveyard from anywhere, exile it instead.");

    // === Orzhov (WB) ===
    card!(PestControl, "Pest Control", ManaCost { white: 1, black: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[White, Black],
        "Destroy all nonland permanents with mana value 1 or less. Cycling {2}.");
    card!(KayaOrzhovUsurper, "Kaya, Orzhov Usurper", ManaCost { white: 1, black: 1, generic: 1, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(3), kw(), &[White, Black],
        "+1: Exile up to two cards from each graveyard. You gain 2 life if at least one creature card was exiled this way. -1: Exile target nonland permanent with mana value 1 or less. -5: Kaya deals damage to target player equal to the number of cards that player owns in exile and you gain that much life.");
    card!(LurrusOfTheDreamDen, "Lurrus of the Dream-Den", ManaCost { white: 1, black: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(3), Some(2), None, lifelink(), &[White, Black],
        "Companion - Each permanent card in your starting deck has mana value 2 or less. Lifelink. During each of your turns, you may cast one permanent spell with mana value 2 or less from your graveyard.");

    // === Izzet (UR) ===
    card!(ExpressiveIteration, "Expressive Iteration", ManaCost { blue: 1, red: 1, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Blue, Red],
        "Look at the top three cards of your library. Put one into your hand, put one on the bottom, and exile one. You may play the exiled card this turn.");
    card!(FlameOfAnor, "Flame of Anor", ManaCost { blue: 1, red: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue, Red],
        "Choose one. If you control a Wizard, you may choose two instead. Destroy target artifact. Target player draws two cards. Flame of Anor deals 5 damage to target creature.");
    card!(PinnacleEmissary, "Pinnacle Emissary", ManaCost { blue: 1, red: 1, generic: 3, ..c }, &[Creature], &[],
        Some(4), Some(4), None, flash_flying(), &[Blue, Red],
        "Flash. Flying. When Pinnacle Emissary enters, it deals 3 damage to target creature or planeswalker.");
    card!(Twincast, "Twincast", ManaCost { blue: 2, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue, Red],
        "Copy target instant or sorcery spell. You may choose new targets for the copy.");
    card!(GalvanicRelay, "Galvanic Relay", ManaCost { red: 1, ..c }, &[Sorcery], &[], None, None, None, storm(), &[Red],
        "Exile the top card of your library. Until end of turn, you may play that card. Storm.");

    // === Golgari (BG) ===
    // Actual cost {B/G} — hybrid approximated as {1} generic since ManaCost lacks hybrid support
    card!(CT(&[CreatureType::Elf, CreatureType::Shaman]) DeathriteShaman, "Deathrite Shaman", ManaCost { generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, kw(), &[Black, Green],
        "{T}: Exile target land card from a graveyard. Add one mana of any color. {B}, {T}: Exile target instant or sorcery card from a graveyard. Each opponent loses 2 life. {G}, {T}: Exile target creature card from a graveyard. You gain 2 life.");
    card!(AbruptDecay, "Abrupt Decay", ManaCost { black: 1, green: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Black, Green],
        "This spell can't be countered. Destroy target nonland permanent with mana value 3 or less.");

    // === Boros (RW) ===
    card!(ForthEorlingas, "Forth Eorlingas!", ManaCost { red: 1, white: 1, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Red, White],
        "Create two 2/2 red Human Knight creature tokens with trample and haste. You become the monarch.");
    card!(CometStellarPup, "Comet, Stellar Pup", ManaCost { red: 1, white: 1, generic: 2, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(5), kw(), &[Red, White],
        "0: Roll a six-sided die. 1-2: +2 loyalty, create two 1/1 green Squirrel creature tokens with haste. 3-4: -1 loyalty, deal damage equal to loyalty to a creature or player. 5-6: +1 loyalty, activate this ability two more times this turn.");

    // === Simic (GU) ===
    card!(GildedDrake, "Gilded Drake", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(3), Some(3), None, flying(), &[Blue],
        "Flying. When Gilded Drake enters the battlefield, exchange control of Gilded Drake and up to one target creature an opponent controls. If you don't make an exchange, sacrifice Gilded Drake.");
    card!(AgentOfTreachery, "Agent of Treachery", ManaCost { blue: 2, generic: 5, ..c }, &[Creature], &[],
        Some(2), Some(3), None, kw(), &[Blue],
        "When Agent of Treachery enters the battlefield, gain control of target permanent.");
    card!(FB(ManaCost { green: 1, ..c }) MemorysJourney, "Memory's Journey", ManaCost { blue: 1, generic: 1, ..c }, &[Instant], &[], None, None, None, kw(), &[Blue, Green],
        "Target player shuffles up to three target cards from their graveyard into their library. Flashback {G}.");
    card!(NaduWingedWisdom, "Nadu, Winged Wisdom", ManaCost { green: 1, blue: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(3), Some(4), None, flying(), &[Green, Blue],
        "Flying. Whenever a creature you control becomes the target of a spell or ability, reveal the top card of your library. If it's a land, put it tapped. Otherwise, put it into your hand. This ability triggers only twice each turn for each creature.");
    card!(OkoThiefOfCrowns, "Oko, Thief of Crowns", ManaCost { green: 1, blue: 1, generic: 1, ..c }, &[Planeswalker], &[Legendary],
        None, None, Some(4), kw(), &[Green, Blue],
        "+2: Create a Food token. +1: Target artifact or creature loses all abilities and becomes a green Elk creature with base power and toughness 3/3. -5: Exchange control of target artifact or creature you control and target creature an opponent controls with power 3 or less.");

    // === Multicolor (3+) ===
    card!(AtraxaGrandUnifier, "Atraxa, Grand Unifier", ManaCost { green: 1, white: 1, blue: 1, black: 1, generic: 3, ..c }, &[Creature], &[Legendary],
        Some(7), Some(7), None, flying_vigilance_deathtouch_lifelink(), &[Green, White, Blue, Black],
        "Flying, vigilance, deathtouch, lifelink. When Atraxa enters, reveal the top ten cards of your library. For each card type, you may put a card of that type from among the revealed cards into your hand. Put the rest on the bottom in a random order.");

    db
}

/// Returns true if the given CardName is a land card.
/// Used when the card DB is not available (e.g., in mana_ability_options).
pub fn is_land_card(name: CardName) -> bool {
    matches!(name,
        CardName::Plains | CardName::Island | CardName::Swamp | CardName::Mountain | CardName::Forest
        | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::Tundra | CardName::TropicalIsland
        | CardName::Badlands | CardName::Bayou | CardName::Plateau | CardName::Savannah
        | CardName::Scrubland | CardName::Taiga
        | CardName::FloodedStrand | CardName::PollutedDelta | CardName::BloodstainedMire
        | CardName::WoodedFoothills | CardName::WindsweptHeath | CardName::MistyRainforest
        | CardName::ScaldingTarn | CardName::VerdantCatacombs | CardName::AridMesa | CardName::MarshFlats
        | CardName::HallowedFountain | CardName::WateryGrave | CardName::BloodCrypt
        | CardName::StompingGround | CardName::TempleGarden | CardName::GodlessShrine
        | CardName::SteamVents | CardName::OvergrownTomb | CardName::SacredFoundry | CardName::BreedingPool
        | CardName::MeticulousArchive | CardName::UndercitySewers | CardName::ThunderingFalls | CardName::HedgeMaze
        | CardName::LibraryOfAlexandria | CardName::StripMine | CardName::Wasteland
        | CardName::TolarianAcademy | CardName::AncientTomb | CardName::MishrasWorkshop
        | CardName::Karakas | CardName::UrborgTombOfYawgmoth | CardName::OtawaraSoaringCity
        | CardName::BoseijuWhoEndures | CardName::GaeasCradle | CardName::YavimayaCradleOfGrowth
        | CardName::CityOfTraitors | CardName::ForbiddenOrchard | CardName::GhostQuarter
        | CardName::SpireOfIndustry | CardName::StartingTown | CardName::TalonGatesOfMadara
        | CardName::ShelldockIsle | CardName::MosswortBridge | CardName::TheMycoSynthGardens
        | CardName::UrzasSaga | CardName::BazaarOfBaghdad | CardName::DryadArbor | CardName::CavernOfSouls
        | CardName::ShatterskullTheHammerPass
        | CardName::VolcanicFissure
        | CardName::WitchBlessedMeadow
    )
}

/// Returns true if the given CardName is an instant or sorcery card.
/// Uses a lazily-initialized lookup table built from the card database for O(1) checks.
pub fn is_instant_or_sorcery(name: CardName) -> bool {
    use std::sync::OnceLock;
    static LOOKUP: OnceLock<Vec<bool>> = OnceLock::new();
    let table = LOOKUP.get_or_init(|| {
        let db = build_card_db();
        let mut flags = vec![false; CardName::_Count as usize];
        for card in &db {
            if card.card_types.contains(&CardType::Instant)
                || card.card_types.contains(&CardType::Sorcery)
            {
                flags[card.name as usize] = true;
            }
        }
        flags
    });
    let idx = name as usize;
    idx < table.len() && table[idx]
}

/// Returns true if the given CardName is a basic land (Plains, Island, Swamp, Mountain, Forest).
pub fn is_basic_land_card(name: CardName) -> bool {
    matches!(name,
        CardName::Plains | CardName::Island | CardName::Swamp | CardName::Mountain | CardName::Forest
    )
}

/// Lookup a card definition by name. O(n) scan but only used during setup.
pub fn find_card(db: &[CardDef], name: CardName) -> Option<&CardDef> {
    db.iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_db_builds() {
        let db = build_card_db();
        assert!(db.len() > 50, "Expected 50+ cards, got {}", db.len());
    }

    #[test]
    fn test_find_black_lotus() {
        let db = build_card_db();
        let lotus = find_card(&db, CardName::BlackLotus).unwrap();
        assert_eq!(lotus.display_name, "Black Lotus");
        assert_eq!(lotus.mana_cost, ManaCost::ZERO);
        assert!(lotus.card_types.contains(&CardType::Artifact));
    }

    #[test]
    fn test_lightning_bolt_cost() {
        let db = build_card_db();
        let bolt = find_card(&db, CardName::LightningBolt).unwrap();
        assert_eq!(bolt.mana_cost.cmc(), 1);
        assert_eq!(bolt.mana_cost.red, 1);
    }
}
