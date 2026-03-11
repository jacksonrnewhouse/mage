use crate::card::*;
use crate::game::*;
use crate::action::*;
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
    state.apply_action(&crate::action::Action::CastSpell { card_id: vault_id, targets: vec![] }, &db);

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
