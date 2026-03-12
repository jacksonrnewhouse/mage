/// Tests for colorless card abilities (#220).
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

fn add_permanent(state: &mut GameState, card_name: CardName, controller: PlayerId, db: &[CardDef]) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    if def.is_changeling {
        perm.creature_types = CreatureType::ALL.to_vec();
    } else {
        perm.creature_types = def.creature_types.to_vec();
    }
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);
    id
}

fn add_to_graveyard(state: &mut GameState, card_name: CardName, player: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].graveyard.push(id);
    id
}

// === Tormod's Crypt ===

#[test]
fn test_tormods_crypt_exile_graveyard() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let crypt_id = add_permanent(&mut state, CardName::TormodsCrypt, 0, &db);

    // Put cards in opponent's graveyard
    add_to_graveyard(&mut state, CardName::SolRing, 1);
    add_to_graveyard(&mut state, CardName::MoxJet, 1);
    assert_eq!(state.players[1].graveyard.len(), 2);

    // Activate Tormod's Crypt targeting player 1
    state.apply_action(&crate::action::Action::ActivateAbility {
        permanent_id: crypt_id,
        ability_index: 0,
        targets: vec![Target::Player(1)],
    }, &db);

    // The ability is on the stack; resolve it
    assert!(!state.stack.is_empty());
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Opponent's graveyard should be exiled
    assert_eq!(state.players[1].graveyard.len(), 0);
    // Crypt should be sacrificed
    assert!(!state.battlefield.iter().any(|p| p.id == crypt_id));
}

// === Soul-Guide Lantern ===

#[test]
fn test_soul_guide_lantern_exile_opponents_graveyard() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let lantern_id = add_permanent(&mut state, CardName::SoulGuideLantern, 0, &db);

    // Put cards in opponent's graveyard
    add_to_graveyard(&mut state, CardName::SolRing, 1);
    add_to_graveyard(&mut state, CardName::MoxJet, 1);
    assert_eq!(state.players[1].graveyard.len(), 2);

    // Activate ability 0: exile each opponent's graveyard
    state.apply_action(&crate::action::Action::ActivateAbility {
        permanent_id: lantern_id,
        ability_index: 0,
        targets: vec![],
    }, &db);

    // Resolve the ability
    assert!(!state.stack.is_empty());
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Opponent's graveyard should be exiled
    assert_eq!(state.players[1].graveyard.len(), 0);
    // Lantern should be sacrificed
    assert!(!state.battlefield.iter().any(|p| p.id == lantern_id));
}

// === Chromatic Star ===

#[test]
fn test_chromatic_star_death_trigger_draws_card() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 a library and hand
    let lib_card_id = state.new_object_id();
    state.card_registry.push((lib_card_id, CardName::Mountain));
    state.players[0].library.push(lib_card_id);

    let hand_count_before = state.players[0].hand.len();

    let star_id = add_permanent(&mut state, CardName::ChromaticStar, 0, &db);

    // Destroy the Chromatic Star (simulates sacrifice)
    state.destroy_permanent(star_id);

    // Should have a triggered ability (ChromaticStarDeath) on the stack
    assert!(!state.stack.is_empty(), "Chromatic Star should trigger on death");

    // Resolve the trigger
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Player 0 should have drawn a card
    assert_eq!(state.players[0].hand.len(), hand_count_before + 1);
}

// === Scrap Trawler ===

#[test]
fn test_scrap_trawler_death_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Scrap Trawler on the battlefield
    let trawler_id = add_permanent(&mut state, CardName::ScrapTrawler, 0, &db);

    // Put a lesser-MV artifact (Sol Ring, CMC 1) in P0's graveyard
    let ring_id = add_to_graveyard(&mut state, CardName::SolRing, 0);

    // Put a bigger artifact (MV 3) on the battlefield and kill it
    let wurm_id = add_permanent(&mut state, CardName::ScrapTrawler, 0, &db);
    // We need a different artifact to die. Let's add a Mox Opal (CMC 0) to graveyard
    // and kill a Sol Ring (CMC 1) to get back the Mox Opal.

    // Actually, let's simplify: Kill the trawler itself (CMC 3).
    // Scrap Trawler triggers on its own death for itself.
    // It should look for artifacts with MV < 3 in graveyard.

    // Clean up the extra trawler
    state.battlefield.retain(|p| p.id != wurm_id);

    // Kill the Scrap Trawler (CMC 3)
    state.destroy_permanent(trawler_id);

    // Should have a triggered ability for Scrap Trawler
    assert!(!state.stack.is_empty(), "Scrap Trawler should trigger on its own death");

    // Resolve the trigger (should return Sol Ring from graveyard to hand)
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Sol Ring should now be in hand
    assert!(state.players[0].hand.contains(&ring_id),
        "Sol Ring should have been returned to hand by Scrap Trawler");
}

// === The Mightstone and Weakstone ===

