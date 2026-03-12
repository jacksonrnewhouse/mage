use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

/// Helper: put a creature on the battlefield for a player (not summoning sick).
fn put_creature(
    state: &mut GameState,
    db: &[CardDef],
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
        None,
        def.keywords,
        def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

// ---------------------------------------------------------------------------
// Test 1: AtBeginningOfNextEndStep fires at the active player's end step
// ---------------------------------------------------------------------------
#[test]
fn test_delayed_trigger_fires_at_end_step() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Register a one-shot delayed trigger: at the beginning of the next end step,
    // draw 1 card for player 0.
    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextEndStep,
        effect: TriggeredEffect::DrawCards(1),
        controller: 0,
        fires_once: true,
        source_id: None,
    });

    assert_eq!(state.delayed_triggers.len(), 1);

    // Transition to the end step
    state.active_player = 0;
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase(); // → Ending / End

    assert_eq!(state.phase, Phase::Ending);
    assert_eq!(state.step, Some(Step::End));

    // The delayed trigger should have fired: stack should have DrawCards(1)
    assert!(
        !state.stack.is_empty(),
        "Delayed trigger should have fired and placed an ability on the stack"
    );

    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::DrawCards(1),
                ..
            }
        ),
        "Top of stack should be DrawCards(1) delayed trigger"
    );
    assert_eq!(top.controller, 0);
}

// ---------------------------------------------------------------------------
// Test 2: One-shot trigger is removed after firing
// ---------------------------------------------------------------------------
#[test]
fn test_one_shot_trigger_removed_after_firing() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextEndStep,
        effect: TriggeredEffect::DrawCards(1),
        controller: 0,
        fires_once: true,
        source_id: None,
    });

    // Advance to end step (fires the trigger)
    state.active_player = 0;
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase();

    // Trigger should have been removed from delayed_triggers list
    assert_eq!(
        state.delayed_triggers.len(),
        0,
        "One-shot delayed trigger should be removed after firing"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Non-one-shot trigger remains after firing and fires again next turn
// ---------------------------------------------------------------------------
#[test]
fn test_repeating_trigger_stays_after_firing() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // A repeating trigger: fires at the beginning of every end step for player 0
    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfEndStep { player: 0 },
        effect: TriggeredEffect::GainLife(1),
        controller: 0,
        fires_once: false,
        source_id: None,
    });

    state.active_player = 0;
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase(); // fires once

    assert!(
        !state.stack.is_empty(),
        "Repeating trigger should fire on first end step"
    );
    // Should still be in delayed_triggers since fires_once is false
    assert_eq!(
        state.delayed_triggers.len(),
        1,
        "Repeating trigger should remain in delayed_triggers"
    );
}

// ---------------------------------------------------------------------------
// Test 4: AtBeginningOfNextEndStep does NOT fire during upkeep
// ---------------------------------------------------------------------------
#[test]
fn test_end_step_trigger_does_not_fire_at_upkeep() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextEndStep,
        effect: TriggeredEffect::DrawCards(1),
        controller: 0,
        fires_once: true,
        source_id: None,
    });

    // Advance to upkeep (should NOT fire the end step trigger)
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Beginning / Upkeep

    assert_eq!(state.phase, Phase::Beginning);
    assert_eq!(state.step, Some(Step::Upkeep));

    // Trigger should NOT have fired
    assert!(
        state.stack.is_empty(),
        "End-step delayed trigger should not fire at upkeep"
    );

    // Trigger should still be pending
    assert_eq!(state.delayed_triggers.len(), 1);
}

// ---------------------------------------------------------------------------
// Test 5: AtBeginningOfNextUpkeep fires at upkeep
// ---------------------------------------------------------------------------
#[test]
fn test_delayed_trigger_fires_at_upkeep() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextUpkeep,
        effect: TriggeredEffect::DrawCards(3),
        controller: 0,
        fires_once: true,
        source_id: None,
    });

    // Advance to upkeep
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Beginning / Upkeep

    assert_eq!(state.phase, Phase::Beginning);
    assert_eq!(state.step, Some(Step::Upkeep));

    // Trigger should have fired
    assert!(
        !state.stack.is_empty(),
        "Upkeep delayed trigger should fire at upkeep"
    );

    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::DrawCards(3),
                ..
            }
        ),
        "Top of stack should be DrawCards(3) delayed trigger"
    );

    // Should be removed since fires_once = true
    assert_eq!(state.delayed_triggers.len(), 0);
}

// ---------------------------------------------------------------------------
// Test 6: AtBeginningOfUpkeep for a specific player only fires on their turn
// ---------------------------------------------------------------------------
#[test]
fn test_upkeep_trigger_only_fires_for_specified_player() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Trigger fires only on player 1's upkeep
    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfUpkeep { player: 1 },
        effect: TriggeredEffect::DrawCards(1),
        controller: 1,
        fires_once: false,
        source_id: None,
    });

    // Advance to player 0's upkeep — should NOT fire
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase();

    assert!(
        state.stack.is_empty(),
        "Trigger for player 1's upkeep should not fire on player 0's upkeep"
    );
    assert_eq!(state.delayed_triggers.len(), 1, "Trigger should still be pending");
}

// ---------------------------------------------------------------------------
// Test 7: SacrificeTarget delayed trigger — Sneak Attack pattern
// ---------------------------------------------------------------------------
#[test]
fn test_sneak_attack_sacrifice_pattern() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a creature on the battlefield for player 0 (simulating Sneak Attack)
    let creature_id = put_creature(&mut state, &db, CardName::EmrakulTheAeonsTorn, 0);

    // Register "at the beginning of the next end step, sacrifice this creature"
    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextEndStep,
        effect: TriggeredEffect::SacrificeTarget { permanent_id: creature_id },
        controller: 0,
        fires_once: true,
        source_id: None,
    });

    // The creature should be on the battlefield before end step
    assert!(
        state.find_permanent(creature_id).is_some(),
        "Creature should be on the battlefield"
    );

    // Advance to end step
    state.active_player = 0;
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase(); // → Ending / End

    // Stack should have the sacrifice trigger
    assert!(
        !state.stack.is_empty(),
        "Sacrifice trigger should be on the stack"
    );

    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::SacrificeTarget { .. },
                ..
            }
        ),
        "Top should be SacrificeTarget trigger"
    );

    // Resolve the trigger: creature should be sacrificed
    state.resolve_top(&db);

    assert!(
        state.find_permanent(creature_id).is_none(),
        "Creature should have been sacrificed after delayed trigger resolved"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Multiple delayed triggers all fire at the same step
// ---------------------------------------------------------------------------
#[test]
fn test_multiple_delayed_triggers_same_step() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextEndStep,
        effect: TriggeredEffect::GainLife(2),
        controller: 0,
        fires_once: true,
        source_id: None,
    });
    state.add_delayed_trigger(DelayedTrigger {
        condition: DelayedTriggerCondition::AtBeginningOfNextEndStep,
        effect: TriggeredEffect::DrawCards(1),
        controller: 0,
        fires_once: true,
        source_id: None,
    });

    state.active_player = 0;
    state.phase = Phase::PostCombatMain;
    state.step = None;
    state.advance_phase();

    // Both triggers should be on the stack
    assert_eq!(
        state.stack.len(),
        2,
        "Both delayed triggers should fire at the same end step"
    );

    // Both should be removed from delayed_triggers
    assert_eq!(state.delayed_triggers.len(), 0);
}
