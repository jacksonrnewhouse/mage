/// Vintage Supreme Draft (VSD) implementation.
///
/// Format: 23 packs of 15 cards. Player picks 2 from each pack = 46 card pool.
/// Some cards appear twice in the cube (130 doubles), others once (250 singles).
/// The cube totals 510 cards; each draft uses 345 (23 * 15), leaving 165 undealt.
/// After drafting, player builds a 40-card deck from their pool, optionally adding basic lands.

use crate::card::CardName;

/// The number of packs in a VSD draft.
pub const PACKS: usize = 23;
/// Cards per pack.
pub const CARDS_PER_PACK: usize = 15;
/// Cards picked per pack.
pub const PICKS_PER_PACK: usize = 2;
/// Total cards in the drafted pool.
pub const POOL_SIZE: usize = PACKS * PICKS_PER_PACK; // 46
/// Minimum deck size.
pub const MIN_DECK_SIZE: usize = 40;
/// Expected total cube size (250 singles + 130 doubles).
pub const CUBE_SIZE: usize = 510;

/// Returns true if a card has two copies in the VSD cube.
/// Based on the official Chris Wolf VSD list.
pub fn has_two_copies(name: CardName) -> bool {
    use CardName::*;
    matches!(
        name,
        // White
        NomadsEnKor
            | VoiceOfVictory
            | ArchonOfEmeria
            | BoromirWardenOfTheTower
            | ClarionConqueror
            | WhitePlumeAdventurer
            | MarchOfOtherworldlyLight
            | SwordsToPlowshares
            | WrathOfTheSkies
            | PortableHole
            | WitchEnchanter
            // Blue
            | TamiyoInquisitiveStudent
            | ThundertrapTrainer
            | Hullbreacher
            | ThassasOracle
            | ChainOfVapor
            | ConsignToMemory
            | Flusterstorm
            | IntoTheFloodMaw
            | SpellPierce
            | BrainFreeze
            | ManaDrain
            | ParadoxicalOutcome
            | ForceOfWill
            | ShowAndTell
            | StockUp
            | Ponder
            | MysticRemora
            | DressDown
            // Black
            | DauthiVoidwalker
            | MaiScornfulStriker
            | OrcishBowmasters
            | Barrowgoyf
            | Grief
            | DarkRitual
            | Entomb
            | FatalPush
            | BitterTriumph
            | SnuffOut
            | Reanimate
            | Thoughtseize
            | HymnToTourach
            | Doomsday
            | BeseechTheMirror
            | TendrilsOfAgony
            | AnimateDead
            | ChainsOfMephistopheles
            // Red
            | GorillaShaman
            | RagavanNimblePilferer
            | BroadsideBombardiers
            | MagusOfTheMoon
            | SimianSpiritGuide
            | CavesOfChaosAdventurer
            | LightningBolt
            | Pyroblast
            | RoilingVortex
            | UnderworldBreach
            | FableOfTheMirrorBreaker
            | BaskingRootwalla
            // Green
            | DelightedHalfling
            | CollectorOuphe
            | Endurance
            | Vengevine
            | HollowOne
            | ForceOfVigor
            | OathOfDruids
            | MasterOfDeath
            | HogaakArisenNecropolis
            | KishlaSkimmer
            | BazaarOfBaghdad
            // Colorless creatures
            | StonecoilSerpent
            | PhyrexianDreadnought
            | PatchworkAutomaton
            | GlaringFleshraker
            | PhyrexianMetamorph
            | ScrapTrawler
            | ScrawlingCrawler
            | MindbreakTrap
            | NoxiousRevival
            | Dismember
            // Colorless artifacts
            | ChromeMox
            | SenseisDiviningTop
            | Shuko
            | SoulGuideLantern
            | DampingSphere
            | DefenseGrid
            | GrimMonolith
            | NullRod
            | SphereOfResistance
            | VoidMirror
            | KrarkClanIronworks
            | TheOneRing
            | CovetedJewel
            // Multicolor + Guilds
            | LaviniaAzoriusRenegade
            | MakdeeAndItlaSkysnarers
            | PsychicFrog
            | WrennAndSix
            | MinscAndBooTimelessHeroes
            | FlameOfAnor
            | DeathriteShaman
            | ForthEorlingas
            | MemorysJourney
            | NaduWingedWisdom
            | OkoThiefOfCrowns
            | AtraxaGrandUnifier
            // Lands
            | FloodedStrand
            | Tundra
            | PollutedDelta
            | UndergroundSea
            | Badlands
            | BloodstainedMire
            | Taiga
            | WoodedFoothills
            | Savannah
            | WindsweptHeath
            | MarshFlats
            | Scrubland
            | ScaldingTarn
            | VolcanicIsland
            | Bayou
            | VerdantCatacombs
            | AridMesa
            | Plateau
            | MistyRainforest
            | TropicalIsland
            | AncientTomb
            | ForbiddenOrchard
            | MishrasWorkshop
            | TalonGatesOfMadara
            | Wasteland
    )
}

