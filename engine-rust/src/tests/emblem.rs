use crate::card::*;
use crate::game::*;
use crate::types::*;
use crate::action::Action;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, ActivatedEffect};

/// Helper: put a permanent on the battlefield for a player.
fn put_permanent(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);
    id
}

/// Helper: seed a player's library with one copy of a card.
fn seed_library(state: &mut GameState, player: PlayerId, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].library.push(id);
    id
}

// === Emblem data model tests ===

#[test]
fn test_create_emblem() {
    let _db = build_card_db();
    let mut state = GameState::new_two_player();

    // No emblems initially
    assert!(state.emblems.is_empty());
    assert!(!state.has_emblem(0, Emblem::DackFayden));

    // Create Dack emblem for player 0
    state.create_emblem(0, Emblem::DackFayden);
    assert_eq!(state.emblems.len(), 1);
    assert!(state.has_emblem(0, Emblem::DackFayden));
    assert!(!state.has_emblem(1, Emblem::DackFayden));
    assert!(!state.has_emblem(0, Emblem::WrennAndSix));
}

#[test]
fn test_multiple_emblems() {
    let _db = build_card_db();
    let mut state = GameState::new_two_player();

    state.create_emblem(0, Emblem::DackFayden);
    state.create_emblem(0, Emblem::WrennAndSix);
    state.create_emblem(1, Emblem::TezzeretCruelCaptain);

    assert!(state.has_emblem(0, Emblem::DackFayden));
    assert!(state.has_emblem(0, Emblem::WrennAndSix));
    assert!(!state.has_emblem(0, Emblem::TezzeretCruelCaptain));
    assert!(state.has_emblem(1, Emblem::TezzeretCruelCaptain));
    assert!(!state.has_emblem(1, Emblem::DackFayden));
    assert!(state.any_player_has_emblem(Emblem::DackFayden));
    assert!(state.any_player_has_emblem(Emblem::TezzeretCruelCaptain));
    assert!(!state.any_player_has_emblem(Emblem::GideonOfTheTrials));
}

#[test]
fn test_emblem_survives_clone() {
    let _db = build_card_db();
    let mut state = GameState::new_two_player();
    state.create_emblem(0, Emblem::WrennAndSix);

    let cloned = state.clone();
    assert!(cloned.has_emblem(0, Emblem::WrennAndSix));
    assert_eq!(cloned.emblems.len(), 1);
}

// === Dack Fayden ultimate ===

#[test]
fn test_dack_ultimate_creates_emblem() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Dack with enough loyalty for the ultimate (-6 from 3 starting = need 6+ loyalty)
    // Give him 9 loyalty to clearly have enough
    let dack_id = put_permanent(&mut state, &db, CardName::DackFayden, 0);
    state.find_permanent_mut(dack_id).unwrap().loyalty = 9;

    // No emblem yet
    assert!(!state.has_emblem(0, Emblem::DackFayden));

    // Activate the ultimate (ability_index = 2)
    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: dack_id,
            ability_index: 2,
            targets: vec![],
        },
        &db,
    );

    // The ultimate should be on the stack
    assert!(!state.stack.is_empty(), "Ultimate should be on the stack");

    // Resolve it
    state.resolve_top(&db);

    // Dack Fayden emblem should now exist for player 0
    assert!(state.has_emblem(0, Emblem::DackFayden), "Dack emblem should be created");
    // Dack's loyalty should have decreased by 6
    assert_eq!(state.find_permanent(dack_id).unwrap().loyalty, 3);
}

// === Wrenn and Six ultimate ===

#[test]
fn test_wrenn_ultimate_creates_emblem() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let wrenn_id = put_permanent(&mut state, &db, CardName::WrennAndSix, 0);
    state.find_permanent_mut(wrenn_id).unwrap().loyalty = 10;

    assert!(!state.has_emblem(0, Emblem::WrennAndSix));

    // Activate ultimate (ability_index = 2, costs -7)
    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: wrenn_id,
            ability_index: 2,
            targets: vec![],
        },
        &db,
    );

    assert!(!state.stack.is_empty(), "Ultimate should be on the stack");
    state.resolve_top(&db);

    assert!(state.has_emblem(0, Emblem::WrennAndSix), "Wrenn emblem should be created");
    assert_eq!(state.find_permanent(wrenn_id).unwrap().loyalty, 3);
}

