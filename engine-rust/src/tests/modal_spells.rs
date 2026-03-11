use crate::card::*;
use crate::game::*;
use crate::action::*;
use crate::types::*;

/// Helper: set up a two-player game with given decks and advance to PreCombatMain.
fn setup_game(p0_deck: &[CardName], p1_deck: &[CardName]) -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.load_deck(0, p0_deck, &db);
    state.load_deck(1, p1_deck, &db);
    state.start_game();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    (state, db)
}

/// Add mana to player's pool.
fn add_mana(state: &mut GameState, player: PlayerId, black: u8, red: u8, generic: u8) {
    let pool = &mut state.players[player as usize].mana_pool;
    pool.black += black;
    pool.red += red;
    pool.colorless += generic;
}

// ─────────────────────────────────────────────────────────────
// Kolaghan's Command tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_kolaghan_command_modes_generated() {
    // Put Kolaghan's Command in player 0's hand and verify mode-combination actions are generated.
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Swamp)
        .take(32)
        .chain(std::iter::repeat(CardName::Mountain).take(7))
        .chain(std::iter::once(CardName::KolaghanCommand))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::Island; 40];

    let (mut state, db) = setup_game(&p0_deck, &p1_deck);

    // Give player 0 mana: {1}{B}{R}
    add_mana(&mut state, 0, 1, 1, 1);

    let actions = state.legal_actions(&db);
    let modal_actions: Vec<_> = actions.iter().filter(|a| {
        if let Action::CastSpell { modes, .. } = a {
            !modes.is_empty()
        } else {
            false
        }
    }).collect();

    // With no artifacts on battlefield and no creatures in graveyard,
    // valid mode combos are those that don't require unavailable targets:
    //   mode 0 (return from gy) needs graveyard creature — unavailable
    //   mode 2 (destroy artifact) needs an artifact — unavailable
    //   mode 1 (discard) and mode 3 (deal 2 damage) always have targets
    // So only combo (1, 3) is valid (modes 1 and 3).
    assert!(!modal_actions.is_empty(), "Should generate modal spell actions");
    let combo_1_3: Vec<_> = modal_actions.iter().filter(|a| {
        if let Action::CastSpell { modes, .. } = a {
            modes == &[1u8, 3u8]
        } else { false }
    }).collect();
    assert!(!combo_1_3.is_empty(), "Should have mode combo [1,3] (discard + deal 2)");

    // mode 0 requires graveyard creatures — should not appear alone in combos with mode 0
    let combo_0_x: Vec<_> = modal_actions.iter().filter(|a| {
        if let Action::CastSpell { modes, .. } = a {
            modes.contains(&0)
        } else { false }
    }).collect();
    assert!(combo_0_x.is_empty(), "Mode 0 needs graveyard creature — should be absent");
}

#[test]
fn test_kolaghan_command_mode_3_deals_damage() {
    // Mode 3: deal 2 damage to a creature.
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Swamp)
        .take(32)
        .chain(std::iter::repeat(CardName::Mountain).take(7))
        .chain(std::iter::once(CardName::KolaghanCommand))
        .collect();
    let p1_deck: Vec<CardName> = std::iter::repeat(CardName::GoblinGuide)
        .take(1)
        .chain(std::iter::repeat(CardName::Mountain).take(39))
        .collect();

    let (mut state, db) = setup_game(&p0_deck, &p1_deck);

    // Put a GoblinGuide on the battlefield for player 1
    let goblin_id = state.new_object_id();
    state.card_registry.push((goblin_id, CardName::GoblinGuide));
    let goblin = crate::permanent::Permanent::new(
        goblin_id, CardName::GoblinGuide, 1, 1,
        Some(2), Some(2), None, Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(goblin);

    // Give player 0 mana for Kolaghan's Command
    add_mana(&mut state, 0, 1, 1, 1);

    // Find the KolaghanCommand card
    let cmd_id = state.players[0].hand.iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::KolaghanCommand))
        .copied()
        .expect("KolaghanCommand should be in hand");

    // Cast with modes [1, 3]: discard (target player 1) + deal 2 damage to goblin
    state.apply_action(&Action::CastSpell {
        card_id: cmd_id,
        targets: vec![Target::Player(1), Target::Object(goblin_id)],
        x_value: 0,
        from_graveyard: false,
        from_library_top: false,
        alt_cost: None,
        modes: vec![1, 3],
    }, &db);

    // Resolve the spell
    state.resolve_top(&db);

    // GoblinGuide has 2 toughness and takes 2 damage — should be dead (0 health)
    // After SBA it will be removed
    let goblin_alive = state.battlefield.iter().any(|p| p.id == goblin_id);
    assert!(!goblin_alive, "GoblinGuide should be dead after taking 2 damage");
}

