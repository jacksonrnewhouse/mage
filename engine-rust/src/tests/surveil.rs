/// Tests for the Surveil mechanic and cards that use it (Consider, surveil lands).

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
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

/// Register a card and push it as the top of a player's library.
fn push_library_top(state: &mut GameState, name: CardName, owner: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    state.players[owner as usize].library.push(id);
    id
}

/// Place a permanent on the battlefield controlled by the given player.
fn put_permanent(state: &mut GameState, db: &[CardDef], name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    let def = find_card(db, name).expect("card not in db");
    let perm = Permanent::new(
        id, name, controller, controller,
        def.power, def.toughness, def.loyalty,
        def.keywords, def.card_types,
    );
    state.battlefield.push(perm);
    id
}

// ─── surveil() primitive ──────────────────────────────────────────────────────

/// surveil(1) with keep-on-top choice (n=0) leaves the card on top of library.
#[test]
fn test_surveil_keep_on_top() {
    let (mut state, _db) = make_main_phase_state();
    let card_id = push_library_top(&mut state, CardName::LightningBolt, 0);

    state.surveil(0, 1, false);

    // Should have a pending SurveilCard choice.
    assert!(state.pending_choice.is_some(), "surveil should set a pending choice");

    let choice = state.pending_choice.take().unwrap();
    // Apply choice: keep on top (n = 0)
    state.resolve_number_choice(choice, 0);

    // Card should still be on top of library.
    assert_eq!(
        state.players[0].library.last().copied(),
        Some(card_id),
        "Card should remain on top of library when choice is 0 (keep)"
    );
    // Graveyard should be empty.
    assert!(state.players[0].graveyard.is_empty(), "Graveyard should be empty");
}

/// surveil(1) with send-to-graveyard choice (n=1) puts the card in graveyard.
#[test]
fn test_surveil_send_to_graveyard() {
    let (mut state, _db) = make_main_phase_state();
    let card_id = push_library_top(&mut state, CardName::LightningBolt, 0);

    state.surveil(0, 1, false);

    let choice = state.pending_choice.take().unwrap();
    // Apply choice: put in graveyard (n = 1)
    state.resolve_number_choice(choice, 1);

    // Library should be empty.
    assert!(state.players[0].library.is_empty(), "Library should be empty after sending to graveyard");
    // Card should be in graveyard.
    assert!(
        state.players[0].graveyard.contains(&card_id),
        "Card should be in graveyard when choice is 1"
    );
}

/// surveil on an empty library sets no pending choice and is a no-op.
#[test]
fn test_surveil_empty_library() {
    let (mut state, _db) = make_main_phase_state();
    // No cards in library.

    state.surveil(0, 1, false);
    assert!(state.pending_choice.is_none(), "surveil on empty library should not set a pending choice");
}

/// surveil with count=0 is a no-op.
#[test]
fn test_surveil_zero_count() {
    let (mut state, _db) = make_main_phase_state();
    push_library_top(&mut state, CardName::LightningBolt, 0);

    state.surveil(0, 0, false);
    assert!(state.pending_choice.is_none(), "surveil(0) should not set a pending choice");
}

// ─── Consider ────────────────────────────────────────────────────────────────

