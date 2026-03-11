use crate::card::*;
use crate::action::*;
use crate::types::*;
use crate::game::*;

/// Helper: put a creature on the battlefield (not summoning sick).
fn put_creature(
    state: &mut GameState,
    db: &[CardDef],
    card_name: CardName,
    controller: PlayerId,
) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = crate::permanent::Permanent::new(
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

/// Helper: put a card (as a sorcery/spell card, no permanent) in a player's hand.
fn give_card_to_hand(state: &mut GameState, card_name: CardName, player: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].hand.push(id);
    id
}

// ─── Annihilator Tests ───────────────────────────────────────────────────────

#[test]
fn test_annihilator_triggers_pending_choice_on_attack() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 1 two permanents to sacrifice
    let island_id = state.new_object_id();
    state.card_registry.push((island_id, CardName::Island));
    let def = find_card(&db, CardName::Island).unwrap();
    let island_perm = crate::permanent::Permanent::new(
        island_id, CardName::Island, 1, 1,
        None, None, None, def.keywords, def.card_types,
    );
    state.battlefield.push(island_perm);

    let swamp_id = state.new_object_id();
    state.card_registry.push((swamp_id, CardName::Swamp));
    let def2 = find_card(&db, CardName::Swamp).unwrap();
    let swamp_perm = crate::permanent::Permanent::new(
        swamp_id, CardName::Swamp, 1, 1,
        None, None, None, def2.keywords, def2.card_types,
    );
    state.battlefield.push(swamp_perm);

    // Put Emrakul for player 0 (not summoning sick)
    let emrakul_id = put_creature(&mut state, &db, CardName::EmrakulTheAeonsTorn, 0);

    // Move to declare attackers
    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;

    assert!(state.pending_choice.is_none(), "No pending choice before attack");

    // Declare Emrakul as attacker — should trigger annihilator 6
    state.apply_action(&Action::DeclareAttacker { creature_id: emrakul_id }, &db);

    // A pending choice should exist for player 1 (defending player) to sacrifice a permanent
    assert!(state.pending_choice.is_some(), "Annihilator should set a pending choice");
    if let Some(ref choice) = state.pending_choice {
        assert_eq!(choice.player, 1, "Defending player must make the annihilator choice");
        if let ChoiceKind::ChooseFromList { options, reason } = &choice.kind {
            assert!(!options.is_empty(), "Annihilator choice should have permanents to choose from");
            assert!(
                matches!(reason, ChoiceReason::AnnihilatorSacrifice { .. }),
                "Reason should be AnnihilatorSacrifice"
            );
        } else {
            panic!("Expected ChooseFromList for annihilator");
        }
    }
}

#[test]
fn test_annihilator_sacrifices_permanent_on_choice() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 1 exactly one permanent
    let island_id = state.new_object_id();
    state.card_registry.push((island_id, CardName::Island));
    let def = find_card(&db, CardName::Island).unwrap();
    let island_perm = crate::permanent::Permanent::new(
        island_id, CardName::Island, 1, 1,
        None, None, None, def.keywords, def.card_types,
    );
    state.battlefield.push(island_perm);

    // Put Emrakul for player 0
    let emrakul_id = put_creature(&mut state, &db, CardName::EmrakulTheAeonsTorn, 0);

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;

    state.apply_action(&Action::DeclareAttacker { creature_id: emrakul_id }, &db);

    // Resolve the first annihilator sacrifice: choose the island
    assert!(state.pending_choice.is_some());
    let before_bf = state.battlefield.len();
    state.apply_action(&Action::ChooseCard(island_id), &db);

    // Island should have been sacrificed
    let island_still_on_bf = state.battlefield.iter().any(|p| p.id == island_id);
    assert!(!island_still_on_bf, "Island should have been sacrificed by annihilator");
    // Battlefield should have shrunk by 1 (Emrakul remains)
    assert_eq!(state.battlefield.len(), before_bf - 1);
}

#[test]
fn test_annihilator_no_permanents_no_pending_choice() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 1 has no permanents
    let emrakul_id = put_creature(&mut state, &db, CardName::EmrakulTheAeonsTorn, 0);

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;

    state.apply_action(&Action::DeclareAttacker { creature_id: emrakul_id }, &db);

    // No permanents to sacrifice → no pending choice
    assert!(
        state.pending_choice.is_none(),
        "Annihilator with no targets should not create a pending choice"
    );
}

