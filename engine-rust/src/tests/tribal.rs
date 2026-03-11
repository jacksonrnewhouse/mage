/// Tests for creature type system and tribal interactions.

use crate::card::*;
use crate::game::*;
use crate::types::*;

/// Helper to create a permanent from a CardDef and put it on the battlefield.
fn put_on_battlefield(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: u8) -> u32 {
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        card_id, card_name, controller, controller,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    // Set creature types from card definition
    if def.is_changeling {
        perm.creature_types = CreatureType::ALL.to_vec();
    } else {
        perm.creature_types = def.creature_types.to_vec();
    }
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    card_id
}

#[test]
fn test_goblin_guide_is_goblin() {
    let db = build_card_db();
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Goblin),
        "Goblin Guide should have creature type Goblin");
    assert!(!def.creature_types.contains(&CreatureType::Human),
        "Goblin Guide should not have creature type Human");
}

#[test]
fn test_monastery_swiftspear_is_human_monk() {
    let db = build_card_db();
    let def = find_card(&db, CardName::MonasterySwiftspear).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Human),
        "Monastery Swiftspear should be a Human");
    assert!(def.creature_types.contains(&CreatureType::Monk),
        "Monastery Swiftspear should be a Monk");
}

#[test]
fn test_snapcaster_mage_is_human_wizard() {
    let db = build_card_db();
    let def = find_card(&db, CardName::SnapcasterMage).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Human),
        "Snapcaster Mage should be a Human");
    assert!(def.creature_types.contains(&CreatureType::Wizard),
        "Snapcaster Mage should be a Wizard");
}

#[test]
fn test_squee_goblin_nabob_is_goblin() {
    let db = build_card_db();
    let def = find_card(&db, CardName::SqueeGoblinNabob).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Goblin),
        "Squee, Goblin Nabob should have creature type Goblin");
}

#[test]
fn test_birds_of_paradise_is_bird() {
    let db = build_card_db();
    let def = find_card(&db, CardName::BirdsOfParadise).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Bird),
        "Birds of Paradise should have creature type Bird");
}

#[test]
fn test_elvish_spirit_guide_is_elf_spirit() {
    let db = build_card_db();
    let def = find_card(&db, CardName::ElvishSpiritGuide).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Elf),
        "Elvish Spirit Guide should have creature type Elf");
    assert!(def.creature_types.contains(&CreatureType::Spirit),
        "Elvish Spirit Guide should have creature type Spirit");
}

#[test]
fn test_permanent_has_creature_types_after_casting() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player mana and a Goblin Guide in hand
    let guide_id = state.new_object_id();
    state.card_registry.push((guide_id, CardName::GoblinGuide));
    state.players[0].hand.push(guide_id);
    state.players[0].mana_pool.red = 1;

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Cast Goblin Guide
    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: guide_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Pass priority twice to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Find the permanent on battlefield
    let goblin = state.battlefield.iter().find(|p| p.card_name == CardName::GoblinGuide);
    assert!(goblin.is_some(), "Goblin Guide should be on battlefield");
    let goblin = goblin.unwrap();
    assert!(goblin.has_creature_type(CreatureType::Goblin),
        "Goblin Guide permanent should have Goblin creature type");
}

#[test]
fn test_cavern_of_souls_produces_mana() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place a Cavern of Souls on the battlefield with Goblin as named type
    let cavern_id = put_on_battlefield(&mut state, &db, CardName::CavernOfSouls, 0);

    // Manually set the cavern's chosen type (as would happen via ETB choice)
    if let Some(perm) = state.find_permanent_mut(cavern_id) {
        perm.cavern_creature_type = Some(CreatureType::Goblin);
    }

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Tap it for red mana
    let result = state.activate_mana_ability(cavern_id, Some(Color::Red));
    assert!(result, "Cavern of Souls should be able to tap for mana");
    assert_eq!(state.players[0].mana_pool.red, 1,
        "Cavern should produce 1 red mana");
    assert!(state.battlefield.iter().any(|p| p.id == cavern_id && p.tapped),
        "Cavern should be tapped after activating");
}

