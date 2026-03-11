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
    state.check_state_based_actions(&db);

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

#[test]
fn test_gain_control_basic() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P1 controls a creature
    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let perm = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Verify P1 controls the creature
    assert_eq!(state.find_permanent(creature_id).unwrap().controller, 1);

    // P0 gains control
    state.gain_control(creature_id, 0);

    // P0 should now control the creature
    assert_eq!(
        state.find_permanent(creature_id).unwrap().controller, 0,
        "gain_control should change controller to P0"
    );
}

#[test]
fn test_agent_of_treachery_etb_gains_control() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P1 controls a creature on the battlefield
    let p1_creature_id = state.new_object_id();
    state.card_registry.push((p1_creature_id, CardName::LightningBolt)); // use any card
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let perm = crate::permanent::Permanent::new(
        p1_creature_id, CardName::GoblinGuide, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // P0 plays Agent of Treachery (ETB should queue a trigger to gain control)
    let agent_id = state.new_object_id();
    state.card_registry.push((agent_id, CardName::AgentOfTreachery));
    let agent_def = find_card(&db, CardName::AgentOfTreachery).unwrap();
    let agent = crate::permanent::Permanent::new(
        agent_id, CardName::AgentOfTreachery, 0, 0,
        agent_def.power, agent_def.toughness, None, agent_def.keywords, agent_def.card_types,
    );
    state.battlefield.push(agent);

    // Fire ETB
    state.handle_etb(CardName::AgentOfTreachery, agent_id, 0);

    // Should have a triggered ability on the stack
    assert!(!state.stack.is_empty(), "Agent of Treachery ETB should push trigger");

    // Resolve the trigger
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // P0 should now control P1's creature
    assert_eq!(
        state.find_permanent(p1_creature_id).unwrap().controller, 0,
        "After Agent of Treachery ETB resolves, P0 should control the target permanent"
    );
}

#[test]
fn test_gilded_drake_etb_exchanges_control() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P1 controls a creature on the battlefield
    let p1_creature_id = state.new_object_id();
    state.card_registry.push((p1_creature_id, CardName::GoblinGuide));
    let goblin_def = find_card(&db, CardName::GoblinGuide).unwrap();
    let perm = crate::permanent::Permanent::new(
        p1_creature_id, CardName::GoblinGuide, 1, 1,
        goblin_def.power, goblin_def.toughness, None, goblin_def.keywords, goblin_def.card_types,
    );
    state.battlefield.push(perm);

    // P0 plays Gilded Drake (ETB queues exchange)
    let drake_id = state.new_object_id();
    state.card_registry.push((drake_id, CardName::GildedDrake));
    let drake_def = find_card(&db, CardName::GildedDrake).unwrap();
    let drake = crate::permanent::Permanent::new(
        drake_id, CardName::GildedDrake, 0, 0,
        drake_def.power, drake_def.toughness, None, drake_def.keywords, drake_def.card_types,
    );
    state.battlefield.push(drake);

    // Fire ETB for Gilded Drake
    state.handle_etb(CardName::GildedDrake, drake_id, 0);

    assert!(!state.stack.is_empty(), "Gilded Drake ETB should push trigger");

    // Resolve the trigger
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Drake should now be controlled by P1; P1's creature by P0
    assert_eq!(
        state.find_permanent(drake_id).unwrap().controller, 1,
        "After exchange, Gilded Drake should be controlled by P1"
    );
    assert_eq!(
        state.find_permanent(p1_creature_id).unwrap().controller, 0,
        "After exchange, P1's creature should be controlled by P0"
    );
}

