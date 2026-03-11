/// Tests for alternative cost casting: Force of Will, Force of Negation,
/// Misdirection, Commandeer, and evoke creatures (Solitude, Grief, Fury, Endurance).

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::types::*;

/// Helper: create a basic game state positioned at pre-combat main for player 0.
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

/// Helper: register a card into the game state and add it to a player's hand.
fn add_to_hand(state: &mut GameState, player_id: usize, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player_id].hand.push(id);
    id
}

/// Helper: put a spell on the stack (simulates opponent casting something).
fn push_spell_on_stack(state: &mut GameState, card_name: CardName, controller: u8) -> ObjectId {
    let spell_id = state.new_object_id();
    state.card_registry.push((spell_id, card_name));
    state.stack.push_with_flags(
        crate::stack::StackItemKind::Spell {
            card_name,
            card_id: spell_id,
            cast_via_evoke: false,
        },
        controller,
        vec![],
        false,
        0,
        false,
    );
    // Return the stack item ID (the stack generates its own ID for items)
    state.stack.items().last().map(|i| i.id).unwrap_or(spell_id)
}

// ==========================================
// Force of Will Tests
// ==========================================

#[test]
fn test_fow_generates_alt_cost_action_when_has_blue_card_and_life() {
    let (mut state, db) = setup_base();

    // Player 0 has Force of Will + a Brainstorm (blue card to exile)
    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    let blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 10;

    // Put a spell on the stack (FoW only works with non-empty stack)
    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let fow_alt_cost_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { card_id, alt_cost: Some(AltCost::ForceOfWill { exile_id }), .. }
            if *card_id == fow_id && *exile_id == blue_id)
    }).collect();

    assert!(
        !fow_alt_cost_actions.is_empty(),
        "Should generate Force of Will alt-cost action when player has blue card to exile and enough life"
    );
}

#[test]
fn test_fow_no_alt_cost_without_blue_card() {
    let (mut state, db) = setup_base();

    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    // Only non-blue card in hand (red)
    let _red_id = add_to_hand(&mut state, 0, CardName::LightningBolt);
    state.players[0].life = 10;

    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let fow_alt_cost_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { card_id, alt_cost: Some(AltCost::ForceOfWill { .. }), .. }
            if *card_id == fow_id)
    }).collect();

    assert!(
        fow_alt_cost_actions.is_empty(),
        "Should NOT generate Force of Will alt-cost action without a blue card to exile"
    );
}

#[test]
fn test_fow_no_alt_cost_with_only_1_life() {
    let (mut state, db) = setup_base();

    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    let _blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 1; // Can't pay 1 life (would be at 0)

    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let fow_alt_cost_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { card_id, alt_cost: Some(AltCost::ForceOfWill { .. }), .. }
            if *card_id == fow_id)
    }).collect();

    assert!(
        fow_alt_cost_actions.is_empty(),
        "Should NOT generate Force of Will alt-cost action when player has only 1 life"
    );
}

#[test]
fn test_fow_no_alt_cost_with_empty_stack() {
    let (mut state, db) = setup_base();

    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    let _blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 10;
    // Stack is empty

    let actions = state.legal_actions(&db);
    let fow_alt_cost_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { card_id, alt_cost: Some(AltCost::ForceOfWill { .. }), .. }
            if *card_id == fow_id)
    }).collect();

    assert!(
        fow_alt_cost_actions.is_empty(),
        "Should NOT generate Force of Will alt-cost action with empty stack (nothing to counter)"
    );
}

#[test]
fn test_fow_alt_cost_exiles_blue_card_and_pays_life() {
    let (mut state, db) = setup_base();

    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    let blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 10;

    let stack_item_id = push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    // Apply the alternate-cost cast
    state.apply_action(
        &Action::CastSpell {
            card_id: fow_id,
            targets: vec![Target::Object(stack_item_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::ForceOfWill { exile_id: blue_id }),
        },
        &db,
    );

    // FoW should be on the stack
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == fow_id)
        }),
        "Force of Will should be on the stack after alt-cost cast"
    );
    // Brainstorm should be exiled
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == blue_id),
        "Brainstorm should be exiled as the alternate cost"
    );
    // Player paid 1 life
    assert_eq!(
        state.players[0].life, 9,
        "Player should have paid 1 life for Force of Will's alternate cost"
    );
    // Brainstorm should not be in hand
    assert!(
        !state.players[0].hand.contains(&blue_id),
        "Exiled card should no longer be in hand"
    );
}

