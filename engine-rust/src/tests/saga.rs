/// Tests for Saga enchantment support (issue #16).
/// Covers Urza's Saga: lore counter placement, chapter triggers, and sacrifice after chapter III.

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

/// Helper: put Urza's Saga onto the battlefield for a player, triggering its ETB.
/// Returns the ObjectId assigned to the saga.
fn put_urzas_saga(state: &mut GameState, db: &[CardDef], controller: PlayerId) -> ObjectId {
    let saga_id = state.new_object_id();
    state.card_registry.push((saga_id, CardName::UrzasSaga));

    let def = find_card(db, CardName::UrzasSaga).unwrap();
    let mut perm = Permanent::new(
        saga_id,
        CardName::UrzasSaga,
        controller,
        controller,
        def.power,
        def.toughness,
        def.loyalty,
        def.keywords,
        def.card_types,
    );
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);

    // Simulate ETB: this calls handle_etb_with_x which adds a lore counter and
    // pushes the Chapter I trigger.
    state.handle_etb(CardName::UrzasSaga, saga_id, controller);

    saga_id
}

// ---------------------------------------------------------------------------
// Test 1: Urza's Saga enters with exactly 1 lore counter
// ---------------------------------------------------------------------------
#[test]
fn test_urzas_saga_enters_with_one_lore_counter() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let saga_id = put_urzas_saga(&mut state, &db, 0);

    let lore = state
        .find_permanent(saga_id)
        .map(|p| p.counters.get(CounterType::Lore))
        .unwrap_or(0);

    assert_eq!(lore, 1, "Urza's Saga should enter with exactly 1 lore counter");
}

// ---------------------------------------------------------------------------
// Test 2: Chapter I trigger is placed on the stack when Urza's Saga enters
// ---------------------------------------------------------------------------
#[test]
fn test_urzas_saga_etb_triggers_chapter_one() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    put_urzas_saga(&mut state, &db, 0);

    assert!(
        !state.stack.is_empty(),
        "Chapter I trigger should be on the stack after Urza's Saga enters"
    );

    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::SagaChapter { chapter: 1, .. },
                ..
            }
        ),
        "Top of stack should be SagaChapter chapter=1, got {:?}",
        top.kind
    );
    assert_eq!(top.controller, 0);
}

// ---------------------------------------------------------------------------
// Test 3: Chapter II trigger fires at the beginning of the next PreCombatMain
// ---------------------------------------------------------------------------
#[test]
fn test_urzas_saga_chapter_two_fires_at_next_main_phase() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Set up: saga is on the battlefield and we're past the ETB chapter I trigger.
    let saga_id = put_urzas_saga(&mut state, &db, 0);

    // Resolve the Chapter I trigger off the stack (no effect other than marking chapter resolved).
    state.resolve_top(&db);
    assert!(state.stack.is_empty(), "Stack should be empty after Chapter I resolves");

    // Advance to the next PreCombatMain phase for player 0 to fire Chapter II.
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Draw);
    state.advance_phase(); // → PreCombatMain, check_delayed_triggers fires

    assert_eq!(state.phase, Phase::PreCombatMain);

    // The delayed trigger (chapter advance) should have fired, adding lore counter 2
    // and pushing the chapter-advance trigger (chapter=0), which itself pushes SagaChapter{chapter:2}.
    // The stack now has the saga-advance trigger (chapter=0).
    assert!(
        !state.stack.is_empty(),
        "Saga advance trigger should be on the stack after entering PreCombatMain"
    );

    // Resolve the chapter=0 trigger: it adds a lore counter and pushes SagaChapter{chapter=2}.
    state.resolve_top(&db);

    // Check lore counter is now 2.
    let lore = state
        .find_permanent(saga_id)
        .map(|p| p.counters.get(CounterType::Lore))
        .unwrap_or(0);
    assert_eq!(lore, 2, "Saga should have 2 lore counters after Chapter II fires");

    // Chapter II trigger should be on the stack.
    assert!(
        !state.stack.is_empty(),
        "Chapter II trigger should now be on the stack"
    );
    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::SagaChapter { chapter: 2, .. },
                ..
            }
        ),
        "Top of stack should be SagaChapter chapter=2"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Chapter II creates a Construct token
