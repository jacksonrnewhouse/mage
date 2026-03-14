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

// -----------------------------------------------------------------------
// Test: Nettlecyst living weapon ETB creates a Germ token and attaches
// -----------------------------------------------------------------------
#[test]
fn test_nettlecyst_living_weapon_creates_germ() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Nettlecyst on the battlefield — it should trigger living weapon ETB
    let nettlecyst_id = {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Nettlecyst));
        let def = find_card(&db, CardName::Nettlecyst).unwrap();
        let perm = crate::permanent::Permanent::new(
            id, CardName::Nettlecyst, 0, 0,
            def.power, def.toughness, None, def.keywords, def.card_types,
        );
        state.battlefield.push(perm);
        // Fire the ETB
        state.handle_etb(CardName::Nettlecyst, id, 0);
        id
    };

    // There should be a Germ token on the battlefield
    let germ = state.battlefield.iter().find(|p| p.card_name == CardName::GermToken);
    assert!(germ.is_some(), "Living weapon should create a Germ token");
    let germ = germ.unwrap();
    let germ_id = germ.id;

    // Germ should be a 0/0 black creature token
    assert!(germ.is_token, "Germ should be a token");
    assert_eq!(germ.base_power, 0, "Germ base power should be 0");
    assert_eq!(germ.base_toughness, 0, "Germ base toughness should be 0");
    assert!(germ.colors.contains(&crate::types::Color::Black), "Germ should be black");

    // Nettlecyst should be attached to the Germ
    let nettlecyst = state.find_permanent(nettlecyst_id).unwrap();
    assert_eq!(
        nettlecyst.attached_to,
        Some(germ_id),
        "Nettlecyst should be attached to the Germ token"
    );

    // Germ's attachments should include Nettlecyst
    let germ = state.find_permanent(germ_id).unwrap();
    assert!(
        germ.attachments.contains(&nettlecyst_id),
        "Germ's attachments should include Nettlecyst"
    );

    // Effective P/T should reflect Nettlecyst bonus (count of artifacts+enchantments)
    // Nettlecyst itself is an artifact, so at minimum the bonus is 1
    let eff_power = state.effective_power(germ_id, &db);
    let eff_toughness = state.effective_toughness(germ_id, &db);
    assert!(
        eff_power >= 1,
        "Germ with Nettlecyst should have effective power >= 1 (got {})", eff_power
    );
    assert!(
        eff_toughness >= 1,
        "Germ with Nettlecyst should have effective toughness >= 1 (got {})", eff_toughness
    );
}

// -----------------------------------------------------------------------
// Test: Batterskull living weapon ETB creates a Germ token and attaches
// -----------------------------------------------------------------------
#[test]
fn test_batterskull_living_weapon_creates_germ() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let batterskull_id = {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Batterskull));
        let def = find_card(&db, CardName::Batterskull).unwrap();
        let perm = crate::permanent::Permanent::new(
            id, CardName::Batterskull, 0, 0,
            def.power, def.toughness, None, def.keywords, def.card_types,
        );
        state.battlefield.push(perm);
        state.handle_etb(CardName::Batterskull, id, 0);
        id
    };

    // There should be a Germ token on the battlefield
    let germ = state.battlefield.iter().find(|p| p.card_name == CardName::GermToken);
    assert!(germ.is_some(), "Living weapon should create a Germ token");
    let germ = germ.unwrap();
    let germ_id = germ.id;

    // Batterskull should be attached to the Germ
    let batterskull = state.find_permanent(batterskull_id).unwrap();
    assert_eq!(
        batterskull.attached_to,
        Some(germ_id),
        "Batterskull should be attached to the Germ token"
    );

    // Germ should have Batterskull's bonuses: +4/+4, vigilance, lifelink
    let germ = state.find_permanent(germ_id).unwrap();
    assert_eq!(germ.power(), 4, "Germ with Batterskull should have 4 power (0 + 4)");
    assert_eq!(germ.toughness(), 4, "Germ with Batterskull should have 4 toughness (0 + 4)");
    assert!(germ.keywords.has(Keyword::Vigilance), "Germ with Batterskull should have vigilance");
    assert!(germ.keywords.has(Keyword::Lifelink), "Germ with Batterskull should have lifelink");
}
