/// Tests for imprint mechanics: Chrome Mox and Isochron Scepter.

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

fn setup_base() -> (GameState, Vec<CardDef>) {
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

/// Helper: add a card to a player's hand by object ID.
fn add_to_hand(state: &mut GameState, player: u8, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].hand.push(id);
    id
}

/// Helper: put a permanent on the battlefield.
fn add_permanent(state: &mut GameState, controller: u8, card_name: CardName) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let perm = Permanent::new(
        id, card_name, controller, controller,
        None, None, None, Keywords::empty(), &[CardType::Artifact],
    );
    state.battlefield.push(perm);
    id
}

/// Chrome Mox ETB: imprinting a nonartifact, nonland card should record the imprint link.
#[test]
fn test_chrome_mox_imprint_records_link() {
    let (mut state, db) = setup_base();

    // Put Chrome Mox on the battlefield for P0
    let mox_id = state.new_object_id();
    state.card_registry.push((mox_id, CardName::ChromeMox));
    let def = find_card(&db, CardName::ChromeMox).unwrap();
    let perm = Permanent::new(
        mox_id, CardName::ChromeMox, 0, 0,
        None, None, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Add a Lightning Bolt (red sorcery-speed instant) to P0's hand to imprint
    let bolt_id = add_to_hand(&mut state, 0, CardName::LightningBolt);

    // Fire the ChromeMoxETB trigger
    use crate::stack::*;
    state.stack.push(
        StackItemKind::TriggeredAbility {
            source_id: mox_id,
            source_name: CardName::ChromeMox,
            effect: TriggeredEffect::ChromeMoxETB { mox_id },
        },
        0,
        vec![],
    );

    // Resolve trigger: P0 passes (ETB resolves, pending choice appears)
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes -> trigger resolves, pending choice set

    // The pending choice should now ask P0 to choose which hand card to imprint.
    assert!(
        state.pending_choice.is_some(),
        "Pending choice should be set after Chrome Mox ETB resolves"
    );

    // P0 chooses Lightning Bolt to imprint
    state.apply_action(&crate::action::Action::ChooseCard(bolt_id), &db);

    // bolt_id should be in exile
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == bolt_id),
        "Imprinted card (Lightning Bolt) should be in exile"
    );

    // imprinted should record (mox_id, bolt_id)
    assert!(
        state.imprinted.iter().any(|(perm_id, card_id)| *perm_id == mox_id && *card_id == bolt_id),
        "imprinted should record (Chrome Mox id, Lightning Bolt id)"
    );

    // bolt_id should not be in P0's hand anymore
    assert!(
        !state.players[0].hand.contains(&bolt_id),
        "Imprinted card should have been removed from hand"
    );
}

/// Chrome Mox with an imprinted red card should produce mana (any color including red).
/// We verify that the mox has a mana ability available when imprinted.
#[test]
fn test_chrome_mox_has_mana_ability_when_imprinted() {
    let (mut state, db) = setup_base();

    let mox_id = add_permanent(&mut state, 0, CardName::ChromeMox);
    // Imprint a Lightning Bolt (red card)
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.exile.push((bolt_id, CardName::LightningBolt, 0));
    state.imprinted.push((mox_id, bolt_id));

    // Check mana ability options for the mox
    let perm = state.find_permanent(mox_id).unwrap().clone();
    let options = state.mana_ability_options_pub(&perm);

    assert!(
        !options.is_empty(),
        "Chrome Mox with imprinted card should have mana ability options"
    );
}

/// Chrome Mox without any imprinted card should produce no mana.
#[test]
fn test_chrome_mox_no_mana_without_imprint() {
    let (mut state, _db) = setup_base();

    let mox_id = add_permanent(&mut state, 0, CardName::ChromeMox);
    // No imprint link

    let perm = state.find_permanent(mox_id).unwrap().clone();
    let options = state.mana_ability_options_pub(&perm);

    assert!(
        options.is_empty(),
        "Chrome Mox without imprinted card should produce no mana"
    );
}

