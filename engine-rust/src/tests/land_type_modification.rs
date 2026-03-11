use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::types::*;

/// Helper: put a permanent on the battlefield for a given player.
fn put_permanent(state: &mut GameState, card_name: CardName, controller: u8, db: &[CardDef]) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

/// Helper: check what mana options a permanent has.
fn mana_options(state: &GameState, permanent_id: ObjectId) -> Vec<Option<Color>> {
    let perm = state.find_permanent(permanent_id).unwrap();
    state.mana_ability_options_pub(perm)
}

// ============================================================================
// Blood Moon tests
// ============================================================================

#[test]
fn test_blood_moon_dual_land_becomes_mountain() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a dual land (Underground Sea) on the battlefield for P0
    let dual_id = put_permanent(&mut state, CardName::UndergroundSea, 0, &db);

    // Without Blood Moon: Underground Sea taps for Blue or Black
    let options = mana_options(&state, dual_id);
    assert!(options.contains(&Some(Color::Blue)), "UndergroundSea should tap for Blue");
    assert!(options.contains(&Some(Color::Black)), "UndergroundSea should tap for Black");

    // Put Blood Moon on the battlefield
    put_permanent(&mut state, CardName::BloodMoon, 1, &db);

    // Under Blood Moon: Underground Sea becomes a Mountain, taps for Red only
    let options = mana_options(&state, dual_id);
    assert_eq!(options, vec![Some(Color::Red)], "Under Blood Moon, dual land should only tap for Red");
}

#[test]
fn test_blood_moon_shock_land_becomes_mountain() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let shock_id = put_permanent(&mut state, CardName::HallowedFountain, 0, &db);
    put_permanent(&mut state, CardName::BloodMoon, 1, &db);

    let options = mana_options(&state, shock_id);
    assert_eq!(options, vec![Some(Color::Red)], "Under Blood Moon, shock land should only tap for Red");
}

#[test]
fn test_blood_moon_does_not_affect_basic_lands() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let plains_id = put_permanent(&mut state, CardName::Plains, 0, &db);
    let island_id = put_permanent(&mut state, CardName::Island, 0, &db);
    let swamp_id = put_permanent(&mut state, CardName::Swamp, 0, &db);
    let forest_id = put_permanent(&mut state, CardName::Forest, 0, &db);
    put_permanent(&mut state, CardName::BloodMoon, 1, &db);

    // Basic lands are unaffected by Blood Moon
    assert_eq!(mana_options(&state, plains_id), vec![Some(Color::White)]);
    assert_eq!(mana_options(&state, island_id), vec![Some(Color::Blue)]);
    assert_eq!(mana_options(&state, swamp_id), vec![Some(Color::Black)]);
    assert_eq!(mana_options(&state, forest_id), vec![Some(Color::Green)]);
}

#[test]
fn test_blood_moon_nonbasic_taps_for_red_via_activate() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 has an Underground Sea, P1 has Blood Moon
    let dual_id = put_permanent(&mut state, CardName::UndergroundSea, 0, &db);
    put_permanent(&mut state, CardName::BloodMoon, 1, &db);

    // The mana actions available for dual_id should only include Red
    let actions = state.legal_actions(&db);
    let mana_actions: Vec<_> = actions.iter()
        .filter(|a| matches!(a, Action::ActivateManaAbility { permanent_id, .. } if *permanent_id == dual_id))
        .collect();

    // Should only have Red option
    assert_eq!(mana_actions.len(), 1, "Should only have 1 mana action (Red) under Blood Moon");
    assert!(
        matches!(mana_actions[0], Action::ActivateManaAbility { color_choice: Some(Color::Red), .. }),
        "Only mana option should be Red"
    );

    // Actually tap the land for mana
    state.activate_mana_ability(dual_id, Some(Color::Red));
    assert_eq!(state.players[0].mana_pool.red, 1, "Should have added 1 Red mana");
    assert_eq!(state.players[0].mana_pool.blue, 0, "Should not have Blue mana");
    assert_eq!(state.players[0].mana_pool.black, 0, "Should not have Black mana");
}

// ============================================================================
// Urborg tests
// ============================================================================

#[test]
fn test_urborg_lets_any_land_tap_for_black() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 has a basic Mountain
    let mountain_id = put_permanent(&mut state, CardName::Mountain, 0, &db);

    // Without Urborg: Mountain taps for Red only
    let options = mana_options(&state, mountain_id);
    assert_eq!(options, vec![Some(Color::Red)]);
    assert!(!options.contains(&Some(Color::Black)));

    // P1 puts Urborg on the battlefield
    put_permanent(&mut state, CardName::UrborgTombOfYawgmoth, 1, &db);

    // Under Urborg: Mountain also taps for Black
    let options = mana_options(&state, mountain_id);
    assert!(options.contains(&Some(Color::Red)), "Mountain should still tap for Red");
    assert!(options.contains(&Some(Color::Black)), "Mountain should also tap for Black under Urborg");
}