/// All CardName variants in the VSD card pool (380 unique cards).
/// This is the canonical list; the cube is built from this.
pub fn vsd_card_pool() -> Vec<CardName> {
    use CardName::*;
    vec![
        // === Lands (from land.txt) ===
        FloodedStrand,
        HallowedFountain,
        MeticulousArchive,
        Tundra,
        PollutedDelta,
        UndercitySewers,
        UndergroundSea,
        WateryGrave,
        Badlands,
        BloodCrypt,
        BloodstainedMire,
        StompingGround,
        Taiga,
        WoodedFoothills,
        Savannah,
        TempleGarden,
        WindsweptHeath,
        GodlessShrine,
        MarshFlats,
        Scrubland,
        ScaldingTarn,
        SteamVents,
        ThunderingFalls,
        VolcanicIsland,
        Bayou,
        OvergrownTomb,
        VerdantCatacombs,
        AridMesa,
        Plateau,
        SacredFoundry,
        BreedingPool,
        HedgeMaze,
        MistyRainforest,
        TropicalIsland,
        AncientTomb,
        CityOfTraitors,
        ForbiddenOrchard,
        GhostQuarter,
        MishrasWorkshop,
        SpireOfIndustry,
        StartingTown,
        StripMine,
        TalonGatesOfMadara,
        TheMycoSynthGardens,
        TolarianAcademy,
        UrzasSaga,
        Wasteland,
        // === White (from white.txt) ===
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
        GideonOfTheTrials,
        EnlightenedTutor,
        MarchOfOtherworldlyLight,
        SwordsToPlowshares,
        Fragmentize,
        PrismaticEnding,
        Balance,
        WrathOfTheSkies,
        PortableHole,
        DeafeningSilence,
        HighNoon,
        RestInPeace,
        SealOfCleansing,
        StonySilence,
        Karakas,
        WitchEnchanter,
        // === Blue (from blue.txt) ===
        TamiyoInquisitiveStudent,
        AphettoAlchemist,
        MercurialSpelldancer,
        SnapcasterMage,
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
        NarsetParterOfVeils,
        AncestralRecall,
        Brainstorm,
        ChainOfVapor,
        ConsignToMemory,
        Flusterstorm,
        IntoTheFloodMaw,
        MysticalTutor,
        SpellPierce,
        Stifle,
        BrainFreeze,
        Daze,
        Flash,
        HurkylsRecall,
        ManaDrain,
        ManaLeak,
        MemoryLapse,
        Remand,
        ForceOfNegation,
        MysticalDispute,
        GiftsUngiven,
        ParadoxicalOutcome,
        ForceOfWill,
        Gush,
        Misdirection,
        Commandeer,
        DigThroughTime,
        CarefulStudy,
        Ponder,
        Preordain,
        MerchantScroll,
        TimeWalk,
        TransmuteArtifact,
        ShowAndTell,
        StockUp,
        Timetwister,
        Tinker,
        Windfall,
        LorienRevealed,
        StepThrough,
        Thoughtcast,
        EchoOfEons,
        MindsDesire,
        TreasureCruise,
        AetherSpellbomb,
        CryogenRelic,
        MysticRemora,
        UnableToScream,
        DressDown,
        EnergyFlux,
        OtawaraSoaringCity,
        SinkIntoStupor,
        // === Black (from black.txt) ===
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
        DarkRitual,
        DemonicConsultation,
        Entomb,
        FatalPush,
        VampiricTutor,
        BitterTriumph,
        CabalRitual,
        SheoldredsEdict,
        SnuffOut,
        Duress,
        ImperialSeal,
        InquisitionOfKozilek,
        MindTwist,
        Reanimate,
        Thoughtseize,
        DemonicTutor,
        Exhume,
        HymnToTourach,
        Doomsday,
        YawgmothsWill,
        BeseechTheMirror,
        TendrilsOfAgony,
        Unmask,
        BolassCitadel,
        AnimateDead,
        ChainsOfMephistopheles,
        Necropotence,
        UrborgTombOfYawgmoth,
        // === Red (from red.txt) ===
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
        LightningBolt,
        Pyroblast,
        RedElementalBlast,
        RedirectLightning,
        Abrade,
        ShrapnelBlast,
        UntimelyMalfunction,
        Crash,
        ChainLightning,
        Meltdown,
        ShatteringSpree,
        Vandalblast,
        Suplex,
        BrotherhoodsEnd,
        WheelOfFortune,
        RoilingVortex,
        UnderworldBreach,
        BloodMoon,
        FableOfTheMirrorBreaker,
        ShatterskullSmashing,
        SunderingEruption,
        BaskingRootwalla,
        BlazingRootwalla,
        DryadArbor,
        SqueeGoblinNabob,
        // === Green (from green.txt) ===
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
        CropRotation,
        NaturesClaim,
        VeilOfSummer,
        OnceUponATime,
        ForceOfVigor,
        GreenSunsZenith,
        Channel,
        LifeFromTheLoam,
        SeedsOfInnocence,
        OathOfDruids,
        MasterOfDeath,
        HogaakArisenNecropolis,
        KishlaSkimmer,
        BazaarOfBaghdad,
        BoseijuWhoEndures,
        GaeasCradle,
        YavimayaCradleOfGrowth,
        // === Colorless (from colorless.txt) ===
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
        TezzeretCruelCaptain,
        KarnTheGreatCreator,
        MentalMisstep,
        MindbreakTrap,
        NoxiousRevival,
        Dismember,
        KozileksCommand,
        GitaxianProbe,
        BlackLotus,
        ChaliceOfTheVoid,
        ChromeMox,
        ClownCar,
        EngineeredExplosives,
        Gleemox,
        LionEyeDiamond,
        LotusPetal,
        ManaCrypt,
        MishrasBauble,
        MoxEmerald,
        MoxJet,
        MoxOpal,
        MoxPearl,
        MoxRuby,
        MoxSapphire,
        TormodsCrypt,
        UrzasBauble,
        ChromaticStar,
        GrafdiggersCage,
        LavaspurBoots,
        ManaVault,
        ManifoldKey,
        PithingNeedle,
        SenseisDiviningTop,
        Shadowspear,
        Shuko,
        SolRing,
        SoulGuideLantern,
        VexingBauble,
        VoltaicKey,
        DampingSphere,
        DefenseGrid,
        DisruptorFlute,
        GrimMonolith,
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
        // === Multicolor + Guilds ===
        LeovoldEmissaryOfTrest,
        AtraxaGrandUnifier,
        LaviniaAzoriusRenegade,
        MakdeeAndItlaSkysnarers,
        DovinHandOfControl,
        TeferiTimeRaveler,
        PsychicFrog,
        MoltenCollapse,
        HidetsuguConsumesAll,
        AncientGrudge,
        Cindervines,
        WrennAndSix,
        MinscAndBooTimelessHeroes,
        DryadMilitant,
        PestControl,
        KayaOrzhovUsurper,
        LurrusOfTheDreamDen,
        ExpressiveIteration,
        DackFayden,
        FlameOfAnor,
        PinnacleEmissary,
        DeathriteShaman,
        AbruptDecay,
        ForthEorlingas,
        CometStellarPup,
        MemorysJourney,
        NaduWingedWisdom,
        OkoThiefOfCrowns,
    ]
}

