/// Tests for the Madness mechanic.
///
/// Madness: when you discard a card with madness, instead of going to the graveyard
/// it goes to exile. A pending choice is created: cast it for the madness cost (0)
/// or put it in the graveyard (1).
///
/// Cards with madness: BaskingRootwalla (madness {0}), BlazingRootwalla (madness {0}).
/// Hollow One: costs {2} less for each card cycled or discarded this turn.

use crate::card::*;
use crate::game::*;
use crate::types::*;

/// Build a minimal main-phase game state with priority on player 0.
fn make_main_phase_state() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    (state, db)
}

/// Register a card and put it in the hand of the given player.
fn add_to_hand(state: &mut GameState, owner: PlayerId, name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    state.players[owner as usize].hand.push(id);
    id
}

// ─── Core madness replacement ─────────────────────────────────────────────────

/// Discarding a card with madness creates a pending choice and exiles the card.
#[test]
fn test_discard_madness_card_creates_choice_and_exiles() {
    let (mut state, db) = make_main_phase_state();
    let rootwalla_id = add_to_hand(&mut state, 0, CardName::BlazingRootwalla);

    // Remove from hand to simulate discard
    state.players[0].hand.retain(|&id| id != rootwalla_id);
    state.discard_card(rootwalla_id, 0, &db);

    // Card should be in exile (madness replacement)
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == rootwalla_id);
    assert!(in_exile, "Madness card should be exiled when discarded");

    // Card should NOT be in graveyard
    assert!(
        !state.players[0].graveyard.contains(&rootwalla_id),
        "Madness card should not go to graveyard on discard"
    );

    // madness_exiled tracking should record it
    assert!(
        state.madness_exiled.iter().any(|(id, _)| *id == rootwalla_id),
        "Card should be tracked in madness_exiled"
    );

    // A pending choice should be set (for the controller of the discarded card)
    assert!(state.pending_choice.is_some(), "Discarding a madness card should create a pending choice");

    if let Some(ref choice) = state.pending_choice {
        assert_eq!(choice.player, 0, "Choice should be for player 0 (the owner)");
        assert!(
            matches!(
                choice.kind,
                ChoiceKind::ChooseNumber {
                    min: 0,
                    max: 1,
                    reason: ChoiceReason::MadnessCast { .. }
                }
            ),
            "Choice kind should be MadnessCast"
        );
    }
}

/// Non-madness cards go to graveyard normally when discarded.
#[test]
fn test_discard_non_madness_card_goes_to_graveyard() {
    let (mut state, db) = make_main_phase_state();
    let bolt_id = add_to_hand(&mut state, 0, CardName::LightningBolt);

    state.players[0].hand.retain(|&id| id != bolt_id);
    state.discard_card(bolt_id, 0, &db);

    // Should be in graveyard, not exile
    assert!(
        state.players[0].graveyard.contains(&bolt_id),
        "Non-madness card should go to graveyard"
    );
    assert!(
        !state.exile.iter().any(|(id, _, _)| *id == bolt_id),
        "Non-madness card should not be exiled"
    );
    assert!(state.pending_choice.is_none(), "Non-madness discard should not create a choice");
}

// ─── Casting for madness cost ─────────────────────────────────────────────────

/// Choosing to cast for madness cost (n=0) pays the cost, puts the spell on the stack,
/// and removes the card from exile.
#[test]
fn test_cast_for_madness_cost_pushes_to_stack() {
    let (mut state, db) = make_main_phase_state();
    let rootwalla_id = add_to_hand(&mut state, 0, CardName::BlazingRootwalla);

    // Discard to trigger madness
    state.players[0].hand.retain(|&id| id != rootwalla_id);
    state.discard_card(rootwalla_id, 0, &db);

    // Madness cost for BlazingRootwalla is {0}: free
    // No mana needed.
    let choice = state.pending_choice.take().unwrap();
    // Choose to cast (n=0)
    state.resolve_number_choice(choice, 0, &db);

    // Spell should be on the stack
    assert_eq!(state.stack.len(), 1, "Spell should be on the stack after casting for madness");

    // Card should not be in exile anymore
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == rootwalla_id);
    assert!(!in_exile, "Card should be removed from exile after casting");

    // Card should not be in graveyard (it's on the stack)
    assert!(
        !state.players[0].graveyard.contains(&rootwalla_id),
        "Card should not be in graveyard while on the stack"
    );

    // madness_exiled should be cleared
    assert!(
        !state.madness_exiled.iter().any(|(id, _)| *id == rootwalla_id),
        "Card should be removed from madness_exiled after casting"
    );
}

