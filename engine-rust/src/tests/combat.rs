use crate::card::*;
use crate::action::*;
use crate::types::*;
use crate::game::*;

#[test]
fn test_creature_combat() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a Goblin Guide on the battlefield for player 0
    let gg_id = state.new_object_id();
    state.card_registry.push((gg_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        gg_id,
        CardName::GoblinGuide,
        0,
        0,
        def.power,
        def.toughness,
        None,
        def.keywords,
        def.card_types,
    );
    perm.entered_this_turn = false; // Not summoning sick
    state.battlefield.push(perm);

    // Move to combat
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;

    // Declare attacker
    state.apply_action(
        &Action::DeclareAttacker { creature_id: gg_id },
        &db,
    );

    assert_eq!(state.attackers.len(), 1);

    // Confirm attackers
    state.apply_action(&Action::ConfirmAttackers, &db);

    // Confirm blockers (no blockers)
    state.apply_action(&Action::ConfirmBlockers, &db);

    // Resolve combat damage
    state.resolve_combat_damage(false);

    // Opponent should have taken 2 damage (Goblin Guide is 2/2 with haste)
    assert_eq!(state.players[1].life, 18);
}