/// Build the VSD cube: each unrestricted card appears twice, restricted cards once.
/// Returns a flat list of CardName suitable for shuffling.
pub fn build_cube() -> Vec<CardName> {
    let pool = vsd_card_pool();
    let mut cube = Vec::with_capacity(CUBE_SIZE);
    for card in &pool {
        cube.push(*card);
        if has_two_copies(*card) {
            cube.push(*card); // second copy
        }
    }
    cube
}

/// A single draft pack: 15 cards face-up for the player to pick from.
#[derive(Debug, Clone)]
pub struct DraftPack {
    pub cards: Vec<CardName>,
}

/// State of a VSD draft in progress.
#[derive(Debug, Clone)]
pub struct DraftState {
    /// Remaining packs to open (index 0 = next pack).
    pub packs: Vec<DraftPack>,
    /// Cards the player has picked so far.
    pub pool: Vec<CardName>,
    /// The current pack being picked from (None if between packs / draft complete).
    pub current_pack: Option<DraftPack>,
}

impl DraftState {
    /// Start a new VSD draft with a shuffled cube.
    /// `rng_shuffle` is a function that shuffles a mutable slice in place.
    /// This avoids requiring a rand dependency — the caller provides the shuffle.
    pub fn new(shuffle: impl FnOnce(&mut [CardName])) -> Self {
        let mut cube = build_cube();
        shuffle(&mut cube);

        let mut packs = Vec::with_capacity(PACKS);
        for i in 0..PACKS {
            let start = i * CARDS_PER_PACK;
            let end = start + CARDS_PER_PACK;
            if end <= cube.len() {
                packs.push(DraftPack {
                    cards: cube[start..end].to_vec(),
                });
            }
        }

        let current_pack = if !packs.is_empty() {
            Some(packs.remove(0))
        } else {
            None
        };

        DraftState {
            packs,
            pool: Vec::with_capacity(POOL_SIZE),
            current_pack,
        }
    }

