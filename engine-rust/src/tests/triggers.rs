use crate::card::*;
use crate::game::*;
use crate::types::*;

#[test]
fn test_myr_retriever_dies_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let myr_id = state.new_object_id();
    state.card_registry.push((myr_id, CardName::MyrRetriever));
    let def = find_card(&db, CardName::MyrRetriever).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        myr_id, CardName::MyrRetriever, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Put an artifact in P0's graveyard to retrieve
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    state.players[0].graveyard.push(ring_id);

    // Kill Myr Retriever
    state.destroy_permanent(myr_id);

    // Should have a triggered ability on the stack
    assert!(
        !state.stack.is_empty() || state.pending_choice.is_some(),
        "Myr Retriever should trigger on death"
    );
}

#[test]
fn test_wurmcoil_engine_dies_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let wurm_id = state.new_object_id();
    state.card_registry.push((wurm_id, CardName::WurmcoilEngine));
    let def = find_card(&db, CardName::WurmcoilEngine).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        wurm_id, CardName::WurmcoilEngine, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    let bf_before = state.battlefield.len();

    // Kill Wurmcoil Engine
    state.destroy_permanent(wurm_id);

    // Should have a triggered ability on the stack (WurmcoilDeath)
    assert!(
        !state.stack.is_empty(),
        "Wurmcoil Engine should trigger on death"
    );

    // Resolve the trigger
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Should have created two 3/3 tokens
    let tokens: Vec<_> = state.battlefield.iter().filter(|p| p.is_token).collect();
    assert_eq!(tokens.len(), 2, "Wurmcoil should create 2 tokens on death");
    assert!(tokens.iter().any(|t| t.power() == 3 && t.toughness() == 3),
        "Tokens should be 3/3");
}

#[test]
fn test_destroy_permanent_fires_dies_trigger_from_sba() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let myr_id = state.new_object_id();
    state.card_registry.push((myr_id, CardName::MyrRetriever));
    let def = find_card(&db, CardName::MyrRetriever).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        myr_id, CardName::MyrRetriever, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    // Deal lethal damage
    perm.damage = 10;
    state.battlefield.push(perm);

    // Put an artifact in P0's graveyard
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    state.players[0].graveyard.push(ring_id);

    // Run state-based actions - should kill Myr Retriever and trigger
    state.check_state_based_actions();

    // Myr Retriever should be in graveyard
    assert!(
        state.players[0].graveyard.contains(&myr_id),
        "Myr Retriever should be in graveyard"
    );

    // Should have a triggered ability on the stack
    assert!(
        !state.stack.is_empty(),
        "Myr Retriever death should trigger from SBA"
    );
}

#[test]
fn test_exile_does_not_fire_dies_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let myr_id = state.new_object_id();
    state.card_registry.push((myr_id, CardName::MyrRetriever));
    let def = find_card(&db, CardName::MyrRetriever).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        myr_id, CardName::MyrRetriever, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Put an artifact in P0's graveyard
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    state.players[0].graveyard.push(ring_id);

    // Exile Myr Retriever (should NOT trigger dies)
    state.remove_permanent_to_zone(myr_id, crate::game::DestinationZone::Exile);

    // Should NOT have any triggered abilities
    assert!(
        state.stack.is_empty(),
        "Exiling should not fire dies triggers"
    );
}

#[test]
fn test_temporary_pt_modification() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // GoblinGuide is 2/2 — verify baseline
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 2, "GoblinGuide base power should be 2");
    assert_eq!(creature.toughness(), 2, "GoblinGuide base toughness should be 2");

    // Apply a temporary +3/+3 effect (like Giant Growth)
    state.add_temporary_effect(TemporaryEffect::ModifyPT {
        target: creature_id,
        power: 3,
        toughness: 3,
    });

    // Creature should now be 5/5
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 5, "Creature should be 5/5 after +3/+3");
    assert_eq!(creature.toughness(), 5, "Creature should be 5/5 after +3/+3");

    // End of turn cleanup reverses temporary effects
    state.end_of_turn_cleanup();

    // Creature should be back to 2/2
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 2, "Creature should return to 2/2 after cleanup");
    assert_eq!(creature.toughness(), 2, "Creature should return to 2/2 after cleanup");

    // Temporary effects list should be empty
    assert!(state.temporary_effects.is_empty(), "Temporary effects should be cleared");
}

#[test]
fn test_temporary_keyword_grant() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let perm = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // GoblinGuide already has Haste. Grant Flying temporarily.
    let creature = state.find_permanent(creature_id).unwrap();
    assert!(!creature.keywords.has(Keyword::Flying), "Should not have Flying initially");

    state.add_temporary_effect(TemporaryEffect::GrantKeyword {
        target: creature_id,
        keyword: Keyword::Flying,
    });

    let creature = state.find_permanent(creature_id).unwrap();
    assert!(creature.keywords.has(Keyword::Flying), "Should have Flying after grant");

    state.end_of_turn_cleanup();

    let creature = state.find_permanent(creature_id).unwrap();
    assert!(!creature.keywords.has(Keyword::Flying), "Flying should be removed after cleanup");
    // Original Haste keyword should be preserved
    assert!(creature.keywords.has(Keyword::Haste), "Haste should still be present");
}

#[test]
fn test_temporary_remove_all_abilities() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let perm = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // GoblinGuide has Haste
    let creature = state.find_permanent(creature_id).unwrap();
    assert!(creature.keywords.has(Keyword::Haste), "GoblinGuide should start with Haste");
    let saved = creature.keywords;

    state.add_temporary_effect(TemporaryEffect::RemoveAllAbilities {
        target: creature_id,
        saved_keywords: saved,
    });

    let creature = state.find_permanent(creature_id).unwrap();
    assert!(!creature.keywords.has(Keyword::Haste), "Haste should be removed");

    state.end_of_turn_cleanup();

    let creature = state.find_permanent(creature_id).unwrap();
    assert!(creature.keywords.has(Keyword::Haste), "Haste should be restored after cleanup");
}

#[test]
fn test_multiple_temporary_effects_stack() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Apply two stacking PT buffs
    state.add_temporary_effect(TemporaryEffect::ModifyPT {
        target: creature_id,
        power: 2,
        toughness: 2,
    });
    state.add_temporary_effect(TemporaryEffect::ModifyPT {
        target: creature_id,
        power: 1,
        toughness: 0,
    });

    // Should be 2+2+1 = 5 power, 2+2+0 = 4 toughness
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 5, "Stacked buffs should sum");
    assert_eq!(creature.toughness(), 4, "Stacked buffs should sum");

    state.end_of_turn_cleanup();

    // Should be back to 2/2
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 2, "Both effects should be reversed");
    assert_eq!(creature.toughness(), 2, "Both effects should be reversed");
}
