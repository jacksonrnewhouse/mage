/// Tests for copy-spell mechanics: Twincast and storm copies.

use crate::card::*;
use crate::game::*;
use crate::stack::{StackItem, StackItemKind};
use crate::types::*;

// ─── helpers ─────────────────────────────────────────────────────────────────

fn setup() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].life = 20;
    state.players[1].life = 20;
    (state, db)
}

/// Register a card and put it in a player's hand. Returns the object id.
fn add_to_hand(state: &mut GameState, player: usize, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player].hand.push(id);
    id
}

/// Push a spell directly onto the stack (simulates it having been cast).
/// Returns the id that the stack assigned to the item.
fn push_spell(state: &mut GameState, card_name: CardName, controller: u8) -> ObjectId {
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, card_name));
    state.stack.push_with_flags(
        StackItemKind::Spell { card_name, card_id, cast_via_evoke: false },
        controller,
        vec![],
        false, 0, false, vec![],
    );
    state.stack.items().last().map(|i| i.id).unwrap()
}

// ─── Twincast tests ───────────────────────────────────────────────────────────

/// Twincast targeting an Ancestral Recall on the stack should push a copy
/// of the spell. After resolving the copy, the target player draws 3 more cards.
#[test]
fn test_twincast_copies_spell_onto_stack() {
    let (mut state, db) = setup();

    // Player 1 casts Ancestral Recall targeting player 0 (standard play).
    // We simulate this by manually pushing the spell onto the stack.
    let recall_stack_id = push_spell(&mut state, CardName::AncestralRecall, 1);
    // Add player 0 as target for the Ancestral Recall copy later.
    // For simplicity we leave targets empty (resolve_card_effect defaults controller).

    // Stack now has: [AncestralRecall (bottom)]
    assert_eq!(state.stack.len(), 1);

    // Player 0 plays Twincast targeting the Ancestral Recall.
    let twincast_id = add_to_hand(&mut state, 0, CardName::Twincast);
    state.players[0].mana_pool.blue = 2;
    let actions = state.legal_actions(&db);
    let cast_action = actions.iter().find(|a| {
        matches!(a, crate::action::Action::CastSpell { card_id, .. } if *card_id == twincast_id)
    });
    assert!(cast_action.is_some(), "should be able to cast Twincast");
    state.apply_action(cast_action.unwrap(), &db);

    // After casting Twincast, the stack has: [AncestralRecall, Twincast (top)]
    // (priority is passed so we can resolve)
    assert_eq!(state.stack.len(), 2, "Twincast should be on top of AncestralRecall");

    // Both players pass priority, then Twincast resolves.
    // Twincast's effect: copy_spell(recall_stack_id) → a copy of AncestralRecall is pushed.
    // We'll drive resolution manually.
    // First, set the Twincast item's target to the AncestralRecall stack item.
    {
        // Get the Twincast stack item and set its target.
        let twincast_stack_id = state.stack.items().last().map(|i| i.id).unwrap();
        // We need to set targets on the twincast item after casting.
        // Twincast targets the spell already on the stack.
        // For the test, directly mutate stack to add the target.
        // (In a real game the target would be chosen during casting.)
        // Access items_mut isn't public; we'll simulate by re-reading the id.
        let _ = twincast_stack_id; // used below
    }

    // We'll rebuild the test more directly: manually create the Twincast stack item
    // with the correct target to avoid the legal_actions complexity.
    drop(state);
    drop(db);

    let (mut state, db) = setup();
    // Give player 0 enough library cards so draw doesn't deck them out.
    for _ in 0..10 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[0].library.push(id);
    }

    // Push AncestralRecall targeting player 0 onto the stack.
    let recall_stack_id = push_spell(&mut state, CardName::AncestralRecall, 1);
    // Set targets on AncestralRecall to target player 0.
    // (the items vec is accessible via items() but not mutably, so we use push_with_flags directly)
    // Remove the top item and re-push with targets.
    let _ = state.stack.remove(recall_stack_id);
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::AncestralRecall));
    state.stack.push_with_flags(
        StackItemKind::Spell { card_name: CardName::AncestralRecall, card_id, cast_via_evoke: false },
        1,
        vec![Target::Player(0)],
        false, 0, false, vec![],
    );
    let recall_stack_id = state.stack.items().last().map(|i| i.id).unwrap();

    // Push Twincast targeting the AncestralRecall.
    let twincast_card_id = state.new_object_id();
    state.card_registry.push((twincast_card_id, CardName::Twincast));
    state.stack.push_with_flags(
        StackItemKind::Spell { card_name: CardName::Twincast, card_id: twincast_card_id, cast_via_evoke: false },
        0,
        vec![Target::Object(recall_stack_id)],
        false, 0, false, vec![],
    );

    // Stack: [AncestralRecall(bottom), Twincast(top)]
    assert_eq!(state.stack.len(), 2);

    // Resolve Twincast: it copies AncestralRecall → stack becomes
    // [AncestralRecall(bottom), AncestralRecall-copy(top)]
    let hand_before = state.players[0].hand.len();
    state.resolve_top(&db);

    // After Twincast resolves, the copy of AncestralRecall should be on the stack.
    assert_eq!(state.stack.len(), 2,
        "after Twincast resolves, AncestralRecall (original) + copy should both be on stack");

    // Top item should be a copy of AncestralRecall.
    let top = state.stack.top().unwrap();
    assert!(top.is_copy, "top stack item should be a copy");
    assert!(matches!(&top.kind, StackItemKind::Spell { card_name: CardName::AncestralRecall, .. }),
        "copy should be an AncestralRecall spell");

    // Resolve the copy: player 0 draws 3 cards.
    let hand_before = state.players[0].hand.len();
    state.resolve_top(&db);
    assert_eq!(
        state.players[0].hand.len(),
        hand_before + 3,
        "resolving the AncestralRecall copy should draw 3 cards"
    );

    // Resolve the original AncestralRecall.
    let hand_before2 = state.players[0].hand.len();
    state.resolve_top(&db);
    assert_eq!(
        state.players[0].hand.len(),
        hand_before2 + 3,
        "original AncestralRecall should also draw 3 cards"
    );

    assert!(state.stack.is_empty(), "stack should be empty after all resolutions");
}