    /// How many packs remain (including the current one).
    pub fn packs_remaining(&self) -> usize {
        let current = if self.current_pack.is_some() { 1 } else { 0 };
        current + self.packs.len()
    }

    /// Whether the draft is complete.
    pub fn is_complete(&self) -> bool {
        self.current_pack.is_none() && self.packs.is_empty()
    }

    /// Pick two cards from the current pack by index.
    /// Returns Err if indices are invalid or the draft is complete.
    pub fn pick(&mut self, idx_a: usize, idx_b: usize) -> Result<(), DraftError> {
        if idx_a == idx_b {
            return Err(DraftError::DuplicateIndex);
        }
        let pack = self
            .current_pack
            .as_ref()
            .ok_or(DraftError::DraftComplete)?;
        if idx_a >= pack.cards.len() || idx_b >= pack.cards.len() {
            return Err(DraftError::IndexOutOfBounds);
        }

        let card_a = pack.cards[idx_a];
        let card_b = pack.cards[idx_b];
        self.pool.push(card_a);
        self.pool.push(card_b);

        // Advance to next pack
        self.current_pack = if !self.packs.is_empty() {
            Some(self.packs.remove(0))
        } else {
            None
        };

        Ok(())
    }
}

/// Errors that can occur during drafting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftError {
    DuplicateIndex,
    IndexOutOfBounds,
    DraftComplete,
}

impl std::fmt::Display for DraftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DraftError::DuplicateIndex => write!(f, "must pick two different cards"),
            DraftError::IndexOutOfBounds => write!(f, "card index out of bounds"),
            DraftError::DraftComplete => write!(f, "draft is already complete"),
        }
    }
}

