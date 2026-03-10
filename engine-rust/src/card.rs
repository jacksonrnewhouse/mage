/// Card definitions: static card data (templates) separated from runtime state.
/// Card behaviors are dispatched by CardName enum for branch-prediction-friendly code.

use crate::mana::ManaCost;
use crate::types::*;

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

    // === Other Lands ===
    LibraryOfAlexandria,
    StripMine,
    Wasteland,
    TolarianAcademy,
    AncientTomb,
    MishrasWorkshop,

    // === Power 9 ===
    BlackLotus,
    AncestralRecall,
    TimeWalk,
    Timetwister,
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

    // === Blue Creatures ===
    SnapcasterMage,
    TrueNameNemesis,
    Hullbreacher,
    OppositionAgent,

    // === Blue Planeswalkers ===
    JaceTheMindSculptor,

    // === Black Spells ===
    DarkRitual,
    DemonicTutor,
    Thoughtseize,
    HymnToTourach,
    ToxicDeluge,
    Reanimate,
    Entomb,
    VampiricTutor,
    YawgmothsWill,
    TendrillsOfAgony,

    // === Black Creatures ===
    SheoldredTheApocalypse,
    DarkConfidant,

    // === Red Spells ===
    LightningBolt,
    WheelOfFortune,
    Pyroblast,
    RedElementalBlast,
    ChainLightning,

    // === Red Creatures ===
    GoblinGuide,
    MonasterySwiftspear,
    RagavanNimblePilferer,
    YoungPyromancer,

    // === White Spells ===
    SwordsToPlowshares,
    Balance,
    CouncilsJudgment,
    PathToExile,
    Armageddon,
    Disenchant,

    // === White Creatures ===
    ThaliaGuardianOfThraben,
    MonasteryMentor,
    Solitude,
    StoneforgeMystic,
    PalaceJailer,

    // === Green Spells ===
    Channel,
    GreenSunsZenith,
    NaturalOrder,
    Regrowth,

    // === Green Creatures ===
    BirdsOfParadise,
    CollectorOuphe,
    Endurance,
    QuirionRanger,

    // === Multicolor ===
    TeferiTimeRaveler,
    LeovoldEmissaryOfTrest,
    KolaghanCommand,
    DackFayden,

    // === Colorless Artifacts ===
    LodestoneGolem,
    WurmcoilEngine,
    Batterskull,
    Trinisphere,
    SkullClamp,

    // Sentinel value for array sizing
    _Count,
}

