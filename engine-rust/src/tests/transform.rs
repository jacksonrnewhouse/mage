/// Tests for transform and double-faced card (DFC) support (#13).
///
/// Delver of Secrets // Insectile Aberration is the primary test case:
/// - Front face: 1/1 Human Wizard (Delver of Secrets)
/// - Back face:  3/2 Human Insect with Flying (Insectile Aberration)
/// At the beginning of your upkeep, look at the top card of your library.
/// If it's an instant or sorcery card, transform Delver of Secrets.

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

/// Helper: put a permanent on the battlefield for a player (not summoning sick).
fn put_on_battlefield(
    state: &mut GameState,
    db: &[CardDef],
    card_name: CardName,
    controller: PlayerId,
) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id,
        card_name,
        controller,
        controller,
        def.power,
        def.toughness,
        None,
        def.keywords,
        def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

/// Helper: add a card to a player's library (top = last element).
fn add_to_library(state: &mut GameState, card_name: CardName, player: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].library.push(id);
    id
}

// ---------------------------------------------------------------------------
// Test: transform_permanent changes stats to back face
// ---------------------------------------------------------------------------
#[test]
fn test_transform_changes_stats_to_back_face() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Delver of Secrets on the battlefield as player 0's creature.
    let delver_id = put_on_battlefield(&mut state, &db, CardName::DelverOfSecrets, 0);

    // Verify front-face stats.
    {
        let perm = state.find_permanent(delver_id).unwrap();
        assert_eq!(perm.card_name, CardName::DelverOfSecrets);
        assert_eq!(perm.base_power, 1);
        assert_eq!(perm.base_toughness, 1);
        assert!(!perm.keywords.has(Keyword::Flying));
        assert!(!perm.transformed);
    }

    // Transform to back face.
    state.transform_permanent(delver_id, &db);

    // Verify back-face stats: 3/2 with Flying.
    {
        let perm = state.find_permanent(delver_id).unwrap();
        assert_eq!(perm.card_name, CardName::InsectileAberration);
        assert_eq!(perm.base_power, 3);
        assert_eq!(perm.base_toughness, 2);
        assert!(perm.keywords.has(Keyword::Flying));
        assert!(perm.transformed);
    }
}

// ---------------------------------------------------------------------------
// Test: transform_permanent is idempotent when called again (flips back)
// ---------------------------------------------------------------------------
#[test]
fn test_transform_flips_back_to_front_face() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let delver_id = put_on_battlefield(&mut state, &db, CardName::DelverOfSecrets, 0);

    // Transform to back face.
    state.transform_permanent(delver_id, &db);
    assert_eq!(
        state.find_permanent(delver_id).unwrap().card_name,
        CardName::InsectileAberration
    );

    // Transform again — should flip back to front face.
    state.transform_permanent(delver_id, &db);
    let perm = state.find_permanent(delver_id).unwrap();
    assert_eq!(perm.card_name, CardName::DelverOfSecrets);
    assert_eq!(perm.base_power, 1);
    assert_eq!(perm.base_toughness, 1);
    assert!(!perm.keywords.has(Keyword::Flying));
    assert!(!perm.transformed);
}

// ---------------------------------------------------------------------------
// Test: Delver ETB registers an upkeep trigger
// ---------------------------------------------------------------------------
#[test]
fn test_delver_etb_registers_upkeep_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let delver_id = put_on_battlefield(&mut state, &db, CardName::DelverOfSecrets, 0);

    // Fire the ETB manually.
    state.handle_etb(CardName::DelverOfSecrets, delver_id, 0);

    // There should be a delayed trigger registered.
    assert!(
        !state.delayed_triggers.is_empty(),
        "Delver ETB should register a recurring upkeep trigger"
    );

    let has_delver_trigger = state.delayed_triggers.iter().any(|dt| {
        matches!(dt.effect, TriggeredEffect::DelverUpkeep { delver_id: id } if id == delver_id)
            && dt.controller == 0
            && !dt.fires_once
    });
    assert!(
        has_delver_trigger,
        "Should have a recurring DelverUpkeep delayed trigger for player 0"
    );
}

