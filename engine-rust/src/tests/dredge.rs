/// Tests for the Dredge replacement effect.
/// Dredge N: if you would draw a card, you may instead mill N cards and return this card
/// from your graveyard to your hand.
/// Cards: Golgari Grave-Troll (dredge 6), Stinkweed Imp (dredge 5), Life from the Loam (dredge 3).

use crate::action::*;
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

/// Register a card and put it in the graveyard of the given player.
fn put_in_graveyard(state: &mut GameState, name: CardName, owner: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    state.players[owner as usize].graveyard.push(id);
    id
}

/// Register a card and push it as the top of a player's library.
fn push_library_top(state: &mut GameState, name: CardName, owner: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    state.players[owner as usize].library.push(id);
    id
}

/// Fill a player's library with N basic lands and return their ids.
fn fill_library(state: &mut GameState, owner: PlayerId, count: usize) -> Vec<ObjectId> {
    let mut ids = Vec::new();
    for _ in 0..count {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Forest));
        state.players[owner as usize].library.push(id);
        ids.push(id);
    }
    ids
}

// ─── Dredge value helper ──────────────────────────────────────────────────────

#[test]
fn test_dredge_values() {
    assert_eq!(GameState::dredge_value(CardName::GolgariGraveTroll), Some(6));
    assert_eq!(GameState::dredge_value(CardName::StinkweedImp), Some(5));
    assert_eq!(GameState::dredge_value(CardName::LifeFromTheLoam), Some(3));
    assert_eq!(GameState::dredge_value(CardName::LightningBolt), None);
    assert_eq!(GameState::dredge_value(CardName::Island), None);
}

// ─── find_dredgeable ──────────────────────────────────────────────────────────

#[test]
fn test_find_dredgeable_with_sufficient_library() {
    let (mut state, db) = make_main_phase_state();
    let troll_id = put_in_graveyard(&mut state, CardName::GolgariGraveTroll, 0);
    fill_library(&mut state, 0, 6);

    let result = state.find_dredgeable(0);
    assert!(result.is_some(), "Should find a dredgeable card");
    let (card_id, n) = result.unwrap();
    assert_eq!(card_id, troll_id);
    assert_eq!(n, 6);
}

#[test]
fn test_find_dredgeable_insufficient_library() {
    let (mut state, db) = make_main_phase_state();
    put_in_graveyard(&mut state, CardName::GolgariGraveTroll, 0);
    fill_library(&mut state, 0, 5); // only 5 cards, need 6

    let result = state.find_dredgeable(0);
    assert!(result.is_none(), "Should not find dredgeable when library is too small");
}

#[test]
fn test_find_dredgeable_empty_graveyard() {
    let (mut state, db) = make_main_phase_state();
    fill_library(&mut state, 0, 10);

    let result = state.find_dredgeable(0);
    assert!(result.is_none(), "Should not find dredgeable with no dredge cards in graveyard");
}

// ─── draw_cards sets a pending dredge choice ──────────────────────────────────

#[test]
fn test_draw_cards_sets_dredge_choice_when_eligible() {
    let (mut state, db) = make_main_phase_state();
    put_in_graveyard(&mut state, CardName::StinkweedImp, 0);
    fill_library(&mut state, 0, 10);

    state.draw_cards(0, 1);

    assert!(
        state.pending_choice.is_some(),
        "draw_cards should set a pending dredge choice when a dredgeable card is in the graveyard"
    );
    if let Some(ref ch) = state.pending_choice {
        assert_eq!(ch.player, 0);
        if let ChoiceKind::ChooseNumber { min, max, reason } = &ch.kind {
            assert_eq!(*min, 0);
            assert_eq!(*max, 1);
            assert!(
                matches!(reason, ChoiceReason::DredgeChoice { dredge_n: 5, .. }),
                "Should be a DredgeChoice with dredge_n=5 for Stinkweed Imp"
            );
        } else {
            panic!("Expected ChooseNumber choice kind");
        }
    }
}

#[test]
fn test_draw_cards_no_dredge_choice_without_eligible_card() {
    let (mut state, db) = make_main_phase_state();
    fill_library(&mut state, 0, 5);
    push_library_top(&mut state, CardName::LightningBolt, 0);

    state.draw_cards(0, 1);

    // No dredge card in graveyard, so no pending choice, card is drawn normally.
    assert!(state.pending_choice.is_none(), "Should not set a dredge choice without dredge card in graveyard");
    assert_eq!(state.players[0].hand.len(), 1, "Should have drawn 1 card normally");
}

// ─── Dredge execution ────────────────────────────────────────────────────────

