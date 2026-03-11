/// Tests for replacement effects: Rest in Peace, Grafdigger's Cage, Containment Priest.

use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

fn place_on_battlefield(state: &mut GameState, card_name: CardName, controller: PlayerId, db: &[CardDef]) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);
    id
}

fn put_in_hand(state: &mut GameState, card_name: CardName, player: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].hand.push(id);
    id
}

fn put_in_graveyard(state: &mut GameState, card_name: CardName, player: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    state.players[player as usize].graveyard.push(id);
    id
}

// ===== Rest in Peace =====

#[test]
fn test_rest_in_peace_etb_exiles_all_graveyards() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put some cards in graveyards for both players
    put_in_graveyard(&mut state, CardName::LightningBolt, 0);
    put_in_graveyard(&mut state, CardName::LightningBolt, 0);
    put_in_graveyard(&mut state, CardName::Counterspell, 1);

    assert_eq!(state.players[0].graveyard.len(), 2);
    assert_eq!(state.players[1].graveyard.len(), 1);
    assert_eq!(state.exile.len(), 0);

    // Rest in Peace enters the battlefield: exile all graveyards
    let rip_id = state.new_object_id();
    state.card_registry.push((rip_id, CardName::RestInPeace));
    let def = find_card(&db, CardName::RestInPeace).unwrap();
    let perm = Permanent::new(
        rip_id, CardName::RestInPeace, 0, 0,
        def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);
    state.handle_etb(CardName::RestInPeace, rip_id, 0);

    // All graveyards should be empty, cards in exile
    assert_eq!(state.players[0].graveyard.len(), 0, "P0 graveyard should be exiled");
    assert_eq!(state.players[1].graveyard.len(), 0, "P1 graveyard should be exiled");
    assert_eq!(state.exile.len(), 3, "All 3 cards should be in exile");
}

#[test]
fn test_rest_in_peace_creatures_go_to_exile_on_death() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Rest in Peace on the battlefield
    place_on_battlefield(&mut state, CardName::RestInPeace, 0, &db);

    // Place a creature
    let bear_id = place_on_battlefield(&mut state, CardName::GoblinGuide, 1, &db);

    // Destroy the creature - should go to exile, not graveyard
    state.destroy_permanent(bear_id);

    assert_eq!(state.players[1].graveyard.len(), 0, "Graveyard should be empty with Rest in Peace");
    assert_eq!(state.exile.len(), 1, "Creature should be in exile");
}

#[test]
fn test_rest_in_peace_instant_goes_to_exile_after_resolution() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Rest in Peace on battlefield controlled by P0
    place_on_battlefield(&mut state, CardName::RestInPeace, 0, &db);

    // Set up P1 to cast Lightning Bolt
    let bolt_id = put_in_hand(&mut state, CardName::LightningBolt, 1);
    state.players[1].mana_pool.red = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 1;
    state.priority_player = 1;

    // Cast bolt, then resolve
    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(0)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: None,
        },
        &db,
    );
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bolt should be in exile, not graveyard
    assert_eq!(state.players[1].graveyard.len(), 0, "P1 graveyard should be empty with Rest in Peace");
    assert_eq!(state.exile.iter().any(|(id, _, _)| *id == bolt_id), true, "Bolt should be exiled");
}

#[test]
fn test_rest_in_peace_permanent_destroyed_goes_to_exile() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Rest in Peace on the battlefield for P0
    place_on_battlefield(&mut state, CardName::RestInPeace, 0, &db);

    // Place an artifact for P1
    let ring_id = place_on_battlefield(&mut state, CardName::SolRing, 1, &db);

    // Remove to graveyard - should be redirected to exile
    state.remove_permanent_to_zone(ring_id, DestinationZone::Graveyard);

    assert_eq!(state.players[1].graveyard.len(), 0, "Graveyard should be empty under Rest in Peace");
    assert_eq!(state.exile.iter().any(|(id, _, _)| *id == ring_id), true, "Sol Ring should be in exile");
}

// ===== Grafdigger's Cage =====

