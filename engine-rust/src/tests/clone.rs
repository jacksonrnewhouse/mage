/// Tests for copy / clone effects (Phyrexian Metamorph, etc.)

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

/// Helper: put a permanent directly onto the battlefield with its card-db stats.
fn put_permanent(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
    );
    if def.is_changeling {
        perm.creature_types = CreatureType::ALL.to_vec();
    } else {
        perm.creature_types = def.creature_types.to_vec();
    }
    perm.colors = def.color_identity.to_vec();
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

#[test]
fn test_phyrexian_metamorph_etb_creates_pending_choice() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a creature on the battlefield for the clone to copy.
    let _ = put_permanent(&mut state, &db, CardName::GoblinGuide, 1);

    // Put Phyrexian Metamorph onto the battlefield and fire its ETB.
    let meta_id = put_permanent(&mut state, &db, CardName::PhyrexianMetamorph, 0);
    state.handle_etb(CardName::PhyrexianMetamorph, meta_id, 0);

    assert!(
        state.pending_choice.is_some(),
        "Phyrexian Metamorph ETB should create a pending choice for clone target"
    );

    // Verify the options include the creature.
    if let Some(PendingChoice { kind: ChoiceKind::ChooseFromList { ref options, .. }, .. }) =
        state.pending_choice
    {
        assert!(
            !options.is_empty(),
            "Clone options list should not be empty when there are targets"
        );
    }
}

#[test]
fn test_phyrexian_metamorph_no_targets_no_choice() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Metamorph on battlefield with no other artifacts or creatures present.
    let meta_id = put_permanent(&mut state, &db, CardName::PhyrexianMetamorph, 0);
    state.handle_etb(CardName::PhyrexianMetamorph, meta_id, 0);

    // With no valid targets, no pending choice is created.
    assert!(
        state.pending_choice.is_none(),
        "Phyrexian Metamorph should not create a pending choice when no targets exist"
    );
}

#[test]
fn test_phyrexian_metamorph_copies_creature_stats() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a creature with known stats on the battlefield.
    let guide_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 1);

    // Put Phyrexian Metamorph and fire ETB.
    let meta_id = put_permanent(&mut state, &db, CardName::PhyrexianMetamorph, 0);
    state.handle_etb(CardName::PhyrexianMetamorph, meta_id, 0);

    // Resolve the pending choice: choose GoblinGuide as the target.
    assert!(state.pending_choice.is_some(), "Should have a pending choice");
    let choice = state.pending_choice.take().unwrap();
    state.resolve_choice(choice, guide_id, &db);

    // The Metamorph should now have GoblinGuide's stats.
    let metamorph = state.find_permanent(meta_id).expect("Metamorph should still be on battlefield");
    assert_eq!(
        metamorph.card_name,
        CardName::GoblinGuide,
        "Metamorph should have copied GoblinGuide's card name"
    );
    assert_eq!(metamorph.base_power, 2, "Metamorph should have GoblinGuide's power (2)");
    assert_eq!(metamorph.base_toughness, 2, "Metamorph should have GoblinGuide's toughness (2)");
    assert!(
        metamorph.keywords.has(Keyword::Haste),
        "Metamorph should have copied GoblinGuide's Haste"
    );
    assert!(
        metamorph.creature_types.contains(&CreatureType::Goblin),
        "Metamorph should have copied GoblinGuide's Goblin creature type"
    );
}

#[test]
fn test_phyrexian_metamorph_copies_creature_and_is_artifact() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a plain creature (no artifact) on the battlefield.
    let guide_id = put_permanent(&mut state, &db, CardName::GoblinGuide, 1);

    // Put Phyrexian Metamorph and fire ETB.
    let meta_id = put_permanent(&mut state, &db, CardName::PhyrexianMetamorph, 0);
    state.handle_etb(CardName::PhyrexianMetamorph, meta_id, 0);

    let choice = state.pending_choice.take().unwrap();
    state.resolve_choice(choice, guide_id, &db);

    // Metamorph must be an artifact in addition to being a creature.
    let metamorph = state.find_permanent(meta_id).expect("Metamorph should be on battlefield");
    assert!(
        metamorph.card_types.contains(&CardType::Artifact),
        "Phyrexian Metamorph should always be an Artifact (even when copying a non-artifact)"
    );
    assert!(
        metamorph.card_types.contains(&CardType::Creature),
        "Phyrexian Metamorph should be a Creature when copying a creature"
    );
}

#[test]
fn test_phyrexian_metamorph_copies_artifact_creature() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put an artifact creature on the battlefield (e.g. WurmcoilEngine is 6/6 artifact creature).
    let wurm_id = put_permanent(&mut state, &db, CardName::WurmcoilEngine, 1);

    let meta_id = put_permanent(&mut state, &db, CardName::PhyrexianMetamorph, 0);
    state.handle_etb(CardName::PhyrexianMetamorph, meta_id, 0);

    let choice = state.pending_choice.take().unwrap();
    state.resolve_choice(choice, wurm_id, &db);

    let metamorph = state.find_permanent(meta_id).expect("Metamorph should be on battlefield");
    assert_eq!(metamorph.card_name, CardName::WurmcoilEngine);
    assert_eq!(metamorph.base_power, 6);
    assert_eq!(metamorph.base_toughness, 6);
    // Must be an artifact (was already, and Metamorph rule preserves it).
    assert!(metamorph.card_types.contains(&CardType::Artifact));
    assert!(metamorph.card_types.contains(&CardType::Creature));
}

#[test]
fn test_phyrexian_metamorph_copies_pure_artifact() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a pure (non-creature) artifact on the battlefield.
    let sol_id = put_permanent(&mut state, &db, CardName::SolRing, 1);

    let meta_id = put_permanent(&mut state, &db, CardName::PhyrexianMetamorph, 0);
    state.handle_etb(CardName::PhyrexianMetamorph, meta_id, 0);

    let choice = state.pending_choice.take().unwrap();
    state.resolve_choice(choice, sol_id, &db);

    let metamorph = state.find_permanent(meta_id).expect("Metamorph should be on battlefield");
    assert_eq!(metamorph.card_name, CardName::SolRing, "Metamorph should copy Sol Ring");
    // Still an artifact.
    assert!(metamorph.card_types.contains(&CardType::Artifact));
    // Not a creature (Sol Ring is not a creature).
    assert!(
        !metamorph.card_types.contains(&CardType::Creature),
        "Metamorph copying Sol Ring should not be a creature"
    );
}
