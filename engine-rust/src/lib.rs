/// mage-engine: High-performance Magic: The Gathering engine for game tree search.
///
/// Focused on Vintage Supreme Draft format cards. Designed for:
/// - Fast state cloning (all owned data, no references)
/// - Efficient move generation
/// - Both MCTS and alpha-beta search support
///
/// # Architecture
///
/// - `types`: Core type definitions (ObjectId, PlayerId, enums)
/// - `mana`: Mana pool and cost system
/// - `card`: Static card definitions database
/// - `permanent`: Runtime permanent state on battlefield
/// - `player`: Player state (life, hand, library, graveyard)
/// - `game`: Central game state (Clone for search)
/// - `stack`: The stack (spells and abilities)
/// - `action`: Legal actions / move types
/// - `combat`: Combat damage resolution
/// - `movegen`: Move generation (legal_actions, apply_action)
/// - `search`: Search algorithms (MCTS, alpha-beta) and evaluation

pub mod types;
pub mod mana;
pub mod card;
pub mod permanent;
pub mod player;
pub mod game;
pub mod stack;
pub mod action;
pub mod combat;
pub mod movegen;
pub mod search;

#[cfg(test)]
mod tests {
    use crate::card::*;
    use crate::game::*;
    use crate::action::*;
    use crate::types::*;

    fn setup_simple_game() -> (GameState, Vec<CardDef>) {
        let db = build_card_db();
        let mut state = GameState::new_two_player();

        // Player 0: interleave Mountains with spells so hand has both
        // Library is LIFO, so last cards added are drawn first
        let p0_deck: Vec<CardName> = std::iter::repeat(CardName::GoblinGuide)
            .take(10)
            .chain(std::iter::repeat(CardName::LightningBolt).take(10))
            .chain(std::iter::repeat(CardName::Mountain).take(4))
            .chain(std::iter::repeat(CardName::LightningBolt).take(3))
            .chain(std::iter::repeat(CardName::Mountain).take(13))
            .collect();
        state.load_deck(0, &p0_deck, &db);

        // Player 1: same approach
        let p1_deck: Vec<CardName> = std::iter::repeat(CardName::AncestralRecall)
            .take(10)
            .chain(std::iter::repeat(CardName::Counterspell).take(10))
            .chain(std::iter::repeat(CardName::Island).take(4))
            .chain(std::iter::repeat(CardName::Counterspell).take(3))
            .chain(std::iter::repeat(CardName::Island).take(13))
            .collect();
        state.load_deck(1, &p1_deck, &db);

        state.start_game();
        // Hand now has: 4 Mountains + 3 Lightning Bolts for P0
        //               4 Islands + 3 Counterspells for P1
        (state, db)
    }

    #[test]
    fn test_game_starts_correctly() {
        let (state, _db) = setup_simple_game();
        assert_eq!(state.players[0].hand.len(), 7);
        assert_eq!(state.players[1].hand.len(), 7);
        assert_eq!(state.players[0].life, 20);
        assert_eq!(state.players[1].life, 20);
        assert_eq!(state.turn_number, 1);
        assert_eq!(state.active_player, 0);
    }

    #[test]
    fn test_legal_actions_include_pass() {
        let (state, db) = setup_simple_game();
        let actions = state.legal_actions(&db);
        assert!(actions.contains(&Action::PassPriority));
    }

    #[test]
    fn test_play_land() {
        let (mut state, db) = setup_simple_game();

        // Advance to main phase
        state.phase = Phase::PreCombatMain;
        state.step = None;

        let actions = state.legal_actions(&db);
        let land_actions: Vec<_> = actions
            .iter()
            .filter(|a| matches!(a, Action::PlayLand(_)))
            .collect();

        // Should be able to play a land
        assert!(!land_actions.is_empty(), "Should have land play actions");

        // Play a land
        if let Some(action) = land_actions.first() {
            state.apply_action(action, &db);
        }

        // Should have one land on battlefield
        assert_eq!(
            state.permanents_controlled_by(0).count(),
            1,
            "Should have 1 permanent"
        );

        // Should have 6 cards in hand
        assert_eq!(state.players[0].hand.len(), 6);
    }

    #[test]
    fn test_tap_land_for_mana() {
        let (mut state, db) = setup_simple_game();
        state.phase = Phase::PreCombatMain;
        state.step = None;

        // Play a Mountain
        let mountain_id = state.players[0]
            .hand
            .iter()
            .find(|&&id| {
                state.card_name_for_id(id) == Some(CardName::Mountain)
            })
            .copied();

        if let Some(id) = mountain_id {
            state.apply_action(&Action::PlayLand(id), &db);

            // Tap for red mana
            let perm_id = state.permanents_controlled_by(0).next().unwrap().id;
            state.apply_action(
                &Action::ActivateManaAbility {
                    permanent_id: perm_id,
                    color_choice: Some(Color::Red),
                },
                &db,
            );

            assert_eq!(state.players[0].mana_pool.red, 1);
        }
    }