// ─── Extra Turn Tests ─────────────────────────────────────────────────────────

#[test]
fn test_time_walk_grants_extra_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 enough mana and a Time Walk in hand
    let time_walk_id = give_card_to_hand(&mut state, CardName::TimeWalk, 0);
    state.players[0].mana_pool.add(Some(Color::Blue), 1);
    state.players[0].mana_pool.add(None, 1); // generic

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    assert_eq!(state.players[0].extra_turns, 0, "No extra turns before Time Walk");

    state.apply_action(
        &Action::CastSpell {
            card_id: time_walk_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Resolve Time Walk (both players pass)
    state.pass_priority(&db);
    state.pass_priority(&db);

    assert_eq!(state.players[0].extra_turns, 1, "Time Walk should grant 1 extra turn");
}

#[test]
fn test_temporal_mastery_grants_extra_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 enough mana and a Temporal Mastery in hand
    let tm_id = give_card_to_hand(&mut state, CardName::TemporalMastery, 0);
    // Temporal Mastery costs {5}{U}{U}
    state.players[0].mana_pool.add(Some(Color::Blue), 2);
    state.players[0].mana_pool.add(None, 5);

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    assert_eq!(state.players[0].extra_turns, 0, "No extra turns before Temporal Mastery");

    state.apply_action(
        &Action::CastSpell {
            card_id: tm_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Resolve Temporal Mastery
    state.pass_priority(&db);
    state.pass_priority(&db);

    assert_eq!(state.players[0].extra_turns, 1, "Temporal Mastery should grant 1 extra turn");
}

#[test]
fn test_extra_turn_is_taken_by_same_player() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 a Time Walk and mana
    let time_walk_id = give_card_to_hand(&mut state, CardName::TimeWalk, 0);
    state.players[0].mana_pool.add(Some(Color::Blue), 1);
    state.players[0].mana_pool.add(None, 1);

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.turn_number = 1;

    state.apply_action(
        &Action::CastSpell {
            card_id: time_walk_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Resolve Time Walk
    state.pass_priority(&db);
    state.pass_priority(&db);

    assert_eq!(state.players[0].extra_turns, 1);

    // Advance through the rest of the turn to next_turn
    // Simulate reaching Cleanup step (next_turn is called)
    // We manually call next_turn to check that player 0 gets the extra turn
    let active_before = state.active_player;
    assert_eq!(active_before, 0);

    state.players[0].extra_turns = 1; // ensure it's set
    // Simulate next_turn being called by the engine
    // We test the logic via advance_phase reaching Cleanup
    // For simplicity, directly call next_turn by advancing to Ending/Cleanup
    state.phase = Phase::Ending;
    state.step = Some(Step::Cleanup);
    // Patch: discard hand to avoid hand size cleanup issues
    state.players[0].hand.clear();

    // Advance from Cleanup → next_turn
    state.advance_phase();

    // Player 0 should still be the active player (consumed 1 extra turn)
    assert_eq!(state.active_player, 0, "Extra turn should be taken by player 0");
    assert_eq!(state.players[0].extra_turns, 0, "Extra turn counter decremented");
    assert_eq!(state.turn_number, 2, "Turn number advances");
}

#[test]
fn test_emrakul_cast_trigger_grants_extra_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 enough mana (15 generic) and Emrakul in hand
    let emrakul_id = give_card_to_hand(&mut state, CardName::EmrakulTheAeonsTorn, 0);
    state.players[0].mana_pool.add(None, 15);

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::PreCombatMain;
    state.step = None;

    assert_eq!(state.players[0].extra_turns, 0);

    state.apply_action(
        &Action::CastSpell {
            card_id: emrakul_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        &db,
    );

    // Stack should have Emrakul spell + EmrakulCast trigger
    // Resolve both (two rounds of both players passing)
    state.pass_priority(&db); // p0 passes
    state.pass_priority(&db); // p1 passes → resolve top (EmrakulCast trigger)

    // After resolving EmrakulCast trigger, extra turn should be granted
    // (Emrakul spell may still be on the stack; resolve it too)
    state.pass_priority(&db);
    state.pass_priority(&db); // resolve Emrakul spell

    assert_eq!(state.players[0].extra_turns, 1, "Emrakul cast should grant 1 extra turn");
}
