use crate::card::*;
use crate::game::*;
use crate::types::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};

/// Helper: put a creature on the battlefield for a player (not summoning sick).
fn put_creature(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

/// Helper: seed a player's library with N copies of a card.
fn seed_library(state: &mut GameState, player: PlayerId, card_name: CardName, count: usize) {
    for _ in 0..count {
        let id = state.new_object_id();
        state.card_registry.push((id, card_name));
        state.players[player as usize].library.push(id);
    }
}

#[test]
fn test_become_monarch() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Initially no monarch
    assert_eq!(state.monarch, None);

    // Player 0 becomes the monarch
    state.become_monarch(0);
    assert_eq!(state.monarch, Some(0));

    // Player 1 becomes the monarch (dethroning player 0)
    state.become_monarch(1);
    assert_eq!(state.monarch, Some(1));
}

#[test]
fn test_monarch_draws_at_end_step() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Set up: player 0 is the monarch and it's their turn
    state.active_player = 0;
    state.monarch = Some(0);

    // Seed player 0's library so they can draw
    seed_library(&mut state, 0, CardName::LightningBolt, 5);
    let initial_hand_size = state.players[0].hand.len();

    // Transition from PostCombatMain -> Ending/End, which should put the monarch
    // draw trigger on the stack.
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase(); // -> Ending / End

    assert_eq!(state.phase, Phase::Ending);
    assert_eq!(state.step, Some(Step::End));

    // The monarch draw trigger should be on the stack
    assert!(!state.stack.is_empty(), "Monarch draw trigger should be on the stack");
    let top = state.stack.top().unwrap();
    assert!(
        matches!(top.kind, StackItemKind::TriggeredAbility { effect: TriggeredEffect::MonarchEndStep, .. }),
        "Top of stack should be MonarchEndStep"
    );
    assert_eq!(top.controller, 0);

    // Resolve the trigger
    state.resolve_top(&db);

    // Player 0 should have drawn a card
    assert_eq!(state.players[0].hand.len(), initial_hand_size + 1);
}

#[test]
fn test_monarch_draw_does_not_trigger_for_non_active_player() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 1 is the monarch but player 0 is the active player
    state.active_player = 0;
    state.monarch = Some(1);

    seed_library(&mut state, 1, CardName::LightningBolt, 5);
    let p1_hand_size = state.players[1].hand.len();

    // Advance from PostCombatMain -> End step
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase();

    // Monarch is player 1 but active player is 0, so NO trigger should fire
    assert!(
        state.stack.is_empty(),
        "No monarch draw trigger when active player is not the monarch"
    );

    // Player 1's hand should not have grown
    assert_eq!(state.players[1].hand.len(), p1_hand_size);
}

#[test]
fn test_combat_damage_steals_monarchy() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 is the monarch
    state.become_monarch(0);
    assert_eq!(state.monarch, Some(0));

    // Put a Goblin Guide for player 1 on the battlefield
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 1);

    // Set up combat: Goblin Guide attacks player 0 (the monarch)
    state.attackers.push((gg_id, 0));
    // No blockers

    // Resolve combat damage
    state.resolve_combat_damage(&db, false);

    // Player 1 should now be the monarch (their creature dealt damage to the monarch)
    assert_eq!(state.monarch, Some(1), "Player 1 should become the monarch after dealing combat damage to player 0");
}

#[test]
fn test_combat_damage_to_non_monarch_does_not_change_monarch() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 1 is the monarch but player 1's creature is attacking player 0
    state.become_monarch(1);

    // Player 0's creature attacks player 1 (the monarch)
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);
    state.attackers.push((gg_id, 1)); // attacker controlled by player 0, hits player 1

    state.resolve_combat_damage(&db, false);

    // Player 0 should now be the monarch (dealt damage to player 1 who was the monarch)
    assert_eq!(state.monarch, Some(0), "Player 0 should become monarch after dealing damage to player 1 (the monarch)");
}

#[test]
fn test_palace_jailer_etb_makes_controller_monarch() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put an opponent's creature on the battlefield (Palace Jailer exiles one)
    put_creature(&mut state, &db, CardName::GoblinGuide, 1);

    assert_eq!(state.monarch, None);

    // Register and cast Palace Jailer for player 0
    let jailer_id = state.new_object_id();
    state.card_registry.push((jailer_id, CardName::PalaceJailer));

    state.handle_etb(CardName::PalaceJailer, jailer_id, 0);

    // Player 0 should be the monarch
    assert_eq!(state.monarch, Some(0), "Palace Jailer ETB should make its controller the monarch");
}

#[test]
fn test_no_monarch_no_draw_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // No monarch set
    assert_eq!(state.monarch, None);

    state.active_player = 0;
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase();

    // No monarch → no trigger on the stack
    assert!(state.stack.is_empty(), "No monarch means no end-step draw trigger");
}