#[test]
fn test_urborg_lets_colorless_land_tap_for_black() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let strip_id = put_permanent(&mut state, CardName::StripMine, 0, &db);
    put_permanent(&mut state, CardName::UrborgTombOfYawgmoth, 1, &db);

    let options = mana_options(&state, strip_id);
    assert!(options.contains(&Some(Color::Black)), "StripMine should tap for Black under Urborg");
}

#[test]
fn test_urborg_itself_taps_for_black() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let urborg_id = put_permanent(&mut state, CardName::UrborgTombOfYawgmoth, 0, &db);

    let options = mana_options(&state, urborg_id);
    assert!(options.contains(&Some(Color::Black)), "Urborg itself taps for Black");
}

#[test]
fn test_urborg_tap_for_black_via_activate() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let mountain_id = put_permanent(&mut state, CardName::Mountain, 0, &db);
    put_permanent(&mut state, CardName::UrborgTombOfYawgmoth, 1, &db);

    // Tap the Mountain for Black (via Urborg)
    state.activate_mana_ability(mountain_id, Some(Color::Black));
    assert_eq!(state.players[0].mana_pool.black, 1, "Mountain should produce Black under Urborg");
    assert_eq!(state.players[0].mana_pool.red, 0, "Should not have Red when tapping for Black");
}

// ============================================================================
// Yavimaya tests
// ============================================================================

#[test]
fn test_yavimaya_lets_any_land_tap_for_green() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 has a basic Island
    let island_id = put_permanent(&mut state, CardName::Island, 0, &db);

    // Without Yavimaya: Island taps for Blue only
    let options = mana_options(&state, island_id);
    assert_eq!(options, vec![Some(Color::Blue)]);

    // P1 puts Yavimaya on the battlefield
    put_permanent(&mut state, CardName::YavimayaCradleOfGrowth, 1, &db);

    // Under Yavimaya: Island also taps for Green
    let options = mana_options(&state, island_id);
    assert!(options.contains(&Some(Color::Blue)), "Island should still tap for Blue");
    assert!(options.contains(&Some(Color::Green)), "Island should also tap for Green under Yavimaya");
}

#[test]
fn test_yavimaya_lets_colorless_land_tap_for_green() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let wasteland_id = put_permanent(&mut state, CardName::Wasteland, 0, &db);
    put_permanent(&mut state, CardName::YavimayaCradleOfGrowth, 1, &db);

    let options = mana_options(&state, wasteland_id);
    assert!(options.contains(&Some(Color::Green)), "Wasteland should tap for Green under Yavimaya");
}

#[test]
fn test_yavimaya_itself_taps_for_green() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let yavimaya_id = put_permanent(&mut state, CardName::YavimayaCradleOfGrowth, 0, &db);

    let options = mana_options(&state, yavimaya_id);
    assert!(options.contains(&Some(Color::Green)), "Yavimaya itself taps for Green");
}

#[test]
fn test_yavimaya_tap_for_green_via_activate() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let island_id = put_permanent(&mut state, CardName::Island, 0, &db);
    put_permanent(&mut state, CardName::YavimayaCradleOfGrowth, 1, &db);

    // Tap the Island for Green (via Yavimaya)
    state.activate_mana_ability(island_id, Some(Color::Green));
    assert_eq!(state.players[0].mana_pool.green, 1, "Island should produce Green under Yavimaya");
    assert_eq!(state.players[0].mana_pool.blue, 0, "Should not have Blue when tapping for Green");
}

// ============================================================================
// Interaction tests
// ============================================================================

#[test]
fn test_blood_moon_overrides_urborg() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Both Blood Moon and Urborg are on the battlefield
    // Nonbasic lands should only tap for R (Blood Moon takes priority)
    let dual_id = put_permanent(&mut state, CardName::UndergroundSea, 0, &db);
    put_permanent(&mut state, CardName::BloodMoon, 1, &db);
    put_permanent(&mut state, CardName::UrborgTombOfYawgmoth, 1, &db);

    // Under Blood Moon, nonbasic land should only tap for Red
    // (Blood Moon replaces the land with a Mountain, which is a basic,
    //  but our implementation applies Blood Moon first and returns early.
    //  In the real rules, the "Mountain" from Blood Moon would then get Urborg's B,
    //  but that's a complex rules interaction. Our simpler model: Blood Moon wins.)
    let options = mana_options(&state, dual_id);
    assert_eq!(options, vec![Some(Color::Red)], "Blood Moon should override Urborg for nonbasic lands");
}

#[test]
fn test_urborg_and_yavimaya_stack() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // A basic Mountain with both Urborg and Yavimaya
    let mountain_id = put_permanent(&mut state, CardName::Mountain, 0, &db);
    put_permanent(&mut state, CardName::UrborgTombOfYawgmoth, 1, &db);
    put_permanent(&mut state, CardName::YavimayaCradleOfGrowth, 1, &db);

    let options = mana_options(&state, mountain_id);
    assert!(options.contains(&Some(Color::Red)), "Mountain taps for Red");
    assert!(options.contains(&Some(Color::Black)), "Mountain taps for Black (Urborg)");
    assert!(options.contains(&Some(Color::Green)), "Mountain taps for Green (Yavimaya)");
}