#[test]
fn test_kolaghan_command_mode_0_returns_from_graveyard() {
    // Mode 0: return a creature from controller's graveyard to hand.
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Swamp)
        .take(32)
        .chain(std::iter::repeat(CardName::Mountain).take(7))
        .chain(std::iter::once(CardName::KolaghanCommand))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::Island; 40];

    let (mut state, db) = setup_game(&p0_deck, &p1_deck);

    // Place a GoblinGuide card in player 0's graveyard
    let goblin_id = state.new_object_id();
    state.card_registry.push((goblin_id, CardName::GoblinGuide));
    state.players[0].graveyard.push(goblin_id);

    // Give mana
    add_mana(&mut state, 0, 1, 1, 1);

    let cmd_id = state.players[0].hand.iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::KolaghanCommand))
        .copied()
        .expect("KolaghanCommand should be in hand");

    let hand_before = state.players[0].hand.len();

    // Cast with modes [0, 1]: return from graveyard + discard
    // targets: [graveyard_creature, discard_player]
    state.apply_action(&Action::CastSpell {
        card_id: cmd_id,
        targets: vec![Target::Object(goblin_id), Target::Player(1)],
        x_value: 0,
        from_graveyard: false,
        from_library_top: false,
        alt_cost: None,
        modes: vec![0, 1],
    }, &db);

    state.resolve_top(&db);

    // GoblinGuide should be in player 0's hand (returned from graveyard)
    let goblin_in_hand = state.players[0].hand.contains(&goblin_id);
    assert!(goblin_in_hand, "GoblinGuide should be returned to hand from graveyard");
    // Graveyard should no longer contain goblin
    let goblin_in_gy = state.players[0].graveyard.contains(&goblin_id);
    assert!(!goblin_in_gy, "GoblinGuide should not be in graveyard after mode 0");
    // Hand grew by 1 (returned creature, KolaghanCommand went to gy, so net: hand = before - 1 + 1 = before)
    // Actually: cmd went to gy, goblin came in from gy => net 0 change in hand
    assert_eq!(state.players[0].hand.len(), hand_before, "Hand size should be unchanged");
}

#[test]
fn test_kolaghan_command_mode_2_destroys_artifact() {
    // Mode 2: destroy target artifact.
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Swamp)
        .take(32)
        .chain(std::iter::repeat(CardName::Mountain).take(7))
        .chain(std::iter::once(CardName::KolaghanCommand))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::Island; 40];

    let (mut state, db) = setup_game(&p0_deck, &p1_deck);

    // Put a Mox Pearl (artifact) on the battlefield for player 1
    let artifact_id = state.new_object_id();
    state.card_registry.push((artifact_id, CardName::MoxPearl));
    let artifact = crate::permanent::Permanent::new(
        artifact_id, CardName::MoxPearl, 1, 1,
        None, None, None, Keywords::empty(), &[CardType::Artifact],
    );
    state.battlefield.push(artifact);

    add_mana(&mut state, 0, 1, 1, 1);

    let cmd_id = state.players[0].hand.iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::KolaghanCommand))
        .copied()
        .expect("KolaghanCommand should be in hand");

    // Cast with modes [1, 2]: discard + destroy artifact
    state.apply_action(&Action::CastSpell {
        card_id: cmd_id,
        targets: vec![Target::Player(1), Target::Object(artifact_id)],
        x_value: 0,
        from_graveyard: false,
        from_library_top: false,
        alt_cost: None,
        modes: vec![1, 2],
    }, &db);

    state.resolve_top(&db);

    let artifact_exists = state.battlefield.iter().any(|p| p.id == artifact_id);
    assert!(!artifact_exists, "Artifact should be destroyed by mode 2");
}

// ─────────────────────────────────────────────────────────────
// Kozilek's Command tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_kozilek_command_mode_1_creates_token() {
    // Mode 1: create a 0/1 Eldrazi Spawn token (no target needed).
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Island)
        .take(32)
        .chain(std::iter::repeat(CardName::Island).take(7))
        .chain(std::iter::once(CardName::KozileksCommand))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::Island; 40];

    // KozileksCommand costs {2}{C}{C} — give colorless mana
    let (mut state, db) = setup_game(&p0_deck, &p1_deck);
    state.players[0].mana_pool.colorless += 4;

    let cmd_id = state.players[0].hand.iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::KozileksCommand))
        .copied()
        .expect("KozileksCommand should be in hand");

    let battlefield_count_before = state.battlefield.len();

    // Cast with modes [0, 1]: draw 2 + create token. Mode 0 targets player 1.
    state.apply_action(&Action::CastSpell {
        card_id: cmd_id,
        targets: vec![Target::Player(1)],
        x_value: 0,
        from_graveyard: false,
        from_library_top: false,
        alt_cost: None,
        modes: vec![0, 1],
    }, &db);

    state.resolve_top(&db);

    // A 0/1 Eldrazi Spawn token should have been created
    let tokens: Vec<_> = state.battlefield.iter()
        .filter(|p| p.card_name == CardName::EldraziSpawnToken && p.is_token)
        .collect();
    assert_eq!(tokens.len(), 1, "Should have one Eldrazi Spawn token");
    assert_eq!(tokens[0].base_power, 0);
    assert_eq!(tokens[0].base_toughness, 1);

    // Player 1 drew 2 cards and lost 2 life
    assert_eq!(state.players[1].life, 18, "Player 1 should have lost 2 life from mode 0");
}

