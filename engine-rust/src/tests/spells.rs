use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::permanent::Counters;
use crate::types::*;

#[test]
fn test_fetch_finds_shock_lands() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Deck: Islands and Hallowed Fountain first (bottom of library), then Flooded Strand last
    // (top of library, drawn first since library.pop() takes from end)
    let deck: Vec<CardName> = std::iter::repeat(CardName::Island)
        .take(32)
        .chain(std::iter::once(CardName::HallowedFountain))
        .chain(std::iter::repeat(CardName::Island).take(6))
        .chain(std::iter::once(CardName::FloodedStrand))
        .collect();
    state.load_deck(0, &deck, &db);
    state.load_deck(1, &(vec![CardName::Mountain; 40]), &db);
    state.start_game();

    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Find and play Flooded Strand from hand
    let strand_id = state.players[0]
        .hand
        .iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::FloodedStrand))
        .copied()
        .expect("Flooded Strand should be in hand");
    state.apply_action(&Action::PlayLand(strand_id), &db);

    // Activate Flooded Strand
    let perm_id = state
        .permanents_controlled_by(0)
        .find(|p| p.card_name == CardName::FloodedStrand)
        .expect("Flooded Strand should be on battlefield")
        .id;

    let actions = state.legal_actions(&db);
    let activate = actions.iter().find(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, .. } if *permanent_id == perm_id)
    });
    assert!(activate.is_some(), "Should be able to activate Flooded Strand");

    state.apply_action(activate.unwrap(), &db);

    // Now there should be a pending choice to search the library
    assert!(
        state.pending_choice.is_some(),
        "Should have pending choice after activating fetch land"
    );

    // The searchable options should include Hallowed Fountain (Plains+Island)
    if let Some(choice) = &state.pending_choice {
        if let ChoiceKind::ChooseFromList { options, .. } = &choice.kind {
            let found_fountain = options.iter().any(|&id| {
                state.card_name_for_id(id) == Some(CardName::HallowedFountain)
            });
            assert!(
                found_fountain,
                "Flooded Strand should be able to fetch Hallowed Fountain (Plains+Island)"
            );
        } else {
            panic!("Expected ChooseFromList pending choice");
        }
    }
}

#[test]
fn test_fetch_finds_survey_lands() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Deck: Swamps and Undercity Sewers first (bottom), then Polluted Delta last (top, drawn first)
    let deck: Vec<CardName> = std::iter::repeat(CardName::Swamp)
        .take(32)
        .chain(std::iter::once(CardName::UndercitySewers))
        .chain(std::iter::repeat(CardName::Swamp).take(6))
        .chain(std::iter::once(CardName::PollutedDelta))
        .collect();
    state.load_deck(0, &deck, &db);
    state.load_deck(1, &(vec![CardName::Mountain; 40]), &db);
    state.start_game();

    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Find and play Polluted Delta from hand
    let delta_id = state.players[0]
        .hand
        .iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::PollutedDelta))
        .copied()
        .expect("Polluted Delta should be in hand");
    state.apply_action(&Action::PlayLand(delta_id), &db);

    // Activate Polluted Delta
    let perm_id = state
        .permanents_controlled_by(0)
        .find(|p| p.card_name == CardName::PollutedDelta)
        .expect("Polluted Delta should be on battlefield")
        .id;

    let actions = state.legal_actions(&db);
    let activate = actions.iter().find(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, .. } if *permanent_id == perm_id)
    });
    assert!(activate.is_some(), "Should be able to activate Polluted Delta");

    state.apply_action(activate.unwrap(), &db);

    assert!(
        state.pending_choice.is_some(),
        "Should have pending choice after activating fetch land"
    );

    if let Some(choice) = &state.pending_choice {
        if let ChoiceKind::ChooseFromList { options, .. } = &choice.kind {
            let found_sewers = options.iter().any(|&id| {
                state.card_name_for_id(id) == Some(CardName::UndercitySewers)
            });
            assert!(
                found_sewers,
                "Polluted Delta should be able to fetch Undercity Sewers (Island+Swamp)"
            );
        } else {
            panic!("Expected ChooseFromList pending choice");
        }
    }
}

