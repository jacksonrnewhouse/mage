use crate::card::*;
use crate::game::*;
use crate::types::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};

/// Helper: put a creature on the battlefield for a player (not summoning sick).
fn put_creature(state: &mut GameState, db: &[CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let def = find_card(db, card_name).unwrap();
    let mut perm = Permanent::new(
        id, card_name, controller, controller,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);
    id
}

/// Helper: seed a player's library with N copies of a card.
fn seed_library(state: &mut GameState, player: PlayerId, card_name: CardName, count: usize) {
    for _ in 0..count {
        let id = state.new_object_id();
        state.card_registry.push((id, card_name));
        state.players[player as usize].library.push(id);
    }
}

/// Helper: drain all items from the stack by resolving them.
fn drain_stack(state: &mut GameState, db: &[CardDef]) {
    while !state.stack.is_empty() {
        state.resolve_top(db);
    }
}

#[test]
fn test_take_initiative() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Initially no initiative
    assert_eq!(state.initiative, None);

    // Player 0 takes the initiative — should also push a venture trigger onto the stack
    state.take_initiative(0);
    assert_eq!(state.initiative, Some(0));

    // Undercity room counter should have advanced from 0 to 1
    assert_eq!(state.undercity_room[0], 1);

    // A venture trigger should be on the stack
    assert!(!state.stack.is_empty(), "Venture trigger should be on the stack after taking initiative");

    // Player 1 takes the initiative (wresting it from player 0)
    drain_stack(&mut state, &db);
    state.take_initiative(1);
    assert_eq!(state.initiative, Some(1));
}

#[test]
fn test_venture_entrance_gain_life() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Room 0 = Entrance: gain 1 life
    let life_before = state.players[0].life;
    state.take_initiative(0);
    assert_eq!(state.undercity_room[0], 1, "After first venture, room should be 1");
    // Resolve the room trigger (Entrance: gain 1 life)
    state.resolve_top(&db);
    assert_eq!(state.players[0].life, life_before + 1, "Entrance: should gain 1 life");
}

#[test]
fn test_venture_archives_treasure() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Skip room 0 (Entrance)
    state.undercity_room[0] = 1;

    // Room 1 = Archives: create a Treasure token
    let bf_before = state.battlefield.len();
    state.take_initiative(0);
    assert_eq!(state.undercity_room[0], 2, "After second venture, room should be 2");
    // resolve_top runs the UndercityRoom(Archives) trigger
    state.resolve_top(&db);
    assert!(state.battlefield.len() > bf_before, "Archives: should create a Treasure token");
}

#[test]
fn test_venture_lost_well_draw() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    seed_library(&mut state, 0, CardName::LightningBolt, 5);

    // Skip to room 2 (Lost Well)
    state.undercity_room[0] = 2;

    let hand_before = state.players[0].hand.len();
    state.take_initiative(0);
    assert_eq!(state.undercity_room[0], 3, "After venture into Lost Well, room should be 3");
    state.resolve_top(&db);
    assert_eq!(state.players[0].hand.len(), hand_before + 1, "Lost Well: should draw a card");
}

#[test]
fn test_venture_forge_token() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Skip to room 3 (Forge)
    state.undercity_room[0] = 3;

    let bf_before = state.battlefield.len();
    state.take_initiative(0);
    assert_eq!(state.undercity_room[0], 4, "After venture into Forge, room should be 4 (complete)");
    state.resolve_top(&db);
    assert!(state.battlefield.len() > bf_before, "Forge: should create a 4/1 Devil token");
}

#[test]
fn test_venture_inner_sanctum_draw_three() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    seed_library(&mut state, 0, CardName::LightningBolt, 10);

    // Room counter already at 4 (dungeon complete, Inner Sanctum fires again)
    state.undercity_room[0] = 4;

    let hand_before = state.players[0].hand.len();
    state.take_initiative(0);
    // Room counter stays at 4 (already complete)
    assert_eq!(state.undercity_room[0], 4, "Dungeon complete: room counter stays at 4");
    state.resolve_top(&db);
    assert_eq!(state.players[0].hand.len(), hand_before + 3, "Inner Sanctum: should draw 3 cards");
}