// ---------------------------------------------------------------------------
// Test: Delver transforms when top of library is an instant
// ---------------------------------------------------------------------------
#[test]
fn test_delver_transforms_when_top_is_instant() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Delver on the battlefield.
    let delver_id = put_on_battlefield(&mut state, &db, CardName::DelverOfSecrets, 0);

    // Put a Lightning Bolt (instant) on top of player 0's library.
    add_to_library(&mut state, CardName::LightningBolt, 0);

    // Register the Delver upkeep trigger manually (as ETB would do).
    state.handle_etb(CardName::DelverOfSecrets, delver_id, 0);

    // Transition to player 0's upkeep — this fires delayed triggers.
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase(); // → Upkeep, fires DelverUpkeep trigger

    // The DelverUpkeep triggered ability should be on the stack.
    assert!(
        !state.stack.is_empty(),
        "DelverUpkeep trigger should be on the stack at upkeep"
    );

    // Resolve the trigger: should transform Delver since top is an instant.
    state.resolve_top(&db);

    // Delver should now be Insectile Aberration.
    let perm = state.find_permanent(delver_id).expect("Delver should still be on battlefield");
    assert_eq!(
        perm.card_name,
        CardName::InsectileAberration,
        "Delver should have transformed to Insectile Aberration when top of library was an instant"
    );
    assert!(perm.keywords.has(Keyword::Flying));
    assert!(perm.transformed);
}

// ---------------------------------------------------------------------------
// Test: Delver does NOT transform when top of library is a land
// ---------------------------------------------------------------------------
#[test]
fn test_delver_does_not_transform_when_top_is_land() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let delver_id = put_on_battlefield(&mut state, &db, CardName::DelverOfSecrets, 0);

    // Put an Island (land, not instant/sorcery) on top of library.
    add_to_library(&mut state, CardName::Island, 0);

    // Register the Delver upkeep trigger manually.
    state.handle_etb(CardName::DelverOfSecrets, delver_id, 0);

    // Advance to upkeep.
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase();

    // There should be a trigger on the stack.
    assert!(!state.stack.is_empty(), "DelverUpkeep trigger should fire");

    // Resolve the trigger.
    state.resolve_top(&db);

    // Delver should remain as Delver of Secrets.
    let perm = state.find_permanent(delver_id).expect("Delver should still be on battlefield");
    assert_eq!(
        perm.card_name,
        CardName::DelverOfSecrets,
        "Delver should NOT transform when top of library is a land"
    );
    assert!(!perm.transformed);
}

// ---------------------------------------------------------------------------
// Test: Delver does NOT transform when top of library is a sorcery (wait — it SHOULD)
// ---------------------------------------------------------------------------
#[test]
fn test_delver_transforms_when_top_is_sorcery() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let delver_id = put_on_battlefield(&mut state, &db, CardName::DelverOfSecrets, 0);

    // Put a Ponder (sorcery) on top of library.
    add_to_library(&mut state, CardName::Ponder, 0);

    state.handle_etb(CardName::DelverOfSecrets, delver_id, 0);

    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase();

    assert!(!state.stack.is_empty(), "DelverUpkeep trigger should fire");

    state.resolve_top(&db);

    let perm = state.find_permanent(delver_id).expect("Delver should still be on battlefield");
    assert_eq!(
        perm.card_name,
        CardName::InsectileAberration,
        "Delver should transform when top of library is a sorcery"
    );
    assert!(perm.keywords.has(Keyword::Flying));
}

// ---------------------------------------------------------------------------
// Test: InsectileAberration has correct card definition
// ---------------------------------------------------------------------------
#[test]
fn test_insectile_aberration_card_def() {
    let db = build_card_db();
    let def = find_card(&db, CardName::InsectileAberration).expect("InsectileAberration should be in card db");
    assert_eq!(def.power, Some(3));
    assert_eq!(def.toughness, Some(2));
    assert!(def.keywords.has(Keyword::Flying));
    assert!(def.back_face.is_none(), "InsectileAberration has no back face");
}

// ---------------------------------------------------------------------------
// Test: DelverOfSecrets has correct card definition including back_face link
// ---------------------------------------------------------------------------
#[test]
fn test_delver_of_secrets_card_def() {
    let db = build_card_db();
    let def = find_card(&db, CardName::DelverOfSecrets).expect("DelverOfSecrets should be in card db");
    assert_eq!(def.power, Some(1));
    assert_eq!(def.toughness, Some(1));
    assert!(!def.keywords.has(Keyword::Flying));
    assert_eq!(
        def.back_face,
        Some(CardName::InsectileAberration),
        "DelverOfSecrets back_face should be InsectileAberration"
    );
}
