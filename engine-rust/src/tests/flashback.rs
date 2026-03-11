/// Tests for graveyard casting and flashback mechanics.

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::mana::ManaCost;
use crate::types::*;

/// Helper: set up a two-player game in the pre-combat main phase with
/// both players having some mana available.
fn setup_game_with_mana() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Mountain)
        .take(33)
        .chain(std::iter::once(CardName::AncientGrudge))
        .chain(std::iter::repeat(CardName::Forest).take(6))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::Island; 40];
    state.load_deck(0, &p0_deck, &db);
    state.load_deck(1, &p1_deck, &db);
    state.start_game();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    (state, db)
}

/// Test: A card with flashback in the graveyard generates a CastSpell action
/// with from_graveyard: true when the controller has the flashback cost available.
#[test]
fn test_flashback_generates_graveyard_cast_action() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Set up player 0 with AncientGrudge in graveyard
    let grudge_id = state.new_object_id();
    state.card_registry.push((grudge_id, CardName::AncientGrudge));
    state.players[0].graveyard.push(grudge_id);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Player needs {G} for the flashback cost (AncientGrudge flashback = {G})
    state.players[0].mana_pool.green = 1;

    // There must be an artifact to target (AncientGrudge destroys an artifact)
    // We'll add a fake artifact to the battlefield for player 1
    // For simplicity, just check that the CastSpell from_graveyard action exists
    // even without a target (AncientGrudge requires a target, but we'll check
    // if the action is generated when valid targets exist).
    // Place a Sol Ring for player 1 to target.
    let sol_id = state.new_object_id();
    state.card_registry.push((sol_id, CardName::SolRing));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        sol_id,
        CardName::SolRing,
        1, // controller
        1, // owner
        None, None, None,
        crate::types::Keywords::empty(),
        &[CardType::Artifact],
    );
    state.battlefield.push(perm);

    let actions = state.legal_actions(&db);
    let flashback_cast = actions.iter().find(|a| {
        matches!(a, Action::CastSpell { card_id, from_graveyard, .. }
            if *card_id == grudge_id && *from_graveyard)
    });
    assert!(
        flashback_cast.is_some(),
        "Should be able to cast AncientGrudge via flashback from graveyard"
    );
}

/// Test: A non-flashback spell in the graveyard cannot be cast normally
/// (without Yawgmoth's Will or Snapcaster Mage granting flashback).
#[test]
fn test_non_flashback_spell_cannot_be_cast_from_graveyard() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a Lightning Bolt (no flashback) in graveyard
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].graveyard.push(bolt_id);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Give lots of mana
    state.players[0].mana_pool.red = 5;

    let actions = state.legal_actions(&db);
    let graveyard_cast = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_graveyard, .. }
            if *card_id == bolt_id && *from_graveyard)
    });
    assert!(
        !graveyard_cast,
        "Lightning Bolt has no flashback — should not be castable from graveyard"
    );
}

/// Test: Casting a spell via flashback exiles the card instead of putting it in graveyard.
#[test]
fn test_flashback_spell_is_exiled_after_resolving() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // AncientGrudge with flashback {G}: destroy target artifact.
    // Place it in graveyard
    let grudge_id = state.new_object_id();
    state.card_registry.push((grudge_id, CardName::AncientGrudge));
    state.players[0].graveyard.push(grudge_id);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Give player 0 {G} for the flashback cost
    state.players[0].mana_pool.green = 1;

    // Place an artifact for player 1 to target
    let sol_id = state.new_object_id();
    state.card_registry.push((sol_id, CardName::SolRing));
    use crate::permanent::Permanent;
    let perm = Permanent::new(
        sol_id, CardName::SolRing, 1, 1, None, None, None,
        crate::types::Keywords::empty(), &[CardType::Artifact],
    );
    state.battlefield.push(perm);

    // Cast AncientGrudge via flashback targeting Sol Ring
    state.apply_action(
        &Action::CastSpell {
            card_id: grudge_id,
            targets: vec![Target::Object(sol_id)],
            x_value: 0,
            from_graveyard: true,
                from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // AncientGrudge should be on the stack, not in graveyard
    assert!(
        !state.players[0].graveyard.contains(&grudge_id),
        "AncientGrudge should be removed from graveyard when cast"
    );
    assert_eq!(state.stack.len(), 1, "AncientGrudge should be on the stack");

    // Both players pass priority to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // The spell resolved — the card should now be in exile, not graveyard
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == grudge_id);
    let in_graveyard = state.players[0].graveyard.contains(&grudge_id);

    assert!(
        in_exile,
        "AncientGrudge cast via flashback should be exiled after resolving"
    );
    assert!(
        !in_graveyard,
        "AncientGrudge cast via flashback should NOT go to graveyard"
    );
}

