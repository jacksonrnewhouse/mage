use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::types::*;

/// Helper: put a card on the battlefield for a player without going through ETB logic.
fn put_on_battlefield(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: u8) -> u32 {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let perm = crate::permanent::Permanent::new(
        id,
        card_name,
        controller,
        controller,
        def.power,
        def.toughness,
        None,
        def.keywords,
        def.card_types,
    );
    state.battlefield.push(perm);
    id
}

// --- Devotion tests ---

#[test]
fn test_devotion_to_blue_counts_symbols() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Counterspell costs {U}{U} — contributes 2 blue devotion
    // Force of Will costs {3}{U}{U} — contributes 2 blue devotion
    // We put them on battlefield as "enchantments" by hacking card types; easier to just use
    // actual creature/non-land cards that are in the pool.
    // Use Snapcaster Mage {1}{U} = 1 blue, Jace TMS {2}{U}{U} = 2 blue
    put_on_battlefield(&mut state, &db, CardName::SnapcasterMage, 0);    // {1}{U} = 1 blue
    put_on_battlefield(&mut state, &db, CardName::JaceTheMindSculptor, 0); // {2}{U}{U} = 2 blue

    let devotion = state.devotion_to(0, Color::Blue, &db);
    // 1 + 2 = 3
    assert_eq!(devotion, 3, "Expected 3 blue devotion, got {}", devotion);
}

#[test]
fn test_devotion_ignores_lands() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Islands are lands — they should NOT count toward devotion even though they produce blue
    put_on_battlefield(&mut state, &db, CardName::Island, 0);
    put_on_battlefield(&mut state, &db, CardName::Island, 0);

    let devotion = state.devotion_to(0, Color::Blue, &db);
    assert_eq!(devotion, 0, "Lands should not count toward devotion");
}

#[test]
fn test_devotion_ignores_opponent_permanents() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Player 1 controls a blue permanent
    put_on_battlefield(&mut state, &db, CardName::SnapcasterMage, 1); // {1}{U}

    // Player 0's devotion to blue should be 0
    let devotion = state.devotion_to(0, Color::Blue, &db);
    assert_eq!(devotion, 0, "Opponent's permanents should not count toward player's devotion");

    // Player 1's devotion to blue should be 1
    let devotion1 = state.devotion_to(1, Color::Blue, &db);
    assert_eq!(devotion1, 1, "Player 1 should have 1 blue devotion");
}

#[test]
fn test_devotion_to_zero_when_no_permanents() {
    let db = build_card_db();
    let state = GameState::new_two_player();

    for &color in &Color::ALL {
        let devotion = state.devotion_to(0, color, &db);
        assert_eq!(devotion, 0, "Empty battlefield should have 0 devotion to {:?}", color);
    }
}

#[test]
fn test_devotion_multicolor_counts_each_symbol() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Lightning Bolt {R} — 1 red symbol
    put_on_battlefield(&mut state, &db, CardName::GoblinGuide, 0); // {R} creature = 1 red
    put_on_battlefield(&mut state, &db, CardName::GoblinGuide, 0); // another {R} = 1 red

    let devotion_red = state.devotion_to(0, Color::Red, &db);
    assert_eq!(devotion_red, 2, "Two R permanents = 2 red devotion");

    let devotion_blue = state.devotion_to(0, Color::Blue, &db);
    assert_eq!(devotion_blue, 0, "No blue permanents = 0 blue devotion");
}

// --- Metalcraft tests ---

#[test]
fn test_metalcraft_false_with_fewer_than_three_artifacts() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    // 0 artifacts
    assert!(!state.metalcraft(0), "0 artifacts: metalcraft should be false");

    // 1 artifact
    put_on_battlefield(&mut state, &db, CardName::SolRing, 0);
    assert!(!state.metalcraft(0), "1 artifact: metalcraft should be false");

    // 2 artifacts
    put_on_battlefield(&mut state, &db, CardName::MoxPearl, 0);
    assert!(!state.metalcraft(0), "2 artifacts: metalcraft should be false");
}

#[test]
fn test_metalcraft_true_with_three_or_more_artifacts() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    put_on_battlefield(&mut state, &db, CardName::SolRing, 0);
    put_on_battlefield(&mut state, &db, CardName::MoxPearl, 0);
    put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);

    assert!(state.metalcraft(0), "3 artifacts: metalcraft should be true");

    // 4 artifacts
    put_on_battlefield(&mut state, &db, CardName::MoxSapphire, 0);
    assert!(state.metalcraft(0), "4 artifacts: metalcraft should still be true");
}

#[test]
fn test_metalcraft_only_counts_controller_artifacts() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Player 1 controls 3 artifacts
    put_on_battlefield(&mut state, &db, CardName::SolRing, 1);
    put_on_battlefield(&mut state, &db, CardName::MoxPearl, 1);
    put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 1);

    assert!(!state.metalcraft(0), "Player 0 should not have metalcraft from opponent's artifacts");
    assert!(state.metalcraft(1), "Player 1 should have metalcraft with 3 artifacts");
}

// --- Mox Opal tests ---

#[test]
fn test_mox_opal_no_mana_without_metalcraft() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Place Mox Opal alone (only 1 artifact, no metalcraft)
    let opal_id = put_on_battlefield(&mut state, &db, CardName::MoxOpal, 0);

    // Mox Opal should not produce mana: activate_mana_ability should return false
    let result = state.activate_mana_ability(opal_id, Some(Color::Blue));
    assert!(!result, "Mox Opal should not tap for mana without metalcraft");

    // Mana pool should be empty
    assert_eq!(state.players[0].mana_pool.total(), 0, "No mana should have been added");

    // Mox Opal should not be tapped
    let perm = state.find_permanent(opal_id).unwrap();
    assert!(!perm.tapped, "Mox Opal should not have tapped without metalcraft");
}

#[test]
fn test_mox_opal_produces_mana_with_metalcraft() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Place Mox Opal + 2 other artifacts (3 total = metalcraft active)
    let opal_id = put_on_battlefield(&mut state, &db, CardName::MoxOpal, 0);
    put_on_battlefield(&mut state, &db, CardName::SolRing, 0);
    put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);

    assert!(state.metalcraft(0), "Should have metalcraft with 3 artifacts");

    let result = state.activate_mana_ability(opal_id, Some(Color::Blue));
    assert!(result, "Mox Opal should produce mana with metalcraft active");

    assert_eq!(state.players[0].mana_pool.blue, 1, "Should have 1 blue mana");

    let perm = state.find_permanent(opal_id).unwrap();
    assert!(perm.tapped, "Mox Opal should be tapped after use");
}

#[test]
fn test_mox_opal_legal_actions_only_with_metalcraft() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let opal_id = put_on_battlefield(&mut state, &db, CardName::MoxOpal, 0);

    // Without metalcraft: no ActivateManaAbility for Mox Opal
    let actions = state.legal_actions(&db);
    let has_opal_tap = actions.iter().any(|a| {
        matches!(a, Action::ActivateManaAbility { permanent_id, .. } if *permanent_id == opal_id)
    });
    assert!(!has_opal_tap, "Mox Opal should not appear as legal mana action without metalcraft");

    // Add two more artifacts to enable metalcraft
    put_on_battlefield(&mut state, &db, CardName::SolRing, 0);
    put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);

    let actions = state.legal_actions(&db);
    let has_opal_tap = actions.iter().any(|a| {
        matches!(a, Action::ActivateManaAbility { permanent_id, .. } if *permanent_id == opal_id)
    });
    assert!(has_opal_tap, "Mox Opal should appear as legal mana action with metalcraft");
}