#[test]
fn test_fow_alt_cost_counters_spell() {
    let (mut state, db) = setup_base();

    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    let blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 10;

    let stack_item_id = push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    // Cast FoW via alternate cost
    state.apply_action(
        &Action::CastSpell {
            card_id: fow_id,
            targets: vec![Target::Object(stack_item_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::ForceOfWill { exile_id: blue_id }),
        },
        &db,
    );

    // Both players pass priority to resolve FoW
    state.pass_priority(&db);
    state.pass_priority(&db);

    // FoW should have resolved and countered the Lightning Bolt
    // The stack should be empty now
    assert!(state.stack.is_empty(), "Stack should be empty after FoW resolves (both FoW and countered spell gone)");
}

// ==========================================
// Force of Negation Tests
// ==========================================

#[test]
fn test_fon_generates_alt_cost_on_opponent_turn() {
    let (mut state, db) = setup_base();

    // Set it to be player 1's turn (player 0 is the responder)
    state.active_player = 1;
    state.priority_player = 0;

    let fon_id = add_to_hand(&mut state, 0, CardName::ForceOfNegation);
    let blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 10;

    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let fon_alt_cost_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { card_id, alt_cost: Some(AltCost::ForceOfNegation { exile_id }), .. }
            if *card_id == fon_id && *exile_id == blue_id)
    }).collect();

    assert!(
        !fon_alt_cost_actions.is_empty(),
        "Should generate Force of Negation alt-cost action on opponent's turn"
    );
}

#[test]
fn test_fon_no_alt_cost_on_own_turn() {
    let (mut state, db) = setup_base();

    // It's player 0's own turn
    state.active_player = 0;
    state.priority_player = 0;

    let fon_id = add_to_hand(&mut state, 0, CardName::ForceOfNegation);
    let blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 10;

    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let fon_alt_cost_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { card_id, alt_cost: Some(AltCost::ForceOfNegation { .. }), .. }
            if *card_id == fon_id)
    }).collect();

    assert!(
        fon_alt_cost_actions.is_empty(),
        "Should NOT generate Force of Negation alt-cost action on player's own turn"
    );
}

#[test]
fn test_fon_alt_cost_exiles_blue_card_without_life_payment() {
    let (mut state, db) = setup_base();

    state.active_player = 1;
    state.priority_player = 0;

    let fon_id = add_to_hand(&mut state, 0, CardName::ForceOfNegation);
    let blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);
    state.players[0].life = 5;

    let stack_item_id = push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    state.apply_action(
        &Action::CastSpell {
            card_id: fon_id,
            targets: vec![Target::Object(stack_item_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::ForceOfNegation { exile_id: blue_id }),
        },
        &db,
    );

    // Blue card exiled
    assert!(state.exile.iter().any(|(id, _, _)| *id == blue_id), "Blue card should be exiled");
    // No life paid (FoN doesn't cost life)
    assert_eq!(state.players[0].life, 5, "Force of Negation should not cost life");
    // FoN on stack
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == fon_id)
        }),
        "Force of Negation should be on the stack"
    );
}

// ==========================================
// Evoke Creature Tests (Solitude)
// ==========================================

#[test]
fn test_solitude_generates_evoke_action_with_white_card() {
    let (mut state, db) = setup_base();

    let solitude_id = add_to_hand(&mut state, 0, CardName::Solitude);
    let white_id = add_to_hand(&mut state, 0, CardName::SwordsToPlowshares); // white card

    // Add a creature to exile (Solitude targets creatures)
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id,
        CardName::GoblinGuide,
        1, 1,
        Some(2), Some(2), None,
        crate::types::Keywords::empty(),
        &[CardType::Creature],
    );
    state.battlefield.push(perm);

    let actions = state.legal_actions(&db);
    let evoke_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::Evoke { exile_id }),
            ..
        } if *card_id == solitude_id && *exile_id == white_id)
    }).collect();

    assert!(
        !evoke_actions.is_empty(),
        "Should generate Solitude evoke action when player has a white card to exile"
    );
}

#[test]
fn test_solitude_no_evoke_without_white_card() {
    let (mut state, db) = setup_base();

    let solitude_id = add_to_hand(&mut state, 0, CardName::Solitude);
    // Only blue card in hand, not white
    let _blue_id = add_to_hand(&mut state, 0, CardName::Brainstorm);

    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    let actions = state.legal_actions(&db);
    let evoke_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::Evoke { .. }),
            ..
        } if *card_id == solitude_id)
    }).collect();

    assert!(
        evoke_actions.is_empty(),
        "Should NOT generate Solitude evoke action without a white card to exile"
    );
}

