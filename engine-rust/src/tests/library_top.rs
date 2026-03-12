/// Tests for play-from-top-of-library mechanics:
/// Bolas's Citadel, Future Sight, Mystic Forge, Experimental Frenzy.

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

/// Helper: build a minimal game state in pre-combat main phase with priority on player 0.
fn make_main_phase_state() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    (state, db)
}

/// Place a permanent on the battlefield controlled by the given player.
fn put_permanent(state: &mut GameState, db: &[CardDef], name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    let def = find_card(db, name).expect("card not in db");
    let perm = Permanent::new(
        id, name, controller, controller,
        def.power, def.toughness, def.loyalty,
        def.keywords, def.card_types,
    );
    state.battlefield.push(perm);
    id
}

/// Register a card and push it as the top of a player's library.
fn push_library_top(state: &mut GameState, name: CardName, owner: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, name));
    state.players[owner as usize].library.push(id);
    id
}

// ─── Bolas's Citadel ───────────────────────────────────────────────────────────

/// Bolas's Citadel: generates a CastSpell(from_library_top=true) action for the top card.
#[test]
fn test_citadel_generates_cast_from_library_top() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::BolassCitadel, 0);
    let bolt_id = push_library_top(&mut state, CardName::LightningBolt, 0);

    // Give player 0 plenty of life (Citadel pays life, not mana)
    state.players[0].life = 20;

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_library_top, .. }
            if *card_id == bolt_id && *from_library_top)
    });
    assert!(found, "Bolas's Citadel should allow casting the top card of library");
}

/// Bolas's Citadel: the spell is paid with life equal to its mana value.
#[test]
fn test_citadel_pays_life_equal_to_mana_value() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::BolassCitadel, 0);
    let bolt_id = push_library_top(&mut state, CardName::LightningBolt, 0); // CMC = 1

    state.players[0].life = 5;
    // No mana in pool

    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
            from_graveyard: false,
            from_library_top: true,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    // Life should drop by 1 (Lightning Bolt CMC = 1)
    assert_eq!(state.players[0].life, 4, "Citadel should pay 1 life for a CMC-1 spell");
    // Card should be removed from library
    assert!(state.players[0].library.is_empty(), "Card should be removed from library");
    // Card should be on the stack
    assert_eq!(state.stack.len(), 1, "Spell should be on the stack");
}

/// Bolas's Citadel: cannot cast if life total would go to 0 or below.
#[test]
fn test_citadel_cannot_cast_if_would_die() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::BolassCitadel, 0);
    // Lightning Bolt CMC = 1; player has exactly 1 life — would die paying it
    push_library_top(&mut state, CardName::LightningBolt, 0);
    state.players[0].life = 1;

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { from_library_top, .. } if *from_library_top)
    });
    assert!(!found, "Citadel should not generate action if player would die paying the life cost");
}

/// Bolas's Citadel: can play a land from the top of the library.
#[test]
fn test_citadel_allows_playing_land_from_library_top() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::BolassCitadel, 0);
    let island_id = push_library_top(&mut state, CardName::Island, 0);

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::PlayLandFromTop(id) if *id == island_id)
    });
    assert!(found, "Bolas's Citadel should allow playing a land from the top of library");
}

/// Bolas's Citadel: playing a land from top removes it from library and puts it on battlefield.
#[test]
fn test_citadel_land_from_top_enters_battlefield() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::BolassCitadel, 0);
    let island_id = push_library_top(&mut state, CardName::Island, 0);

    state.apply_action(&Action::PlayLandFromTop(island_id), &db);

    assert!(state.players[0].library.is_empty(), "Island should be removed from library");
    let on_bf = state.battlefield.iter().any(|p| p.id == island_id);
    assert!(on_bf, "Island should be on the battlefield");
    assert_eq!(state.players[0].land_plays_remaining, 0, "Land play should be consumed");
}

// ─── Future Sight ─────────────────────────────────────────────────────────────

/// Future Sight: can cast spells from the top of the library at normal mana cost.
#[test]
fn test_future_sight_cast_from_top_at_normal_cost() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::FutureSight, 0);
    let bolt_id = push_library_top(&mut state, CardName::LightningBolt, 0); // {R}

    state.players[0].mana_pool.red = 1;

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_library_top, .. }
            if *card_id == bolt_id && *from_library_top)
    });
    assert!(found, "Future Sight should allow casting the top card of library");
}

/// Future Sight: can play a land from the top of the library.
#[test]
fn test_future_sight_play_land_from_top() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::FutureSight, 0);
    let plains_id = push_library_top(&mut state, CardName::Plains, 0);

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::PlayLandFromTop(id) if *id == plains_id)
    });
    assert!(found, "Future Sight should allow playing a land from the top of library");
}

/// Future Sight: paying mana removes the card from library and puts it on the stack.
#[test]
fn test_future_sight_cast_removes_card_from_library() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::FutureSight, 0);
    let bolt_id = push_library_top(&mut state, CardName::LightningBolt, 0);

    state.players[0].mana_pool.red = 1;

    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
            x_value: 0,
            from_graveyard: false,
            from_library_top: true,
            alt_cost: None,
        modes: vec![],
        },
        &db,
    );

    assert!(state.players[0].library.is_empty(), "Bolt should be removed from library");
    assert_eq!(state.stack.len(), 1, "Bolt should be on the stack");
    assert_eq!(state.players[0].mana_pool.red, 0, "Mana should be paid");
}