    #[test]
    fn test_cast_lightning_bolt() {
        let db = build_card_db();
        let mut state = GameState::new_two_player();

        // Manually set up a specific hand: 1 Mountain, 1 Lightning Bolt
        let mountain_id = state.new_object_id();
        let bolt_id = state.new_object_id();
        state.card_registry.push((mountain_id, CardName::Mountain));
        state.card_registry.push((bolt_id, CardName::LightningBolt));
        state.players[0].hand.push(mountain_id);
        state.players[0].hand.push(bolt_id);

        state.turn_number = 1;
        state.phase = Phase::PreCombatMain;
        state.step = None;
        state.active_player = 0;
        state.priority_player = 0;

        // Play Mountain
        state.apply_action(&Action::PlayLand(mountain_id), &db);
        assert_eq!(state.permanents_controlled_by(0).count(), 1);

        // Tap Mountain for red mana
        let perm_id = state.permanents_controlled_by(0).next().unwrap().id;
        state.apply_action(
            &Action::ActivateManaAbility {
                permanent_id: perm_id,
                color_choice: Some(Color::Red),
            },
            &db,
        );
        assert_eq!(state.players[0].mana_pool.red, 1);

        // Cast Lightning Bolt targeting opponent
        state.apply_action(
            &Action::CastSpell {
                card_id: bolt_id,
                targets: vec![Target::Player(1)],
            },
            &db,
        );
        assert_eq!(state.stack.len(), 1);
        assert_eq!(state.players[0].mana_pool.red, 0); // Mana spent

        // Both players pass priority to resolve
        state.pass_priority(&db); // P0 passes
        state.pass_priority(&db); // P1 passes -> resolves

        // After resolution, opponent should have taken 3 damage
        assert_eq!(state.players[1].life, 17);
    }

    #[test]
    fn test_state_clone_for_search() {
        let (state, _db) = setup_simple_game();
        let cloned = state.clone();

        // Verify clone is independent
        assert_eq!(state.players[0].life, cloned.players[0].life);
        assert_eq!(state.players[0].hand.len(), cloned.players[0].hand.len());
        assert_eq!(state.turn_number, cloned.turn_number);
    }

    #[test]
    fn test_vintage_power() {
        let db = build_card_db();
        let mut state = GameState::new_two_player();

        // Build a deck with Power 9
        let deck: Vec<CardName> = vec![
            CardName::BlackLotus,
            CardName::MoxSapphire,
            CardName::MoxJet,
            CardName::MoxRuby,
            CardName::MoxPearl,
            CardName::MoxEmerald,
            CardName::AncestralRecall,
            CardName::TimeWalk,
            CardName::SolRing,
            CardName::ManaCrypt,
        ]
        .into_iter()
        .chain(std::iter::repeat(CardName::Island).take(30))
        .collect();

        state.load_deck(0, &deck, &db);
        state.load_deck(1, &(vec![CardName::Mountain; 40]), &db);
        state.start_game();

        // Verify game setup
        assert_eq!(state.players[0].hand.len(), 7);
        assert_eq!(state.players[0].library.len(), 33);
    }

    #[test]
    fn test_creature_combat() {
        let db = build_card_db();
        let mut state = GameState::new_two_player();

        // Put a Goblin Guide on the battlefield for player 0
        let gg_id = state.new_object_id();
        state.card_registry.push((gg_id, CardName::GoblinGuide));
        let def = find_card(&db, CardName::GoblinGuide).unwrap();
        let mut perm = crate::permanent::Permanent::new(
            gg_id,
            CardName::GoblinGuide,
            0,
            0,
            def.power,
            def.toughness,
            None,
            def.keywords,
            def.card_types,
        );
        perm.entered_this_turn = false; // Not summoning sick
        state.battlefield.push(perm);

        // Move to combat
        state.phase = Phase::Combat;
        state.step = Some(Step::DeclareAttackers);
        state.action_context = ActionContext::DeclareAttackers;

        // Declare attacker
        state.apply_action(
            &Action::DeclareAttacker { creature_id: gg_id },
            &db,
        );

        assert_eq!(state.attackers.len(), 1);

        // Confirm attackers
        state.apply_action(&Action::ConfirmAttackers, &db);

        // Confirm blockers (no blockers)
        state.apply_action(&Action::ConfirmBlockers, &db);

        // Resolve combat damage
        state.resolve_combat_damage(false);

        // Opponent should have taken 2 damage (Goblin Guide is 2/2 with haste)
        assert_eq!(state.players[1].life, 18);
    }

    #[test]
    fn test_game_result() {
        let (mut state, _db) = setup_simple_game();
        assert_eq!(state.result, GameResult::InProgress);
        assert!(!state.is_terminal());

        state.players[1].life = 0;
        state.check_state_based_actions();

        assert!(state.is_terminal());
        assert_eq!(state.result, GameResult::Win(0));
    }

    #[test]
    fn test_perft_depth_0() {
        let (state, db) = setup_simple_game();
        let count = crate::search::bench::perft(&state, &db, 0);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_perft_depth_1() {
        let (mut state, db) = setup_simple_game();
        state.phase = Phase::PreCombatMain;
        state.step = None;
        let count = crate::search::bench::perft(&state, &db, 1);
        let actions = state.legal_actions(&db);
        assert_eq!(count, actions.len() as u64);
    }

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
}