#[test]
fn test_solitude_evoke_exiles_white_card_and_enters_battlefield() {
    let (mut state, db) = setup_base();

    let solitude_id = add_to_hand(&mut state, 0, CardName::Solitude);
    let white_id = add_to_hand(&mut state, 0, CardName::SwordsToPlowshares);

    // Target creature for Solitude's ETB
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    // Apply evoke cast
    state.apply_action(
        &Action::CastSpell {
            card_id: solitude_id,
            targets: vec![Target::Object(target_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::Evoke { exile_id: white_id }),
        },
        &db,
    );

    // White card should be exiled
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == white_id),
        "White card should be exiled as the evoke cost"
    );

    // Solitude should be on the stack (as a spell)
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == solitude_id)
        }),
        "Solitude should be on the stack after evoke cast"
    );

    // White card should not be in hand
    assert!(
        !state.players[0].hand.contains(&white_id),
        "Exiled white card should no longer be in hand"
    );
}

#[test]
fn test_solitude_evoke_enters_then_is_sacrificed() {
    let (mut state, db) = setup_base();

    let solitude_id = add_to_hand(&mut state, 0, CardName::Solitude);
    let white_id = add_to_hand(&mut state, 0, CardName::SwordsToPlowshares);

    // Target creature for Solitude's ETB
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    // Cast Solitude via evoke
    state.apply_action(
        &Action::CastSpell {
            card_id: solitude_id,
            targets: vec![Target::Object(target_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::Evoke { exile_id: white_id }),
        },
        &db,
    );

    // Both players pass to resolve Solitude (the creature spell)
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes

    // Solitude should now be on the battlefield (after the spell resolves)
    let solitude_on_bf = state.battlefield.iter().any(|p| p.card_name == CardName::Solitude);
    assert!(solitude_on_bf, "Solitude should be on the battlefield after resolving");

    // The stack should now have the evoke sacrifice trigger AND the ETB trigger
    // (evoke sacrifice is pushed under ETB, so ETB resolves first)
    // Let both resolve to see the final state
    // ETB trigger (SolitudeETB) resolves first
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes ETB trigger resolves, GoblinGuide exiled

    // Evoke sacrifice trigger resolves
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes evoke sacrifice resolves

    // Solitude should be gone from battlefield (sacrificed via evoke)
    let solitude_still_on_bf = state.battlefield.iter().any(|p| p.card_name == CardName::Solitude);
    assert!(
        !solitude_still_on_bf,
        "Solitude should be sacrificed (leaves battlefield) after evoke trigger resolves"
    );

    // Target creature should be exiled by Solitude's ETB
    let target_exiled = state.exile.iter().any(|(id, _, _)| *id == target_id);
    assert!(
        target_exiled,
        "Target creature should be exiled by Solitude's ETB trigger"
    );
}

// ==========================================
// Grief Evoke Tests
// ==========================================

#[test]
fn test_grief_generates_evoke_action_with_black_card() {
    let (mut state, db) = setup_base();

    let grief_id = add_to_hand(&mut state, 0, CardName::Grief);
    let black_id = add_to_hand(&mut state, 0, CardName::Thoughtseize); // black card

    let actions = state.legal_actions(&db);
    let evoke_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::Evoke { exile_id }),
            ..
        } if *card_id == grief_id && *exile_id == black_id)
    }).collect();

    assert!(
        !evoke_actions.is_empty(),
        "Should generate Grief evoke action when player has a black card to exile"
    );
}

#[test]
fn test_fury_generates_evoke_action_with_red_card() {
    let (mut state, db) = setup_base();

    let fury_id = add_to_hand(&mut state, 0, CardName::Fury);
    let red_id = add_to_hand(&mut state, 0, CardName::LightningBolt); // red card

    // Add a creature target for Fury's ETB (Fury deals damage to creatures)
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    let actions = state.legal_actions(&db);
    let evoke_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::Evoke { exile_id }),
            ..
        } if *card_id == fury_id && *exile_id == red_id)
    }).collect();

    assert!(
        !evoke_actions.is_empty(),
        "Should generate Fury evoke action when player has a red card to exile"
    );
}

