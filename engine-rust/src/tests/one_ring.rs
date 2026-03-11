/// Tests for The One Ring damage prevention and redirection (#54).
use crate::action::{Action, ActionContext};
use crate::card::{build_card_db, find_card, CardName};
use crate::game::GameState;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn put_permanent(
    state: &mut GameState,
    db: &[crate::card::CardDef],
    card_name: CardName,
    controller: PlayerId,
) -> ObjectId {
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
        def.loyalty,
        def.keywords,
        def.card_types,
    );
    perm.entered_this_turn = false;
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);
    id
}

// ---------------------------------------------------------------------------
// Test 1: The One Ring ETB grants protection from everything (damage prevented)
// ---------------------------------------------------------------------------

/// When The One Ring enters the battlefield (and was cast), the controller
/// should gain protection from everything until their next turn — meaning all
/// damage dealt to them is prevented.
#[test]
fn test_one_ring_etb_protection_prevents_damage() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place The One Ring on the battlefield for player 0.
    let ring_id = put_permanent(&mut state, &db, CardName::TheOneRing, 0);

    // Manually trigger the ETB (simulating "if you cast it" — the protection trigger).
    state.handle_etb(CardName::TheOneRing, ring_id, 0);

    // Resolve the TheOneRingETB trigger by passing priority twice.
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Player 0 should now have protection from everything.
    assert!(
        state.players[0].protection_from_everything,
        "Player 0 should have protection from everything after The One Ring ETB resolves"
    );

    let life_before = state.players[0].life;

    // Simulate direct damage to player 0 (e.g., a Lightning Bolt).
    // Use deal_damage_to_target directly.
    // Protection should prevent this.
    state.players[0].mana_pool.red = 1;
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[1].hand.push(bolt_id);

    // Set up for player 1 to cast a bolt at player 0.
    state.priority_player = 1;
    state.active_player = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.players[1].mana_pool.red = 1;

    let cast_action = Action::CastSpell {
        card_id: bolt_id,
        targets: vec![Target::Player(0)],
        x_value: 0,
        from_graveyard: false,
        from_library_top: false,
        alt_cost: None,
        modes: vec![],
    };
    state.apply_action(&cast_action, &db);

    // Resolve the bolt (both players pass).
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Player 0's life should be unchanged — damage is prevented by protection.
    assert_eq!(
        state.players[0].life, life_before,
        "Damage to player 0 should be prevented by The One Ring's protection from everything"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Protection from everything wears off at the start of the next turn
// ---------------------------------------------------------------------------

/// The One Ring's protection should expire at the start of the controller's
/// next turn (reset_for_turn clears the flag).
#[test]
fn test_one_ring_protection_expires_next_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Grant protection manually.
    state.players[0].protection_from_everything = true;

    assert!(
        state.players[0].protection_from_everything,
        "Player 0 should have protection before their next turn"
    );

    // Simulate the start of player 0's next turn by calling reset_for_turn.
    state.players[0].reset_for_turn();

    assert!(
        !state.players[0].protection_from_everything,
        "Player 0's protection should expire at the start of their next turn"
    );
}

// ---------------------------------------------------------------------------
// Test 3: The One Ring upkeep trigger loses life per burden counter
// ---------------------------------------------------------------------------

/// At the beginning of the controller's upkeep, they should lose 1 life per
/// burden counter on The One Ring.  With 2 burden counters the loss is 2.
#[test]
fn test_one_ring_upkeep_loses_life_per_burden_counter() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place The One Ring on the battlefield for player 0.
    let ring_id = put_permanent(&mut state, &db, CardName::TheOneRing, 0);

    // Add 2 burden counters manually.
    if let Some(perm) = state.find_permanent_mut(ring_id) {
        perm.counters.add(CounterType::Burden, 2);
    }

    let life_before = state.players[0].life;

    // Simulate the upkeep trigger resolving.
    let upkeep_effect = TriggeredEffect::TheOneRingUpkeep { ring_id };
    state.phase = Phase::Beginning;
    state.step = Some(Step::Upkeep);
    state.active_player = 0;
    state.priority_player = 0;

    state.stack.push(
        StackItemKind::TriggeredAbility {
            source_id: ring_id,
            source_name: CardName::TheOneRing,
            effect: upkeep_effect,
        },
        0,
        vec![],
    );

    // Both players pass priority to resolve the trigger.
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Player 0 should have lost 2 life (2 burden counters).
    assert_eq!(
        state.players[0].life, life_before - 2,
        "Player 0 should lose 1 life per burden counter on The One Ring"
    );

    // After the trigger resolves, there should be 3 burden counters (added 1).
    let burden_after = state.find_permanent(ring_id)
        .map(|p| p.counters.get(CounterType::Burden))
        .unwrap_or(0);
    assert_eq!(
        burden_after, 3,
        "Burden counter should have been added, going from 2 to 3"
    );
}

