/// Tests for the Hideaway mechanic: Shelldock Isle and Mosswort Bridge.
/// Hideaway lands enter tapped, look at top N cards of library, exile one face-down,
/// and let the controller cast the hidden card for free when a condition is met.

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

fn setup_base() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].life = 20;
    state.players[1].life = 20;
    (state, db)
}

/// Load `count` copies of `card_name` into P0's library (bottom to top = first to last in vec).
fn load_library(state: &mut GameState, player: u8, cards: &[CardName]) -> Vec<ObjectId> {
    let mut ids = Vec::new();
    for &cn in cards {
        let id = state.new_object_id();
        state.card_registry.push((id, cn));
        state.players[player as usize].library.push(id);
        ids.push(id);
    }
    ids
}

/// Put a permanent on the battlefield.
fn add_permanent(state: &mut GameState, controller: u8, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let perm = Permanent::new(
        id, card_name, controller, controller,
        None, None, None, Keywords::empty(), &[CardType::Land],
    );
    state.battlefield.push(perm);
    id
}

fn add_creature(state: &mut GameState, controller: u8, card_name: CardName, power: i16, toughness: i16) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let perm = Permanent::new(
        id, card_name, controller, controller,
        Some(power), Some(toughness), None,
        Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);
    id
}

/// Playing Shelldock Isle should:
/// 1. Enter tapped
/// 2. Trigger HideawayETB (look at top 4, choose 1 to exile)
/// 3. Record the exile link in hideaway_exiled
#[test]
fn test_shelldock_isle_etb_exiles_card() {
    let (mut state, db) = setup_base();

    // Load library with some cards (we need at least 4)
    let lib_cards = load_library(&mut state, 0, &[
        CardName::Plains,
        CardName::Island,
        CardName::Swamp,
        CardName::Mountain, // this will be the topmost card (index 3 = last in vec = top)
    ]);
    let top_card_id = lib_cards[3]; // Mountain is the top card

    // Play Shelldock Isle from hand by adding it to hand and applying PlayLand
    let isle_id = state.new_object_id();
    state.card_registry.push((isle_id, CardName::ShelldockIsle));
    state.players[0].hand.push(isle_id);
    state.players[0].land_plays_remaining = 1;

    state.apply_action(&crate::action::Action::PlayLand(isle_id), &db);

    // Shelldock Isle should be on the battlefield (entered tapped)
    let perm = state.find_permanent(isle_id);
    assert!(perm.is_some(), "ShelldockIsle should be on the battlefield");
    assert!(
        perm.unwrap().tapped,
        "ShelldockIsle should enter tapped"
    );

    // HideawayETB trigger should be on the stack (look at top 4 cards)
    assert!(!state.stack.is_empty(), "HideawayETB trigger should be on the stack");

    // Resolve the trigger: both players pass priority
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes -> trigger resolves

    // After resolution there should be a pending choice (since we had 4 cards in library)
    // OR one card was auto-exiled if only 1 card was available.
    // We have 4 cards, so a pending choice should appear.
    assert!(
        state.pending_choice.is_some() || state.hideaway_exiled.iter().any(|(lid, _)| *lid == isle_id),
        "Either pending choice or card auto-exiled"
    );

    if let Some(ref _choice) = state.pending_choice {
        // Player chooses the top card (Mountain)
        state.apply_action(&crate::action::Action::ChooseCard(top_card_id), &db);
    }

    // The chosen card should be in exile
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == top_card_id),
        "Chosen card should be in exile after HideawayETB"
    );

    // hideaway_exiled should record (isle_id, chosen_card_id)
    assert!(
        state.hideaway_exiled.iter().any(|(lid, cid)| *lid == isle_id && *cid == top_card_id),
        "hideaway_exiled should record (ShelldockIsle id, exiled card id)"
    );

    // The other 3 cards should be at the bottom of the library (not in exile)
    assert_eq!(
        state.players[0].library.len(), 3,
        "The other 3 cards should remain in library"
    );
}