/// Static card definition. Immutable, shared across all game states.
#[derive(Debug, Clone)]
pub struct CardDef {
    pub name: CardName,
    pub display_name: &'static str,
    pub mana_cost: ManaCost,
    pub card_types: &'static [CardType],
    pub supertypes: &'static [SuperType],
    pub power: Option<i16>,
    pub toughness: Option<i16>,
    pub loyalty: Option<i8>,
    pub keywords: Keywords,
    pub color_identity: &'static [Color],
    pub oracle_text: &'static str,
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
                card_types: $types,
                supertypes: $supers,
                power: $pow,
                toughness: $tou,
                loyalty: $loy,
                keywords: $kw,
                color_identity: $colors,
                oracle_text: $text,
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
    card!(SnapcasterMage, "Snapcaster Mage", ManaCost { blue: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(1), None, flash(), &[Blue],
        "Flash. When Snapcaster Mage enters, target instant or sorcery card in your graveyard gains flashback until end of turn.");
    card!(TrueNameNemesis, "True-Name Nemesis", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(1), None, kw(), &[Blue],
        "As True-Name Nemesis enters, choose a player. True-Name Nemesis has protection from the chosen player.");
    card!(Hullbreacher, "Hullbreacher", ManaCost { blue: 1, generic: 2, ..c }, &[Creature], &[],
        Some(3), Some(2), None, flash(), &[Blue],
        "Flash. If an opponent would draw a card except the first one they draw in each of their draw steps, instead you create a Treasure token.");
    card!(OppositionAgent, "Opposition Agent", ManaCost { black: 1, generic: 2, ..c }, &[Creature], &[],
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
    card!(TendrillsOfAgony, "Tendrils of Agony", ManaCost { black: 2, generic: 2, ..c }, &[Sorcery], &[], None, None, None, kw(), &[Black],
        "Target player loses 2 life and you gain 2 life. Storm.");

    // === Black Creatures ===
    card!(SheoldredTheApocalypse, "Sheoldred, the Apocalypse", ManaCost { black: 2, generic: 2, ..c }, &[Creature], &[Legendary],
        Some(4), Some(5), None, kw(), &[Black],
        "Deathtouch. Whenever you draw a card, you gain 2 life. Whenever an opponent draws a card, they lose 2 life.");
    card!(DarkConfidant, "Dark Confidant", ManaCost { black: 1, generic: 1, ..c }, &[Creature], &[],
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
    card!(GoblinGuide, "Goblin Guide", ManaCost::r(1), &[Creature], &[],
        Some(2), Some(2), None, haste(), &[Red],
        "Haste. Whenever Goblin Guide attacks, defending player reveals the top card of their library. If it's a land card, that player puts it into their hand.");
    card!(MonasterySwiftspear, "Monastery Swiftspear", ManaCost::r(1), &[Creature], &[],
        Some(1), Some(2), None, prowess_haste(), &[Red],
        "Haste. Prowess.");
    card!(RagavanNimblePilferer, "Ragavan, Nimble Pilferer", ManaCost::r(1), &[Creature], &[Legendary],
        Some(2), Some(1), None, kw(), &[Red],
        "Whenever Ragavan deals combat damage to a player, create a Treasure token and exile the top card of that player's library. You may cast that card this turn.");
    card!(YoungPyromancer, "Young Pyromancer", ManaCost { red: 1, generic: 1, ..c }, &[Creature], &[],
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
    card!(ThaliaGuardianOfThraben, "Thalia, Guardian of Thraben", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[Legendary],
        Some(2), Some(1), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::FirstStrike);
            k
        }, &[White],
        "First strike. Noncreature spells cost {1} more to cast.");
    card!(MonasteryMentor, "Monastery Mentor", ManaCost { white: 1, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Prowess);
            k
        }, &[White],
        "Prowess. Whenever you cast a noncreature spell, create a 1/1 white Monk creature token with prowess.");
    card!(Solitude, "Solitude", ManaCost { white: 2, generic: 3, ..c }, &[Creature], &[],
        Some(3), Some(2), None, flash_flying(), &[White],
        "Flash. Flying. Lifelink. When Solitude enters, exile up to one other target creature. That creature's controller gains life equal to its power. Evoke - Exile a white card from your hand.");
    card!(StoneforgeMystic, "Stoneforge Mystic", ManaCost { white: 1, generic: 1, ..c }, &[Creature], &[],
        Some(1), Some(2), None, kw(), &[White],
        "When Stoneforge Mystic enters, you may search your library for an Equipment card, reveal it, put it into your hand, then shuffle. {1}{W}, {T}: You may put an Equipment card from your hand onto the battlefield.");
    card!(PalaceJailer, "Palace Jailer", ManaCost { white: 2, generic: 2, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[White],
        "When Palace Jailer enters, you become the monarch. When Palace Jailer enters, exile target creature an opponent controls until an opponent becomes the monarch.");

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
    card!(BirdsOfParadise, "Birds of Paradise", ManaCost::g(1), &[Creature], &[],
        Some(0), Some(1), None, flying(), &[Green],
        "Flying. {T}: Add one mana of any color.");
    card!(CollectorOuphe, "Collector Ouphe", ManaCost { green: 1, generic: 1, ..c }, &[Creature], &[],
        Some(2), Some(2), None, kw(), &[Green],
        "Activated abilities of artifacts can't be activated.");
    card!(Endurance, "Endurance", ManaCost { green: 2, generic: 1, ..c }, &[Creature], &[],
        Some(3), Some(4), None, flash(), &[Green],
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
    card!(LodestoneGolem, "Lodestone Golem", ManaCost::generic(4), &[Artifact, Creature], &[],
        Some(5), Some(3), None, kw(), &[],
        "Nonartifact spells cost {1} more to cast.");
    card!(WurmcoilEngine, "Wurmcoil Engine", ManaCost::generic(6), &[Artifact, Creature], &[],
        Some(6), Some(6), None, {
            let mut k = Keywords::empty();
            k.add(Keyword::Deathtouch);
            k.add(Keyword::Lifelink);
            k
        }, &[],
        "Deathtouch, lifelink. When Wurmcoil Engine dies, create a 3/3 colorless Wurm artifact creature token with deathtouch and a 3/3 colorless Wurm artifact creature token with lifelink.");
    card!(Batterskull, "Batterskull", ManaCost::generic(5), &[Artifact], &[],
        None, None, None, kw(), &[],
        "Living weapon. Equipped creature gets +4/+4 and has vigilance and lifelink. {3}: Return Batterskull to its owner's hand. Equip {5}.");
    card!(Trinisphere, "Trinisphere", ManaCost::generic(3), &[Artifact], &[],
        None, None, None, kw(), &[],
        "As long as Trinisphere is untapped, each spell that would cost less than {3} costs {3} to cast.");
    card!(SkullClamp, "Skullclamp", ManaCost::generic(1), &[Artifact], &[],
        None, None, None, kw(), &[],
        "Equipped creature gets +1/-1. When equipped creature dies, draw two cards. Equip {1}.");

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

    db
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
