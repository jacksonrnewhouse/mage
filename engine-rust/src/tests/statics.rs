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

#[test]
fn test_manglehorn_opponents_artifacts_enter_tapped() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 controls Manglehorn
    let manglehorn_id = state.new_object_id();
    state.card_registry.push((manglehorn_id, CardName::Manglehorn));
    let def = find_card(&db, CardName::Manglehorn).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        manglehorn_id, CardName::Manglehorn, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // P1 (opponent) puts an artifact onto the battlefield
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    let def2 = find_card(&db, CardName::SolRing).unwrap();
    let perm2 = crate::permanent::Permanent::new(
        ring_id, CardName::SolRing, 1, 1,
        def2.power, def2.toughness, None, def2.keywords, def2.card_types,
    );
    state.battlefield.push(perm2);
    state.apply_enters_tapped_statics(ring_id, 1);

    // The opponent's artifact should enter tapped due to Manglehorn
    let sol_ring = state.battlefield.iter().find(|p| p.id == ring_id).unwrap();
    assert!(sol_ring.tapped, "Opponent's artifact should enter tapped with Manglehorn on the battlefield");

    // P0's own artifacts should NOT enter tapped
    let own_ring_id = state.new_object_id();
    state.card_registry.push((own_ring_id, CardName::SolRing));
    let perm3 = crate::permanent::Permanent::new(
        own_ring_id, CardName::SolRing, 0, 0,
        def2.power, def2.toughness, None, def2.keywords, def2.card_types,
    );
    state.battlefield.push(perm3);
    state.apply_enters_tapped_statics(own_ring_id, 0);
    let own_ring = state.battlefield.iter().find(|p| p.id == own_ring_id).unwrap();
    assert!(!own_ring.tapped, "Controller's own artifacts should NOT enter tapped with Manglehorn");
}

#[test]
fn test_manglehorn_treasure_tokens_enter_tapped() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 controls Manglehorn
    let manglehorn_id = state.new_object_id();
    state.card_registry.push((manglehorn_id, CardName::Manglehorn));
    let def = find_card(&db, CardName::Manglehorn).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        manglehorn_id, CardName::Manglehorn, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // P1 (opponent) creates a Treasure token — it should enter tapped
    let treasure_id = state.create_treasure_token(1);
    let treasure = state.battlefield.iter().find(|p| p.id == treasure_id).unwrap();
    assert!(treasure.tapped, "Opponent's Treasure token should enter tapped with Manglehorn");

    // P0's own Treasure token should NOT enter tapped
    let own_treasure_id = state.create_treasure_token(0);
    let own_treasure = state.battlefield.iter().find(|p| p.id == own_treasure_id).unwrap();
    assert!(!own_treasure.tapped, "Controller's own Treasure should NOT enter tapped with Manglehorn");
}

#[test]
fn test_damping_sphere_ancient_tomb_produces_one_colorless() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Damping Sphere on the battlefield
    let sphere_id = state.new_object_id();
    state.card_registry.push((sphere_id, CardName::DampingSphere));
    let def = find_card(&db, CardName::DampingSphere).unwrap();
    let mut sphere = crate::permanent::Permanent::new(
        sphere_id, CardName::DampingSphere, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    sphere.entered_this_turn = false;
    state.battlefield.push(sphere);

    // Put Ancient Tomb on the battlefield
    let tomb_id = state.new_object_id();
    state.card_registry.push((tomb_id, CardName::AncientTomb));
    let tomb_def = find_card(&db, CardName::AncientTomb).unwrap();
    let mut tomb = crate::permanent::Permanent::new(
        tomb_id, CardName::AncientTomb, 0, 0,
        tomb_def.power, tomb_def.toughness, None, tomb_def.keywords, tomb_def.card_types,
    );
    tomb.entered_this_turn = false;
    state.battlefield.push(tomb);

    let mana_before = state.players[0].mana_pool.colorless;
    state.activate_mana_ability(tomb_id, None);
    let mana_after = state.players[0].mana_pool.colorless;

    // Ancient Tomb normally produces 2 colorless, but Damping Sphere reduces it to 1
    assert_eq!(mana_after - mana_before, 1,
        "Ancient Tomb should produce only 1 colorless mana under Damping Sphere");
}

#[test]
fn test_damping_sphere_does_not_affect_single_mana_lands() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Damping Sphere on the battlefield
    let sphere_id = state.new_object_id();
    state.card_registry.push((sphere_id, CardName::DampingSphere));
    let def = find_card(&db, CardName::DampingSphere).unwrap();
    let mut sphere = crate::permanent::Permanent::new(
        sphere_id, CardName::DampingSphere, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    sphere.entered_this_turn = false;
    state.battlefield.push(sphere);

    // Put a basic Island on the battlefield
    let island_id = state.new_object_id();
    state.card_registry.push((island_id, CardName::Island));
    let island_def = find_card(&db, CardName::Island).unwrap();
    let mut island = crate::permanent::Permanent::new(
        island_id, CardName::Island, 0, 0,
        island_def.power, island_def.toughness, None, island_def.keywords, island_def.card_types,
    );
    island.entered_this_turn = false;
    state.battlefield.push(island);

    let blue_before = state.players[0].mana_pool.blue;
    state.activate_mana_ability(island_id, Some(Color::Blue));
    let blue_after = state.players[0].mana_pool.blue;

    // Basic lands produce only 1 mana, so Damping Sphere should not affect them
    assert_eq!(blue_after - blue_before, 1,
        "Basic Island should still produce 1 blue mana under Damping Sphere");
}

#[test]
fn test_damping_sphere_does_not_affect_sol_ring() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Damping Sphere on the battlefield
    let sphere_id = state.new_object_id();
    state.card_registry.push((sphere_id, CardName::DampingSphere));
    let def = find_card(&db, CardName::DampingSphere).unwrap();
    let mut sphere = crate::permanent::Permanent::new(
        sphere_id, CardName::DampingSphere, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    sphere.entered_this_turn = false;
    state.battlefield.push(sphere);

    // Put Sol Ring on the battlefield
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    let ring_def = find_card(&db, CardName::SolRing).unwrap();
    let mut ring = crate::permanent::Permanent::new(
        ring_id, CardName::SolRing, 0, 0,
        ring_def.power, ring_def.toughness, None, ring_def.keywords, ring_def.card_types,
    );
    ring.entered_this_turn = false;
    state.battlefield.push(ring);

    let mana_before = state.players[0].mana_pool.colorless;
    state.activate_mana_ability(ring_id, None);
    let mana_after = state.players[0].mana_pool.colorless;

    // Sol Ring is an artifact, not a land, so Damping Sphere should NOT affect it
    assert_eq!(mana_after - mana_before, 2,
        "Sol Ring should still produce 2 colorless mana under Damping Sphere (it's not a land)");
}
