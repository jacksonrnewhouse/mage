use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::types::*;

/// Helper: place a permanent directly onto the battlefield (with entered_this_turn = false).
fn put_permanent(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

/// Equip `equip_id` to `creature_id` directly (bypassing the stack, for test setup).
fn equip_directly(state: &mut GameState, equip_id: ObjectId, creature_id: ObjectId) {
    state.do_attach_equipment(equip_id, creature_id);
}

// -----------------------------------------------------------------------
// Test: Skullclamp equip grants +1/-1
// -----------------------------------------------------------------------
#[test]
fn test_skullclamp_equip_grants_bonus() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a 2/2 Grizzly Bears equivalent on the field
    let creature_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let equip_id = put_permanent(&mut state, &db, CardName::SkullClamp, 0);

    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let base_power = def.power.unwrap();
    let base_toughness = def.toughness.unwrap();

    equip_directly(&mut state, equip_id, creature_id);

    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(
        creature.power(),
        base_power + 1,
        "Skullclamp should give +1 power"
    );
    assert_eq!(
        creature.toughness(),
        base_toughness - 1,
        "Skullclamp should give -1 toughness"
    );
    assert_eq!(
        creature.attached_to, None,
        "Creature should not have attached_to (it's the host)"
    );
    // Equipment's attached_to should point at creature
    let equip = state.find_permanent(equip_id).unwrap();
    assert_eq!(
        equip.attached_to,
        Some(creature_id),
        "Equipment should track which creature it's attached to"
    );
    // Creature's attachments should include the equipment
    let creature = state.find_permanent(creature_id).unwrap();
    assert!(
        creature.attachments.contains(&equip_id),
        "Creature's attachments should include the equipment"
    );
}

// -----------------------------------------------------------------------
// Test: Batterskull grants +4/+4, vigilance, lifelink
// -----------------------------------------------------------------------
#[test]
fn test_batterskull_equip_grants_bonus() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let equip_id = put_permanent(&mut state, &db, CardName::Batterskull, 0);

    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let base_power = def.power.unwrap();
    let base_toughness = def.toughness.unwrap();

    equip_directly(&mut state, equip_id, creature_id);

    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), base_power + 4, "Batterskull should give +4 power");
    assert_eq!(creature.toughness(), base_toughness + 4, "Batterskull should give +4 toughness");
    assert!(creature.keywords.has(Keyword::Vigilance), "Batterskull grants vigilance");
    assert!(creature.keywords.has(Keyword::Lifelink), "Batterskull grants lifelink");
}

// -----------------------------------------------------------------------
// Test: Equipment falls off (bonuses removed) when equipped creature dies
// -----------------------------------------------------------------------
#[test]
fn test_equipment_detaches_on_creature_death() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let equip_id = put_permanent(&mut state, &db, CardName::SkullClamp, 0);

    equip_directly(&mut state, equip_id, creature_id);

    // Verify it was attached
    assert_eq!(
        state.find_permanent(equip_id).unwrap().attached_to,
        Some(creature_id)
    );

    // Kill the creature
    state.destroy_permanent(creature_id);

    // Equipment should still be on battlefield
    assert!(
        state.find_permanent(equip_id).is_some(),
        "Equipment stays on battlefield when creature dies"
    );

    // Equipment should be unattached
    assert_eq!(
        state.find_permanent(equip_id).unwrap().attached_to,
        None,
        "Equipment should be unattached after host dies"
    );
}

// -----------------------------------------------------------------------
// Test: Skullclamp trigger fires when equipped creature dies (draw 2)
// -----------------------------------------------------------------------
#[test]
fn test_skullclamp_draws_on_equipped_creature_death() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Load some cards into P0's library so they can be drawn
    for _ in 0..10 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Mountain));
        state.players[0].library.push(id);
    }

    let creature_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let equip_id = put_permanent(&mut state, &db, CardName::SkullClamp, 0);

    equip_directly(&mut state, equip_id, creature_id);

    // Kill the creature - this should push a SkullclampDeath trigger onto the stack
    state.destroy_permanent(creature_id);

    // The stack should have a SkullclampDeath triggered ability
    assert!(
        !state.stack.is_empty(),
        "Skullclamp trigger should be on the stack when equipped creature dies"
    );

    // Resolve it
    let hand_before = state.players[0].hand.len();
    state.resolve_top(&db);

    assert_eq!(
        state.players[0].hand.len(),
        hand_before + 2,
        "Skullclamp should draw 2 cards when equipped creature dies"
    );
}

// -----------------------------------------------------------------------
// Test: Equip to a different creature (re-equip) removes old bonuses and applies new ones
// -----------------------------------------------------------------------
#[test]
fn test_reequip_moves_bonuses() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let creature1_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let creature2_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let equip_id = put_permanent(&mut state, &db, CardName::SkullClamp, 0);

    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let base_power = def.power.unwrap();
    let base_toughness = def.toughness.unwrap();

    // Equip to creature 1
    equip_directly(&mut state, equip_id, creature1_id);
    assert_eq!(state.find_permanent(creature1_id).unwrap().power(), base_power + 1);

    // Re-equip to creature 2
    equip_directly(&mut state, equip_id, creature2_id);

    // Creature 1 should have base stats again
    let c1 = state.find_permanent(creature1_id).unwrap();
    assert_eq!(c1.power(), base_power, "Old host should have base power after re-equip");
    assert_eq!(c1.toughness(), base_toughness, "Old host should have base toughness after re-equip");
    assert!(!c1.attachments.contains(&equip_id), "Old host should no longer track equipment");

    // Creature 2 should have bonuses
    let c2 = state.find_permanent(creature2_id).unwrap();
    assert_eq!(c2.power(), base_power + 1, "New host should have +1 power");
    assert_eq!(c2.toughness(), base_toughness - 1, "New host should have -1 toughness");
    assert!(c2.attachments.contains(&equip_id), "New host should track equipment");

    // Equipment should point to creature 2
    assert_eq!(
        state.find_permanent(equip_id).unwrap().attached_to,
        Some(creature2_id)
    );
}

// -----------------------------------------------------------------------
// Test: Equip action is generated in legal_actions at sorcery speed
// -----------------------------------------------------------------------
#[test]
fn test_equip_action_generated() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Sorcery speed setup for player 0
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Give player 0 mana to pay equip cost
    state.players[0].mana_pool.colorless = 5;

    let creature_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 0);
    let equip_id = put_permanent(&mut state, &db, CardName::SkullClamp, 0);

    let actions = state.legal_actions(&db);
    let equip_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, ability_index: 20, .. }
            if *permanent_id == equip_id)
    }).collect();

    assert!(
        !equip_actions.is_empty(),
        "Equip action should be available at sorcery speed"
    );

    // Check the target is our creature
    let targets_creature = equip_actions.iter().any(|a| {
        matches!(a, Action::ActivateAbility { targets, .. }
            if targets.contains(&Target::Object(creature_id)))
    });
    assert!(targets_creature, "Equip action should target our creature");
}