#[test]
fn test_crop_rotation_searches_for_land() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let forest_id = state.new_object_id();
    let crop_id = state.new_object_id();
    state.card_registry.push((forest_id, CardName::Forest));
    state.card_registry.push((crop_id, CardName::CropRotation));
    state.players[0].hand.push(crop_id);

    // Put a Forest on the battlefield to sacrifice
    let def = find_card(&db, CardName::Forest).unwrap();
    let perm = crate::permanent::Permanent::new(
        forest_id, CardName::Forest, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Put a land in library to find
    let gaea_id = state.new_object_id();
    state.card_registry.push((gaea_id, CardName::GaeasCradle));
    state.players[0].library.push(gaea_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.green = 1;

    // Cast Crop Rotation targeting the Forest to sacrifice
    state.apply_action(
        &Action::CastSpell {
            card_id: crop_id,
            targets: vec![Target::Object(forest_id)],
            x_value: 0,
        },
        &db,
    );

    // Both players pass priority to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Forest should be gone (sacrificed)
    let has_forest = state.battlefield.iter().any(|p| p.card_name == CardName::Forest);
    assert!(!has_forest, "Forest should have been sacrificed");

    // Gaea's Cradle should be on battlefield (via GenericSearch resolution)
    // or we should have a pending choice to search
    let has_cradle = state.battlefield.iter().any(|p| p.card_name == CardName::GaeasCradle);
    assert!(
        has_cradle || state.pending_choice.is_some(),
        "Should have searched for a land (Gaea's Cradle on battlefield) or have pending choice"
    );
}

#[test]
fn test_abrupt_decay_cant_be_countered() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let decay_id = state.new_object_id();
    let counter_id = state.new_object_id();
    state.card_registry.push((decay_id, CardName::AbruptDecay));
    state.card_registry.push((counter_id, CardName::Counterspell));
    state.players[0].hand.push(decay_id);
    state.players[1].hand.push(counter_id);

    // Put Sol Ring on the battlefield as the target for Abrupt Decay
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::SolRing));
    let def = find_card(&db, CardName::SolRing).unwrap();
    let perm = crate::permanent::Permanent::new(
        target_id, CardName::SolRing, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 1;
    state.players[0].mana_pool.green = 1;

    // Cast Abrupt Decay targeting Sol Ring
    state.apply_action(
        &Action::CastSpell {
            card_id: decay_id,
            targets: vec![Target::Object(target_id)],
            x_value: 0,
        },
        &db,
    );
    assert_eq!(state.stack.len(), 1, "Abrupt Decay should be on the stack");

    // Verify the stack item has cant_be_countered set
    let stack_item = state.stack.top().unwrap();
    assert!(stack_item.cant_be_countered, "Abrupt Decay should have cant_be_countered=true");

    // P0 passes priority, P1 tries to counter with Counterspell
    state.pass_priority(&db); // gives priority to P1

    // P1 casts Counterspell targeting Abrupt Decay
    let decay_stack_id = state.stack.items()[0].id;
    state.players[1].mana_pool.blue = 2;
    state.apply_action(
        &Action::CastSpell {
            card_id: counter_id,
            targets: vec![Target::Object(decay_stack_id)],
            x_value: 0,
        },
        &db,
    );
    assert_eq!(state.stack.len(), 2, "Counterspell should be on the stack");

    // Both players pass priority - Counterspell resolves first (LIFO)
    state.pass_priority(&db); // P1 passes
    state.pass_priority(&db); // P0 passes -> Counterspell resolves

    // Counterspell should have resolved and fizzled (Abrupt Decay still on stack)
    assert_eq!(state.stack.len(), 1, "Abrupt Decay should still be on the stack (can't be countered)");

    // Both players pass priority again - Abrupt Decay resolves
    state.pass_priority(&db); // active player passes
    state.pass_priority(&db); // other player passes -> Abrupt Decay resolves

    // Sol Ring should be destroyed
    let has_sol_ring = state.battlefield.iter().any(|p| p.card_name == CardName::SolRing);
    assert!(!has_sol_ring, "Sol Ring should have been destroyed by Abrupt Decay");
}

#[test]
fn test_mana_vault_doesnt_untap() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let vault_id = state.new_object_id();
    state.card_registry.push((vault_id, CardName::ManaVault));
    let def = find_card(&db, CardName::ManaVault).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        vault_id, CardName::ManaVault, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.tapped = true;
    perm.doesnt_untap = true;
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Run untap step as active player 0
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.active_player = 0;
    state.untap_step();

    // Mana Vault should still be tapped
    let vault = state.find_permanent(vault_id).unwrap();
    assert!(vault.tapped, "Mana Vault should NOT untap during untap step");
}

#[test]
fn test_normal_permanent_untaps() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let sol_id = state.new_object_id();
    state.card_registry.push((sol_id, CardName::SolRing));
    let def = find_card(&db, CardName::SolRing).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        sol_id, CardName::SolRing, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.tapped = true;
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.active_player = 0;
    state.untap_step();

    let sol = state.find_permanent(sol_id).unwrap();
    assert!(!sol.tapped, "Sol Ring should untap normally during untap step");
}

