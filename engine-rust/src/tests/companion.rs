/// Tests for the companion mechanic (paying {3} to put companion from sideboard into hand).

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::types::*;

/// Build a minimal game with player 0 in the pre-combat main phase having priority.
fn setup_main_phase() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    (state, db)
}

// ───── Companion action generation ─────

#[test]
fn test_companion_action_generated_with_enough_mana() {
    let (mut state, db) = setup_main_phase();

    // Register Lurrus as player 0's companion (outside the game)
    let _lurrus_id = state.set_companion(0, CardName::LurrusOfTheDreamDen);

    // Give player 0 exactly 3 generic mana
    state.players[0].mana_pool.colorless = 3;

    let actions = state.legal_actions(&db);
    let has_companion_action = actions.iter().any(|a| matches!(a, Action::CompanionFromSideboard));
    assert!(has_companion_action, "CompanionFromSideboard should be generated when player has 3 mana");
}

#[test]
fn test_companion_action_not_generated_without_mana() {
    let (mut state, db) = setup_main_phase();

    let _lurrus_id = state.set_companion(0, CardName::LurrusOfTheDreamDen);

    // No mana in pool
    state.players[0].mana_pool.colorless = 0;

    let actions = state.legal_actions(&db);
    let has_companion_action = actions.iter().any(|a| matches!(a, Action::CompanionFromSideboard));
    assert!(!has_companion_action, "CompanionFromSideboard should NOT be generated without 3 mana");
}

#[test]
fn test_companion_action_not_generated_with_insufficient_mana() {
    let (mut state, db) = setup_main_phase();

    let _lurrus_id = state.set_companion(0, CardName::LurrusOfTheDreamDen);

    // Only 2 mana — not enough for the {3} cost
    state.players[0].mana_pool.colorless = 2;

    let actions = state.legal_actions(&db);
    let has_companion_action = actions.iter().any(|a| matches!(a, Action::CompanionFromSideboard));
    assert!(!has_companion_action, "CompanionFromSideboard should NOT be generated with only 2 mana");
}

#[test]
fn test_companion_action_not_generated_without_companion() {
    let (mut state, db) = setup_main_phase();

    // No companion set — player.companion is None
    assert!(state.players[0].companion.is_none());

    state.players[0].mana_pool.colorless = 5;

    let actions = state.legal_actions(&db);
    let has_companion_action = actions.iter().any(|a| matches!(a, Action::CompanionFromSideboard));
    assert!(!has_companion_action, "CompanionFromSideboard should NOT be generated when player has no companion");
}

// ───── Companion action resolution ─────

#[test]
fn test_paying_three_puts_companion_into_hand() {
    let (mut state, db) = setup_main_phase();

    let lurrus_id = state.set_companion(0, CardName::LurrusOfTheDreamDen);

    // Give player 0 enough mana
    state.players[0].mana_pool.colorless = 3;

    let hand_size_before = state.players[0].hand.len();

    state.apply_action(&Action::CompanionFromSideboard, &db);

    // companion field should be cleared
    assert!(
        state.players[0].companion.is_none(),
        "companion field should be None after moving companion to hand"
    );

    // Lurrus should now be in hand
    assert!(
        state.players[0].hand.contains(&lurrus_id),
        "Lurrus should be in hand after paying {{3}}"
    );
    assert_eq!(
        state.players[0].hand.len(),
        hand_size_before + 1,
        "Hand should grow by exactly 1"
    );

    // Mana should have been paid
    assert_eq!(
        state.players[0].mana_pool.colorless,
        0,
        "Mana pool should be reduced by {{3}}"
    );
}

#[test]
fn test_companion_cant_be_put_into_hand_twice() {
    let (mut state, db) = setup_main_phase();

    let lurrus_id = state.set_companion(0, CardName::LurrusOfTheDreamDen);

    // Give enough mana for two uses
    state.players[0].mana_pool.colorless = 6;

    // First use: should succeed
    state.apply_action(&Action::CompanionFromSideboard, &db);
    assert!(state.players[0].hand.contains(&lurrus_id));
    assert!(state.players[0].companion.is_none());

    let hand_size_after_first = state.players[0].hand.len();

    // Second attempt: action shouldn't exist (no companion) but if called directly it should be a no-op
    let actions = state.legal_actions(&db);
    assert!(
        !actions.iter().any(|a| matches!(a, Action::CompanionFromSideboard)),
        "Should no longer generate CompanionFromSideboard after companion has been moved to hand"
    );

    // Calling apply_action directly should be a no-op (companion is None)
    state.apply_action(&Action::CompanionFromSideboard, &db);
    assert_eq!(
        state.players[0].hand.len(),
        hand_size_after_first,
        "Hand size should not change on second attempt"
    );
}

#[test]
fn test_companion_can_be_cast_after_moving_to_hand() {
    let (mut state, db) = setup_main_phase();

    let lurrus_id = state.set_companion(0, CardName::LurrusOfTheDreamDen);

    // Give enough mana: 3 for companion ability + 3 for Lurrus itself (WB + 1 generic = WBG1, actually {1}{W}{B})
    // Lurrus costs {1}{W}{B} = 1 generic + 1 white + 1 black, but we only test it ends up in hand here
    state.players[0].mana_pool.colorless = 3;

    state.apply_action(&Action::CompanionFromSideboard, &db);

    // Lurrus should be in hand and castable if we have the mana
    assert!(state.players[0].hand.contains(&lurrus_id));
}

// ───── Companion is generated at instant speed ─────

#[test]
fn test_companion_action_generated_during_opponents_turn() {
    let (mut state, db) = setup_main_phase();

    // Player 1 has companion, it is player 0's turn
    let lurrus_id = state.set_companion(1, CardName::LurrusOfTheDreamDen);
    let _ = lurrus_id;

    state.active_player = 0;
    // Give priority to player 1 (e.g. during opponent's upkeep)
    state.priority_player = 1;
    state.players[1].mana_pool.colorless = 3;

    let actions = state.legal_actions(&db);
    let has_companion_action = actions.iter().any(|a| matches!(a, Action::CompanionFromSideboard));
    assert!(has_companion_action, "CompanionFromSideboard should be available at instant speed (opponent's turn)");
}