/// When choosing to dredge (n=1): mill N cards, return dredge card from graveyard to hand.
#[test]
fn test_dredge_mills_and_returns_card() {
    let (mut state, db) = make_main_phase_state();
    let troll_id = put_in_graveyard(&mut state, CardName::GolgariGraveTroll, 0);
    // Fill library with exactly 6 cards (the dredge amount).
    fill_library(&mut state, 0, 6);

    // Trigger dredge choice.
    state.draw_cards(0, 1);
    assert!(state.pending_choice.is_some(), "Should have a pending dredge choice");

    let choice = state.pending_choice.take().unwrap();
    // Choose to dredge (n=1).
    state.resolve_number_choice(choice, 1, &db);

    // The 6 library cards should now be in the graveyard (milled).
    assert_eq!(
        state.players[0].library.len(),
        0,
        "Library should be empty after dredge 6 milling all 6 cards"
    );
    // The graveyard should have the 6 milled cards but NOT the troll (it went to hand).
    assert!(
        !state.players[0].graveyard.contains(&troll_id),
        "Troll should not be in graveyard after dredging (should be in hand)"
    );
    // The troll should be in hand.
    assert!(
        state.players[0].hand.contains(&troll_id),
        "Troll should be in hand after dredging"
    );
    // Graveyard should have exactly 6 milled cards.
    assert_eq!(
        state.players[0].graveyard.len(),
        6,
        "Graveyard should have 6 milled cards"
    );
    // draws_this_turn should NOT increment (dredge replaces the draw).
    assert_eq!(
        state.players[0].draws_this_turn, 0,
        "draws_this_turn should not increment when dredging"
    );
}

/// When choosing NOT to dredge (n=0): draw normally.
#[test]
fn test_dredge_choice_decline_draws_normally() {
    let (mut state, db) = make_main_phase_state();
    put_in_graveyard(&mut state, CardName::LifeFromTheLoam, 0);
    // Fill library first, then push the Island on top so it's the top card.
    fill_library(&mut state, 0, 2); // extra cards
    let top_id = push_library_top(&mut state, CardName::Island, 0); // top of library

    // Trigger draw (which sets a pending dredge choice).
    state.draw_cards(0, 1);
    assert!(state.pending_choice.is_some(), "Should have pending dredge choice");

    let choice = state.pending_choice.take().unwrap();
    // Decline to dredge (n=0): draw normally.
    state.resolve_number_choice(choice, 0, &db);

    // top_id (Island) should be drawn.
    assert!(
        state.players[0].hand.contains(&top_id),
        "Top card should be drawn when declining to dredge"
    );
    // draws_this_turn should increment.
    assert_eq!(
        state.players[0].draws_this_turn, 1,
        "draws_this_turn should increment when drawing normally"
    );
    // Life from the Loam should still be in graveyard.
    assert!(
        state.players[0].graveyard.iter().any(|&id| {
            state.card_name_for_id(id) == Some(CardName::LifeFromTheLoam)
        }),
        "Life from the Loam should remain in graveyard when not dredging"
    );
}

/// Can't dredge when library has fewer cards than dredge N.
#[test]
fn test_cant_dredge_with_insufficient_library() {
    let (mut state, db) = make_main_phase_state();
    put_in_graveyard(&mut state, CardName::GolgariGraveTroll, 0); // dredge 6
    fill_library(&mut state, 0, 3); // only 3 cards, need 6
    push_library_top(&mut state, CardName::Forest, 0);

    // draw_cards should NOT set a pending choice because we can't dredge.
    state.draw_cards(0, 1);
    assert!(
        state.pending_choice.is_none(),
        "Should not set dredge choice when library is smaller than dredge value"
    );
    // Should have drawn normally.
    assert_eq!(state.players[0].hand.len(), 1, "Should have drawn normally");
}

/// Stinkweed Imp dredge 5: verify the correct number of cards are milled.
#[test]
fn test_stinkweed_imp_dredge_5() {
    let (mut state, db) = make_main_phase_state();
    let imp_id = put_in_graveyard(&mut state, CardName::StinkweedImp, 0);
    fill_library(&mut state, 0, 10);

    state.draw_cards(0, 1);
    let choice = state.pending_choice.take().unwrap();

    // Verify dredge_n is 5.
    if let ChoiceKind::ChooseNumber { reason, .. } = &choice.kind {
        assert!(
            matches!(reason, ChoiceReason::DredgeChoice { dredge_n: 5, .. }),
            "StinkweedImp should have dredge_n=5"
        );
    }

    state.resolve_number_choice(choice, 1, &db);

    // 5 cards milled, library has 5 remaining.
    assert_eq!(state.players[0].library.len(), 5, "Library should have 5 cards remaining after dredge 5");
    // Graveyard has 5 milled cards (imp is in hand).
    assert_eq!(state.players[0].graveyard.len(), 5, "Graveyard should have 5 milled cards");
    // Imp is in hand.
    assert!(state.players[0].hand.contains(&imp_id), "Imp should be in hand after dredging");
}