#[test]
fn test_mana_vault_etb_sets_doesnt_untap() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 a Mana Vault in hand and mana to cast it
    let vault_id = state.new_object_id();
    state.players[0].hand.push(vault_id);
    state.card_registry.push((vault_id, CardName::ManaVault));
    state.players[0].mana_pool.colorless += 1;

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.action_context = ActionContext::Priority;

    // Cast Mana Vault
    state.apply_action(&crate::action::Action::CastSpell { card_id: vault_id, targets: vec![], x_value: 0 }, &db);

    // Resolve it (pass priority twice)
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Check that it's on the battlefield with doesnt_untap set
    let vault = state.find_permanent(vault_id).unwrap();
    assert!(vault.doesnt_untap, "Mana Vault should have doesnt_untap set after ETB");
}

#[test]
fn test_shock_land_enters_tapped_if_no_life_paid() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let fountain_id = state.new_object_id();
    state.card_registry.push((fountain_id, CardName::HallowedFountain));
    state.players[0].hand.push(fountain_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    state.apply_action(&Action::PlayLand(fountain_id), &db);

    assert!(state.pending_choice.is_some(), "Should have pending choice for shock land");

    // Choose 0 = enter tapped (no life paid)
    state.apply_action(&Action::ChooseNumber(0), &db);

    let fountain = state.find_permanent(fountain_id).expect("Hallowed Fountain should be on battlefield");
    assert!(fountain.tapped, "Hallowed Fountain should enter tapped when player chooses not to pay 2 life");
}

#[test]
fn test_shock_land_enters_untapped_if_life_paid() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let fountain_id = state.new_object_id();
    state.card_registry.push((fountain_id, CardName::HallowedFountain));
    state.players[0].hand.push(fountain_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let life_before = state.players[0].life;

    state.apply_action(&Action::PlayLand(fountain_id), &db);
    assert!(state.pending_choice.is_some(), "Should have pending choice for shock land");

    // Choose 1 = pay 2 life, enter untapped
    state.apply_action(&Action::ChooseNumber(1), &db);

    let fountain = state.find_permanent(fountain_id).expect("Hallowed Fountain should be on battlefield");
    assert!(!fountain.tapped, "Hallowed Fountain should enter untapped when player pays 2 life");
    assert_eq!(state.players[0].life, life_before - 2, "Player should have paid 2 life");
}

#[test]
fn test_shock_land_choice_covers_all_ten() {
    let db = build_card_db();
    let shock_lands = [
        CardName::HallowedFountain,
        CardName::WateryGrave,
        CardName::BloodCrypt,
        CardName::StompingGround,
        CardName::TempleGarden,
        CardName::GodlessShrine,
        CardName::SteamVents,
        CardName::OvergrownTomb,
        CardName::SacredFoundry,
        CardName::BreedingPool,
    ];
    for card_name in shock_lands {
        let mut state = GameState::new_two_player();
        let card_id = state.new_object_id();
        state.card_registry.push((card_id, card_name));
        state.players[0].hand.push(card_id);
        state.turn_number = 1;
        state.phase = Phase::PreCombatMain;
        state.step = None;
        state.active_player = 0;
        state.priority_player = 0;
        state.apply_action(&Action::PlayLand(card_id), &db);
        assert!(
            state.pending_choice.is_some(),
            "{:?} should trigger a pending choice on ETB",
            card_name
        );
    }
}

#[test]
fn test_memory_lapse_puts_on_top() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let bolt_id = state.new_object_id();
    let lapse_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.card_registry.push((lapse_id, CardName::MemoryLapse));
    state.players[0].hand.push(bolt_id);
    state.players[1].hand.push(lapse_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.red = 1;
    state.players[1].mana_pool.blue = 2;

    // P0 casts Bolt
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
        },
        &db,
    );
    let bolt_stack_id = state.stack.top().unwrap().id;

    state.pass_priority(&db);

    // P1 casts Memory Lapse targeting the bolt
    state.apply_action(
        &Action::CastSpell {
            card_id: lapse_id,
            targets: vec![Target::Object(bolt_stack_id)],
            x_value: 0,
        },
        &db,
    );

    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bolt should be on top of P0's library, not in graveyard
    let top_of_library = state.players[0].library.last().copied();
    assert_eq!(
        state.card_name_for_id(top_of_library.unwrap()),
        Some(CardName::LightningBolt),
        "Memory Lapse should put countered spell on top of library"
    );
    assert!(
        !state.players[0].graveyard.contains(&bolt_id),
        "Countered spell should NOT be in graveyard"
    );
}

