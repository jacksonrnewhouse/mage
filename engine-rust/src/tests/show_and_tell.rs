use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::types::*;

/// Helper: cast Show and Tell and pass priority until it resolves, returning the game state
/// after resolution (with pending_choice set for the active player's choice).
fn cast_show_and_tell(
    state: &mut GameState,
    db: &[CardDef],
    show_and_tell_id: ObjectId,
) {
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Give player 0 enough mana (2U)
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 2;

    state.apply_action(
        &Action::CastSpell {
            card_id: show_and_tell_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        },
        db,
    );

    // Both players pass priority → Show and Tell resolves
    state.apply_action(&Action::PassPriority, db);
    state.apply_action(&Action::PassPriority, db);
}

/// Test: both players put permanents onto the battlefield via Show and Tell.
#[test]
fn test_show_and_tell_both_players_put_permanents() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Register cards
    let show_id = state.new_object_id();
    let emrakul_id = state.new_object_id();
    let griselbrands_id = state.new_object_id();

    state.card_registry.push((show_id, CardName::ShowAndTell));
    state.card_registry.push((emrakul_id, CardName::EmrakulTheAeonsTorn));
    state.card_registry.push((griselbrands_id, CardName::Griselbrand));

    // Player 0: Show and Tell + Emrakul in hand
    state.players[0].hand.push(show_id);
    state.players[0].hand.push(emrakul_id);

    // Player 1: Griselbrand in hand
    state.players[1].hand.push(griselbrands_id);

    state.turn_number = 1;

    cast_show_and_tell(&mut state, &db, show_id);

    // After Show and Tell resolves, player 0 (active) should have a pending choice
    assert!(
        state.pending_choice.is_some(),
        "Should have a pending choice for player 0 after Show and Tell resolves"
    );

    let choice = state.pending_choice.as_ref().unwrap();
    assert_eq!(choice.player, 0, "Player 0 should be the one making the first choice");
    assert!(
        matches!(&choice.kind, ChoiceKind::ChooseFromList { reason: ChoiceReason::ShowAndTellChoose { .. }, .. }),
        "Choice kind should be ShowAndTellChoose"
    );

    // Player 0 chooses Emrakul
    state.apply_action(&Action::ChooseCard(emrakul_id), &db);

    // Now player 1 should have a pending choice
    assert!(
        state.pending_choice.is_some(),
        "Should have a pending choice for player 1 after player 0 resolves"
    );
    let choice1 = state.pending_choice.as_ref().unwrap();
    assert_eq!(choice1.player, 1, "Player 1 should be the one making the second choice");
    assert!(
        matches!(&choice1.kind, ChoiceKind::ChooseFromList { reason: ChoiceReason::ShowAndTellChoose { next_player: None }, .. }),
        "Second choice should have no next_player"
    );

    // Player 1 chooses Griselbrand
    state.apply_action(&Action::ChooseCard(griselbrands_id), &db);

    // No more pending choices
    assert!(
        state.pending_choice.is_none(),
        "Should have no pending choice after both players choose"
    );

    // Both permanents should be on the battlefield
    let has_emrakul = state.battlefield.iter().any(|p| p.card_name == CardName::EmrakulTheAeonsTorn);
    let has_griselbrand = state.battlefield.iter().any(|p| p.card_name == CardName::Griselbrand);

    assert!(has_emrakul, "Emrakul should be on the battlefield");
    assert!(has_griselbrand, "Griselbrand should be on the battlefield");

    // Both cards should be removed from their owners' hands
    assert!(
        !state.players[0].hand.contains(&emrakul_id),
        "Emrakul should not be in player 0's hand"
    );
    assert!(
        !state.players[1].hand.contains(&griselbrands_id),
        "Griselbrand should not be in player 1's hand"
    );
}

