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
}