#[test]
fn test_remand_returns_to_hand_and_draws() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let bolt_id = state.new_object_id();
    let remand_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.card_registry.push((remand_id, CardName::Remand));
    state.players[0].hand.push(bolt_id);
    state.players[1].hand.push(remand_id);

    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[1].library.push(id);
    }

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.red = 1;
    state.players[1].mana_pool.blue = 1;
    state.players[1].mana_pool.colorless = 1;

    let p1_hand_size = state.players[1].hand.len();

    // P0 casts Bolt
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
        },
        &db,
    );
    let bolt_stack_id = state.stack.top().unwrap().id;
    state.pass_priority(&db);

    // P1 casts Remand
    state.apply_action(
        &Action::CastSpell {
            card_id: remand_id,
            targets: vec![Target::Object(bolt_stack_id)],
            x_value: 0,
        },
        &db,
    );
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bolt should be back in P0's hand
    assert!(state.players[0].hand.contains(&bolt_id), "Remand should return spell to hand");
    // P1 should have drawn a card (hand size: original - remand + 1 draw)
    assert_eq!(state.players[1].hand.len(), p1_hand_size, "Remand controller should draw a card");
}

#[test]
fn test_sheoldreds_edict_forces_opponent_to_sacrifice_creature() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 casts Sheoldred's Edict, P1 has a Goblin Guide on the battlefield
    let edict_id = state.new_object_id();
    let goblin_id = state.new_object_id();
    state.card_registry.push((edict_id, CardName::SheoldredsEdict));
    state.card_registry.push((goblin_id, CardName::GoblinGuide));
    state.players[0].hand.push(edict_id);

    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        goblin_id, CardName::GoblinGuide, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Give P0 enough mana (BB = 2 black)
    state.players[0].mana_pool.black = 2;

    // Cast Sheoldred's Edict targeting P1
    state.apply_action(
        &Action::CastSpell {
            card_id: edict_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
        },
        &db,
    );
    assert_eq!(state.stack.len(), 1, "Edict should be on the stack");

    // Both players pass priority to resolve
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes -> resolves

    // Should now have a pending choice for P1 to pick which creature to sacrifice
    assert!(
        state.pending_choice.is_some(),
        "Edict should create a pending choice for the opponent"
    );

    // Verify the choice is for P1 and contains the Goblin Guide
    if let Some(ref choice) = state.pending_choice {
        assert_eq!(choice.player, 1, "P1 should be the one making the sacrifice choice");
        if let crate::game::ChoiceKind::ChooseFromList { options, .. } = &choice.kind {
            assert!(
                options.contains(&goblin_id),
                "Goblin Guide should be in the options to sacrifice"
            );
        } else {
            panic!("Expected ChooseFromList choice kind");
        }
    }

    // P1 chooses to sacrifice the Goblin Guide
    state.apply_action(&Action::ChooseCard(goblin_id), &db);

    // Goblin Guide should now be in P1's graveyard, not on the battlefield
    let goblin_on_bf = state.battlefield.iter().any(|p| p.id == goblin_id);
    assert!(!goblin_on_bf, "Goblin Guide should have been sacrificed");

    let goblin_in_gy = state.players[1].graveyard.contains(&goblin_id);
    assert!(goblin_in_gy, "Goblin Guide should be in P1's graveyard after sacrifice");
}

#[test]
fn test_sheoldreds_edict_no_effect_when_opponent_has_no_creatures() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 casts Sheoldred's Edict, P1 has NO creatures
    let edict_id = state.new_object_id();
    state.card_registry.push((edict_id, CardName::SheoldredsEdict));
    state.players[0].hand.push(edict_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 2;

    let p1_bf_before = state.battlefield.iter().filter(|p| p.controller == 1).count();

    // Cast Sheoldred's Edict targeting P1
    state.apply_action(
        &Action::CastSpell {
            card_id: edict_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
        },
        &db,
    );

    // Both players pass priority to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // No pending choice should exist (no creatures to sacrifice)
    assert!(
        state.pending_choice.is_none(),
        "No pending choice when opponent has no creatures"
    );

    // P1's battlefield should be unchanged
    let p1_bf_after = state.battlefield.iter().filter(|p| p.controller == 1).count();
    assert_eq!(
        p1_bf_before, p1_bf_after,
        "P1's battlefield should be unchanged when they have no creatures"
    );
}

