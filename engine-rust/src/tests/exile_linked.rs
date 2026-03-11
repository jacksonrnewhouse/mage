/// Tests for exile-until-leaves-battlefield tracking.
/// Covers Solitude (exile creature, return when Solitude leaves)
/// and Skyclave Apparition (exile nonland nontoken MV<=4, give opponent X/X token on leaves).

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

fn add_creature(state: &mut GameState, controller: u8, card_name: CardName, power: i16, toughness: i16) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let perm = Permanent::new(
        id, card_name, controller, controller,
        Some(power), Some(toughness), None,
        Keywords::empty(), &[CardType::Creature],
    );
    state.battlefield.push(perm);
    id
}

/// When Solitude's ETB exiles a creature, `exile_linked` should record the link.
#[test]
fn test_solitude_etb_records_exile_link() {
    let (mut state, db) = setup_base();

    // Put Solitude on the battlefield controlled by P0
    let solitude_id = state.new_object_id();
    state.card_registry.push((solitude_id, CardName::Solitude));
    let def = find_card(&db, CardName::Solitude).unwrap();
    let perm = Permanent::new(
        solitude_id, CardName::Solitude, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Put an opponent creature on the battlefield
    let target_id = add_creature(&mut state, 1, CardName::GoblinGuide, 2, 2);

    // Manually fire the Solitude ETB trigger resolution (simulates what happens when ETB resolves)
    // We do this by pushing a SolitudeETB trigger and resolving it
    use crate::stack::*;
    state.stack.push(
        StackItemKind::TriggeredAbility {
            source_id: solitude_id,
            source_name: CardName::Solitude,
            effect: TriggeredEffect::SolitudeETB,
        },
        0,
        vec![Target::Object(target_id)],
    );

    // Resolve the trigger (both players pass)
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes -> trigger resolves

    // Creature should be in exile
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == target_id),
        "Target creature should be in exile after Solitude ETB"
    );

    // exile_linked should record (solitude_id, target_id)
    assert!(
        state.exile_linked.iter().any(|(exiler, exiled)| *exiler == solitude_id && *exiled == target_id),
        "exile_linked should record (Solitude id, exiled creature id)"
    );
}

