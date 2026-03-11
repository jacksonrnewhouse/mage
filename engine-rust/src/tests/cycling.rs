/// Tests for cycling and channel abilities (activated from hand).

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

/// Helper: build a minimal game state in the pre-combat main phase with player 0 having priority.
fn setup_main_phase() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    (state, db)
}

// ───────────── Basic cycling: discard + draw ─────────────

#[test]
fn test_lorien_revealed_cycling_generates_action() {
    let (mut state, db) = setup_main_phase();

    // Put Lorien Revealed in player 0's hand (cycling cost {1})
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::LorienRevealed));
    state.players[0].hand.push(card_id);

    // Give player 1 generic mana to pay the cycling cost
    state.players[0].mana_pool.colorless = 1;

    // Put a card in the library to draw
    let draw_id = state.new_object_id();
    state.card_registry.push((draw_id, CardName::Island));
    state.players[0].library.push(draw_id);

    let actions = state.legal_actions(&db);
    let cycling_action = actions.iter().find(|a| {
        matches!(a, Action::ActivateFromHand {
            card_id: id,
            ability_index: 0,
            x_value: 0,
            ..
        } if *id == card_id)
    });
    assert!(cycling_action.is_some(), "Should be able to cycle Lorien Revealed");
}

#[test]
fn test_lorien_revealed_cycling_no_mana_no_action() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::LorienRevealed));
    state.players[0].hand.push(card_id);

    // No mana in pool → cannot afford cycling cost {1}
    state.players[0].mana_pool = Default::default();

    let actions = state.legal_actions(&db);
    let cycling_action = actions.iter().find(|a| {
        matches!(a, Action::ActivateFromHand { card_id: id, ability_index: 0, .. } if *id == card_id)
    });
    assert!(cycling_action.is_none(), "Should NOT be able to cycle without mana");
}

#[test]
fn test_cycling_discards_card_and_draws() {
    let (mut state, db) = setup_main_phase();

    // Put Lorien Revealed in hand
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::LorienRevealed));
    state.players[0].hand.push(card_id);
    state.players[0].mana_pool.colorless = 1;

    // Put a card in library to draw
    let draw_id = state.new_object_id();
    state.card_registry.push((draw_id, CardName::Island));
    state.players[0].library.push(draw_id);

    let hand_before = state.players[0].hand.len();
    assert_eq!(hand_before, 1);

    // Apply cycling action
    let cycle_action = Action::ActivateFromHand {
        card_id,
        ability_index: 0,
        targets: vec![],
        x_value: 0,
    };
    state.apply_action(&cycle_action, &db);

    // After activation: Lorien Revealed is discarded, cycling effect is on the stack
    assert!(
        !state.players[0].hand.contains(&card_id),
        "Lorien Revealed should be removed from hand"
    );
    assert!(
        state.players[0].graveyard.contains(&card_id),
        "Lorien Revealed should be in graveyard"
    );
    assert!(!state.stack.is_empty(), "Cycling effect should be on the stack");

    // Resolve the cycling effect (draw a card)
    state.resolve_top(&db);

    // Player drew a card
    assert!(
        state.players[0].hand.contains(&draw_id),
        "Player should have drawn a card via cycling"
    );
    // Library is now empty
    assert!(state.players[0].library.is_empty());
}

#[test]
fn test_street_wraith_cycling_costs_life() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::StreetWraith));
    state.players[0].hand.push(card_id);
    // No mana needed, but player needs at least 2 life above zero
    state.players[0].life = 20;

    let draw_id = state.new_object_id();
    state.card_registry.push((draw_id, CardName::Island));
    state.players[0].library.push(draw_id);

    let actions = state.legal_actions(&db);
    let cycling_action = actions.iter().find(|a| {
        matches!(a, Action::ActivateFromHand { card_id: id, ability_index: 0, .. } if *id == card_id)
    });
    assert!(cycling_action.is_some(), "Street Wraith should have cycling action");

    // Apply it
    state.apply_action(cycling_action.unwrap(), &db);
    // Life should be reduced by 2
    assert_eq!(state.players[0].life, 18, "Cycling Street Wraith should cost 2 life");

    // Resolve draw
    state.resolve_top(&db);
    assert!(state.players[0].hand.contains(&draw_id), "Should have drawn a card");
}

// ───────────── Shark Typhoon cycling ─────────────

#[test]
fn test_shark_typhoon_cycling_generates_actions() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::SharkTyphoon));
    state.players[0].hand.push(card_id);

    // Give 3 mana: 1 blue (required) + 2 generic for X=2
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 2;

    let actions = state.legal_actions(&db);
    let cycling_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::ActivateFromHand { card_id: id, ability_index: 0, .. } if *id == card_id)
    }).collect();

    assert!(!cycling_actions.is_empty(), "Should have Shark Typhoon cycling actions");
    // Should have options for X=0, 1, 2
    let has_x2 = cycling_actions.iter().any(|a| matches!(a, Action::ActivateFromHand { x_value: 2, .. }));
    assert!(has_x2, "Should have X=2 cycling option");
}

