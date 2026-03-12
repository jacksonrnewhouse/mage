use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

/// Helper: place a permanent onto the battlefield.
fn place_permanent(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);
    id
}

/// Helper: add a card to a player's library (top).
fn add_to_library_top(state: &mut GameState, player: PlayerId, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].library.push(id);
    id
}

#[test]
fn test_nadu_trigger_spell_targeting_creature() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Nadu on the battlefield for player 0
    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);

    // Place a creature for player 0 that will be targeted
    let creature_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 0);

    // Put a nonland card on top of library
    let top_card_id = add_to_library_top(&mut state, 0, CardName::LightningBolt);

    // Simulate targeting: call check_nadu_targeting_triggers with the creature
    state.check_nadu_targeting_triggers(&[creature_id]);

    // Should have a NaduTrigger on the stack
    assert_eq!(state.stack.len(), 1, "Nadu trigger should be on the stack");
    let top = state.stack.top().unwrap();
    assert!(
        matches!(&top.kind, StackItemKind::TriggeredAbility { effect: TriggeredEffect::NaduTrigger, .. }),
        "Stack item should be NaduTrigger"
    );

    // Resolve the trigger
    state.resolve_top(&db);

    // The top card was a nonland (Lightning Bolt), so it should go to hand
    assert!(
        state.players[0].hand.contains(&top_card_id),
        "Nonland card should be put into hand"
    );
}

#[test]
fn test_nadu_trigger_land_goes_to_battlefield() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);
    let creature_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 0);

    // Put a land on top of library
    let land_id = add_to_library_top(&mut state, 0, CardName::Forest);

    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 1);

    // Resolve
    state.resolve_top(&db);

    // The land should be on the battlefield (tapped)
    let on_bf = state.battlefield.iter().find(|p| p.id == land_id);
    assert!(on_bf.is_some(), "Land should be put onto the battlefield");
    assert!(on_bf.unwrap().tapped, "Land from Nadu should enter tapped");
}

#[test]
fn test_nadu_trigger_twice_per_creature_limit() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);
    let creature_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 0);

    // Add cards to library
    add_to_library_top(&mut state, 0, CardName::Mountain);
    add_to_library_top(&mut state, 0, CardName::Island);
    add_to_library_top(&mut state, 0, CardName::Forest);

    // First targeting: should trigger
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 1, "First targeting should trigger Nadu");

    // Second targeting: should trigger
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 2, "Second targeting should trigger Nadu");

    // Third targeting: should NOT trigger (twice per turn limit)
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 2, "Third targeting should NOT trigger Nadu (limit reached)");
}

#[test]
fn test_nadu_trigger_different_creatures_separate_limits() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);
    let creature1_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let creature2_id = place_permanent(&mut state, &db, CardName::MonasteryMentor, 0);

    // Add cards to library
    for _ in 0..6 {
        add_to_library_top(&mut state, 0, CardName::Mountain);
    }

    // Exhaust creature1's triggers
    state.check_nadu_targeting_triggers(&[creature1_id]);
    state.check_nadu_targeting_triggers(&[creature1_id]);
    assert_eq!(state.stack.len(), 2);

    // creature2 should still trigger
    state.check_nadu_targeting_triggers(&[creature2_id]);
    assert_eq!(state.stack.len(), 3, "Different creature should have separate trigger limit");

    // creature1 should NOT trigger again
    state.check_nadu_targeting_triggers(&[creature1_id]);
    assert_eq!(state.stack.len(), 3, "creature1 should be at limit");
}

#[test]
fn test_nadu_does_not_trigger_for_opponent_creatures() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Nadu controlled by player 0
    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);

    // Creature controlled by player 1 (opponent)
    let opp_creature_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 1);

    add_to_library_top(&mut state, 0, CardName::Mountain);

    // Targeting opponent's creature should NOT trigger Nadu
    state.check_nadu_targeting_triggers(&[opp_creature_id]);
    assert_eq!(state.stack.len(), 0, "Nadu should not trigger for opponent's creatures");
}

#[test]
fn test_nadu_trigger_empty_library() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);
    let creature_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 0);

    // Library is empty
    state.players[0].library.clear();

    // Should still trigger (goes on stack)
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 1, "Nadu should trigger even with empty library");

    // Resolving with empty library should be a no-op (no crash)
    state.resolve_top(&db);
    assert_eq!(state.stack.len(), 0);
}

#[test]
fn test_nadu_trigger_resets_on_new_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _nadu_id = place_permanent(&mut state, &db, CardName::NaduWingedWisdom, 0);
    let creature_id = place_permanent(&mut state, &db, CardName::GoblinGuide, 0);

    for _ in 0..4 {
        add_to_library_top(&mut state, 0, CardName::Mountain);
    }

    // Use up both triggers
    state.check_nadu_targeting_triggers(&[creature_id]);
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 2);

    // Third should not trigger
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 2);

    // Clear the tracking (simulating new turn)
    state.nadu_triggers_this_turn.clear();

    // Now it should trigger again
    state.check_nadu_targeting_triggers(&[creature_id]);
    assert_eq!(state.stack.len(), 3, "After turn reset, Nadu should trigger again");
}
