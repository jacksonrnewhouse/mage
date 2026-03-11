use crate::card::*;
use crate::action::*;
use crate::types::*;
use crate::game::*;

#[test]
fn test_spirit_of_the_labyrinth_limits_draws() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let spirit_id = state.new_object_id();
    state.card_registry.push((spirit_id, CardName::SpiritOfTheLabyrinth));
    let def = find_card(&db, CardName::SpiritOfTheLabyrinth).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        spirit_id, CardName::SpiritOfTheLabyrinth, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    for _ in 0..10 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[1].library.push(id);
    }

    // P1 draws first card (should work)
    let hand_before = state.players[1].hand.len();
    state.draw_cards(1, 1);
    assert_eq!(state.players[1].hand.len(), hand_before + 1, "First draw should succeed");

    // P1 tries to draw again (should be blocked)
    let hand_after_first = state.players[1].hand.len();
    state.draw_cards(1, 1);
    assert_eq!(state.players[1].hand.len(), hand_after_first, "Second draw should be blocked by Spirit");
}

#[test]
fn test_narset_limits_opponent_draws() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 controls Narset
    let narset_id = state.new_object_id();
    state.card_registry.push((narset_id, CardName::NarsetParterOfVeils));
    let def = find_card(&db, CardName::NarsetParterOfVeils).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        narset_id, CardName::NarsetParterOfVeils, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.controller = 0;
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    for _ in 0..10 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[1].library.push(id);
        let id2 = state.new_object_id();
        state.card_registry.push((id2, CardName::Island));
        state.players[0].library.push(id2);
    }

    // P1 (opponent) draws first card (should work)
    let hand_before = state.players[1].hand.len();
    state.draw_cards(1, 1);
    assert_eq!(state.players[1].hand.len(), hand_before + 1, "Opponent first draw should succeed");

    // P1 tries to draw again (should be blocked by Narset)
    let hand_after_first = state.players[1].hand.len();
    state.draw_cards(1, 1);
    assert_eq!(state.players[1].hand.len(), hand_after_first, "Opponent second draw should be blocked by Narset");

    // P0 (Narset controller) can still draw multiple cards
    let p0_hand_before = state.players[0].hand.len();
    state.draw_cards(0, 2);
    assert_eq!(state.players[0].hand.len(), p0_hand_before + 2, "Narset controller should still draw freely");
}

#[test]
fn test_ethersworn_canonist_limits_nonartifact_spells() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let canonist_id = state.new_object_id();
    state.card_registry.push((canonist_id, CardName::EtherswornCanonist));
    let def = find_card(&db, CardName::EtherswornCanonist).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        canonist_id, CardName::EtherswornCanonist, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    let bolt1_id = state.new_object_id();
    let bolt2_id = state.new_object_id();
    state.card_registry.push((bolt1_id, CardName::LightningBolt));
    state.card_registry.push((bolt2_id, CardName::LightningBolt));
    state.players[1].hand.push(bolt1_id);
    state.players[1].hand.push(bolt2_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 1;
    state.priority_player = 1;
    state.players[1].mana_pool.red = 2;

    // P1 casts first bolt (should work)
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt1_id,
            targets: vec![Target::Player(0)],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Resolve bolt
    state.pass_priority(&db);
    state.pass_priority(&db);

    // P1 should NOT be able to cast second bolt
    let actions = state.legal_actions(&db);
    let can_cast_second = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == bolt2_id));
    assert!(!can_cast_second, "Canonist should prevent second nonartifact spell");
}