#[test]
fn test_shark_typhoon_cycling_creates_shark_token() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::SharkTyphoon));
    state.players[0].hand.push(card_id);
    // Pay {2}{U} for X=2
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 2;

    let draw_id = state.new_object_id();
    state.card_registry.push((draw_id, CardName::Island));
    state.players[0].library.push(draw_id);

    let cycle_x2 = Action::ActivateFromHand {
        card_id,
        ability_index: 0,
        targets: vec![],
        x_value: 2,
    };
    state.apply_action(&cycle_x2, &db);

    // Shark Typhoon should be discarded
    assert!(state.players[0].graveyard.contains(&card_id));
    // Resolve the cycling effect
    state.resolve_top(&db);

    // Should have a 2/2 Shark token with flying on the battlefield
    let shark = state.battlefield.iter().find(|p| p.card_name == CardName::SharkToken);
    assert!(shark.is_some(), "Should have a Shark token");
    let shark = shark.unwrap();
    assert_eq!(shark.base_power, 2, "Shark should be 2/2 (X=2)");
    assert_eq!(shark.base_toughness, 2, "Shark should be 2/2 (X=2)");
    assert!(shark.keywords.has(Keyword::Flying), "Shark should have flying");

    // Player drew a card
    assert!(state.players[0].hand.contains(&draw_id));
}

// ───────────── Channel abilities ─────────────

#[test]
fn test_boseiju_channel_generates_action_against_nonbasic_land() {
    let (mut state, db) = setup_main_phase();

    // Boseiju in player 0's hand
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::BoseijuWhoEndures));
    state.players[0].hand.push(card_id);
    // Channel cost {1}{G}
    state.players[0].mana_pool.green = 1;
    state.players[0].mana_pool.colorless = 1;

    // Opponent has a nonbasic land (e.g., Wasteland)
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::Wasteland));
    let target_perm = Permanent::new(
        target_id,
        CardName::Wasteland,
        1, // controller = player 1
        1,
        None,
        None,
        None,
        Keywords::empty(),
        &[CardType::Land],
    );
    state.battlefield.push(target_perm);

    let actions = state.legal_actions(&db);
    let channel_action = actions.iter().find(|a| {
        matches!(a, Action::ActivateFromHand {
            card_id: id,
            ability_index: 1,
            targets,
            ..
        } if *id == card_id && targets.contains(&Target::Object(target_id)))
    });
    assert!(channel_action.is_some(), "Should be able to channel Boseiju targeting nonbasic land");
}

#[test]
fn test_boseiju_channel_no_action_against_basic_land() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::BoseijuWhoEndures));
    state.players[0].hand.push(card_id);
    state.players[0].mana_pool.green = 1;
    state.players[0].mana_pool.colorless = 1;

    // Opponent has only a basic land (Forest)
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::Forest));
    let target_perm = Permanent::new(
        target_id,
        CardName::Forest,
        1,
        1,
        None,
        None,
        None,
        Keywords::empty(),
        &[CardType::Land],
    );
    state.battlefield.push(target_perm);

    let actions = state.legal_actions(&db);
    let channel_action = actions.iter().find(|a| {
        matches!(a, Action::ActivateFromHand { card_id: id, ability_index: 1, .. } if *id == card_id)
    });
    assert!(channel_action.is_none(), "Boseiju channel should NOT target basic lands");
}

#[test]
fn test_boseiju_channel_destroys_target() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::BoseijuWhoEndures));
    state.players[0].hand.push(card_id);
    state.players[0].mana_pool.green = 1;
    state.players[0].mana_pool.colorless = 1;

    // Opponent's Sol Ring (artifact)
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::SolRing));
    let target_perm = Permanent::new(
        target_id,
        CardName::SolRing,
        1,
        1,
        None,
        None,
        None,
        Keywords::empty(),
        &[CardType::Artifact],
    );
    state.battlefield.push(target_perm);

    let channel_action = Action::ActivateFromHand {
        card_id,
        ability_index: 1,
        targets: vec![Target::Object(target_id)],
        x_value: 0,
    };
    state.apply_action(&channel_action, &db);

    // Boseiju should be discarded
    assert!(state.players[0].graveyard.contains(&card_id), "Boseiju should be discarded");
    // Channel effect on the stack
    assert!(!state.stack.is_empty(), "Channel effect should be on the stack");

    // Resolve
    state.resolve_top(&db);

    // Sol Ring should be gone
    let sol_still_on_battlefield = state.battlefield.iter().any(|p| p.id == target_id);
    assert!(!sol_still_on_battlefield, "Sol Ring should be destroyed by Boseiju channel");
}

#[test]
fn test_otawara_channel_bounces_creature() {
    let (mut state, db) = setup_main_phase();

    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::OtawaraSoaringCity));
    state.players[0].hand.push(card_id);
    // Otawara channel cost {3}{U}
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 3;

    // Opponent's creature
    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let creature_perm = Permanent::new(
        creature_id,
        CardName::GoblinGuide,
        1, // controller = player 1
        1,
        Some(2),
        Some(2),
        None,
        Keywords::empty(),
        &[CardType::Creature],
    );
    state.battlefield.push(creature_perm);

    // Apply the channel action directly
    let channel_action = Action::ActivateFromHand {
        card_id,
        ability_index: 1,
        targets: vec![Target::Object(creature_id)],
        x_value: 0,
    };
    state.apply_action(&channel_action, &db);

    // Otawara should be discarded
    assert!(state.players[0].graveyard.contains(&card_id));
    // Effect on stack
    assert!(!state.stack.is_empty());

    // Resolve
    state.resolve_top(&db);

    // Creature should be back in player 1's hand
    let still_on_battlefield = state.battlefield.iter().any(|p| p.id == creature_id);
    assert!(!still_on_battlefield, "Creature should be bounced off battlefield");
    assert!(
        state.players[1].hand.contains(&creature_id),
        "Creature should be returned to its owner's hand"
    );
}