#[test]
fn test_endurance_generates_evoke_action_with_green_card() {
    let (mut state, db) = setup_base();

    let endurance_id = add_to_hand(&mut state, 0, CardName::Endurance);
    let green_id = add_to_hand(&mut state, 0, CardName::Tarmogoyf); // green card

    let actions = state.legal_actions(&db);
    let evoke_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::Evoke { exile_id }),
            ..
        } if *card_id == endurance_id && *exile_id == green_id)
    }).collect();

    assert!(
        !evoke_actions.is_empty(),
        "Should generate Endurance evoke action when player has a green card to exile"
    );
}

// ==========================================
// Ensure normal mana-cost casting still works
// ==========================================

#[test]
fn test_fow_can_still_be_cast_for_mana() {
    let (mut state, db) = setup_base();

    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);
    // Give player enough mana for the normal cost {3}{U}{U}: 2 blue + 3 colorless
    state.players[0].mana_pool = crate::mana::ManaPool {
        white: 0,
        blue: 2,
        black: 0,
        red: 0,
        green: 0,
        colorless: 3,
    };

    let stack_item_id = push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    // Should have both normal-cost and alt-cost actions for FoW
    let normal_fow_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: None,
            ..
        } if *card_id == fow_id)
    }).collect();

    assert!(
        !normal_fow_actions.is_empty(),
        "Force of Will should also be castable for its normal mana cost when player has enough mana"
    );
}

// ==========================================
// Phyrexian Mana Tests — Gitaxian Probe
// ==========================================

#[test]
fn test_gitaxian_probe_generates_phyrexian_alt_cost_at_sorcery_speed() {
    let (mut state, db) = setup_base();

    // Player 0 has Gitaxian Probe and enough life but no mana
    let probe_id = add_to_hand(&mut state, 0, CardName::GitaxianProbe);
    state.players[0].life = 10;
    state.players[0].mana_pool = crate::mana::ManaPool::default();

    // Sorcery speed: main phase, empty stack, active player
    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { life_paid: 2, .. }),
            ..
        } if *card_id == probe_id)
    }).collect();

    assert!(
        !phyrexian_actions.is_empty(),
        "Gitaxian Probe should generate a Phyrexian mana alt-cost action at sorcery speed"
    );
}

#[test]
fn test_gitaxian_probe_phyrexian_not_available_at_instant_speed() {
    let (mut state, db) = setup_base();

    let probe_id = add_to_hand(&mut state, 0, CardName::GitaxianProbe);
    state.players[0].life = 10;

    // Not sorcery speed: opponent's turn
    state.active_player = 1;
    state.priority_player = 0;
    // Put something on the stack to ensure it's not sorcery speed
    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { .. }),
            ..
        } if *card_id == probe_id)
    }).collect();

    assert!(
        phyrexian_actions.is_empty(),
        "Gitaxian Probe (sorcery) should NOT generate Phyrexian alt-cost action at instant speed"
    );
}

#[test]
fn test_gitaxian_probe_phyrexian_not_available_with_low_life() {
    let (mut state, db) = setup_base();

    let probe_id = add_to_hand(&mut state, 0, CardName::GitaxianProbe);
    state.players[0].life = 2; // Exactly 2 life: life > 2 check fails

    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { .. }),
            ..
        } if *card_id == probe_id)
    }).collect();

    assert!(
        phyrexian_actions.is_empty(),
        "Gitaxian Probe Phyrexian cost should not be available when player has only 2 life (would be lethal)"
    );
}

#[test]
fn test_gitaxian_probe_phyrexian_deducts_2_life_and_draws() {
    let (mut state, db) = setup_base();

    let probe_id = add_to_hand(&mut state, 0, CardName::GitaxianProbe);
    state.players[0].life = 10;
    state.players[0].mana_pool = crate::mana::ManaPool::default();
    // Give player 1 a card in library (to draw from)
    let draw_card_id = state.new_object_id();
    state.card_registry.push((draw_card_id, CardName::Island));
    state.players[0].library.push(draw_card_id);

    state.apply_action(
        &Action::CastSpell {
            card_id: probe_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::PhyrexianMana {
                life_paid: 2,
                normal_cost: crate::mana::ManaCost::ZERO,
            }),
        },
        &db,
    );

    // Life should be reduced by 2
    assert_eq!(
        state.players[0].life, 8,
        "Player should have paid 2 life for Gitaxian Probe's Phyrexian cost"
    );

    // Probe should be on the stack
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == probe_id)
        }),
        "Gitaxian Probe should be on the stack"
    );

    // Probe should not be in hand
    assert!(
        !state.players[0].hand.contains(&probe_id),
        "Gitaxian Probe should have left hand"
    );

    // Resolve the probe (both players pass)
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes → probe resolves, draws a card

    // Player 0 should have drawn a card
    assert!(
        state.players[0].hand.contains(&draw_card_id),
        "Player 0 should have drawn a card from Gitaxian Probe resolving"
    );
}