// === Tezzeret, Cruel Captain ultimate ===

#[test]
fn test_tezzeret_ultimate_creates_emblem() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let tezz_id = put_permanent(&mut state, &db, CardName::TezzeretCruelCaptain, 0);
    state.find_permanent_mut(tezz_id).unwrap().loyalty = 10;

    assert!(!state.has_emblem(0, Emblem::TezzeretCruelCaptain));

    // Activate ultimate (ability_index = 2, costs -7)
    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: tezz_id,
            ability_index: 2,
            targets: vec![],
        },
        &db,
    );

    assert!(!state.stack.is_empty());
    state.resolve_top(&db);

    assert!(state.has_emblem(0, Emblem::TezzeretCruelCaptain), "Tezzeret emblem should be created");
    assert_eq!(state.find_permanent(tezz_id).unwrap().loyalty, 3);
}

// === Tezzeret emblem effect: search library for artifact when casting artifact spell ===

#[test]
fn test_tezzeret_emblem_triggers_on_artifact_cast() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has the Tezzeret emblem
    state.create_emblem(0, Emblem::TezzeretCruelCaptain);

    // Put an artifact spell in hand
    let mox_id = state.new_object_id();
    state.card_registry.push((mox_id, CardName::MoxSapphire));
    state.players[0].hand.push(mox_id);

    // Put an artifact in library to find (the trigger gives a pending search choice)
    seed_library(&mut state, 0, CardName::SolRing);
    seed_library(&mut state, 0, CardName::Counterspell); // non-artifact, shouldn't be searchable

    // Set up mana for player 0 to cast Mox (free)
    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::PreCombatMain;

    // Cast Mox Sapphire (free artifact)
    state.apply_action(
        &Action::CastSpell {
            card_id: mox_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // The Tezzeret emblem trigger should be on the stack
    let has_tezz_trigger = state.stack.items().iter().any(|item| {
        matches!(&item.kind, StackItemKind::TriggeredAbility { effect, .. }
            if matches!(effect, crate::stack::TriggeredEffect::TezzeretEmblemArtifact))
    });
    assert!(has_tezz_trigger, "Tezzeret emblem trigger should fire when casting artifact");
}

// === Dack emblem effect: gain control when targeting permanents ===

#[test]
fn test_dack_emblem_triggers_on_permanent_targeting_spell() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has the Dack emblem
    state.create_emblem(0, Emblem::DackFayden);

    // Put a creature on battlefield for player 1 (will be targeted)
    let goblin_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 1);

    // Put Swords to Plowshares in player 0's hand
    let swords_id = state.new_object_id();
    state.card_registry.push((swords_id, CardName::SwordsToPlowshares));
    state.players[0].hand.push(swords_id);

    // Give player 0 white mana
    state.players[0].mana_pool.add(Some(Color::White), 1);

    state.active_player = 0;
    state.priority_player = 0;
    state.phase = Phase::PreCombatMain;

    // Cast Swords to Plowshares targeting the Goblin Guide
    state.apply_action(
        &Action::CastSpell {
            card_id: swords_id,
            targets: vec![Target::Object(goblin_id)],
            x_value: 0,
            from_graveyard: false,
            from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // The Dack emblem trigger should be on the stack
    let has_dack_trigger = state.stack.items().iter().any(|item| {
        matches!(&item.kind, StackItemKind::TriggeredAbility { effect, .. }
            if matches!(effect, crate::stack::TriggeredEffect::DackEmblemControl))
    });
    assert!(has_dack_trigger, "Dack emblem trigger should fire when casting spell targeting permanent");
}

#[test]
fn test_dack_emblem_gains_control_on_resolve() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has Dack emblem
    state.create_emblem(0, Emblem::DackFayden);

    // Put a Mox Sapphire on battlefield controlled by player 1
    let mox_id = put_permanent(&mut state, &db, CardName::MoxSapphire, 1);
    assert_eq!(state.find_permanent(mox_id).unwrap().controller, 1);

    // Manually push a DackEmblemControl trigger targeting the Mox
    state.stack.push(
        crate::stack::StackItemKind::TriggeredAbility {
            source_id: 0,
            source_name: CardName::Plains,
            effect: crate::stack::TriggeredEffect::DackEmblemControl,
        },
        0, // player 0 controls the emblem
        vec![Target::Object(mox_id)],
    );

    // Resolve it
    state.resolve_top(&db);

    // Player 0 should now control the Mox
    assert_eq!(
        state.find_permanent(mox_id).unwrap().controller, 0,
        "Dack emblem should give player 0 control of the permanent"
    );
}