/// A copy of a spell is not "cast", so it does not go to the graveyard on resolution.
#[test]
fn test_copy_does_not_go_to_graveyard() {
    let (mut state, db) = setup();

    // Give player 0 library cards to draw.
    for _ in 0..6 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[0].library.push(id);
    }

    // Push an AncestralRecall copy directly (simulating a Twincast copy).
    let template = StackItem {
        id: 0,
        kind: StackItemKind::Spell {
            card_name: CardName::AncestralRecall,
            card_id: 0,
            cast_via_evoke: false,
        },
        controller: 0,
        targets: vec![Target::Player(0)],
        cant_be_countered: false,
        x_value: 0,
        cast_from_graveyard: false,
        modes: vec![],
        is_copy: false,
    };
    state.stack.push_copy(&template);
    assert_eq!(state.stack.len(), 1);

    let gy_before = state.players[0].graveyard.len();

    // Resolve the copy.
    state.resolve_top(&db);

    // Stack should be empty.
    assert!(state.stack.is_empty());
    // Graveyard should NOT have grown (copies cease to exist, they aren't card objects).
    assert_eq!(state.players[0].graveyard.len(), gy_before,
        "a spell copy should not go to the graveyard on resolution");
}

// ─── Storm copy tests ─────────────────────────────────────────────────────────

/// With storm_count = 2 (two spells cast before this one), casting a storm spell
/// should push 2 copies onto the stack. Total spells resolving: original + 2 copies.
#[test]
fn test_storm_creates_copies_on_stack() {
    let (mut state, db) = setup();

    // Simulate two spells having been cast this turn.
    state.storm_count = 2;

    // Give player 0 plenty of library cards for GalvanicRelay to exile.
    for _ in 0..10 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Mountain));
        state.players[0].library.push(id);
    }

    // Push GalvanicRelay directly onto the stack (simulates it being cast).
    let relay_card_id = state.new_object_id();
    state.card_registry.push((relay_card_id, CardName::GalvanicRelay));
    state.stack.push_with_flags(
        StackItemKind::Spell { card_name: CardName::GalvanicRelay, card_id: relay_card_id, cast_via_evoke: false },
        0,
        vec![],
        false, 0, false, vec![],
    );

    assert_eq!(state.stack.len(), 1);

    // Resolve GalvanicRelay (the original).
    // This should push storm_count (2) copies onto the stack and execute the base effect once.
    let hand_before = state.players[0].hand.len();
    state.resolve_top(&db);

    // Base effect: 1 card moved to hand.
    assert_eq!(state.players[0].hand.len(), hand_before + 1,
        "original GalvanicRelay should put one card in hand");

    // 2 storm copies should now be on the stack.
    assert_eq!(state.stack.len(), 2,
        "storm_count=2 means 2 copies on stack after the original resolves");

    // Each copy should be marked as_copy.
    for item in state.stack.items() {
        assert!(item.is_copy, "storm copies should have is_copy=true");
    }

    // Resolve both copies. Each should put one card in hand.
    let hand_after_original = state.players[0].hand.len();
    state.resolve_top(&db); // copy 1
    state.resolve_top(&db); // copy 2

    assert_eq!(state.players[0].hand.len(), hand_after_original + 2,
        "each storm copy should put one card in hand (2 copies = 2 extra cards)");

    assert!(state.stack.is_empty(), "stack should be empty after all storm copies resolve");
}

/// With storm_count = 0 (no spells cast before), a storm spell should create no copies.
#[test]
fn test_storm_no_copies_when_storm_count_zero() {
    let (mut state, db) = setup();
    state.storm_count = 0;

    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Mountain));
        state.players[0].library.push(id);
    }

    let relay_card_id = state.new_object_id();
    state.card_registry.push((relay_card_id, CardName::GalvanicRelay));
    state.stack.push_with_flags(
        StackItemKind::Spell { card_name: CardName::GalvanicRelay, card_id: relay_card_id, cast_via_evoke: false },
        0, vec![], false, 0, false, vec![],
    );

    state.resolve_top(&db);

    assert!(state.stack.is_empty(),
        "with storm_count=0 no copies should be pushed");
}