#[test]
fn test_create_treasure_token() {
    let _db = build_card_db();
    let mut state = GameState::new_two_player();

    // Initially no permanents
    assert_eq!(state.battlefield.len(), 0);

    // Create a treasure token for player 0
    let token_id = state.create_treasure_token(0);

    // Token should be on the battlefield
    assert_eq!(state.battlefield.len(), 1);
    let token = state.find_permanent(token_id).unwrap();
    assert_eq!(token.card_name, CardName::TreasureToken);
    assert_eq!(token.controller, 0);
    assert!(token.is_token, "Treasure should be a token");
    assert!(token.is_artifact(), "Treasure should be an artifact");
    assert!(!token.is_creature(), "Treasure should not be a creature");
}

#[test]
fn test_sacrifice_treasure_for_mana() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Create a treasure token for player 0
    let token_id = state.create_treasure_token(0);

    // Verify it appears as an activatable ability
    let actions = state.legal_actions(&db);
    let has_activate = actions.iter().any(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, ability_index: 0, .. }
            if *permanent_id == token_id)
    });
    assert!(has_activate, "Should be able to sacrifice treasure token");

    // Sacrifice the treasure (ability_index 0)
    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: token_id,
            ability_index: 0,
            targets: vec![],
        },
        &db,
    );

    // Should now have a pending color choice
    assert!(
        state.pending_choice.is_some(),
        "Should have a pending color choice after sacrificing Treasure"
    );
    if let Some(ref choice) = state.pending_choice {
        assert!(matches!(
            choice.kind,
            crate::game::ChoiceKind::ChooseColor { .. }
        ));
    }

    // Token should be gone from battlefield
    assert!(
        state.find_permanent(token_id).is_none(),
        "Treasure token should be removed after sacrifice"
    );

    // Choose green mana
    state.apply_action(&Action::ChooseColor(Color::Green), &db);

    // Player 0 should now have 1 green mana
    assert_eq!(
        state.players[0].mana_pool.green, 1,
        "Should have 1 green mana after sacrificing Treasure for green"
    );

    // Token should not be in any graveyard (tokens cease to exist)
    let in_gy = state.players[0].graveyard.iter().any(|&id| id == token_id)
        || state.players[1].graveyard.iter().any(|&id| id == token_id);
    assert!(!in_gy, "Treasure token should not end up in any graveyard");
}

#[test]
fn test_generous_plunderer_etb_creates_treasures_for_both() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Give P0 enough mana and put GenerousPlunderer in hand
    let plunderer_id = state.new_object_id();
    state.card_registry.push((plunderer_id, CardName::GenerousPlunderer));
    state.players[0].hand.push(plunderer_id);
    state.players[0].mana_pool.red = 2;

    let bf_before = state.battlefield.len();

    // Cast GenerousPlunderer
    state.apply_action(
        &Action::CastSpell {
            card_id: plunderer_id,
            targets: vec![],
            x_value: 0,
        },
        &db,
    );
    // Both players pass priority to resolve the spell
    state.pass_priority(&db);
    state.pass_priority(&db);

    // The permanent itself plus 2 treasure tokens (one per player)
    let bf_after = state.battlefield.len();
    assert_eq!(
        bf_after,
        bf_before + 3,
        "GenerousPlunderer + 2 Treasure tokens should be on battlefield"
    );

    // Verify there are exactly 2 treasure tokens (one for each player)
    let p0_treasures = state.battlefield.iter()
        .filter(|p| p.card_name == CardName::TreasureToken && p.controller == 0)
        .count();
    let p1_treasures = state.battlefield.iter()
        .filter(|p| p.card_name == CardName::TreasureToken && p.controller == 1)
        .count();
    assert_eq!(p0_treasures, 1, "P0 should have 1 Treasure token");
    assert_eq!(p1_treasures, 1, "P1 should have 1 Treasure token");
}

#[test]
fn test_treasure_lockdown_under_null_rod() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Create a Treasure token for player 0
    let token_id = state.create_treasure_token(0);

    // Put a Null Rod on the battlefield (belonging to player 1)
    let rod_id = state.new_object_id();
    state.card_registry.push((rod_id, CardName::NullRod));
    let def = find_card(&db, CardName::NullRod).unwrap();
    let rod_perm = crate::permanent::Permanent::new(
        rod_id, CardName::NullRod, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(rod_perm);

    // Should NOT be able to sacrifice the treasure under Null Rod
    let actions = state.legal_actions(&db);
    let has_activate = actions.iter().any(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, .. }
            if *permanent_id == token_id)
    });
    assert!(
        !has_activate,
        "Treasure token ability should be locked down by Null Rod"
    );
}

// === Sacrifice-as-cost spell tests ===