// ---------------------------------------------------------------------------
// Test 4: The One Ring upkeep trigger fires each turn (recurring)
// ---------------------------------------------------------------------------

/// The One Ring registers a recurring upkeep trigger that fires every time the
/// controller's upkeep begins. We verify the trigger is placed on the stack
/// when entering the upkeep step.
#[test]
fn test_one_ring_upkeep_trigger_is_recurring() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place The One Ring on the battlefield for player 0.
    let ring_id = put_permanent(&mut state, &db, CardName::TheOneRing, 0);

    // Trigger ETB to register the delayed trigger.
    state.handle_etb(CardName::TheOneRing, ring_id, 0);

    // Resolve the ETB trigger.
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // There should now be a delayed trigger registered for player 0's upkeep.
    let has_upkeep_trigger = state.delayed_triggers.iter().any(|dt| {
        matches!(
            &dt.effect,
            TriggeredEffect::TheOneRingUpkeep { ring_id: rid } if *rid == ring_id
        ) && matches!(
            dt.condition,
            DelayedTriggerCondition::AtBeginningOfUpkeep { player: 0 }
        ) && !dt.fires_once
    });
    assert!(
        has_upkeep_trigger,
        "The One Ring should have a recurring upkeep trigger registered for player 0"
    );
}

// ---------------------------------------------------------------------------
// Test 5: The One Ring tap ability draws cards equal to burden counters
// ---------------------------------------------------------------------------

/// Activating The One Ring's tap ability ({T}) should put a burden counter on
/// it and draw cards equal to the total burden counters.
#[test]
fn test_one_ring_tap_ability_draws_cards() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let ring_id = put_permanent(&mut state, &db, CardName::TheOneRing, 0);
    // Start with 1 existing burden counter; after activation there will be 2
    // and we should draw 2 cards.
    if let Some(perm) = state.find_permanent_mut(ring_id) {
        perm.counters.add(CounterType::Burden, 1);
    }

    // Populate library with cards to draw.
    for _ in 0..5 {
        let card_id = state.new_object_id();
        state.card_registry.push((card_id, CardName::Island));
        state.players[0].library.push(card_id);
    }
    let hand_size_before = state.players[0].hand.len();

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Generate actions and find the Ring activation.
    let actions = state.legal_actions(&db);
    let ring_action = actions.iter().find(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, ability_index: 0, .. }
            if *permanent_id == ring_id)
    });
    assert!(ring_action.is_some(), "Should have an activate action for The One Ring");

    state.apply_action(ring_action.unwrap(), &db);

    // Resolve the activated ability.
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Ring should now be tapped and have 2 burden counters.
    let ring = state.find_permanent(ring_id).expect("Ring should still be on battlefield");
    assert!(ring.tapped, "The One Ring should be tapped after activating");
    assert_eq!(
        ring.counters.get(CounterType::Burden), 2,
        "The One Ring should have 2 burden counters after activation (1 existing + 1 new)"
    );

    // Player 0 should have drawn 2 cards (equal to 2 burden counters).
    assert_eq!(
        state.players[0].hand.len(),
        hand_size_before + 2,
        "Player 0 should have drawn 2 cards (equal to burden counter count)"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Protection from everything prevents combat damage
// ---------------------------------------------------------------------------

/// When a player has protection from everything, combat damage from attackers
/// should be prevented.
#[test]
fn test_one_ring_protection_prevents_combat_damage() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 1 has a 3/3 attacker.
    let attacker_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 1);

    // Grant player 0 protection from everything (as The One Ring would).
    state.players[0].protection_from_everything = true;

    let life_before = state.players[0].life;

    // Set up unblocked attacker going at player 0.
    state.phase = Phase::Combat;
    state.step = Some(Step::CombatDamage);
    state.active_player = 1;
    state.attackers.push((attacker_id, 0)); // attacker targets player 0

    state.resolve_combat_damage(&db, false);

    // Player 0 should not have taken damage.
    assert_eq!(
        state.players[0].life, life_before,
        "Player 0 with protection from everything should not take combat damage"
    );
}