/// Test: player with no valid cards in hand puts nothing.
#[test]
fn test_show_and_tell_player_with_no_valid_cards_passes() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let show_id = state.new_object_id();
    let emrakul_id = state.new_object_id();
    // Player 1 only has an Island in hand (not a valid permanent type for Show and Tell)
    let island_id = state.new_object_id();
    let lightning_id = state.new_object_id();

    state.card_registry.push((show_id, CardName::ShowAndTell));
    state.card_registry.push((emrakul_id, CardName::EmrakulTheAeonsTorn));
    state.card_registry.push((island_id, CardName::Island));
    state.card_registry.push((lightning_id, CardName::LightningBolt));

    // Player 0: Show and Tell + Emrakul in hand
    state.players[0].hand.push(show_id);
    state.players[0].hand.push(emrakul_id);

    // Player 1: only Island + Lightning Bolt (lands and instants are not valid Show and Tell targets)
    state.players[1].hand.push(island_id);
    state.players[1].hand.push(lightning_id);

    state.turn_number = 1;

    cast_show_and_tell(&mut state, &db, show_id);

    // Player 0 chooses Emrakul
    assert!(state.pending_choice.is_some(), "Player 0 should have a pending choice");
    state.apply_action(&Action::ChooseCard(emrakul_id), &db);

    // Player 1 should have a pending choice with no valid options (only pass available)
    assert!(
        state.pending_choice.is_some(),
        "Player 1 should still get a pending choice (with empty options)"
    );
    let choice1 = state.pending_choice.as_ref().unwrap();
    assert_eq!(choice1.player, 1, "Player 1 should be the one making the second choice");

    // The valid options list should be empty (Island and Lightning Bolt are not valid)
    if let ChoiceKind::ChooseFromList { options, .. } = &choice1.kind {
        assert!(
            options.is_empty(),
            "Player 1 should have no valid options (land and instant are not valid Show and Tell targets)"
        );
    } else {
        panic!("Expected ChooseFromList kind");
    }

    // Player 1 passes (ChooseCard(0) = sentinel for no card)
    state.apply_action(&Action::ChooseCard(0), &db);

    // No more pending choices
    assert!(
        state.pending_choice.is_none(),
        "No more pending choices after player 1 passes"
    );

    // Emrakul should be on the battlefield (player 0's choice)
    let has_emrakul = state.battlefield.iter().any(|p| p.card_name == CardName::EmrakulTheAeonsTorn);
    assert!(has_emrakul, "Emrakul should be on the battlefield");

    // Player 1's hand should be unchanged (Island and Lightning Bolt still there)
    assert!(state.players[1].hand.contains(&island_id), "Island should still be in player 1's hand");
    assert!(state.players[1].hand.contains(&lightning_id), "Lightning Bolt should still be in player 1's hand");
}

/// Test: both players may pass (choose nothing) via Show and Tell.
#[test]
fn test_show_and_tell_both_players_pass() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let show_id = state.new_object_id();
    let sorcery_id = state.new_object_id();

    state.card_registry.push((show_id, CardName::ShowAndTell));
    state.card_registry.push((sorcery_id, CardName::DemonicTutor));

    // Player 0: Show and Tell only (only has a sorcery beside it, which is not valid)
    state.players[0].hand.push(show_id);
    state.players[0].hand.push(sorcery_id);

    state.turn_number = 1;

    cast_show_and_tell(&mut state, &db, show_id);

    // Player 0 should have a choice but with no valid permanents (only DemonicTutor = Sorcery)
    assert!(state.pending_choice.is_some(), "Player 0 should have a pending choice");
    if let Some(ref choice) = state.pending_choice {
        if let ChoiceKind::ChooseFromList { options, .. } = &choice.kind {
            assert!(options.is_empty(), "Player 0 has no valid permanent cards");
        }
    }

    // Player 0 passes
    state.apply_action(&Action::ChooseCard(0), &db);

    // Player 1 should get a choice
    assert!(state.pending_choice.is_some(), "Player 1 should have a pending choice");

    // Player 1 passes
    state.apply_action(&Action::ChooseCard(0), &db);

    // No pending choice remains
    assert!(state.pending_choice.is_none(), "No pending choices remain");

    // No permanents added
    assert!(state.battlefield.is_empty(), "No permanents should be on battlefield");
}

/// Test: legal_actions includes the pass option (ChooseCard(0)) for Show and Tell choice.
#[test]
fn test_show_and_tell_legal_actions_include_pass() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let show_id = state.new_object_id();
    let emrakul_id = state.new_object_id();

    state.card_registry.push((show_id, CardName::ShowAndTell));
    state.card_registry.push((emrakul_id, CardName::EmrakulTheAeonsTorn));

    state.players[0].hand.push(show_id);
    state.players[0].hand.push(emrakul_id);

    state.turn_number = 1;

    cast_show_and_tell(&mut state, &db, show_id);

    assert!(state.pending_choice.is_some(), "Should have a pending choice");

    let actions = state.legal_actions(&db);

    // Should include ChooseCard(emrakul_id) for putting Emrakul down
    assert!(
        actions.contains(&Action::ChooseCard(emrakul_id)),
        "Should be able to choose Emrakul"
    );
    // Should also include ChooseCard(0) as a pass option
    assert!(
        actions.contains(&Action::ChooseCard(0)),
        "Should be able to pass (ChooseCard(0))"
    );
}