#[test]
fn test_village_rites_sacrifices_creature_and_draws() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let rites_id = state.new_object_id();
    let creature_id = state.new_object_id();
    state.card_registry.push((rites_id, CardName::VillageRites));
    state.card_registry.push((creature_id, CardName::DarkConfidant));
    state.players[0].hand.push(rites_id);

    // Put a creature on the battlefield for player 0
    let def = find_card(&db, CardName::DarkConfidant).unwrap();
    let perm = crate::permanent::Permanent::new(
        creature_id, CardName::DarkConfidant, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Populate library so draw works
    for _ in 0..5 {
        let card_id = state.new_object_id();
        state.card_registry.push((card_id, CardName::Swamp));
        state.players[0].library.push(card_id);
    }

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 1;

    // Village Rites should generate a target (the creature)
    let actions = state.legal_actions(&db);
    let cast_action = actions.iter().find(|a| {
        matches!(a, Action::CastSpell { card_id, targets, .. }
            if *card_id == rites_id && targets.len() == 1
            && matches!(targets[0], Target::Object(id) if id == creature_id))
    });
    assert!(cast_action.is_some(), "Should be able to cast Village Rites targeting the creature");

    // Apply the cast action
    state.apply_action(cast_action.unwrap(), &db);

    // Both players pass priority to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // The creature should be gone (sacrificed on resolution)
    let has_creature = state.battlefield.iter().any(|p| p.card_name == CardName::DarkConfidant);
    assert!(!has_creature, "Dark Confidant should have been sacrificed by Village Rites");

    // Player 0 should have drawn 2 cards
    // Initial hand was [rites_id] (1 card), cast removes it (-1), draw 2 (+2) = 2 cards
    assert_eq!(
        state.players[0].hand.len(), 2,
        "Player should have drawn 2 cards from Village Rites"
    );
}

#[test]
fn test_village_rites_requires_creature_to_cast() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let rites_id = state.new_object_id();
    state.card_registry.push((rites_id, CardName::VillageRites));
    state.players[0].hand.push(rites_id);

    // No creatures on battlefield
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 1;

    // Village Rites should NOT be castable without a creature
    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == rites_id)
    });
    assert!(!can_cast, "Village Rites should not be castable without a creature to sacrifice");
}

#[test]
fn test_deadly_dispute_sacrifices_artifact_draws_and_creates_treasure() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let dispute_id = state.new_object_id();
    let artifact_id = state.new_object_id();
    state.card_registry.push((dispute_id, CardName::DeadlyDispute));
    state.card_registry.push((artifact_id, CardName::SolRing));
    state.players[0].hand.push(dispute_id);

    // Put Sol Ring on the battlefield for player 0
    let def = find_card(&db, CardName::SolRing).unwrap();
    let perm = crate::permanent::Permanent::new(
        artifact_id, CardName::SolRing, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Populate library so draw works
    for _ in 0..5 {
        let card_id = state.new_object_id();
        state.card_registry.push((card_id, CardName::Swamp));
        state.players[0].library.push(card_id);
    }

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 1;
    state.players[0].mana_pool.colorless = 1;

    // Deadly Dispute should generate a target (the artifact)
    let actions = state.legal_actions(&db);
    let cast_action = actions.iter().find(|a| {
        matches!(a, Action::CastSpell { card_id, targets, .. }
            if *card_id == dispute_id && targets.len() == 1
            && matches!(targets[0], Target::Object(id) if id == artifact_id))
    });
    assert!(cast_action.is_some(), "Should be able to cast Deadly Dispute targeting Sol Ring");

    let initial_hand_size = state.players[0].hand.len(); // 1 (just dispute_id)

    // Apply the cast action
    state.apply_action(cast_action.unwrap(), &db);

    // Both players pass priority to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Sol Ring should be gone (sacrificed)
    let has_sol_ring = state.battlefield.iter().any(|p| p.card_name == CardName::SolRing);
    assert!(!has_sol_ring, "Sol Ring should have been sacrificed by Deadly Dispute");

    // Player 0 should have drawn 2 cards (hand was empty after casting, now +2)
    let hand_after = state.players[0].hand.len();
    assert_eq!(
        hand_after,
        initial_hand_size - 1 + 2,  // cast spell (-1) then draw 2 (+2)
        "Player should have drawn 2 cards from Deadly Dispute (had {}, now {})",
        initial_hand_size, hand_after
    );

    // A Treasure token should be on the battlefield
    let has_treasure = state.battlefield.iter().any(|p| p.card_name == CardName::TreasureToken);
    assert!(has_treasure, "Deadly Dispute should have created a Treasure token");
}

