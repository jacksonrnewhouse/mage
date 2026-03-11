/// Tests for coin flip and random effects (issue #34).
///
/// Coin flips are modeled as a PendingChoice with ChoiceReason::CoinFlip so
/// the search tree can explore both outcomes deterministically.
///   ChooseNumber(0) = heads  → win the flip, no consequence
///   ChooseNumber(1) = tails  → lose the flip, negative consequence applies

use crate::action::Action;
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

/// Helper: put a permanent on the battlefield for a player.
fn put_on_battlefield(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
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
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

// ---------------------------------------------------------------------------
// Test: Mana Crypt ETB registers a recurring upkeep trigger
// ---------------------------------------------------------------------------
#[test]
fn test_mana_crypt_etb_registers_upkeep_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let mana_crypt_id = state.new_object_id();
    state.card_registry.push((mana_crypt_id, CardName::ManaCrypt));
    let def = find_card(&db, CardName::ManaCrypt).unwrap();
    let perm = Permanent::new(
        mana_crypt_id,
        CardName::ManaCrypt,
        0,
        0,
        def.power,
        def.toughness,
        None,
        def.keywords,
        def.card_types,
    );
    state.battlefield.push(perm);

    // Fire the ETB manually.
    state.handle_etb(CardName::ManaCrypt, mana_crypt_id, 0);

    // There should be a recurring delayed trigger registered.
    assert!(
        !state.delayed_triggers.is_empty(),
        "Mana Crypt ETB should register a recurring upkeep trigger"
    );

    // The trigger should be a ManaCryptUpkeep triggered effect.
    let has_mana_crypt_trigger = state.delayed_triggers.iter().any(|dt| {
        matches!(dt.effect, TriggeredEffect::ManaCryptUpkeep)
            && dt.controller == 0
            && !dt.fires_once
    });
    assert!(
        has_mana_crypt_trigger,
        "Should have a recurring ManaCryptUpkeep delayed trigger for player 0"
    );
}

// ---------------------------------------------------------------------------
// Test: Mana Crypt upkeep trigger creates a coin-flip PendingChoice
// ---------------------------------------------------------------------------
#[test]
fn test_mana_crypt_upkeep_creates_coin_flip_choice() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Mana Crypt on battlefield and register its upkeep trigger.
    let _crypt_id = put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);
    state.handle_etb(CardName::ManaCrypt, _crypt_id, 0);

    // Transition to the upkeep step — delayed trigger should fire.
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Upkeep; fires delayed triggers

    // ManaCryptUpkeep should now be on the stack.
    assert!(
        !state.stack.is_empty(),
        "ManaCryptUpkeep trigger should be on the stack after advancing to upkeep"
    );

    // Both players pass priority to resolve the trigger.
    state.pass_priority(&db);
    state.pass_priority(&db);

    // After resolution, there should be a coin-flip PendingChoice.
    assert!(
        state.pending_choice.is_some(),
        "Resolving ManaCryptUpkeep should create a PendingChoice for the coin flip"
    );

    let choice = state.pending_choice.as_ref().unwrap();
    assert_eq!(choice.player, 0, "Coin flip choice should be for the Mana Crypt controller");

    match &choice.kind {
        ChoiceKind::ChooseNumber { min, max, reason } => {
            assert_eq!(*min, 0, "min should be 0 (heads)");
            assert_eq!(*max, 1, "max should be 1 (tails)");
            assert!(
                matches!(reason, ChoiceReason::CoinFlip),
                "reason should be CoinFlip"
            );
        }
        other => panic!("Expected ChooseNumber for coin flip, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Test: Coin flip heads (0) — no damage
// ---------------------------------------------------------------------------
#[test]
fn test_coin_flip_heads_no_damage() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _crypt_id = put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);
    state.handle_etb(CardName::ManaCrypt, _crypt_id, 0);

    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Upkeep

    // Resolve the trigger.
    state.pass_priority(&db);
    state.pass_priority(&db);

    let life_before = state.players[0].life;

    // Choose heads (0) — no damage.
    state.apply_action(&Action::ChooseNumber(0), &db);

    assert_eq!(
        state.players[0].life, life_before,
        "Heads: no damage should be dealt"
    );
    assert!(
        state.pending_choice.is_none(),
        "PendingChoice should be cleared after resolving coin flip"
    );
}

// ---------------------------------------------------------------------------
// Test: Coin flip tails (1) — Mana Crypt deals 3 damage
// ---------------------------------------------------------------------------
#[test]
fn test_coin_flip_tails_deals_3_damage() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _crypt_id = put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);
    state.handle_etb(CardName::ManaCrypt, _crypt_id, 0);

    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Upkeep

    // Resolve the trigger.
    state.pass_priority(&db);
    state.pass_priority(&db);

    let life_before = state.players[0].life;

    // Choose tails (1) — take 3 damage.
    state.apply_action(&Action::ChooseNumber(1), &db);

    assert_eq!(
        state.players[0].life,
        life_before - 3,
        "Tails: Mana Crypt should deal 3 damage to the controller"
    );
    assert!(
        state.pending_choice.is_none(),
        "PendingChoice should be cleared after resolving coin flip"
    );
}

// ---------------------------------------------------------------------------
// Test: legal_actions during coin flip only offers heads and tails
// ---------------------------------------------------------------------------
#[test]
fn test_coin_flip_legal_actions_are_heads_and_tails() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _crypt_id = put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);
    state.handle_etb(CardName::ManaCrypt, _crypt_id, 0);

    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Upkeep

    state.pass_priority(&db);
    state.pass_priority(&db); // ManaCryptUpkeep resolves, creates PendingChoice

    // Only heads (0) and tails (1) should be legal.
    let actions = state.legal_actions(&db);
    assert!(
        actions.contains(&Action::ChooseNumber(0)),
        "Heads (0) should be a legal action"
    );
    assert!(
        actions.contains(&Action::ChooseNumber(1)),
        "Tails (1) should be a legal action"
    );
    assert_eq!(
        actions.len(),
        2,
        "Only two coin-flip outcomes should be legal, got {:?}",
        actions
    );
}

// ---------------------------------------------------------------------------
// Test: Mana Crypt upkeep trigger fires every upkeep (recurring trigger)
// ---------------------------------------------------------------------------
#[test]
fn test_mana_crypt_upkeep_trigger_is_recurring() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let _crypt_id = put_on_battlefield(&mut state, &db, CardName::ManaCrypt, 0);
    state.handle_etb(CardName::ManaCrypt, _crypt_id, 0);

    // First upkeep.
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Upkeep

    assert!(
        !state.stack.is_empty(),
        "ManaCryptUpkeep should fire on the first upkeep"
    );

    // Resolve trigger (heads — no damage).
    state.pass_priority(&db);
    state.pass_priority(&db);
    state.apply_action(&Action::ChooseNumber(0), &db);

    // The delayed trigger should still be registered (fires_once = false).
    let still_registered = state.delayed_triggers.iter().any(|dt| {
        matches!(dt.effect, TriggeredEffect::ManaCryptUpkeep)
    });
    assert!(
        still_registered,
        "ManaCryptUpkeep delayed trigger should still be registered after the first upkeep"
    );
}