/// Test: Snapcaster Mage ETB grants flashback to a target instant/sorcery in graveyard.
#[test]
fn test_snapcaster_mage_grants_flashback() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a Lightning Bolt in player 0's graveyard
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].graveyard.push(bolt_id);

    // Put Snapcaster Mage in player 0's hand
    let snapcaster_id = state.new_object_id();
    state.card_registry.push((snapcaster_id, CardName::SnapcasterMage));
    state.players[0].hand.push(snapcaster_id);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Give player 0 {1}{U} for Snapcaster Mage
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 1;

    // Cast Snapcaster Mage targeting the Lightning Bolt in graveyard
    state.apply_action(
        &Action::CastSpell {
            card_id: snapcaster_id,
            targets: vec![Target::Object(bolt_id)],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Resolve Snapcaster Mage (both players pass)
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Snapcaster Mage should be on the battlefield
    let snapcaster_on_bf = state.battlefield.iter().any(|p| p.card_name == CardName::SnapcasterMage);
    assert!(snapcaster_on_bf, "Snapcaster Mage should be on the battlefield");

    // The bolt should now have flashback (be in snapcaster_flashback_cards)
    let has_flashback = state.snapcaster_flashback_cards.contains(&bolt_id);
    assert!(
        has_flashback,
        "Lightning Bolt should have flashback granted by Snapcaster Mage"
    );

    // Now the Lightning Bolt in graveyard should be castable via flashback
    // Give player 0 {R} for the bolt's mana cost (snapcaster grants normal cost as flashback cost)
    state.players[0].mana_pool.red = 1;

    let actions = state.legal_actions(&db);
    let can_cast_from_gyd = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_graveyard, .. }
            if *card_id == bolt_id && *from_graveyard)
    });
    assert!(
        can_cast_from_gyd,
        "Lightning Bolt should be castable from graveyard via Snapcaster Mage flashback"
    );
}

/// Test: Yawgmoth's Will allows casting spells from graveyard for their normal cost.
#[test]
fn test_yawgmoths_will_enables_graveyard_casting() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a Lightning Bolt in player 0's graveyard
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].graveyard.push(bolt_id);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Activate Yawgmoth's Will (simulate it having resolved)
    state.players[0].yawgmoth_will_active = true;

    // Give player 0 {R}
    state.players[0].mana_pool.red = 1;

    let actions = state.legal_actions(&db);
    let can_cast_from_gyd = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_graveyard, .. }
            if *card_id == bolt_id && *from_graveyard)
    });
    assert!(
        can_cast_from_gyd,
        "Lightning Bolt should be castable from graveyard with Yawgmoth's Will active"
    );
}

/// Test: Yawgmoth's Will graveyard casting exiles the card after resolution.
#[test]
fn test_yawgmoths_will_cast_exiles_on_resolution() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a Lightning Bolt in player 0's graveyard
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].graveyard.push(bolt_id);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Activate Yawgmoth's Will
    state.players[0].yawgmoth_will_active = true;

    // Give mana
    state.players[0].mana_pool.red = 1;

    // Cast Lightning Bolt from graveyard
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
            from_graveyard: true,
                from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Should be in exile, not graveyard
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == bolt_id);
    let in_graveyard = state.players[0].graveyard.contains(&bolt_id);

    assert!(in_exile, "Bolt cast via Yawgmoth's Will should be exiled after resolving");
    assert!(!in_graveyard, "Bolt cast via Yawgmoth's Will should NOT go to graveyard");
}

/// Test: Snapcaster-granted flashback is cleared at end of turn.
#[test]
fn test_snapcaster_flashback_cleared_at_end_of_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Simulate Snapcaster Mage having granted flashback to bolt_id
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].graveyard.push(bolt_id);
    state.snapcaster_flashback_cards.push(bolt_id);

    assert!(!state.snapcaster_flashback_cards.is_empty(), "Sanity: flashback list should be populated");

    // Advance from End step to Cleanup step, which runs cleanup_step()
    state.phase = Phase::Ending;
    state.step = Some(Step::End);
    state.advance_phase(); // Transitions to Cleanup, calls cleanup_step()

    assert!(
        state.snapcaster_flashback_cards.is_empty(),
        "Snapcaster flashback grants should be cleared at end of turn"
    );
}