/// When Solitude leaves the battlefield, the exiled creature should return.
#[test]
fn test_solitude_leaves_returns_exiled_creature() {
    let (mut state, db) = setup_base();

    // Put Solitude on the battlefield
    let solitude_id = state.new_object_id();
    state.card_registry.push((solitude_id, CardName::Solitude));
    let def = find_card(&db, CardName::Solitude).unwrap();
    let perm = Permanent::new(
        solitude_id, CardName::Solitude, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Put opponent creature on battlefield, then exile it manually via exile_linked
    let goblin_id = state.new_object_id();
    state.card_registry.push((goblin_id, CardName::GoblinGuide));
    state.exile.push((goblin_id, CardName::GoblinGuide, 1));
    state.exile_linked.push((solitude_id, goblin_id));

    // Remove Solitude from the battlefield (destroy it)
    // This should trigger check_leaves_triggers and push ExileLinkedReturn
    state.remove_permanent_to_zone(solitude_id, DestinationZone::Graveyard);

    // Stack should now have the ExileLinkedReturn trigger
    assert!(
        !state.stack.is_empty(),
        "Stack should have ExileLinkedReturn trigger after Solitude leaves"
    );

    // exile_linked should be cleared for this exiler
    assert!(
        !state.exile_linked.iter().any(|(exiler, _)| *exiler == solitude_id),
        "exile_linked should be cleared after Solitude leaves"
    );

    // Resolve the return trigger
    state.pass_priority(&db); // P0 passes
    state.pass_priority(&db); // P1 passes -> trigger resolves

    // Goblin should no longer be in exile
    assert!(
        !state.exile.iter().any(|(id, _, _)| *id == goblin_id),
        "GoblinGuide should no longer be in exile after return trigger resolves"
    );

    // Goblin should be back on the battlefield
    assert!(
        state.battlefield.iter().any(|p| p.id == goblin_id),
        "GoblinGuide should be back on the battlefield"
    );
}

/// Skyclave Apparition ETB: exiles opponent's permanent, records exile link and token MV.
#[test]
fn test_skyclave_apparition_etb_records_link() {
    let (mut state, db) = setup_base();

    // Put Skyclave Apparition on battlefield controlled by P0
    let app_id = state.new_object_id();
    state.card_registry.push((app_id, CardName::SkyclaveApparition));
    let def = find_card(&db, CardName::SkyclaveApparition).unwrap();
    let perm = Permanent::new(
        app_id, CardName::SkyclaveApparition, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Put an opponent nonland permanent on battlefield (GoblinGuide, MV = 1)
    let target_id = add_creature(&mut state, 1, CardName::GoblinGuide, 2, 2);

    // Fire the SkyclaveApparitionETB trigger
    use crate::stack::*;
    state.stack.push(
        StackItemKind::TriggeredAbility {
            source_id: app_id,
            source_name: CardName::SkyclaveApparition,
            effect: TriggeredEffect::SkyclaveApparitionETB,
        },
        0,
        vec![Target::Object(target_id)],
    );

    // Resolve trigger
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Target should be exiled
    assert!(
        state.exile.iter().any(|(id, _, _)| *id == target_id),
        "Target should be exiled by Skyclave Apparition ETB"
    );

    // exile_linked should record the link
    assert!(
        state.exile_linked.iter().any(|(exiler, exiled)| *exiler == app_id && *exiled == target_id),
        "exile_linked should record (Skyclave id, exiled permanent id)"
    );

    // skyclave_token_mv should record MV (GoblinGuide MV = 1)
    let goblin_mv = find_card(&db, CardName::GoblinGuide).unwrap().mana_cost.cmc() as u32;
    assert!(
        state.skyclave_token_mv.iter().any(|(app, mv)| *app == app_id && *mv == goblin_mv),
        "skyclave_token_mv should record MV of exiled card"
    );
}

/// When Skyclave Apparition leaves, opponent gets an X/X token and exiled card returns.
#[test]
fn test_skyclave_apparition_leaves_creates_token_and_returns_card() {
    let (mut state, db) = setup_base();

    // Set up Skyclave Apparition on battlefield
    let app_id = state.new_object_id();
    state.card_registry.push((app_id, CardName::SkyclaveApparition));
    let def = find_card(&db, CardName::SkyclaveApparition).unwrap();
    let perm = Permanent::new(
        app_id, CardName::SkyclaveApparition, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Exiled card: GoblinGuide with MV = 1, owned by P1
    let goblin_id = state.new_object_id();
    state.card_registry.push((goblin_id, CardName::GoblinGuide));
    state.exile.push((goblin_id, CardName::GoblinGuide, 1));
    state.exile_linked.push((app_id, goblin_id));
    let token_mv: u32 = 1;
    state.skyclave_token_mv.push((app_id, token_mv));

    let bf_before = state.battlefield.len();

    // Remove Skyclave Apparition from battlefield
    state.remove_permanent_to_zone(app_id, DestinationZone::Graveyard);

    // Two triggers should be on the stack: ExileLinkedReturn and SkyclaveApparitionLeaves
    let stack_len = state.stack.items().len();
    assert!(
        stack_len >= 2,
        "Stack should have at least 2 triggers (return + token), got {}", stack_len
    );

    // skyclave_token_mv should be cleared
    assert!(
        !state.skyclave_token_mv.iter().any(|(a, _)| *a == app_id),
        "skyclave_token_mv should be cleared after Apparition leaves"
    );

    // Resolve all triggers
    state.pass_priority(&db); // P0
    state.pass_priority(&db); // P1 -> first trigger resolves
    state.pass_priority(&db); // P0
    state.pass_priority(&db); // P1 -> second trigger resolves

    // Exiled card should be back on battlefield
    assert!(
        state.battlefield.iter().any(|p| p.id == goblin_id),
        "GoblinGuide should return to battlefield after Apparition leaves"
    );

    // Opponent (P1) should have a token on the battlefield (X/X where X = 1)
    let p1_tokens: Vec<_> = state.battlefield.iter()
        .filter(|p| p.controller == 1 && p.is_token && p.power() == token_mv as i16)
        .collect();
    assert!(
        !p1_tokens.is_empty(),
        "P1 should have an X/X token on the battlefield (X = {})", token_mv
    );
}