impl std::error::Error for DraftError {}

/// A constructed deck built from a draft pool.
#[derive(Debug, Clone)]
pub struct Deck {
    /// The main deck (minimum 40 cards).
    pub main: Vec<CardName>,
    /// Remaining pool cards not in the main deck (sideboard).
    pub sideboard: Vec<CardName>,
}

/// Errors that can occur during deck building.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckError {
    /// Deck is too small (< 40 cards).
    TooFewCards { count: usize },
    /// A non-basic-land card was included that isn't in the pool.
    CardNotInPool(CardName),
    /// More copies of a card were included than are available in the pool.
    TooManyCopies { card: CardName, available: usize, requested: usize },
}

impl std::fmt::Display for DeckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeckError::TooFewCards { count } => {
                write!(f, "deck has {} cards, minimum is {}", count, MIN_DECK_SIZE)
            }
            DeckError::CardNotInPool(name) => {
                write!(f, "{:?} is not in the draft pool", name)
            }
            DeckError::TooManyCopies { card, available, requested } => {
                write!(
                    f,
                    "{:?}: requested {} copies but only {} available",
                    card, requested, available
                )
            }
        }
    }
}

impl std::error::Error for DeckError {}

/// The five basic land types that can be added freely during deck building.
const BASIC_LANDS: [CardName; 5] = [
    CardName::Plains,
    CardName::Island,
    CardName::Swamp,
    CardName::Mountain,
    CardName::Forest,
];

fn is_basic_land(name: CardName) -> bool {
    BASIC_LANDS.contains(&name)
}

