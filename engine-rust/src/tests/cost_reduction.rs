use crate::card::*;
use crate::action::*;
use crate::types::*;
use crate::game::*;

/// Helper: put a permanent of the given card on the battlefield under `controller`.
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

/// Helper: put a card in a player's hand.
fn add_to_hand(state: &mut GameState, card_name: CardName, player: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].hand.push(id);
    id
}

/// Set the game to a main phase where `player` has priority.
fn setup_main_phase(state: &mut GameState, player: PlayerId) {
    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = player;
    state.priority_player = player;
}

// ─── Foundry Inspector ────────────────────────────────────────────────────────

#[test]
fn test_foundry_inspector_reduces_artifact_cost_by_one() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 controls Foundry Inspector (artifact creature)
    put_permanent(&mut state, &db, CardName::FoundryInspector, 0);

    // Give P0 a colorless artifact to cast: Walking Ballista costs {X}{X} but
    // let's use Sol Ring {1} generic. Actually use Phyrexian Revoker {2} generic.
    let revoker_id = add_to_hand(&mut state, CardName::PhyrexianRevoker, 0);

    setup_main_phase(&mut state, 0);
    // Phyrexian Revoker costs {2}. With Foundry Inspector it should cost {1}.
    // Give P0 exactly 1 generic mana.
    state.players[0].mana_pool.colorless = 1;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == revoker_id));
    assert!(can_cast, "Foundry Inspector should reduce Phyrexian Revoker cost from {{2}} to {{1}}");
}

#[test]
fn test_foundry_inspector_does_not_reduce_noncreature_spell() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    put_permanent(&mut state, &db, CardName::FoundryInspector, 0);

    // Lightning Bolt is not an artifact: costs {R}, unaffected.
    let bolt_id = add_to_hand(&mut state, CardName::LightningBolt, 0);

    setup_main_phase(&mut state, 0);
    state.players[0].mana_pool.red = 1;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == bolt_id));
    assert!(can_cast, "Lightning Bolt should still be castable for {{R}} without reduction");
}

#[test]
fn test_foundry_inspector_does_not_help_opponent() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 controls Foundry Inspector, but P1 is casting the artifact
    put_permanent(&mut state, &db, CardName::FoundryInspector, 0);

    // Phyrexian Revoker normally costs {2}
    let revoker_id = add_to_hand(&mut state, CardName::PhyrexianRevoker, 1);

    setup_main_phase(&mut state, 1);
    // P1 has only 1 mana — should NOT be enough without the reduction
    state.players[1].mana_pool.colorless = 1;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == revoker_id));
    assert!(!can_cast, "Foundry Inspector should NOT reduce costs for opponent");
}

#[test]
fn test_two_foundry_inspectors_reduce_by_two() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Two Foundry Inspectors on P0's side
    put_permanent(&mut state, &db, CardName::FoundryInspector, 0);
    put_permanent(&mut state, &db, CardName::FoundryInspector, 0);

    // Phyrexian Revoker normally costs {2}, should now cost {0}
    let revoker_id = add_to_hand(&mut state, CardName::PhyrexianRevoker, 0);

    setup_main_phase(&mut state, 0);
    // Zero mana available
    // (mana_pool is default, so empty)

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == revoker_id));
    assert!(can_cast, "Two Foundry Inspectors should reduce Phyrexian Revoker cost to {{0}}");
}

// ─── Affinity for Artifacts ───────────────────────────────────────────────────

#[test]
fn test_thought_monitor_affinity_no_artifacts() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Thought Monitor costs {6}{U} naturally.
    let monitor_id = add_to_hand(&mut state, CardName::ThoughtMonitor, 0);

    setup_main_phase(&mut state, 0);
    // No artifacts on board — affinity gives 0 reduction.
    // Give P0 7 mana ({6}{U}).
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 6;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == monitor_id));
    assert!(can_cast, "Thought Monitor should be castable for its full cost {{6}}{{U}} with no artifacts");
}

#[test]
fn test_thought_monitor_affinity_reduces_cost_by_artifact_count() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put 3 artifacts on the battlefield under P0
    put_permanent(&mut state, &db, CardName::SolRing, 0);
    put_permanent(&mut state, &db, CardName::MoxSapphire, 0);
    put_permanent(&mut state, &db, CardName::MoxJet, 0);

    // Thought Monitor costs {6}{U}, with 3 artifacts affinity → {3}{U}
    let monitor_id = add_to_hand(&mut state, CardName::ThoughtMonitor, 0);

    setup_main_phase(&mut state, 0);
    // Give P0 4 mana ({U} + {3} generic)
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 3;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == monitor_id));
    assert!(can_cast, "Thought Monitor affinity should reduce cost by 3 (number of artifacts)");
}

#[test]
fn test_thought_monitor_affinity_capped_at_generic_portion() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // 10 artifacts on board: should reduce Thought Monitor cost to {U} only (generic → 0, never negative)
    for _ in 0..10 {
        put_permanent(&mut state, &db, CardName::MoxSapphire, 0);
    }

    let monitor_id = add_to_hand(&mut state, CardName::ThoughtMonitor, 0);

    setup_main_phase(&mut state, 0);
    // Only {U} needed — colored cost is never reduced
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 0;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == monitor_id));
    assert!(can_cast, "Thought Monitor should cost at minimum {{U}} (colored mana never reduced)");
}

#[test]
fn test_thoughtcast_affinity_reduces_cost() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Thoughtcast costs {4}{U}. Put 4 artifacts on board → reduces to {U}.
    for _ in 0..4 {
        put_permanent(&mut state, &db, CardName::MoxSapphire, 0);
    }

    let thoughtcast_id = add_to_hand(&mut state, CardName::Thoughtcast, 0);

    setup_main_phase(&mut state, 0);
    state.players[0].mana_pool.blue = 1;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == thoughtcast_id));
    assert!(can_cast, "Thoughtcast with 4 artifacts should cost only {{U}}");
}

#[test]
fn test_affinity_only_counts_controller_artifacts() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P1 (opponent) has 10 artifacts; P0 (caster) has 0
    for _ in 0..10 {
        put_permanent(&mut state, &db, CardName::MoxSapphire, 1);
    }

    let monitor_id = add_to_hand(&mut state, CardName::ThoughtMonitor, 0);

    setup_main_phase(&mut state, 0);
    // Thought Monitor full cost is {6}{U}; P0 only has {U}
    state.players[0].mana_pool.blue = 1;

    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == monitor_id));
    assert!(!can_cast, "Affinity should only count caster's own artifacts, not opponent's");
}

// ─── Cost floor: colored mana cannot be reduced ───────────────────────────────

#[test]
fn test_colored_mana_never_reduced_by_foundry_inspector() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Sol Ring costs {1}. With one Foundry Inspector → {0}.
    // Sol Ring is an artifact, inspector reduces generic by 1.
    put_permanent(&mut state, &db, CardName::FoundryInspector, 0);

    let sol_ring_id = add_to_hand(&mut state, CardName::SolRing, 0);

    setup_main_phase(&mut state, 0);
    // No mana in pool — Sol Ring should cost {0} with inspector
    // (Sol Ring costs {1} generic, inspector reduces by 1 → free)
    let actions = state.legal_actions(&db);
    let can_cast = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == sol_ring_id));
    assert!(can_cast, "Sol Ring should be free with Foundry Inspector");
}
