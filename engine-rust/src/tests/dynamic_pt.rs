/// Tests for dynamic P/T calculation (lhurgoyf-style creatures like Tarmogoyf).

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

/// Helper: put Tarmogoyf on the battlefield for a given player.
fn put_tarmogoyf(state: &mut GameState, db: &[CardDef], controller: u8) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, CardName::Tarmogoyf));
    let def = find_card(db, CardName::Tarmogoyf).unwrap();
    let mut perm = Permanent::new(
        id,
        CardName::Tarmogoyf,
        controller,
        controller,
        def.power,
        def.toughness,
        None,
        def.keywords,
        def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

/// Helper: put a card object into a player's graveyard.
fn put_in_graveyard(state: &mut GameState, card_name: CardName, owner: u8) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[owner as usize].graveyard.push(id);
    id
}

#[test]
fn test_tarmogoyf_empty_graveyard() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // With an empty graveyard, Tarmogoyf should be 0/1
    assert_eq!(state.effective_power(goyf_id, &db), 0);
    assert_eq!(state.effective_toughness(goyf_id, &db), 1);
}

#[test]
fn test_tarmogoyf_one_card_type_in_graveyard() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // Put one creature (Goblin Guide) in the graveyard: 1 card type (Creature)
    put_in_graveyard(&mut state, CardName::GoblinGuide, 0);

    assert_eq!(state.effective_power(goyf_id, &db), 1);
    assert_eq!(state.effective_toughness(goyf_id, &db), 2);
}

#[test]
fn test_tarmogoyf_multiple_card_types() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // Creature in P0 graveyard
    put_in_graveyard(&mut state, CardName::GoblinGuide, 0);
    // Instant in P1 graveyard
    put_in_graveyard(&mut state, CardName::LightningBolt, 1);
    // Sorcery in P0 graveyard
    put_in_graveyard(&mut state, CardName::Ponder, 0);
    // Land in P0 graveyard
    put_in_graveyard(&mut state, CardName::Mountain, 0);

    // 4 distinct types: Creature, Instant, Sorcery, Land → 4/5
    assert_eq!(state.effective_power(goyf_id, &db), 4);
    assert_eq!(state.effective_toughness(goyf_id, &db), 5);
}

#[test]
fn test_tarmogoyf_duplicate_card_types_do_not_stack() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // Multiple creatures: still only 1 card type
    put_in_graveyard(&mut state, CardName::GoblinGuide, 0);
    put_in_graveyard(&mut state, CardName::Tarmogoyf, 0);
    put_in_graveyard(&mut state, CardName::ElvishSpiritGuide, 1);

    assert_eq!(state.effective_power(goyf_id, &db), 1);
    assert_eq!(state.effective_toughness(goyf_id, &db), 2);
}

#[test]
fn test_tarmogoyf_artifact_type() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // Artifact (Sol Ring) + Instant (Lightning Bolt) = 2 types → 2/3
    put_in_graveyard(&mut state, CardName::SolRing, 0);
    put_in_graveyard(&mut state, CardName::LightningBolt, 1);

    assert_eq!(state.effective_power(goyf_id, &db), 2);
    assert_eq!(state.effective_toughness(goyf_id, &db), 3);
}

#[test]
fn test_graveyard_card_type_count_uses_all_players_graveyards() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 graveyard has a creature, P1 graveyard has a land
    put_in_graveyard(&mut state, CardName::GoblinGuide, 0);
    put_in_graveyard(&mut state, CardName::Forest, 1);

    // 2 distinct types across both graveyards
    assert_eq!(state.graveyard_card_type_count(&db), 2);
}

#[test]
fn test_tarmogoyf_sba_kills_when_toughness_zero() {
    // Tarmogoyf is 0/1 on empty graveyard, so putting 1 damage on it should NOT kill it
    // (damage >= toughness check: 1 >= 1 → lethal). But 0 damage should not kill (0 >= 1 → false).
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // With empty graveyard Tarmogoyf is 0/1. Deal 1 damage → should die from SBA.
    if let Some(perm) = state.find_permanent_mut(goyf_id) {
        perm.damage = 1;
    }

    state.check_state_based_actions(&db);

    // Tarmogoyf should have died and moved to graveyard
    assert!(state.find_permanent(goyf_id).is_none(), "Tarmogoyf should have died from lethal damage");
    assert!(
        state.players[0].graveyard.contains(&goyf_id),
        "Tarmogoyf should be in graveyard"
    );
}

#[test]
fn test_tarmogoyf_survives_with_higher_toughness() {
    // Add cards to graveyard first so Tarmogoyf has more toughness,
    // then damage that would be lethal at base stats is no longer lethal.
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // 3 card types in graveyard before Tarmogoyf enters: Creature, Instant, Land → Goyf is 3/4
    put_in_graveyard(&mut state, CardName::GoblinGuide, 0);
    put_in_graveyard(&mut state, CardName::LightningBolt, 0);
    put_in_graveyard(&mut state, CardName::Mountain, 0);

    let goyf_id = put_tarmogoyf(&mut state, &db, 0);

    // Deal 3 damage — enough to kill base 0/1 but not 3/4
    if let Some(perm) = state.find_permanent_mut(goyf_id) {
        perm.damage = 3;
    }

    state.check_state_based_actions(&db);

    // Tarmogoyf should survive (3 damage < 4 toughness)
    assert!(state.find_permanent(goyf_id).is_some(), "Tarmogoyf should survive with 3 damage and 4 toughness");
}