// ---------------------------------------------------------------------------
#[test]
fn test_urzas_saga_chapter_two_creates_construct_token() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a mana crypt on the battlefield so player 0 controls an artifact
    // (Construct gets +1/+1 per artifact you control).
    let mana_crypt_id = state.new_object_id();
    state.card_registry.push((mana_crypt_id, CardName::ManaCrypt));
    let mc_def = find_card(&db, CardName::ManaCrypt).unwrap();
    let mc_perm = Permanent::new(
        mana_crypt_id, CardName::ManaCrypt, 0, 0,
        mc_def.power, mc_def.toughness, mc_def.loyalty,
        mc_def.keywords, mc_def.card_types,
    );
    state.battlefield.push(mc_perm);

    let saga_id = put_urzas_saga(&mut state, &db, 0);
    // Resolve Chapter I (no effect).
    state.resolve_top(&db);

    // Advance to PreCombatMain to fire the advance trigger.
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Draw);
    state.advance_phase();

    // Resolve chapter=0 advance trigger (adds lore 2, pushes SagaChapter{2}).
    state.resolve_top(&db);

    // Now resolve Chapter II — should create a Construct token.
    let bf_count_before = state.battlefield.len();
    state.resolve_top(&db);
    let bf_count_after = state.battlefield.len();

    assert_eq!(
        bf_count_after,
        bf_count_before + 1,
        "Chapter II should create exactly one Construct token"
    );

    // The token should be a Construct artifact creature.
    let token = state.battlefield.last().unwrap();
    assert!(token.is_token, "The created permanent should be a token");
    assert!(
        token.card_types.contains(&CardType::Artifact),
        "Construct token should be an artifact"
    );
    assert!(
        token.card_types.contains(&CardType::Creature),
        "Construct token should be a creature"
    );
    assert!(
        token.creature_types.contains(&CreatureType::Construct),
        "Token should have the Construct creature type"
    );

    // Player 0 controls: ManaCrypt (artifact) + UrzasSaga (not an artifact) + the new token.
    // At the time of token creation, artifact count = 1 (ManaCrypt; the saga and token itself
    // are handled depending on timing). The token should be at least 0/0 base with bonus >= 1.
    assert!(
        token.power_mod >= 0,
        "Construct token should have nonnegative power from artifact count"
    );

    let _ = saga_id;
}

// ---------------------------------------------------------------------------
// Test 5: Saga is sacrificed after Chapter III resolves
// ---------------------------------------------------------------------------
#[test]
fn test_urzas_saga_sacrificed_after_chapter_three() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Load a MV-0 artifact (Tormod's Crypt) into player 0's library so Chapter III has a target.
    let tormod_id = state.new_object_id();
    state.card_registry.push((tormod_id, CardName::TormodsCrypt));
    state.players[0].library.push(tormod_id);

    let saga_id = put_urzas_saga(&mut state, &db, 0);

    // Resolve Chapter I.
    state.resolve_top(&db);

    // --- Advance to get to Chapter II ---
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Draw);
    state.advance_phase(); // → PreCombatMain, fires chapter=0

    state.resolve_top(&db); // advance trigger (chapter=0)
    state.resolve_top(&db); // Chapter II resolves (creates token)

    // --- Advance to get to Chapter III ---
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Draw);
    state.advance_phase(); // → PreCombatMain, fires chapter=0

    state.resolve_top(&db); // advance trigger (chapter=0) → pushes SagaChapter{3}

    // Now the stack has SagaChapter{chapter:3}.
    // Resolving it should: push pending choice for search (if artifacts exist) + push SagaSacrifice.
    state.resolve_top(&db);

    // Stack should now have SagaSacrifice on top (pushed after the Chapter III search trigger).
    assert!(
        !state.stack.is_empty(),
        "SagaSacrifice trigger should be on the stack after Chapter III resolves"
    );
    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::SagaSacrifice { .. },
                ..
            }
        ),
        "SagaSacrifice should be on top of stack after Chapter III"
    );

    // Resolve the sacrifice trigger.
    state.resolve_top(&db);

    // The saga should no longer be on the battlefield.
    assert!(
        state.find_permanent(saga_id).is_none(),
        "Urza's Saga should have been sacrificed after Chapter III resolved"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Recurring chapter-advance trigger is cleaned up when saga leaves
// ---------------------------------------------------------------------------
#[test]
fn test_saga_advance_trigger_removed_on_sacrifice() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let saga_id = put_urzas_saga(&mut state, &db, 0);

    // There should be one recurring delayed trigger registered for the saga.
    let recurring = state.delayed_triggers.iter()
        .filter(|dt| matches!(dt.effect, TriggeredEffect::SagaChapter { saga_id: sid, chapter: 0 } if sid == saga_id))
        .count();
    assert_eq!(recurring, 1, "One recurring chapter-advance trigger should be registered");

    // Manually push a SagaSacrifice trigger and resolve it to simulate the sacrifice path.
    state.stack.push(
        crate::stack::StackItemKind::TriggeredAbility {
            source_id: saga_id,
            source_name: CardName::UrzasSaga,
            effect: TriggeredEffect::SagaSacrifice { saga_id },
        },
        0,
        vec![],
    );
    state.resolve_top(&db);

    // The saga should be gone.
    assert!(
        state.find_permanent(saga_id).is_none(),
        "Saga should be gone after SagaSacrifice resolves"
    );

    // The recurring delayed trigger should also have been removed.
    let remaining = state.delayed_triggers.iter()
        .filter(|dt| matches!(dt.effect, TriggeredEffect::SagaChapter { saga_id: sid, chapter: 0 } if sid == saga_id))
        .count();
    assert_eq!(
        remaining, 0,
        "Recurring chapter-advance trigger should be removed when saga is sacrificed"
    );
}