/// Isochron Scepter ETB: imprinting an instant with MV <= 2 records the link.
#[test]
fn test_isochron_scepter_imprint_records_link() {
    let (mut state, db) = setup_base();

    // Put Isochron Scepter on the battlefield for P0
    let scepter_id = state.new_object_id();
    state.card_registry.push((scepter_id, CardName::IsochronScepter));
    let def = find_card(&db, CardName::IsochronScepter).unwrap();
    let perm = Permanent::new(
        scepter_id, CardName::IsochronScepter, 0, 0,
        None, None, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Add Counterspell (instant, MV = 2) to P0's hand
    let cs_id = add_to_hand(&mut state, 0, CardName::Counterspell);

    // Fire the IsochronScepterETB trigger
    use crate::stack::*;
    state.stack.push(
        StackItemKind::TriggeredAbility {
            source_id: scepter_id,
            source_name: CardName::IsochronScepter,
            effect: TriggeredEffect::IsochronScepterETB { scepter_id },
        },
        0,
        vec![],
    );

    // Resolve trigger
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Pending choice should be set
    assert!(
        state.pending_choice.is_some(),
        "Pending choice should be set after Isochron Scepter ETB resolves"
    );

    // P0 chooses Counterspell
    state.apply_action(&crate::action::Action::ChooseCard(cs_id), &db);

    // Counterspell should be in exile
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == cs_id),
        "Imprinted Counterspell should be in exile"
    );

    // imprinted should record (scepter_id, cs_id)
    assert!(
        state.imprinted.iter().any(|(perm_id, card_id)| *perm_id == scepter_id && *card_id == cs_id),
        "imprinted should record (Isochron Scepter id, Counterspell id)"
    );
}

/// Isochron Scepter should NOT allow imprinting a card with MV > 2.
#[test]
fn test_isochron_scepter_rejects_high_mv_card() {
    let (mut state, db) = setup_base();

    let scepter_id = state.new_object_id();
    state.card_registry.push((scepter_id, CardName::IsochronScepter));
    let def = find_card(&db, CardName::IsochronScepter).unwrap();
    let perm = Permanent::new(
        scepter_id, CardName::IsochronScepter, 0, 0,
        None, None, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Add Counterspell (MV 2, OK) and Force of Will (MV 5, NOT OK) to hand
    let cs_id = add_to_hand(&mut state, 0, CardName::Counterspell);
    let fow_id = add_to_hand(&mut state, 0, CardName::ForceOfWill);

    use crate::stack::*;
    state.stack.push(
        StackItemKind::TriggeredAbility {
            source_id: scepter_id,
            source_name: CardName::IsochronScepter,
            effect: TriggeredEffect::IsochronScepterETB { scepter_id },
        },
        0,
        vec![],
    );

    state.pass_priority(&db);
    state.pass_priority(&db);

    // Check that the pending choice only includes Counterspell, not Force of Will
    if let Some(ref choice) = state.pending_choice {
        if let ChoiceKind::ChooseFromList { options, .. } = &choice.kind {
            assert!(
                options.contains(&cs_id),
                "Counterspell (MV 2) should be a valid imprint choice"
            );
            assert!(
                !options.contains(&fow_id),
                "Force of Will (MV 5) should NOT be a valid imprint choice"
            );
        }
    }
}

/// Isochron Scepter activated ability should cast the imprinted instant's effect.
/// We test with Ancestral Recall (instant, MV 1) — the controller should draw 3 cards.
#[test]
fn test_isochron_scepter_activates_and_casts_imprinted_instant() {
    let (mut state, db) = setup_base();

    let scepter_id = state.new_object_id();
    state.card_registry.push((scepter_id, CardName::IsochronScepter));
    let def = find_card(&db, CardName::IsochronScepter).unwrap();
    let perm = Permanent::new(
        scepter_id, CardName::IsochronScepter, 0, 0,
        None, None, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Imprint Ancestral Recall
    let recall_id = state.new_object_id();
    state.card_registry.push((recall_id, CardName::AncestralRecall));
    state.exile.push((recall_id, CardName::AncestralRecall, 0));
    state.imprinted.push((scepter_id, recall_id));

    // Give P0 2 mana to pay activation cost
    state.players[0].mana_pool.add(None, 2);

    // Put some cards in library for drawing
    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Plains));
        state.players[0].library.push(id);
    }

    let hand_before = state.players[0].hand.len();

    // Activate Isochron Scepter
    state.apply_action(
        &crate::action::Action::ActivateAbility {
            permanent_id: scepter_id,
            ability_index: 0,
            targets: vec![],
        },
        &db,
    );

    // Resolve the activated ability (pass priority twice)
    state.pass_priority(&db);
    state.pass_priority(&db);

    // P0 should have drawn 3 cards (Ancestral Recall casts for free)
    let hand_after = state.players[0].hand.len();
    assert!(
        hand_after >= hand_before + 3,
        "Isochron Scepter should have cast Ancestral Recall, drawing 3 cards (before: {}, after: {})",
        hand_before, hand_after
    );

    // The scepter should be tapped
    assert!(
        state.find_permanent(scepter_id).map(|p| p.tapped).unwrap_or(false),
        "Isochron Scepter should be tapped after activation"
    );
}