/// Shelldock Isle: the activated ability should be available when library has <= 20 cards.
#[test]
fn test_shelldock_isle_activation_condition_library_size() {
    let (mut state, db) = setup_base();

    // Place Shelldock Isle on battlefield (untapped, with a hidden card)
    let isle_id = state.new_object_id();
    state.card_registry.push((isle_id, CardName::ShelldockIsle));
    let perm = Permanent::new(
        isle_id, CardName::ShelldockIsle, 0, 0,
        None, None, None, Keywords::empty(), &[CardType::Land],
    );
    state.battlefield.push(perm);

    // Register a hidden card (e.g., AncestralRecall)
    let hidden_id = state.new_object_id();
    state.card_registry.push((hidden_id, CardName::AncestralRecall));
    state.exile.push((hidden_id, CardName::AncestralRecall, 0));
    state.hideaway_exiled.push((isle_id, hidden_id));

    // Give P0 some cards in library for drawing later
    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Plains));
        state.players[0].library.push(id);
    }

    // With 5 cards in library (<= 20), Shelldock Isle ability should be available.
    let actions = state.legal_actions(&db);
    let has_activation = actions.iter().any(|a| matches!(a,
        crate::action::Action::ActivateAbility { permanent_id, ability_index: 1, .. }
        if *permanent_id == isle_id
    ));
    assert!(
        has_activation,
        "Shelldock Isle activation should be available when library <= 20 cards"
    );

    // Fill library to 25 cards (> 20): ability should NOT be available.
    for _ in 0..20 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Plains));
        state.players[0].library.push(id);
    }
    // Now 25 cards in library
    assert_eq!(state.players[0].library.len(), 25);

    let actions2 = state.legal_actions(&db);
    let has_activation2 = actions2.iter().any(|a| matches!(a,
        crate::action::Action::ActivateAbility { permanent_id, ability_index: 1, .. }
        if *permanent_id == isle_id
    ));
    assert!(
        !has_activation2,
        "Shelldock Isle activation should NOT be available when library > 20 cards"
    );
}

/// Shelldock Isle: casting the hidden card for free should work.
/// We hide AncestralRecall (draw 3) and verify the activation draws 3 cards.
#[test]
fn test_shelldock_isle_casts_hidden_card_for_free() {
    let (mut state, db) = setup_base();

    // Place Shelldock Isle on battlefield (untapped)
    let isle_id = state.new_object_id();
    state.card_registry.push((isle_id, CardName::ShelldockIsle));
    let perm = Permanent::new(
        isle_id, CardName::ShelldockIsle, 0, 0,
        None, None, None, Keywords::empty(), &[CardType::Land],
    );
    state.battlefield.push(perm);

    // Register hidden card: AncestralRecall
    let hidden_id = state.new_object_id();
    state.card_registry.push((hidden_id, CardName::AncestralRecall));
    state.exile.push((hidden_id, CardName::AncestralRecall, 0));
    state.hideaway_exiled.push((isle_id, hidden_id));

    // Give P0 cards in library to draw (need <= 20 cards for condition)
    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Plains));
        state.players[0].library.push(id);
    }

    let hand_before = state.players[0].hand.len();

    // Activate Shelldock Isle's hideaway ability (index 1)
    state.apply_action(
        &crate::action::Action::ActivateAbility {
            permanent_id: isle_id,
            ability_index: 1,
            targets: vec![],
        },
        &db,
    );

    // The ability should be on the stack
    assert!(!state.stack.is_empty(), "HideawayActivated should be on stack");

    // Resolve (pass priority twice)
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes -> resolve

    // AncestralRecall resolves: controller draws 3 cards
    // Depending on whether it was pushed as a Spell (triggering another priority round) or resolved directly,
    // we may need to resolve it again
    if !state.stack.is_empty() {
        state.pass_priority(&db);
        state.pass_priority(&db);
    }

    let hand_after = state.players[0].hand.len();
    assert!(
        hand_after >= hand_before + 3,
        "Shelldock Isle should have cast AncestralRecall for free, drawing 3 cards (before: {}, after: {})",
        hand_before, hand_after
    );

    // The isle should be tapped after activation
    assert!(
        state.find_permanent(isle_id).map(|p| p.tapped).unwrap_or(false),
        "ShelldockIsle should be tapped after activation"
    );

    // The hidden card should no longer be in exile or hideaway_exiled
    assert!(
        !state.hideaway_exiled.iter().any(|(lid, _)| *lid == isle_id),
        "hideaway_exiled should be cleared after Shelldock Isle activates"
    );
}