#[test]
fn test_deadly_dispute_requires_sacrifice_target() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let dispute_id = state.new_object_id();
    state.card_registry.push((dispute_id, CardName::DeadlyDispute));
    state.players[0].hand.push(dispute_id);

    // No artifacts or creatures on battlefield
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 1;
    state.players[0].mana_pool.colorless = 1;

    // Deadly Dispute should NOT be castable without a valid sacrifice target
    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == dispute_id)
    });
    assert!(!can_cast, "Deadly Dispute should not be castable without an artifact or creature to sacrifice");
}

#[test]
fn test_shrapnel_blast_sacrifices_artifact_and_deals_damage() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let blast_id = state.new_object_id();
    let artifact_id = state.new_object_id();
    state.card_registry.push((blast_id, CardName::ShrapnelBlast));
    state.card_registry.push((artifact_id, CardName::SolRing));
    state.players[0].hand.push(blast_id);

    // Put Sol Ring on the battlefield for player 0
    let def = find_card(&db, CardName::SolRing).unwrap();
    let perm = crate::permanent::Permanent::new(
        artifact_id, CardName::SolRing, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;

    let initial_life_p1 = state.players[1].life;

    // Find the CastSpell action targeting opponent with Sol Ring as sacrifice
    let actions = state.legal_actions(&db);
    let cast_action = actions.iter().find(|a| {
        matches!(a, Action::CastSpell { card_id, targets, .. }
            if *card_id == blast_id
            && targets.len() == 2
            && matches!(targets[0], Target::Object(id) if id == artifact_id)
            && matches!(targets[1], Target::Player(1)))
    });
    assert!(cast_action.is_some(), "Should be able to cast Shrapnel Blast sacrificing Sol Ring targeting opponent");

    state.apply_action(cast_action.unwrap(), &db);

    // Both players pass priority
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Sol Ring should be gone (sacrificed)
    let has_sol_ring = state.battlefield.iter().any(|p| p.card_name == CardName::SolRing);
    assert!(!has_sol_ring, "Sol Ring should have been sacrificed by Shrapnel Blast");

    // Opponent should have taken 5 damage
    assert_eq!(
        state.players[1].life,
        initial_life_p1 - 5,
        "Opponent should have taken 5 damage from Shrapnel Blast"
    );
}

#[test]
fn test_shrapnel_blast_requires_artifact() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let blast_id = state.new_object_id();
    state.card_registry.push((blast_id, CardName::ShrapnelBlast));
    state.players[0].hand.push(blast_id);

    // No artifacts on battlefield
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;

    // Shrapnel Blast should NOT be castable without an artifact
    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, .. } if *card_id == blast_id)
    });
    assert!(!can_cast, "Shrapnel Blast should not be castable without an artifact to sacrifice");
}

// ============================================================
// X spell cost tests
// ============================================================

/// Walking Ballista with X=0: enters as 0/0, immediately dies to SBAs.
#[test]
fn test_walking_ballista_x0_dies_immediately() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Set up: give player 0 a Walking Ballista in hand and 0 mana
    let ballista_id = state.new_object_id();
    state.card_registry.push((ballista_id, CardName::WalkingBallista));
    state.players[0].hand.push(ballista_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // 0 mana: can only cast with X=0

    // Should be able to cast with X=0 (base cost {X}{X} = 0 when X=0)
    let actions = state.legal_actions(&db);
    let can_cast_x0 = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, x_value, .. }
            if *card_id == ballista_id && *x_value == 0)
    });
    assert!(can_cast_x0, "Should be able to cast Walking Ballista with X=0");

    // Cast with X=0
    state.apply_action(
        &Action::CastSpell {
            card_id: ballista_id,
            targets: vec![],
            x_value: 0,
        },
        &db,
    );
    assert_eq!(state.stack.len(), 1, "Ballista should be on the stack");

    // Resolve: both players pass priority
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Ballista enters as 0/0, SBAs should kill it
    let on_battlefield = state.battlefield.iter().any(|p| p.card_name == CardName::WalkingBallista);
    assert!(!on_battlefield, "0/0 Walking Ballista should die immediately to SBAs");
}