#[test]
fn test_initiative_upkeep_ventures() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give initiative to player 0 (also ventures once immediately)
    state.take_initiative(0);
    assert_eq!(state.undercity_room[0], 1);
    // Drain the stack from the take_initiative venture
    drain_stack(&mut state, &db);

    // Set up for upkeep: player 0 is active, phase = Beginning/Untap
    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);

    // Advancing from Untap to Upkeep should push an InitiativeUpkeep trigger
    state.advance_phase();
    assert_eq!(state.step, Some(Step::Upkeep));

    // Stack should have the initiative upkeep trigger
    assert!(!state.stack.is_empty(), "Initiative upkeep trigger should be on the stack");
    let top = state.stack.top().unwrap();
    assert!(
        matches!(top.kind, StackItemKind::TriggeredAbility { effect: TriggeredEffect::InitiativeUpkeep, .. }),
        "Top of stack should be InitiativeUpkeep, got {:?}", top.kind
    );
}

#[test]
fn test_initiative_upkeep_does_not_trigger_for_non_holder() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 1 has the initiative, but player 0 is active
    state.take_initiative(1);
    drain_stack(&mut state, &db); // drain the immediate venture trigger

    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase();

    // No initiative upkeep trigger should fire (wrong player's upkeep)
    assert!(state.stack.is_empty(), "Initiative upkeep should not trigger on non-holder's upkeep");
}

#[test]
fn test_combat_damage_steals_initiative() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has the initiative
    state.take_initiative(0);
    drain_stack(&mut state, &db); // drain venture trigger
    assert_eq!(state.initiative, Some(0));

    // Put a Goblin Guide for player 1 on the battlefield
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 1);

    // Goblin Guide (player 1) attacks player 0 (the initiative holder)
    state.attackers.push((gg_id, 0));

    // Resolve combat damage
    state.resolve_combat_damage(&db, false);

    // Player 1 should now have the initiative
    assert_eq!(state.initiative, Some(1), "Player 1 should take the initiative after dealing combat damage to player 0");
}

#[test]
fn test_combat_damage_to_initiative_holder_steals_initiative() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 1 has the initiative
    state.take_initiative(1);
    drain_stack(&mut state, &db);
    assert_eq!(state.initiative, Some(1));

    // Player 0's creature attacks player 1 (the initiative holder)
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);
    state.attackers.push((gg_id, 1));

    state.resolve_combat_damage(&db, false);

    // Player 0 should now have the initiative
    assert_eq!(state.initiative, Some(0), "Player 0 should take the initiative after dealing damage to player 1 (the holder)");
}

#[test]
fn test_white_plume_adventurer_etb_takes_initiative() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    assert_eq!(state.initiative, None);

    // Simulate White Plume Adventurer entering the battlefield for player 0
    let wpa_id = state.new_object_id();
    state.card_registry.push((wpa_id, CardName::WhitePlumeAdventurer));

    state.handle_etb(CardName::WhitePlumeAdventurer, wpa_id, 0);

    // Player 0 should have the initiative
    assert_eq!(state.initiative, Some(0), "White Plume Adventurer ETB should give its controller the initiative");

    // A venture trigger should be on the stack
    assert!(!state.stack.is_empty(), "A venture trigger should be on the stack after White Plume ETB");
}

#[test]
fn test_no_initiative_no_upkeep_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // No initiative set
    assert_eq!(state.initiative, None);

    state.active_player = 0;
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.advance_phase();

    // No initiative → no upkeep trigger
    assert!(state.stack.is_empty(), "No initiative means no upkeep venture trigger");
}