#[test]
fn test_deafening_silence_limits_noncreature_spells() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let silence_id = state.new_object_id();
    state.card_registry.push((silence_id, CardName::DeafeningSilence));
    let def = find_card(&db, CardName::DeafeningSilence).unwrap();
    let perm = crate::permanent::Permanent::new(
        silence_id, CardName::DeafeningSilence, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    let bolt1_id = state.new_object_id();
    let bolt2_id = state.new_object_id();
    state.card_registry.push((bolt1_id, CardName::LightningBolt));
    state.card_registry.push((bolt2_id, CardName::LightningBolt));
    state.players[1].hand.push(bolt1_id);
    state.players[1].hand.push(bolt2_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 1;
    state.priority_player = 1;
    state.players[1].mana_pool.red = 2;

    // P1 casts first bolt (noncreature) - should succeed
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt1_id,
            targets: vec![Target::Player(0)],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Resolve bolt
    state.pass_priority(&db);
    state.pass_priority(&db);

    // P1 should NOT be able to cast second bolt (noncreature)
    let actions = state.legal_actions(&db);
    let can_cast_second = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == bolt2_id));
    assert!(!can_cast_second, "Deafening Silence should prevent second noncreature spell");
}

#[test]
fn test_archon_of_emeria_limits_one_spell_per_turn() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let archon_id = state.new_object_id();
    state.card_registry.push((archon_id, CardName::ArchonOfEmeria));
    let def = find_card(&db, CardName::ArchonOfEmeria).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        archon_id, CardName::ArchonOfEmeria, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    let bolt1_id = state.new_object_id();
    let bolt2_id = state.new_object_id();
    state.card_registry.push((bolt1_id, CardName::LightningBolt));
    state.card_registry.push((bolt2_id, CardName::LightningBolt));
    state.players[1].hand.push(bolt1_id);
    state.players[1].hand.push(bolt2_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 1;
    state.priority_player = 1;
    state.players[1].mana_pool.red = 2;

    // P1 casts first bolt - should succeed
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt1_id,
            targets: vec![Target::Player(0)],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Resolve bolt
    state.pass_priority(&db);
    state.pass_priority(&db);

    // P1 should NOT be able to cast second bolt
    let actions = state.legal_actions(&db);
    let can_cast_second = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == bolt2_id));
    assert!(!can_cast_second, "Archon of Emeria should prevent second spell this turn");
}

#[test]
fn test_null_rod_prevents_artifact_abilities() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let rod_id = state.new_object_id();
    state.card_registry.push((rod_id, CardName::NullRod));
    let def = find_card(&db, CardName::NullRod).unwrap();
    let perm = crate::permanent::Permanent::new(
        rod_id, CardName::NullRod, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    let def2 = find_card(&db, CardName::SolRing).unwrap();
    let mut perm2 = crate::permanent::Permanent::new(
        ring_id, CardName::SolRing, 0, 0,
        def2.power, def2.toughness, None, def2.keywords, def2.card_types,
    );
    perm2.entered_this_turn = false;
    state.battlefield.push(perm2);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let actions = state.legal_actions(&db);
    let can_tap_ring = actions.iter().any(|a| matches!(a, Action::ActivateManaAbility { permanent_id, .. } if *permanent_id == ring_id));
    assert!(!can_tap_ring, "Null Rod should prevent Sol Ring activation");
}

#[test]
fn test_stony_silence_prevents_artifact_abilities() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let silence_id = state.new_object_id();
    state.card_registry.push((silence_id, CardName::StonySilence));
    let def = find_card(&db, CardName::StonySilence).unwrap();
    let perm = crate::permanent::Permanent::new(
        silence_id, CardName::StonySilence, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    let def2 = find_card(&db, CardName::SolRing).unwrap();
    let mut perm2 = crate::permanent::Permanent::new(
        ring_id, CardName::SolRing, 0, 0,
        def2.power, def2.toughness, None, def2.keywords, def2.card_types,
    );
    perm2.entered_this_turn = false;
    state.battlefield.push(perm2);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let actions = state.legal_actions(&db);
    let can_tap_ring = actions.iter().any(|a| matches!(a, Action::ActivateManaAbility { permanent_id, .. } if *permanent_id == ring_id));
    assert!(!can_tap_ring, "Stony Silence should prevent Sol Ring activation");
}