/// Walking Ballista with X=3: enters with 3 +1/+1 counters, costs {X}{X}=6 mana.
#[test]
fn test_walking_ballista_x3_enters_with_counters() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let ballista_id = state.new_object_id();
    state.card_registry.push((ballista_id, CardName::WalkingBallista));
    state.players[0].hand.push(ballista_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Give 6 generic mana so X=3 is affordable (X*2 = 6)
    state.players[0].mana_pool.colorless = 6;

    // Legal actions should include casting with X=0, X=1, X=2, X=3
    let actions = state.legal_actions(&db);
    let can_cast_x3 = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, x_value, .. }
            if *card_id == ballista_id && *x_value == 3)
    });
    assert!(can_cast_x3, "Should be able to cast Walking Ballista with X=3");

    // Cast with X=3 (costs 6 generic mana)
    state.apply_action(
        &Action::CastSpell {
            card_id: ballista_id,
            targets: vec![],
            x_value: 3,
        },
        &db,
    );
    assert_eq!(state.players[0].mana_pool.colorless, 0, "Should have spent 6 mana");

    // Both players pass priority to resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Ballista should be on the battlefield with 3 +1/+1 counters
    let ballista_perm = state.battlefield.iter()
        .find(|p| p.card_name == CardName::WalkingBallista);
    assert!(ballista_perm.is_some(), "Walking Ballista should be on the battlefield");

    let perm = ballista_perm.unwrap();
    assert_eq!(perm.power(), 3, "Walking Ballista should be 3/3 with X=3");
    assert_eq!(perm.toughness(), 3, "Walking Ballista should be 3/3 with X=3");
    assert_eq!(
        perm.counters.get(CounterType::PlusOnePlusOne),
        3,
        "Walking Ballista should have 3 +1/+1 counters"
    );
}

/// Stonecoil Serpent with X=4: enters with 4 +1/+1 counters, costs {X}=4 mana.
#[test]
fn test_stonecoil_serpent_x4_enters_with_counters() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let serpent_id = state.new_object_id();
    state.card_registry.push((serpent_id, CardName::StonecoilSerpent));
    state.players[0].hand.push(serpent_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Give 4 colorless mana (X=4, costs {X}=4)
    state.players[0].mana_pool.colorless = 4;

    // Cast with X=4
    state.apply_action(
        &Action::CastSpell {
            card_id: serpent_id,
            targets: vec![],
            x_value: 4,
        },
        &db,
    );
    assert_eq!(state.players[0].mana_pool.colorless, 0, "Should have spent 4 mana");

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    let perm = state.battlefield.iter()
        .find(|p| p.card_name == CardName::StonecoilSerpent)
        .expect("Stonecoil Serpent should be on the battlefield");

    assert_eq!(perm.power(), 4, "Stonecoil Serpent should be 4/4 with X=4");
    assert_eq!(perm.toughness(), 4, "Stonecoil Serpent should be 4/4 with X=4");
}

/// Chalice of the Void with X=1: enters with 1 charge counter.
#[test]
fn test_chalice_of_the_void_x1_enters_with_charge_counter() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let chalice_id = state.new_object_id();
    state.card_registry.push((chalice_id, CardName::ChaliceOfTheVoid));
    state.players[0].hand.push(chalice_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Give 2 colorless mana (X=1, costs {X}{X}=2)
    state.players[0].mana_pool.colorless = 2;

    let actions = state.legal_actions(&db);
    let can_cast_x1 = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, x_value, .. }
            if *card_id == chalice_id && *x_value == 1)
    });
    assert!(can_cast_x1, "Should be able to cast Chalice with X=1");

    state.apply_action(
        &Action::CastSpell {
            card_id: chalice_id,
            targets: vec![],
            x_value: 1,
        },
        &db,
    );

    state.pass_priority(&db);
    state.pass_priority(&db);

    let perm = state.battlefield.iter()
        .find(|p| p.card_name == CardName::ChaliceOfTheVoid)
        .expect("Chalice should be on the battlefield");

    assert_eq!(
        perm.counters.get(CounterType::Charge),
        1,
        "Chalice should have 1 charge counter when cast with X=1"
    );
}

/// X spell movegen: cannot cast with X values requiring more mana than available.
#[test]
fn test_x_spell_movegen_respects_mana_limit() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let ballista_id = state.new_object_id();
    state.card_registry.push((ballista_id, CardName::WalkingBallista));
    state.players[0].hand.push(ballista_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Only 4 mana: max X for Walking Ballista ({X}{X}) is 2
    state.players[0].mana_pool.colorless = 4;

    let actions = state.legal_actions(&db);
    let x_values: Vec<u8> = actions.iter().filter_map(|a| {
        if let Action::CastSpell { card_id, x_value, .. } = a {
            if *card_id == ballista_id { Some(*x_value) } else { None }
        } else {
            None
        }
    }).collect();

    // Should be able to cast with X=0, 1, 2 but not X=3 (would cost 6)
    assert!(x_values.contains(&0), "Should be able to cast with X=0");
    assert!(x_values.contains(&1), "Should be able to cast with X=1");
    assert!(x_values.contains(&2), "Should be able to cast with X=2");
    assert!(!x_values.contains(&3), "Should NOT be able to cast with X=3 (costs 6, only have 4)");
}