/// After the madness spell resolves, it goes to exile (not graveyard).
/// (Cards cast via madness are exiled after they resolve, similar to flashback.)
#[test]
fn test_madness_spell_exiled_after_resolving() {
    let (mut state, db) = make_main_phase_state();
    let rootwalla_id = add_to_hand(&mut state, 0, CardName::BaskingRootwalla);

    // Discard to trigger madness
    state.players[0].hand.retain(|&id| id != rootwalla_id);
    state.discard_card(rootwalla_id, 0, &db);

    // Cast for madness cost {0} (free)
    let choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(choice, 0, &db);

    assert_eq!(state.stack.len(), 1, "Spell should be on stack");

    // Both players pass priority to resolve the spell
    state.pass_priority(&db);
    state.pass_priority(&db);

    // BaskingRootwalla is a creature — should enter the battlefield
    let on_bf = state.battlefield.iter().any(|p| p.card_name == CardName::BaskingRootwalla);
    assert!(on_bf, "BaskingRootwalla should enter the battlefield after resolving");

    // Should not be in graveyard
    assert!(
        !state.players[0].graveyard.contains(&rootwalla_id),
        "BaskingRootwalla should not be in graveyard"
    );
}

// ─── Declining madness ────────────────────────────────────────────────────────

/// Declining to cast for madness cost (n=1) moves the card from exile to graveyard.
#[test]
fn test_decline_madness_moves_to_graveyard() {
    let (mut state, db) = make_main_phase_state();
    let rootwalla_id = add_to_hand(&mut state, 0, CardName::BlazingRootwalla);

    // Discard to trigger madness
    state.players[0].hand.retain(|&id| id != rootwalla_id);
    state.discard_card(rootwalla_id, 0, &db);

    // Verify in exile
    assert!(state.exile.iter().any(|(id, _, _)| *id == rootwalla_id));

    // Decline (n=1)
    let choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(choice, 1, &db);

    // Card should now be in graveyard
    assert!(
        state.players[0].graveyard.contains(&rootwalla_id),
        "Declining madness should move card to graveyard"
    );

    // Card should not be in exile anymore
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == rootwalla_id);
    assert!(!in_exile, "Card should be removed from exile after declining");

    // madness_exiled cleared
    assert!(
        !state.madness_exiled.iter().any(|(id, _)| *id == rootwalla_id),
        "Card should be removed from madness_exiled after declining"
    );

    // No spell on stack
    assert_eq!(state.stack.len(), 0, "No spell should be on the stack after declining");
}

// ─── Hollow One cost reduction ────────────────────────────────────────────────

/// Hollow One costs {2} less for each card discarded this turn.
/// After 1 discard: 5 - 2 = 3 generic mana.
/// After 2 discards: 5 - 4 = 1 generic mana.
/// After 3 discards (or 2.5+): 5 - 6 = 0 (saturating).
#[test]
fn test_hollow_one_cost_reduction_one_discard() {
    let (mut state, db) = make_main_phase_state();

    // Discard one card to increment counter
    let bolt_id = add_to_hand(&mut state, 0, CardName::LightningBolt);
    state.players[0].hand.retain(|&id| id != bolt_id);
    state.discard_card(bolt_id, 0, &db);

    assert_eq!(state.players[0].cards_discarded_this_turn, 1);

    // Add HollowOne to hand
    let hollow_id = add_to_hand(&mut state, 0, CardName::HollowOne);

    // Check effective cost: 5 generic - (1 * 2) = 3
    let def = find_card(&db, CardName::HollowOne).unwrap();
    let cost = state.effective_cost(def, 0);
    assert_eq!(cost.generic, 3, "Hollow One should cost 3 after 1 discard");
    let _ = hollow_id;
}