/// Consider: resolving with keep-on-top still draws the kept card.
#[test]
fn test_consider_keep_on_top_then_draw() {
    let (mut state, db) = make_main_phase_state();
    let top_id = push_library_top(&mut state, CardName::Island, 0);

    // Cast Consider (need {U} mana)
    let consider_id = state.new_object_id();
    state.card_registry.push((consider_id, CardName::Consider));
    state.players[0].hand.push(consider_id);
    state.players[0].mana_pool.blue = 1;

    state.apply_action(
        &Action::CastSpell {
            card_id: consider_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Stack should have Consider on it; pass priority to resolve.
    assert_eq!(state.stack.len(), 1);
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes — resolves

    // After resolving Consider, there should be a SurveilCard pending choice.
    assert!(state.pending_choice.is_some(), "Consider should create a surveil pending choice");

    let choice = state.pending_choice.take().unwrap();
    let hand_before = state.players[0].hand.len();
    // Choose to keep on top (n = 0)
    state.resolve_number_choice(choice, 0);

    // After surveil resolves (keep on top), the card is still on library and then drawn.
    // draw_after=true means the draw fires after surveil resolution.
    assert_eq!(
        state.players[0].hand.len(),
        hand_before + 1,
        "Consider should draw 1 card after surveil"
    );
    assert!(
        state.players[0].hand.contains(&top_id),
        "The kept card should have been drawn"
    );
}

/// Consider: send-to-graveyard puts the top card in the graveyard and then draws a different card.
#[test]
fn test_consider_send_to_graveyard_then_draw_next() {
    let (mut state, db) = make_main_phase_state();
    // Put two cards in library: bottom = Island, top = Swamp.
    let island_id = state.new_object_id();
    state.card_registry.push((island_id, CardName::Island));
    state.players[0].library.push(island_id);

    let swamp_id = state.new_object_id();
    state.card_registry.push((swamp_id, CardName::Swamp));
    state.players[0].library.push(swamp_id); // top of library

    let consider_id = state.new_object_id();
    state.card_registry.push((consider_id, CardName::Consider));
    state.players[0].hand.push(consider_id);
    state.players[0].mana_pool.blue = 1;

    state.apply_action(
        &Action::CastSpell {
            card_id: consider_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    state.pass_priority(&db);
    state.pass_priority(&db); // resolves Consider

    // Choose to send top card (Swamp) to graveyard (n = 1)
    let choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(choice, 1);

    // Swamp should be in graveyard.
    assert!(
        state.players[0].graveyard.contains(&swamp_id),
        "Swamp should be in graveyard after surveil choice to discard"
    );

    // Island should have been drawn (it is now the top card, drawn after surveil).
    assert!(
        state.players[0].hand.contains(&island_id),
        "Island should be drawn after surveil sends top card to graveyard"
    );
}

// ─── Surveil lands ────────────────────────────────────────────────────────────

/// Playing a surveil land creates a SurveilLandShock pending choice first.
#[test]
fn test_surveil_land_etb_creates_shock_choice() {
    let (mut state, db) = make_main_phase_state();

    let land_id = state.new_object_id();
    state.card_registry.push((land_id, CardName::UndercitySewers));
    state.players[0].hand.push(land_id);

    state.apply_action(&Action::PlayLand(land_id), &db);

    // Should have a SurveilLandShock pending choice (tapped vs pay life).
    assert!(state.pending_choice.is_some(), "Surveil land should create pending choice on ETB");

    if let Some(ref ch) = state.pending_choice {
        if let ChoiceKind::ChooseNumber { reason, .. } = &ch.kind {
            assert!(
                matches!(reason, ChoiceReason::SurveilLandShock { .. }),
                "Choice reason should be SurveilLandShock"
            );
        } else {
            panic!("Expected ChooseNumber choice");
        }
    }
}

/// Surveil land: choose enter tapped (n=0), then surveil choice follows.
#[test]
fn test_surveil_land_enter_tapped_then_surveil() {
    let (mut state, db) = make_main_phase_state();
    let top_id = push_library_top(&mut state, CardName::Island, 0);

    let land_id = state.new_object_id();
    state.card_registry.push((land_id, CardName::MeticulousArchive));
    state.players[0].hand.push(land_id);
    state.apply_action(&Action::PlayLand(land_id), &db);

    // Resolve shock choice: enter tapped (n=0)
    let shock_choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(shock_choice, 0);

    // Land should be tapped.
    let land_perm = state.battlefield.iter().find(|p| p.id == land_id).expect("land on battlefield");
    assert!(land_perm.tapped, "Surveil land should enter tapped when choosing 0");

    // Now there should be a surveil pending choice.
    assert!(
        state.pending_choice.is_some(),
        "After shock choice, surveil choice should follow"
    );

    // Resolve surveil: keep on top (n=0)
    let surveil_choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(surveil_choice, 0);

    // Card still on top of library.
    assert_eq!(
        state.players[0].library.last().copied(),
        Some(top_id),
        "Card should remain on top of library"
    );
}

/// Surveil land: choose pay 2 life (n=1), land enters untapped, then surveil.
#[test]
fn test_surveil_land_pay_life_untapped_then_surveil() {
    let (mut state, db) = make_main_phase_state();
    state.players[0].life = 20;
    let top_id = push_library_top(&mut state, CardName::Swamp, 0);

    let land_id = state.new_object_id();
    state.card_registry.push((land_id, CardName::UndercitySewers));
    state.players[0].hand.push(land_id);
    state.apply_action(&Action::PlayLand(land_id), &db);

    // Resolve shock choice: pay 2 life (n=1)
    let shock_choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(shock_choice, 1);

    // Life should be reduced by 2.
    assert_eq!(state.players[0].life, 18, "Player should have paid 2 life");

    // Land should NOT be tapped.
    let land_perm = state.battlefield.iter().find(|p| p.id == land_id).expect("land on battlefield");
    assert!(!land_perm.tapped, "Surveil land should enter untapped when paying 2 life");

    // Surveil choice follows.
    assert!(state.pending_choice.is_some(), "Surveil choice should follow after life payment");

    // Resolve surveil: send to graveyard (n=1)
    let surveil_choice = state.pending_choice.take().unwrap();
    state.resolve_number_choice(surveil_choice, 1);

    assert!(
        state.players[0].graveyard.contains(&top_id),
        "Top card should be in graveyard after surveil"
    );
}