/// Mosswort Bridge: activation should be available only when controlling a 10+ power creature.
#[test]
fn test_mosswort_bridge_activation_condition() {
    let (mut state, db) = setup_base();

    // Place Mosswort Bridge on battlefield (untapped)
    let bridge_id = state.new_object_id();
    state.card_registry.push((bridge_id, CardName::MosswortBridge));
    let perm = Permanent::new(
        bridge_id, CardName::MosswortBridge, 0, 0,
        None, None, None, Keywords::empty(), &[CardType::Land],
    );
    state.battlefield.push(perm);

    // Register a hidden card
    let hidden_id = state.new_object_id();
    state.card_registry.push((hidden_id, CardName::LightningBolt));
    state.exile.push((hidden_id, CardName::LightningBolt, 0));
    state.hideaway_exiled.push((bridge_id, hidden_id));

    // No big creatures yet: ability should NOT be available
    let actions = state.legal_actions(&db);
    let has_activation = actions.iter().any(|a| matches!(a,
        crate::action::Action::ActivateAbility { permanent_id, ability_index: 1, .. }
        if *permanent_id == bridge_id
    ));
    assert!(
        !has_activation,
        "Mosswort Bridge activation should NOT be available without a 10-power creature"
    );

    // Add a 10/10 creature
    let big_creature_id = add_creature(&mut state, 0, CardName::EmrakulTheAeonsTorn, 15, 15);
    let _ = big_creature_id;

    let actions2 = state.legal_actions(&db);
    let has_activation2 = actions2.iter().any(|a| matches!(a,
        crate::action::Action::ActivateAbility { permanent_id, ability_index: 1, .. }
        if *permanent_id == bridge_id
    ));
    assert!(
        has_activation2,
        "Mosswort Bridge activation should be available when controlling a 10-power creature"
    );
}

/// Mosswort Bridge: casting the hidden permanent for free puts it onto the battlefield.
#[test]
fn test_mosswort_bridge_casts_hidden_permanent() {
    let (mut state, db) = setup_base();

    // Place Mosswort Bridge on battlefield (untapped)
    let bridge_id = state.new_object_id();
    state.card_registry.push((bridge_id, CardName::MosswortBridge));
    let perm = Permanent::new(
        bridge_id, CardName::MosswortBridge, 0, 0,
        None, None, None, Keywords::empty(), &[CardType::Land],
    );
    state.battlefield.push(perm);

    // Register hidden card: GoblinGuide (creature)
    let hidden_id = state.new_object_id();
    state.card_registry.push((hidden_id, CardName::GoblinGuide));
    state.exile.push((hidden_id, CardName::GoblinGuide, 0));
    state.hideaway_exiled.push((bridge_id, hidden_id));

    // Add a 10/10 creature to satisfy the condition
    add_creature(&mut state, 0, CardName::EmrakulTheAeonsTorn, 15, 15);

    let bf_before = state.battlefield.len();

    // Activate Mosswort Bridge (ability_index 1)
    state.apply_action(
        &crate::action::Action::ActivateAbility {
            permanent_id: bridge_id,
            ability_index: 1,
            targets: vec![],
        },
        &db,
    );

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Resolve any remaining stack items (e.g. ETB triggers from GoblinGuide)
    while !state.stack.is_empty() {
        state.pass_priority(&db);
        state.pass_priority(&db);
    }

    // GoblinGuide should now be on the battlefield
    assert!(
        state.battlefield.iter().any(|p| p.id == hidden_id),
        "Hidden creature (GoblinGuide) should be on the battlefield after Mosswort Bridge activation"
    );
    assert!(
        state.battlefield.len() > bf_before,
        "Battlefield should have grown"
    );

    // The bridge should be tapped
    assert!(
        state.find_permanent(bridge_id).map(|p| p.tapped).unwrap_or(false),
        "Mosswort Bridge should be tapped after activation"
    );

    // hideaway_exiled should be cleared
    assert!(
        !state.hideaway_exiled.iter().any(|(lid, _)| *lid == bridge_id),
        "hideaway_exiled should be cleared after Mosswort Bridge activates"
    );
}