#[test]
fn test_exchange_control() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 controls a creature
    let p0_creature_id = state.new_object_id();
    state.card_registry.push((p0_creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let perm_a = crate::permanent::Permanent::new(
        p0_creature_id, CardName::GoblinGuide, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm_a);

    // P1 controls a creature
    let p1_creature_id = state.new_object_id();
    state.card_registry.push((p1_creature_id, CardName::GoblinGuide));
    let perm_b = crate::permanent::Permanent::new(
        p1_creature_id, CardName::GoblinGuide, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm_b);

    // Exchange control
    state.exchange_control(p0_creature_id, p1_creature_id);

    assert_eq!(
        state.find_permanent(p0_creature_id).unwrap().controller, 1,
        "P0's creature should now be controlled by P1"
    );
    assert_eq!(
        state.find_permanent(p1_creature_id).unwrap().controller, 0,
        "P1's creature should now be controlled by P0"
    );
    let _ = db;
}

#[test]
fn test_young_pyromancer_triggers_on_noncreature_spell() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Young Pyromancer on the battlefield under P0's control
    let pyro_id = state.new_object_id();
    state.card_registry.push((pyro_id, CardName::YoungPyromancer));
    let def = find_card(&db, CardName::YoungPyromancer).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        pyro_id, CardName::YoungPyromancer, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Give P0 enough mana to cast Lightning Bolt
    state.players[0].mana_pool.red += 1;

    // Put Lightning Bolt in P0's hand
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].hand.push(bolt_id);

    // Set up priority
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Apply CastSpell action
    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Stack should have both the spell and the trigger
    assert!(
        state.stack.len() >= 2,
        "Stack should have the spell and the Young Pyromancer trigger, got {} items",
        state.stack.len()
    );

    // Resolve both stack items (trigger first, then spell — or spell resolves first depending on order)
    // Pass priority twice to resolve the top item
    state.pass_priority(&db);
    state.pass_priority(&db);
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Young Pyromancer should have created a 1/1 Elemental token
    let tokens: Vec<_> = state.battlefield.iter().filter(|p| p.is_token).collect();
    assert_eq!(tokens.len(), 1, "Young Pyromancer should create exactly 1 token");
    assert_eq!(tokens[0].power(), 1, "Token should be 1/1");
    assert_eq!(tokens[0].toughness(), 1, "Token should be 1/1");
}

#[test]
fn test_young_pyromancer_does_not_trigger_on_creature_spell() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Young Pyromancer on the battlefield under P0's control
    let pyro_id = state.new_object_id();
    state.card_registry.push((pyro_id, CardName::YoungPyromancer));
    let def = find_card(&db, CardName::YoungPyromancer).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        pyro_id, CardName::YoungPyromancer, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Give P0 enough mana to cast Goblin Guide (1R) — 2 red covers the generic {1} as well
    state.players[0].mana_pool.red += 2;

    // Put Goblin Guide in P0's hand
    let goblin_id = state.new_object_id();
    state.card_registry.push((goblin_id, CardName::GoblinGuide));
    state.players[0].hand.push(goblin_id);

    // Set up priority
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Apply CastSpell action for Goblin Guide (a creature spell)
    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: goblin_id,
            targets: vec![],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Stack should have exactly 1 item (the spell, no trigger)
    assert_eq!(
        state.stack.len(),
        1,
        "Casting a creature should not trigger Young Pyromancer, got {} items",
        state.stack.len()
    );
}

#[test]
fn test_monastery_mentor_triggers_on_noncreature_spell() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Monastery Mentor on the battlefield under P0's control
    let mentor_id = state.new_object_id();
    state.card_registry.push((mentor_id, CardName::MonasteryMentor));
    let def = find_card(&db, CardName::MonasteryMentor).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        mentor_id, CardName::MonasteryMentor, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Give P0 enough mana to cast Lightning Bolt
    state.players[0].mana_pool.red += 1;

    // Put Lightning Bolt in P0's hand
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].hand.push(bolt_id);

    // Set up priority
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Apply CastSpell action
    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
            from_graveyard: false,
                from_library_top: false,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Stack should have both the spell and the trigger
    assert!(
        state.stack.len() >= 2,
        "Stack should have the spell and the Monastery Mentor trigger, got {} items",
        state.stack.len()
    );

    // Resolve all stack items
    state.pass_priority(&db);
    state.pass_priority(&db);
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Monastery Mentor should have created a 1/1 Monk token with Prowess
    let tokens: Vec<_> = state.battlefield.iter().filter(|p| p.is_token).collect();
    assert_eq!(tokens.len(), 1, "Monastery Mentor should create exactly 1 token");
    assert_eq!(tokens[0].power(), 1, "Monk token should be 1/1");
    assert_eq!(tokens[0].toughness(), 1, "Monk token should be 1/1");
    assert!(
        tokens[0].keywords.has(crate::types::Keyword::Prowess),
        "Monk token should have Prowess"
    );
}