#[test]
fn test_cavern_of_souls_colorless_mana() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let cavern_id = put_on_battlefield(&mut state, &db, CardName::CavernOfSouls, 0);
    if let Some(perm) = state.find_permanent_mut(cavern_id) {
        perm.cavern_creature_type = Some(CreatureType::Human);
    }

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Tap for colorless
    let result = state.activate_mana_ability(cavern_id, None);
    assert!(result, "Cavern of Souls should produce colorless mana");
    assert_eq!(state.players[0].mana_pool.colorless, 1,
        "Cavern should produce 1 colorless mana");
}

#[test]
fn test_cavern_of_souls_makes_goblin_spell_uncounterable() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a Cavern of Souls on battlefield naming Goblin
    let cavern_id = put_on_battlefield(&mut state, &db, CardName::CavernOfSouls, 0);
    if let Some(perm) = state.find_permanent_mut(cavern_id) {
        perm.cavern_creature_type = Some(CreatureType::Goblin);
    }

    // Check that GoblinGuide would be uncounterable under Cavern
    let guide_def = find_card(&db, CardName::GoblinGuide).unwrap();
    let uncounterable = state.cavern_makes_uncounterable(0, guide_def, CardName::GoblinGuide);
    assert!(uncounterable,
        "Goblin Guide should be uncounterable with Goblin Cavern");
}

#[test]
fn test_cavern_of_souls_does_not_make_non_matching_spell_uncounterable() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Cavern named Goblin should NOT affect Human spells
    let cavern_id = put_on_battlefield(&mut state, &db, CardName::CavernOfSouls, 0);
    if let Some(perm) = state.find_permanent_mut(cavern_id) {
        perm.cavern_creature_type = Some(CreatureType::Goblin);
    }

    // Snapcaster Mage is Human, not Goblin
    let snappy_def = find_card(&db, CardName::SnapcasterMage).unwrap();
    let uncounterable = state.cavern_makes_uncounterable(0, snappy_def, CardName::SnapcasterMage);
    assert!(!uncounterable,
        "Snapcaster Mage should NOT be uncounterable with Goblin Cavern");
}

#[test]
fn test_cavern_no_type_chosen_does_not_make_uncounterable() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Cavern without any type chosen (no ETB choice yet)
    let cavern_id = put_on_battlefield(&mut state, &db, CardName::CavernOfSouls, 0);
    // cavern_creature_type is None by default

    let guide_def = find_card(&db, CardName::GoblinGuide).unwrap();
    let uncounterable = state.cavern_makes_uncounterable(0, guide_def, CardName::GoblinGuide);
    assert!(!uncounterable,
        "Cavern with no chosen type should not make spells uncounterable");

    let _ = cavern_id;
}

#[test]
fn test_thalia_is_human_soldier() {
    let db = build_card_db();
    let def = find_card(&db, CardName::ThaliaGuardianOfThraben).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Human),
        "Thalia should be a Human");
    assert!(def.creature_types.contains(&CreatureType::Soldier),
        "Thalia should be a Soldier");
}

#[test]
fn test_myr_retriever_is_myr() {
    let db = build_card_db();
    let def = find_card(&db, CardName::MyrRetriever).unwrap();
    assert!(def.creature_types.contains(&CreatureType::Myr),
        "Myr Retriever should have creature type Myr");
}

#[test]
fn test_cavern_naming_human_enables_thalia() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let cavern_id = put_on_battlefield(&mut state, &db, CardName::CavernOfSouls, 0);
    if let Some(perm) = state.find_permanent_mut(cavern_id) {
        perm.cavern_creature_type = Some(CreatureType::Human);
    }

    let thalia_def = find_card(&db, CardName::ThaliaGuardianOfThraben).unwrap();
    let uncounterable = state.cavern_makes_uncounterable(0, thalia_def, CardName::ThaliaGuardianOfThraben);
    assert!(uncounterable,
        "Thalia should be uncounterable when Cavern names Human");
}

#[test]
fn test_creature_type_all_includes_common_types() {
    let all = CreatureType::ALL;
    assert!(all.contains(&CreatureType::Human));
    assert!(all.contains(&CreatureType::Goblin));
    assert!(all.contains(&CreatureType::Elf));
    assert!(all.contains(&CreatureType::Wizard));
    assert!(all.contains(&CreatureType::Zombie));
    assert!(all.contains(&CreatureType::Spirit));
    assert!(all.contains(&CreatureType::Elemental));
}
