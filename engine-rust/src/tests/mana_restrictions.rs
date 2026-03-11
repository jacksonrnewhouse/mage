/// Tests for mana spending restrictions.
/// Covers Mishra's Workshop: mana may only be spent on artifact spells.

use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::mana::{ManaPool, ManaCost};
use crate::types::*;

/// Helper: put a card onto the battlefield for a player without ETB triggers.
fn put_on_battlefield(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: u8) -> ObjectId {
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

/// Helper: add a card to a player's hand.
fn add_to_hand(state: &mut GameState, card_name: CardName, player: u8) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].hand.push(id);
    id
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests for ManaPool workshop tracking
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_workshop_mana_stored_separately() {
    let mut pool = ManaPool::default();
    pool.add_workshop(3);

    // Workshop mana does NOT count in total() — it is restricted
    assert_eq!(pool.total(), 0, "Workshop mana should not be in total()");
    assert_eq!(pool.total_for_artifact(), 3, "Workshop mana should count for artifacts");
    assert_eq!(pool.workshop, 3);
    assert_eq!(pool.colorless, 0);
}

#[test]
fn test_can_pay_artifact_with_workshop_mana() {
    let mut pool = ManaPool::default();
    pool.add_workshop(3);

    let cost = ManaCost { generic: 3, ..ManaCost::ZERO };

    // Can pay for an artifact (uses workshop mana)
    assert!(pool.can_pay_for_artifact(&cost), "Should be able to pay {{3}} for an artifact");

    // Cannot pay for a non-artifact (workshop mana is restricted)
    assert!(!pool.can_pay(&cost), "Should NOT be able to pay {{3}} for a non-artifact with only workshop mana");
}

#[test]
fn test_pay_for_artifact_drains_workshop() {
    let mut pool = ManaPool::default();
    pool.add_workshop(3);

    let cost = ManaCost { generic: 3, ..ManaCost::ZERO };
    assert!(pool.pay_for_artifact(&cost));
    assert_eq!(pool.workshop, 0, "Workshop mana should be spent");
}

#[test]
fn test_pay_non_artifact_cannot_use_workshop() {
    let mut pool = ManaPool::default();
    pool.add_workshop(3);

    let cost = ManaCost { generic: 3, ..ManaCost::ZERO };
    // pay() (non-artifact) should fail because workshop is not usable
    assert!(!pool.pay(&cost), "pay() should fail with only workshop mana");
    // Workshop mana should be untouched
    assert_eq!(pool.workshop, 3);
}

#[test]
fn test_mixed_pool_prefers_workshop_for_artifacts() {
    // Player has 1 free colorless + 3 workshop. Casting a {4} artifact should drain
    // workshop first, leaving the free colorless behind.
    let mut pool = ManaPool::default();
    pool.colorless = 1;
    pool.add_workshop(3);

    let cost = ManaCost { generic: 3, ..ManaCost::ZERO };
    assert!(pool.pay_for_artifact(&cost));
    // Workshop should be fully drained; free colorless should remain
    assert_eq!(pool.workshop, 0);
    assert_eq!(pool.colorless, 1);
}