/// Life from the Loam dredge 3: verify the correct number of cards are milled.
#[test]
fn test_life_from_the_loam_dredge_3() {
    let (mut state, db) = make_main_phase_state();
    let loam_id = put_in_graveyard(&mut state, CardName::LifeFromTheLoam, 0);
    fill_library(&mut state, 0, 5);

    state.draw_cards(0, 1);
    let choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(choice, 1, &db);

    // 3 cards milled, library has 2 remaining.
    assert_eq!(state.players[0].library.len(), 2, "Library should have 2 cards remaining after dredge 3");
    assert_eq!(state.players[0].graveyard.len(), 3, "Graveyard should have 3 milled cards");
    assert!(state.players[0].hand.contains(&loam_id), "Loam should be in hand after dredging");
}

/// Drawing multiple cards: dredge intercepts the first draw and remaining_draws is set correctly.
#[test]
fn test_dredge_with_multiple_draws_then_normal() {
    let (mut state, db) = make_main_phase_state();
    let troll_id = put_in_graveyard(&mut state, CardName::GolgariGraveTroll, 0);
    fill_library(&mut state, 0, 8); // library has 8 cards
    // We need a known "second draw" card.
    let top_id = push_library_top(&mut state, CardName::Island, 0); // 9th card = top

    // draw_cards(0, 2): first draw triggers dredge choice, remaining_draws=1.
    state.draw_cards(0, 2);
    assert!(state.pending_choice.is_some(), "Should have a pending dredge choice");

    // Verify remaining_draws.
    if let Some(ref ch) = state.pending_choice {
        if let ChoiceKind::ChooseNumber { reason, .. } = &ch.kind {
            assert!(
                matches!(reason, ChoiceReason::DredgeChoice { remaining_draws: 1, .. }),
                "remaining_draws should be 1 for the second draw"
            );
        }
    }

    let choice = state.pending_choice.take().unwrap();
    // Choose to dredge (n=1): mills 6 cards from library (leaving 3: top + fill - 6 = 9-6=3).
    state.resolve_number_choice(choice, 1, &db);

    // After dredge, the remaining draw should fire. But with the troll now in graveyard gone,
    // we might get another dredge choice OR draw normally depending on what's in graveyard.
    // The 6 milled cards might include dredge cards — but we filled with Forest.
    // After dredging: library has 3 cards (top_id + 2 forests). Troll is in hand.
    // The remaining draw should draw from library (no dredge card eligible, troll is in hand now).
    // So pending_choice should be None and a card drawn.
    assert!(
        state.players[0].hand.contains(&troll_id),
        "Troll should be in hand after dredging"
    );
    // The second draw should have happened normally (troll is no longer in graveyard).
    assert_eq!(
        state.players[0].hand.len(),
        2,
        "Should have 2 cards in hand: troll (dredge) + 1 normal draw"
    );
}

/// Dredge with empty graveyard after dredge: second draw goes normally.
#[test]
fn test_dredge_then_normal_draw_no_second_dredge() {
    let (mut state, db) = make_main_phase_state();
    let loam_id = put_in_graveyard(&mut state, CardName::LifeFromTheLoam, 0);
    fill_library(&mut state, 0, 5);

    // Draw 2: first draw = dredge choice (loam dredge 3), second draw = normal.
    state.draw_cards(0, 2);
    let choice = state.pending_choice.take().unwrap();

    // Verify remaining_draws = 1.
    if let ChoiceKind::ChooseNumber { reason, .. } = &choice.kind {
        assert!(matches!(reason, ChoiceReason::DredgeChoice { remaining_draws: 1, .. }));
    }

    // Choose to dredge.
    state.resolve_number_choice(choice, 1, &db);

    // Loam is now in hand, 3 cards milled, library has 2.
    // No more dredge cards in graveyard (unless milled ones have dredge — but we filled with Forest).
    // The remaining draw should proceed normally.
    assert!(state.players[0].hand.contains(&loam_id), "Loam should be in hand");
    // 2 cards in hand: loam + 1 drawn normally.
    assert_eq!(state.players[0].hand.len(), 2, "Should have loam + 1 normally drawn card");
    assert_eq!(state.players[0].library.len(), 1, "Library should have 1 card left");
}