#[test]
fn test_hollow_one_cost_reduction_two_discards() {
    let (mut state, db) = make_main_phase_state();

    // Discard two cards
    for name in [CardName::LightningBolt, CardName::GoblinGuide] {
        let id = add_to_hand(&mut state, 0, name);
        state.players[0].hand.retain(|&card_id| card_id != id);
        state.discard_card(id, 0, &db);
        // If the first card has madness, we need to clear the pending choice
        if state.pending_choice.is_some() {
            let choice = state.pending_choice.take().unwrap();
            state.resolve_number_choice(choice, 1, &db); // decline madness if any
        }
    }

    assert_eq!(state.players[0].cards_discarded_this_turn, 2);

    let def = find_card(&db, CardName::HollowOne).unwrap();
    let cost = state.effective_cost(def, 0);
    assert_eq!(cost.generic, 1, "Hollow One should cost 1 after 2 discards");
}

#[test]
fn test_hollow_one_free_after_three_discards() {
    let (mut state, db) = make_main_phase_state();

    // Discard three cards
    for name in [CardName::LightningBolt, CardName::GoblinGuide, CardName::GoblinGuide] {
        let id = add_to_hand(&mut state, 0, name);
        state.players[0].hand.retain(|&card_id| card_id != id);
        state.discard_card(id, 0, &db);
        if state.pending_choice.is_some() {
            let choice = state.pending_choice.take().unwrap();
            state.resolve_number_choice(choice, 1, &db);
        }
    }

    assert_eq!(state.players[0].cards_discarded_this_turn, 3);

    let def = find_card(&db, CardName::HollowOne).unwrap();
    let cost = state.effective_cost(def, 0);
    assert_eq!(cost.generic, 0, "Hollow One should cost 0 (free) after 3+ discards");
}

/// Cycling counts as a discard for Hollow One.
#[test]
fn test_hollow_one_reduced_by_cycling() {
    let (mut state, db) = make_main_phase_state();

    // Manually set discarded_this_turn (simulating cycling)
    state.players[0].cards_discarded_this_turn = 2;

    let def = find_card(&db, CardName::HollowOne).unwrap();
    let cost = state.effective_cost(def, 0);
    assert_eq!(cost.generic, 1, "Hollow One should cost 1 after 2 cycling/discard events");
}

/// Hollow One base cost is 5 when nothing has been discarded.
#[test]
fn test_hollow_one_full_cost_no_discards() {
    let (mut state, db) = make_main_phase_state();

    let def = find_card(&db, CardName::HollowOne).unwrap();
    let cost = state.effective_cost(def, 0);
    assert_eq!(cost.generic, 5, "Hollow One should cost 5 with no discards");
}

// ─── discard_card increments counter ─────────────────────────────────────────

/// discard_card increments cards_discarded_this_turn.
#[test]
fn test_discard_increments_discarded_this_turn() {
    let (mut state, db) = make_main_phase_state();

    assert_eq!(state.players[0].cards_discarded_this_turn, 0);

    let bolt_id = add_to_hand(&mut state, 0, CardName::LightningBolt);
    state.players[0].hand.retain(|&id| id != bolt_id);
    state.discard_card(bolt_id, 0, &db);

    assert_eq!(state.players[0].cards_discarded_this_turn, 1, "Counter should be 1 after one discard");

    let bolt2_id = add_to_hand(&mut state, 0, CardName::LightningBolt);
    state.players[0].hand.retain(|&id| id != bolt2_id);
    state.discard_card(bolt2_id, 0, &db);

    assert_eq!(state.players[0].cards_discarded_this_turn, 2, "Counter should be 2 after two discards");
}

/// cards_discarded_this_turn resets at start of turn.
#[test]
fn test_discarded_this_turn_resets_on_turn() {
    let (mut state, _db) = make_main_phase_state();

    state.players[0].cards_discarded_this_turn = 3;
    state.players[0].reset_for_turn();

    assert_eq!(
        state.players[0].cards_discarded_this_turn, 0,
        "cards_discarded_this_turn should reset at start of turn"
    );
}