#[test]
fn test_mightstone_weakstone_etb_draw_mode() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 a library
    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Mountain));
        state.players[0].library.push(id);
    }

    let hand_before = state.players[0].hand.len();

    // Add The Mightstone and Weakstone and trigger ETB
    let mw_id = add_permanent(&mut state, CardName::TheMightstoneAndWeakstone, 0, &db);
    state.handle_etb(CardName::TheMightstoneAndWeakstone, mw_id, 0);

    // Should have a trigger on the stack
    assert!(!state.stack.is_empty(), "Mightstone and Weakstone should have ETB trigger");

    // No opponent creatures exist, so the trigger should default to draw-2 mode (no targets)
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    assert_eq!(state.players[0].hand.len(), hand_before + 2,
        "Mightstone and Weakstone should draw 2 cards when no creature target");
}

#[test]
fn test_mightstone_weakstone_etb_minus_five_mode() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put an opponent creature on the battlefield
    let creature_id = add_permanent(&mut state, CardName::GoblinGuide, 1, &db);

    // Add The Mightstone and Weakstone and trigger ETB
    let mw_id = add_permanent(&mut state, CardName::TheMightstoneAndWeakstone, 0, &db);
    state.handle_etb(CardName::TheMightstoneAndWeakstone, mw_id, 0);

    // Should have a trigger on the stack (with creature target for -5/-5 mode)
    assert!(!state.stack.is_empty(), "Mightstone and Weakstone should have ETB trigger");

    // Resolve the trigger
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // The creature should have -5/-5 applied via temporary effect
    let creature = state.find_permanent(creature_id);
    if let Some(c) = creature {
        assert!(c.power() <= -3, "Goblin Guide should have -5 power modifier (was {} base)", c.base_power);
    }
    // Note: Goblin Guide is 2/2, so after -5/-5 it's -3/-3, which means it dies to SBA
}

// === Golos, Tireless Pilgrim ===

#[test]
fn test_golos_etb_search_land() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put lands in player 0's library
    let island_id = state.new_object_id();
    state.card_registry.push((island_id, CardName::Island));
    state.players[0].library.push(island_id);

    let mountain_id = state.new_object_id();
    state.card_registry.push((mountain_id, CardName::Mountain));
    state.players[0].library.push(mountain_id);

    let bf_before = state.battlefield.len();

    // Add Golos and trigger ETB
    let golos_id = add_permanent(&mut state, CardName::GolosTirelessPilgrim, 0, &db);
    state.handle_etb(CardName::GolosTirelessPilgrim, golos_id, 0);

    // Should have a trigger on the stack
    assert!(!state.stack.is_empty(), "Golos should have ETB trigger");

    // Resolve the trigger — should set up a pending choice
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Should now have a pending choice for land search
    assert!(state.pending_choice.is_some(), "Golos should present land search choice");

    // Choose the island
    state.apply_action(&crate::action::Action::ChooseCard(island_id), &db);

    // Island should be on the battlefield tapped
    let island_on_bf = state.battlefield.iter().find(|p| p.id == island_id);
    assert!(island_on_bf.is_some(), "Chosen land should be on the battlefield");
    assert!(island_on_bf.unwrap().tapped, "Golos-fetched land should enter tapped");
}

// === Manifold Key unblockable ===

#[test]
fn test_manifold_key_unblockable() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let key_id = add_permanent(&mut state, CardName::ManifoldKey, 0, &db);
    let creature_id = add_permanent(&mut state, CardName::GoblinGuide, 0, &db);

    // Give player 0 enough mana for {3}
    state.players[0].mana_pool.colorless = 3;

    // Activate ability 1: make creature unblockable
    state.apply_action(&crate::action::Action::ActivateAbility {
        permanent_id: key_id,
        ability_index: 1,
        targets: vec![Target::Object(creature_id)],
    }, &db);

    // Resolve the ability
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Creature should have CantBeBlocked keyword
    let creature = state.find_permanent(creature_id).unwrap();
    assert!(creature.keywords.has(Keyword::CantBeBlocked),
        "Manifold Key should grant CantBeBlocked");
}

// === Soul-Guide Lantern ETB ===

#[test]
fn test_soul_guide_lantern_etb_exiles_card() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a card in opponent's graveyard
    let gy_card_id = add_to_graveyard(&mut state, CardName::SolRing, 1);
    assert_eq!(state.players[1].graveyard.len(), 1);

    // Add Soul-Guide Lantern and trigger ETB
    let lantern_id = add_permanent(&mut state, CardName::SoulGuideLantern, 0, &db);
    state.handle_etb(CardName::SoulGuideLantern, lantern_id, 0);

    // Should have a trigger on the stack
    assert!(!state.stack.is_empty(), "Soul-Guide Lantern should have ETB trigger");

    // Resolve the trigger
    state.phase = Phase::PreCombatMain;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    // The card should be exiled from the graveyard
    assert_eq!(state.players[1].graveyard.len(), 0,
        "Soul-Guide Lantern ETB should exile a card from graveyard");
    assert!(state.exile.iter().any(|(id, _, _)| *id == gy_card_id),
        "The exiled card should be in the exile zone");
}