#[test]
fn test_kozilek_command_mode_3_reduces_pt() {
    // Mode 3: target creature gets -3/-3 until end of turn.
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Island)
        .take(39)
        .chain(std::iter::once(CardName::KozileksCommand))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::GoblinGuide; 1]
        .into_iter()
        .chain(std::iter::repeat(CardName::Mountain).take(39))
        .collect();

    let (mut state, db) = setup_game(&p0_deck, &p1_deck);

    // Put a 4/4 creature on battlefield for player 1
    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let creature = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 1, 1,
        Some(4), Some(4), None, Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(creature);

    // Give player 0 4 colorless mana
    state.players[0].mana_pool.colorless += 4;

    let cmd_id = state.players[0].hand.iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::KozileksCommand))
        .copied()
        .expect("KozileksCommand should be in hand");

    // Cast with modes [1, 3]: create token + -3/-3 to creature
    state.apply_action(&Action::CastSpell {
        card_id: cmd_id,
        targets: vec![Target::Object(creature_id)],
        x_value: 0,
        from_graveyard: false,
        from_library_top: false,
        alt_cost: None,
        modes: vec![1, 3],
    }, &db);

    state.resolve_top(&db);

    // The creature should have -3/-3 applied (now 1/1)
    let creature_perm = state.battlefield.iter().find(|p| p.id == creature_id);
    assert!(creature_perm.is_some(), "Creature should still be alive (4/4 - 3/3 = 1/1)");
    let perm = creature_perm.unwrap();
    assert_eq!(perm.power(), 1, "Creature power should be 1 (4 - 3)");
    assert_eq!(perm.toughness(), 1, "Creature toughness should be 1 (4 - 3)");
}

#[test]
fn test_kolaghan_command_all_six_combos_available() {
    // With all mode targets available, all 6 mode combinations should be offered.
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::Swamp)
        .take(32)
        .chain(std::iter::repeat(CardName::Mountain).take(7))
        .chain(std::iter::once(CardName::KolaghanCommand))
        .collect();
    let p1_deck: Vec<CardName> = vec![CardName::Island; 40];

    let (mut state, db) = setup_game(&p0_deck, &p1_deck);

    // Put a creature in player 0's graveyard (enables mode 0)
    let gy_creature_id = state.new_object_id();
    state.card_registry.push((gy_creature_id, CardName::GoblinGuide));
    state.players[0].graveyard.push(gy_creature_id);

    // Put an artifact on battlefield (enables mode 2)
    let artifact_id = state.new_object_id();
    state.card_registry.push((artifact_id, CardName::MoxPearl));
    let artifact = crate::permanent::Permanent::new(
        artifact_id, CardName::MoxPearl, 1, 1,
        None, None, None, Keywords::empty(), &[CardType::Artifact],
    );
    state.battlefield.push(artifact);

    // Put a creature on battlefield for damage target (mode 3)
    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let creature = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 1, 1,
        Some(2), Some(2), None, Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(creature);

    add_mana(&mut state, 0, 1, 1, 1);

    let actions = state.legal_actions(&db);
    let modal_actions: Vec<_> = actions.iter().filter(|a| {
        matches!(a, Action::CastSpell { modes, .. } if !modes.is_empty())
    }).collect();

    // Should have all 6 combinations: (0,1), (0,2), (0,3), (1,2), (1,3), (2,3)
    let combos: std::collections::HashSet<Vec<u8>> = modal_actions.iter()
        .filter_map(|a| {
            if let Action::CastSpell { modes, .. } = a {
                Some(modes.clone())
            } else { None }
        })
        .collect();

    assert!(combos.contains(&vec![0u8, 1u8]), "Should have combo (0,1)");
    assert!(combos.contains(&vec![0u8, 2u8]), "Should have combo (0,2)");
    assert!(combos.contains(&vec![0u8, 3u8]), "Should have combo (0,3)");
    assert!(combos.contains(&vec![1u8, 2u8]), "Should have combo (1,2)");
    assert!(combos.contains(&vec![1u8, 3u8]), "Should have combo (1,3)");
    assert!(combos.contains(&vec![2u8, 3u8]), "Should have combo (2,3)");
}
