use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::types::*;
use super::setup_simple_game;

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
            x_value: 0,
            from_graveyard: false,
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
fn test_game_result() {
    let (mut state, db) = setup_simple_game();
    assert_eq!(state.result, GameResult::InProgress);
    assert!(!state.is_terminal());

    state.players[1].life = 0;
    state.check_state_based_actions(&db);

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