#[test]
fn test_grafdiggers_cage_blocks_casting_from_graveyard() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Grafdigger's Cage on battlefield
    place_on_battlefield(&mut state, CardName::GrafdiggersCage, 1, &db);

    // Put a flashback card in P0's graveyard
    let grudge_id = put_in_graveyard(&mut state, CardName::AncientGrudge, 0);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    // Give P0 enough mana to cast from graveyard
    state.players[0].mana_pool.green = 1;
    state.players[0].mana_pool.red = 1;

    // Cage should prevent casting from graveyard
    let actions = state.legal_actions(&db);
    let can_cast_from_gy = actions.iter().any(|a| {
        matches!(a, crate::action::Action::CastSpell { card_id, from_graveyard: true, .. } if *card_id == grudge_id)
    });
    assert!(!can_cast_from_gy, "Grafdigger's Cage should prevent casting from graveyard");
}

#[test]
fn test_grafdiggers_cage_blocks_reanimate() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Grafdigger's Cage
    place_on_battlefield(&mut state, CardName::GrafdiggersCage, 1, &db);

    // Put a creature in P1's graveyard
    let bear_id = put_in_graveyard(&mut state, CardName::GoblinGuide, 1);

    // P0 casts Reanimate targeting the bear in P1's graveyard
    // Simplified: directly call resolve with the target
    let reanimate_id = state.new_object_id();
    state.card_registry.push((reanimate_id, CardName::Reanimate));
    state.players[0].hand.push(reanimate_id);
    state.players[0].mana_pool.black = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: reanimate_id,
            targets: vec![Target::Object(bear_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: None,
        },
        &db,
    );
    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bear should NOT be on the battlefield - Cage prevents it
    let bear_on_battlefield = state.battlefield.iter().any(|p| p.id == bear_id);
    assert!(!bear_on_battlefield, "Grafdigger's Cage should prevent reanimation");
    // Bear should be in exile (Cage redirects it)
    let bear_in_exile = state.exile.iter().any(|(id, _, _)| *id == bear_id);
    assert!(bear_in_exile, "Grafdigger's Cage should exile the creature instead");
}

// ===== Containment Priest =====

#[test]
fn test_containment_priest_blocks_reanimate() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place Containment Priest on the battlefield for P0
    place_on_battlefield(&mut state, CardName::ContainmentPriest, 0, &db);

    // Put a creature in P1's graveyard
    let bear_id = put_in_graveyard(&mut state, CardName::GoblinGuide, 1);

    // P0 casts Reanimate targeting the bear
    let reanimate_id = state.new_object_id();
    state.card_registry.push((reanimate_id, CardName::Reanimate));
    state.players[0].hand.push(reanimate_id);
    state.players[0].mana_pool.black = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    state.apply_action(
        &crate::action::Action::CastSpell {
            card_id: reanimate_id,
            targets: vec![Target::Object(bear_id)],
            x_value: 0,
            from_graveyard: false,
            alt_cost: None,
        },
        &db,
    );
    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bear should NOT be on the battlefield - Containment Priest blocks uncasted creatures
    let bear_on_battlefield = state.battlefield.iter().any(|p| p.id == bear_id);
    assert!(!bear_on_battlefield, "Containment Priest should prevent reanimation onto the battlefield");
    // Bear should be in exile
    let bear_in_exile = state.exile.iter().any(|(id, _, _)| *id == bear_id);
    assert!(bear_in_exile, "Containment Priest should exile the creature instead");
}

#[test]
fn test_grafdiggers_cage_active_check() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // No cage at first
    assert!(!state.grafdiggers_cage_active(), "No cage - should return false");

    // Add cage
    place_on_battlefield(&mut state, CardName::GrafdiggersCage, 0, &db);
    assert!(state.grafdiggers_cage_active(), "Cage on battlefield - should return true");
}

#[test]
fn test_rest_in_peace_graveyard_destination() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Without RIP: destination is Graveyard
    assert_eq!(
        state.graveyard_destination(0),
        DestinationZone::Graveyard,
        "Without Rest in Peace, destination should be Graveyard"
    );

    // With RIP on battlefield: destination is Exile
    place_on_battlefield(&mut state, CardName::RestInPeace, 0, &db);
    assert_eq!(
        state.graveyard_destination(0),
        DestinationZone::Exile,
        "With Rest in Peace, destination should be Exile"
    );
    // Also applies to the opponent's cards
    assert_eq!(
        state.graveyard_destination(1),
        DestinationZone::Exile,
        "Rest in Peace applies to all players"
    );
}