// ─── Mystic Forge ─────────────────────────────────────────────────────────────

/// Mystic Forge: can cast artifact spells from the top of the library.
#[test]
fn test_mystic_forge_allows_artifact_from_top() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::MysticForge, 0);
    let sol_id = push_library_top(&mut state, CardName::SolRing, 0); // Artifact, CMC=1

    state.players[0].mana_pool.colorless = 1;

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_library_top, .. }
            if *card_id == sol_id && *from_library_top)
    });
    assert!(found, "Mystic Forge should allow casting an artifact from the top of library");
}

/// Mystic Forge: cannot cast non-artifact, non-colorless spells from the top of the library.
#[test]
fn test_mystic_forge_does_not_allow_non_artifact_spell() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::MysticForge, 0);
    push_library_top(&mut state, CardName::LightningBolt, 0); // Non-artifact, non-colorless

    state.players[0].mana_pool.red = 5;

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { from_library_top, .. } if *from_library_top)
    });
    assert!(!found, "Mystic Forge should NOT allow casting a non-artifact, non-colorless spell from library");
}

/// Mystic Forge: can cast colorless non-artifact spells from the top.
/// Kozilek's Command has {C}{C}{2} cost — no colored pips, so it's colorless.
#[test]
fn test_mystic_forge_allows_colorless_instant_from_top() {
    let (mut state, db) = make_main_phase_state();

    put_permanent(&mut state, &db, CardName::MysticForge, 0);
    // Kozilek's Command: {C}{C}{2} — colorless instant, not an artifact
    let kc_id = push_library_top(&mut state, CardName::KozileksCommand, 0);

    // Kozilek's Command costs {C}{C}{2}: need 2 specifically colorless + 2 generic
    state.players[0].mana_pool.colorless = 4; // 2 for {C}{C} pips + 2 for generic

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, from_library_top, .. }
            if *card_id == kc_id && *from_library_top)
    });
    assert!(found, "Mystic Forge should allow casting colorless (non-artifact) spells from top of library");
}

/// Mystic Forge: {T}, Pay 1 life: Exile the top card of your library.
#[test]
fn test_mystic_forge_exile_activated_ability() {
    let (mut state, db) = make_main_phase_state();

    let forge_id = put_permanent(&mut state, &db, CardName::MysticForge, 0);
    let bolt_id = push_library_top(&mut state, CardName::LightningBolt, 0);

    state.players[0].life = 10;

    // Should generate an ActivateAbility action for the exile ability
    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, ability_index, .. }
            if *permanent_id == forge_id && *ability_index == 0)
    });
    assert!(found, "Mystic Forge should offer tap, Pay 1 life: Exile top card ability");

    // Apply the ability
    state.apply_action(
        &Action::ActivateAbility {
            permanent_id: forge_id,
            ability_index: 0,
            targets: vec![],
        },
        &db,
    );

    // Life should be paid
    assert_eq!(state.players[0].life, 9, "Should pay 1 life");

    // Forge should be tapped
    let forge = state.battlefield.iter().find(|p| p.id == forge_id).unwrap();
    assert!(forge.tapped, "Mystic Forge should be tapped");

    // Ability should be on the stack
    assert_eq!(state.stack.len(), 1, "Exile ability should be on the stack");

    // Resolve the ability (pass priority twice)
    state.apply_action(&Action::PassPriority, &db);
    state.apply_action(&Action::PassPriority, &db);

    // Top card should be exiled
    assert!(state.players[0].library.is_empty(), "Top card should be removed from library");
    let exiled = state.exile.iter().any(|(id, _, _)| *id == bolt_id);
    assert!(exiled, "Top card should be in exile");
}

/// Mystic Forge: cannot activate exile ability when tapped.
#[test]
fn test_mystic_forge_exile_not_when_tapped() {
    let (mut state, db) = make_main_phase_state();

    let forge_id = put_permanent(&mut state, &db, CardName::MysticForge, 0);
    push_library_top(&mut state, CardName::LightningBolt, 0);
    state.players[0].life = 10;

    // Tap the forge
    state.battlefield.iter_mut().find(|p| p.id == forge_id).unwrap().tapped = true;

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, ability_index, .. }
            if *permanent_id == forge_id && *ability_index == 0)
    });
    assert!(!found, "Mystic Forge should NOT offer exile ability when tapped");
}

/// Mystic Forge: cannot activate exile ability with insufficient life.
#[test]
fn test_mystic_forge_exile_not_with_low_life() {
    let (mut state, db) = make_main_phase_state();

    let forge_id = put_permanent(&mut state, &db, CardName::MysticForge, 0);
    push_library_top(&mut state, CardName::LightningBolt, 0);
    state.players[0].life = 1; // Too low (need >= 2 to avoid suicide)

    let actions = state.legal_actions(&db);
    let found = actions.iter().any(|a| {
        matches!(a, Action::ActivateAbility { permanent_id, ability_index, .. }
            if *permanent_id == forge_id && *ability_index == 0)
    });
    assert!(!found, "Mystic Forge should NOT offer exile ability when life is too low");
}