// ==========================================
// Phyrexian Mana Tests — Mental Misstep
// ==========================================

#[test]
fn test_mental_misstep_phyrexian_available_with_stack_spell() {
    let (mut state, db) = setup_base();

    let misstep_id = add_to_hand(&mut state, 0, CardName::MentalMisstep);
    state.players[0].life = 10;
    state.players[0].mana_pool = crate::mana::ManaPool::default();

    push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { life_paid: 2, .. }),
            ..
        } if *card_id == misstep_id)
    }).collect();

    assert!(
        !phyrexian_actions.is_empty(),
        "Mental Misstep should generate a Phyrexian mana alt-cost action when stack is non-empty"
    );
}

#[test]
fn test_mental_misstep_phyrexian_not_available_with_empty_stack() {
    let (mut state, db) = setup_base();

    let misstep_id = add_to_hand(&mut state, 0, CardName::MentalMisstep);
    state.players[0].life = 10;

    // Stack is empty
    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { .. }),
            ..
        } if *card_id == misstep_id)
    }).collect();

    assert!(
        phyrexian_actions.is_empty(),
        "Mental Misstep Phyrexian cost should not be available when stack is empty (no target)"
    );
}

#[test]
fn test_mental_misstep_phyrexian_deducts_2_life() {
    let (mut state, db) = setup_base();

    let misstep_id = add_to_hand(&mut state, 0, CardName::MentalMisstep);
    state.players[0].life = 10;
    state.players[0].mana_pool = crate::mana::ManaPool::default();

    let stack_item_id = push_spell_on_stack(&mut state, CardName::LightningBolt, 1);

    state.apply_action(
        &Action::CastSpell {
            card_id: misstep_id,
            targets: vec![Target::Object(stack_item_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::PhyrexianMana {
                life_paid: 2,
                normal_cost: crate::mana::ManaCost::ZERO,
            }),
        },
        &db,
    );

    assert_eq!(
        state.players[0].life, 8,
        "Player should have paid 2 life for Mental Misstep's Phyrexian cost"
    );
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == misstep_id)
        }),
        "Mental Misstep should be on the stack"
    );
}

// ==========================================
// Phyrexian Mana Tests — Dismember
// ==========================================

#[test]
fn test_dismember_generates_phyrexian_half_life_option() {
    let (mut state, db) = setup_base();

    let dismember_id = add_to_hand(&mut state, 0, CardName::Dismember);
    state.players[0].life = 20;
    // Give player {1}{B} mana so they can pay the half-life variant
    state.players[0].mana_pool = crate::mana::ManaPool {
        black: 1,
        colorless: 1,
        ..Default::default()
    };

    // Add a creature to target
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    let actions = state.legal_actions(&db);

    // Should have life_paid=2 option ({1}{B} + 2 life)
    let half_life_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { life_paid: 2, .. }),
            ..
        } if *card_id == dismember_id)
    }).collect();

    assert!(
        !half_life_actions.is_empty(),
        "Dismember should generate a Phyrexian action paying 2 life + {{1}}{{B}}"
    );
}

#[test]
fn test_dismember_generates_phyrexian_full_life_option() {
    let (mut state, db) = setup_base();

    let dismember_id = add_to_hand(&mut state, 0, CardName::Dismember);
    state.players[0].life = 20;
    // Give player {1} mana so they can pay the full-life variant
    state.players[0].mana_pool = crate::mana::ManaPool {
        colorless: 1,
        ..Default::default()
    };

    // Add a creature to target
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    let actions = state.legal_actions(&db);

    // Should have life_paid=4 option ({1} + 4 life)
    let full_life_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { life_paid: 4, .. }),
            ..
        } if *card_id == dismember_id)
    }).collect();

    assert!(
        !full_life_actions.is_empty(),
        "Dismember should generate a Phyrexian action paying 4 life + {{1}}"
    );
}