// === Gideon of the Trials emblem: can't lose while controlling Gideon ===

#[test]
fn test_gideon_emblem_prevents_loss_while_gideon_controlled() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has the Gideon emblem AND controls a Gideon planeswalker
    let gideon_id = put_permanent(&mut state, &db, CardName::GideonOfTheTrials, 0);
    state.create_emblem(0, Emblem::GideonOfTheTrials);

    // Reduce player 0's life to 0 — they would normally lose
    state.players[0].life = 0;
    state.check_state_based_actions(&db);

    // Player 0 should NOT have lost because they have the Gideon emblem and control Gideon
    assert!(!state.players[0].has_lost, "Player with Gideon emblem + Gideon planeswalker should not lose");
    assert_eq!(state.result, GameResult::InProgress);

    // Now remove Gideon from the battlefield
    state.remove_permanent(gideon_id);

    // Re-check SBAs — now player 0 has no Gideon planeswalker, so the protection doesn't apply
    state.check_state_based_actions(&db);
    assert!(state.players[0].has_lost, "Player should lose once Gideon is off the battlefield");
}

#[test]
fn test_gideon_emblem_does_not_prevent_loss_without_gideon_permanent() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has emblem but NO Gideon on battlefield
    state.create_emblem(0, Emblem::GideonOfTheTrials);
    state.players[0].life = 0;
    state.check_state_based_actions(&db);

    assert!(state.players[0].has_lost, "Emblem alone doesn't prevent loss if Gideon is not on battlefield");
}

// === Tezzeret +1 ability ===

#[test]
fn test_tezzeret_plus1_draws_card_with_artifact() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let tezz_id = put_permanent(&mut state, &db, CardName::TezzeretCruelCaptain, 0);
    state.find_permanent_mut(tezz_id).unwrap().loyalty = 4;

    // Put an artifact on battlefield so the draw triggers
    put_permanent(&mut state, &db, CardName::MoxSapphire, 0);

    // Seed library
    seed_library(&mut state, 0, CardName::LightningBolt);
    let initial_hand = state.players[0].hand.len();

    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: tezz_id,
            ability_index: 0,
            targets: vec![],
        },
        &db,
    );
    state.resolve_top(&db);

    assert_eq!(
        state.players[0].hand.len(),
        initial_hand + 1,
        "Tezzeret +1 should draw a card when you control an artifact"
    );
    assert_eq!(state.find_permanent(tezz_id).unwrap().loyalty, 5);
}

// === Tezzeret -2 ability: create Thopter token ===

#[test]
fn test_tezzeret_minus2_creates_thopter() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let tezz_id = put_permanent(&mut state, &db, CardName::TezzeretCruelCaptain, 0);
    state.find_permanent_mut(tezz_id).unwrap().loyalty = 4;

    let initial_battlefield = state.battlefield.len();

    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: tezz_id,
            ability_index: 1,
            targets: vec![],
        },
        &db,
    );
    state.resolve_top(&db);

    // A new token should be on the battlefield
    assert_eq!(
        state.battlefield.len(),
        initial_battlefield + 1,
        "Tezzeret -2 should create a Thopter token"
    );
    let thopter = state.battlefield.iter()
        .find(|p| p.card_name == CardName::ThopterToken)
        .expect("Should have a Thopter token");
    assert_eq!(thopter.base_power, 1);
    assert_eq!(thopter.base_toughness, 1);
    assert!(thopter.keywords.has(Keyword::Flying), "Thopter should have flying");
    assert!(thopter.is_artifact(), "Thopter should be an artifact");
    assert!(thopter.is_creature(), "Thopter should be a creature");
    assert!(thopter.is_token, "Thopter should be a token");
    assert_eq!(state.find_permanent(tezz_id).unwrap().loyalty, 2);
}