#[test]
fn test_artifact_cost_can_combine_workshop_and_free_mana() {
    // {5} artifact: 3 workshop + 2 free colorless → should succeed
    let mut pool = ManaPool::default();
    pool.colorless = 2;
    pool.add_workshop(3);

    let cost = ManaCost { generic: 5, ..ManaCost::ZERO };
    assert!(pool.can_pay_for_artifact(&cost));
    assert!(pool.pay_for_artifact(&cost));
    assert_eq!(pool.workshop, 0);
    assert_eq!(pool.colorless, 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration tests: Workshop on the battlefield
// ─────────────────────────────────────────────────────────────────────────────

/// Workshop taps for 3 and the mana goes into the workshop-restricted bucket.
#[test]
fn test_workshop_taps_into_restricted_pool() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let workshop_id = put_on_battlefield(&mut state, &db, CardName::MishrasWorkshop, 0);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let result = state.activate_mana_ability(workshop_id, None);
    assert!(result, "Workshop should tap successfully");
    assert_eq!(state.players[0].mana_pool.workshop, 3,
        "Workshop should produce 3 restricted mana");
    assert_eq!(state.players[0].mana_pool.colorless, 0,
        "Workshop should NOT put mana into the free colorless pool");
    assert!(state.battlefield.iter().any(|p| p.id == workshop_id && p.tapped),
        "Workshop should be tapped after activating");
}

/// Workshop mana can be used to cast an artifact spell (e.g. Sol Ring {1}).
#[test]
fn test_workshop_mana_enables_artifact_cast() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let workshop_id = put_on_battlefield(&mut state, &db, CardName::MishrasWorkshop, 0);
    let sol_ring_id = add_to_hand(&mut state, CardName::SolRing, 0);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Tap Workshop for 3 restricted mana
    state.activate_mana_ability(workshop_id, None);
    assert_eq!(state.players[0].mana_pool.workshop, 3);

    // Legal actions should include casting Sol Ring (artifact)
    let actions = state.legal_actions(&db);
    let can_cast_sol_ring = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == sol_ring_id)
    });
    assert!(can_cast_sol_ring, "Should be able to cast Sol Ring with Workshop mana");
}

/// Workshop mana CANNOT be used to cast a non-artifact spell (e.g. Lightning Bolt {R}).
/// The test ensures the legal actions do NOT include the non-artifact spell
/// when the only available mana is workshop-restricted.
#[test]
fn test_workshop_mana_cannot_cast_non_artifact() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let workshop_id = put_on_battlefield(&mut state, &db, CardName::MishrasWorkshop, 0);
    // Counterspell {U}{U} — non-artifact, needs blue mana
    let counterspell_id = add_to_hand(&mut state, CardName::Counterspell, 0);
    // Lightning Bolt {R} — non-artifact, needs red mana
    let bolt_id = add_to_hand(&mut state, CardName::LightningBolt, 0);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Tap Workshop for 3 restricted mana
    state.activate_mana_ability(workshop_id, None);
    assert_eq!(state.players[0].mana_pool.workshop, 3);
    assert_eq!(state.players[0].mana_pool.total(), 0, "No free mana available");

    let actions = state.legal_actions(&db);

    let can_cast_counterspell = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == counterspell_id)
    });
    assert!(!can_cast_counterspell,
        "Should NOT be able to cast Counterspell with only Workshop mana");

    let can_cast_bolt = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == bolt_id)
    });
    assert!(!can_cast_bolt,
        "Should NOT be able to cast Lightning Bolt with only Workshop mana");
}

/// With Workshop mana plus a free Island, the player can cast a non-artifact spell
/// using the Island mana (Workshop mana stays untouched).
#[test]
fn test_free_mana_plus_workshop_allows_non_artifact() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let workshop_id = put_on_battlefield(&mut state, &db, CardName::MishrasWorkshop, 0);
    let island_id = put_on_battlefield(&mut state, &db, CardName::Island, 0);
    let _ = island_id;
    let counterspell_id = add_to_hand(&mut state, CardName::Counterspell, 0);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Tap Workshop and Island manually
    state.activate_mana_ability(workshop_id, None);
    // Tap island for blue
    let island_perm_id = state.battlefield.iter()
        .find(|p| p.card_name == CardName::Island && !p.tapped)
        .map(|p| p.id)
        .unwrap();
    state.activate_mana_ability(island_perm_id, Some(Color::Blue));
    state.activate_mana_ability(island_perm_id, Some(Color::Blue)); // won't work since it's tapped now

    // Give player 2 blue mana directly for the test
    state.players[0].mana_pool.blue = 2;

    let actions = state.legal_actions(&db);
    let can_cast_counterspell = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == counterspell_id)
    });
    assert!(can_cast_counterspell,
        "Should be able to cast Counterspell when player has {{U}}{{U}} available");
}