/// Build a deck from a draft pool.
///
/// `pool` is the 46 cards drafted. `main_deck` is the list of CardNames the player
/// wants in their main deck. Basic lands can be added freely (they don't need to be
/// in the pool). Non-basic cards must come from the pool.
///
/// Returns a Deck with main deck and sideboard, or an error.
pub fn build_deck(pool: &[CardName], main_deck: &[CardName]) -> Result<Deck, DeckError> {
    if main_deck.len() < MIN_DECK_SIZE {
        return Err(DeckError::TooFewCards {
            count: main_deck.len(),
        });
    }

    // Count available copies from pool (excluding basics, which are unlimited).
    let mut pool_counts: std::collections::HashMap<CardName, usize> =
        std::collections::HashMap::new();
    for &card in pool {
        if !is_basic_land(card) {
            *pool_counts.entry(card).or_insert(0) += 1;
        }
    }

    // Validate main deck selections.
    let mut used_counts: std::collections::HashMap<CardName, usize> =
        std::collections::HashMap::new();
    for &card in main_deck {
        if is_basic_land(card) {
            continue; // basics are unlimited
        }
        let available = pool_counts.get(&card).copied().unwrap_or(0);
        if available == 0 {
            return Err(DeckError::CardNotInPool(card));
        }
        let used = used_counts.entry(card).or_insert(0);
        *used += 1;
        if *used > available {
            return Err(DeckError::TooManyCopies {
                card,
                available,
                requested: *used,
            });
        }
    }

    // Build sideboard from remaining pool cards.
    let mut remaining_pool = pool_counts.clone();
    for (&card, &count) in &used_counts {
        if let Some(rem) = remaining_pool.get_mut(&card) {
            *rem = rem.saturating_sub(count);
        }
    }
    let mut sideboard = Vec::new();
    for (&card, &count) in &remaining_pool {
        for _ in 0..count {
            sideboard.push(card);
        }
    }

    Ok(Deck {
        main: main_deck.to_vec(),
        sideboard,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cube_size() {
        let cube = build_cube();
        let pool = vsd_card_pool();
        assert_eq!(cube.len(), CUBE_SIZE, "Cube should have exactly {} cards", CUBE_SIZE);
        for card in &pool {
            let count = cube.iter().filter(|c| *c == card).count();
            if has_two_copies(*card) {
                assert_eq!(count, 2, "{:?} should appear twice", card);
            } else {
                assert_eq!(count, 1, "{:?} should appear once", card);
            }
        }
    }

    #[test]
    fn test_cube_has_enough_cards_for_draft() {
        let cube = build_cube();
        assert!(
            cube.len() >= PACKS * CARDS_PER_PACK,
            "Cube has {} cards but needs {} for {} packs of {}",
            cube.len(),
            PACKS * CARDS_PER_PACK,
            PACKS,
            CARDS_PER_PACK
        );
    }

    #[test]
    fn test_draft_flow() {
        // Use a deterministic "shuffle" (identity) for testing.
        let mut draft = DraftState::new(|_| {});
        assert_eq!(draft.packs_remaining(), PACKS);
        assert!(!draft.is_complete());

        // Pick from all 23 packs.
        for _ in 0..PACKS {
            assert!(draft.current_pack.is_some());
            draft.pick(0, 1).unwrap();
        }

        assert!(draft.is_complete());
        assert_eq!(draft.pool.len(), POOL_SIZE);
    }

    #[test]
    fn test_draft_pick_errors() {
        let mut draft = DraftState::new(|_| {});

        // Duplicate index
        assert_eq!(draft.pick(0, 0), Err(DraftError::DuplicateIndex));

        // Out of bounds
        assert_eq!(draft.pick(0, 100), Err(DraftError::IndexOutOfBounds));
    }

    #[test]
    fn test_build_deck_basic() {
        // Create a simple pool
        let pool = vec![CardName::LightningBolt; 2];
        let mut main = vec![CardName::LightningBolt; 2];
        // Fill remaining slots with basic lands
        for _ in 0..38 {
            main.push(CardName::Mountain);
        }
        let deck = build_deck(&pool, &main).unwrap();
        assert_eq!(deck.main.len(), 40);
        assert_eq!(deck.sideboard.len(), 0);
    }

    #[test]
    fn test_build_deck_too_few_cards() {
        let pool = vec![CardName::LightningBolt];
        let main = vec![CardName::LightningBolt];
        assert!(matches!(
            build_deck(&pool, &main),
            Err(DeckError::TooFewCards { count: 1 })
        ));
    }

    #[test]
    fn test_build_deck_card_not_in_pool() {
        let pool = vec![CardName::LightningBolt; 2];
        let mut main = vec![CardName::LightningBolt, CardName::Abrade];
        for _ in 0..38 {
            main.push(CardName::Mountain);
        }
        assert!(matches!(
            build_deck(&pool, &main),
            Err(DeckError::CardNotInPool(CardName::Abrade))
        ));
    }

    #[test]
    fn test_build_deck_too_many_copies() {
        let pool = vec![CardName::LightningBolt];
        let mut main = vec![CardName::LightningBolt; 2];
        for _ in 0..38 {
            main.push(CardName::Mountain);
        }
        assert!(matches!(
            build_deck(&pool, &main),
            Err(DeckError::TooManyCopies { .. })
        ));
    }

    #[test]
    fn test_build_deck_sideboard() {
        let pool = vec![CardName::LightningBolt; 2];
        let mut main = vec![CardName::LightningBolt];
        for _ in 0..39 {
            main.push(CardName::Mountain);
        }
        let deck = build_deck(&pool, &main).unwrap();
        assert_eq!(deck.main.len(), 40);
        assert_eq!(deck.sideboard.len(), 1);
        assert_eq!(deck.sideboard[0], CardName::LightningBolt);
    }

    #[test]
    fn test_vsd_pool_size() {
        assert_eq!(vsd_card_pool().len(), 380);
    }

    #[test]
    fn test_doubles_count() {
        let pool = vsd_card_pool();
        let doubles: Vec<_> = pool
            .iter()
            .filter(|c| has_two_copies(**c))
            .collect();
        assert_eq!(
            doubles.len(), 130,
            "Expected 130 cards with two copies, got {}",
            doubles.len()
        );
    }
}