#[test]
fn test_dismember_phyrexian_deducts_life_and_mana() {
    let (mut state, db) = setup_base();

    let dismember_id = add_to_hand(&mut state, 0, CardName::Dismember);
    state.players[0].life = 20;
    state.players[0].mana_pool = crate::mana::ManaPool {
        colorless: 1,
        ..Default::default()
    };

    // Add a creature to target
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::GoblinGuide));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        target_id, CardName::GoblinGuide, 1, 1, Some(2), Some(2), None,
        crate::types::Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);

    // Cast Dismember paying {1} + 4 life
    state.apply_action(
        &Action::CastSpell {
            card_id: dismember_id,
            targets: vec![Target::Object(target_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::PhyrexianMana {
                life_paid: 4,
                normal_cost: crate::mana::ManaCost { generic: 1, ..crate::mana::ManaCost::ZERO },
            }),
        },
        &db,
    );

    // Should have paid 4 life
    assert_eq!(
        state.players[0].life, 16,
        "Player should have paid 4 life for Dismember's full Phyrexian cost"
    );

    // Generic mana should be consumed
    assert_eq!(
        state.players[0].mana_pool.total(), 0,
        "Player's mana pool should be empty after paying {{1}} for Dismember"
    );

    // Dismember should be on the stack
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == dismember_id)
        }),
        "Dismember should be on the stack"
    );
}

#[test]
fn test_dismember_phyrexian_no_actions_without_creature_targets() {
    let (mut state, db) = setup_base();

    let dismember_id = add_to_hand(&mut state, 0, CardName::Dismember);
    state.players[0].life = 20;
    state.players[0].mana_pool = crate::mana::ManaPool {
        colorless: 1,
        ..Default::default()
    };

    // No creatures on the battlefield

    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { .. }),
            ..
        } if *card_id == dismember_id)
    }).collect();

    assert!(
        phyrexian_actions.is_empty(),
        "Dismember Phyrexian cost should not be available when there are no creature targets"
    );
}

// ==========================================
// Phyrexian Mana Tests — Surgical Extraction
// ==========================================

#[test]
fn test_surgical_extraction_phyrexian_available_with_graveyard_target() {
    let (mut state, db) = setup_base();

    let surgical_id = add_to_hand(&mut state, 0, CardName::SurgicalExtraction);
    state.players[0].life = 10;
    state.players[0].mana_pool = crate::mana::ManaPool::default();

    // Put a card in opponent's graveyard
    let graveyard_card_id = state.new_object_id();
    state.card_registry.push((graveyard_card_id, CardName::LightningBolt));
    state.players[1].graveyard.push(graveyard_card_id);

    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { life_paid: 2, .. }),
            ..
        } if *card_id == surgical_id)
    }).collect();

    assert!(
        !phyrexian_actions.is_empty(),
        "Surgical Extraction should generate a Phyrexian alt-cost action when graveyard has targets"
    );
}

#[test]
fn test_surgical_extraction_phyrexian_not_available_with_empty_graveyards() {
    let (mut state, db) = setup_base();

    let surgical_id = add_to_hand(&mut state, 0, CardName::SurgicalExtraction);
    state.players[0].life = 10;

    // Both graveyards are empty

    let actions = state.legal_actions(&db);
    let phyrexian_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell {
            card_id,
            alt_cost: Some(AltCost::PhyrexianMana { .. }),
            ..
        } if *card_id == surgical_id)
    }).collect();

    assert!(
        phyrexian_actions.is_empty(),
        "Surgical Extraction Phyrexian cost should not be available without graveyard targets"
    );
}

#[test]
fn test_surgical_extraction_phyrexian_deducts_2_life() {
    let (mut state, db) = setup_base();

    let surgical_id = add_to_hand(&mut state, 0, CardName::SurgicalExtraction);
    state.players[0].life = 10;
    state.players[0].mana_pool = crate::mana::ManaPool::default();

    let graveyard_card_id = state.new_object_id();
    state.card_registry.push((graveyard_card_id, CardName::LightningBolt));
    state.players[1].graveyard.push(graveyard_card_id);

    state.apply_action(
        &Action::CastSpell {
            card_id: surgical_id,
            targets: vec![Target::Object(graveyard_card_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: Some(AltCost::PhyrexianMana {
                life_paid: 2,
                normal_cost: crate::mana::ManaCost::ZERO,
            }),
        },
        &db,
    );

    assert_eq!(
        state.players[0].life, 8,
        "Player should have paid 2 life for Surgical Extraction's Phyrexian cost"
    );
    assert!(
        state.stack.items().iter().any(|i| {
            matches!(&i.kind, crate::stack::StackItemKind::Spell { card_id, .. } if *card_id == surgical_id)
        }),
        "Surgical Extraction should be on the stack after Phyrexian mana cast"
    );
    // Surgical Extraction not in hand anymore
    assert!(
        !state.players[0].hand.contains(&surgical_id),
        "Surgical Extraction should have left hand"
    );
}
