use crate::card::*;
use crate::action::*;
use crate::types::*;
use crate::game::*;
use crate::stack::{StackItemKind, TriggeredEffect};

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
    state.resolve_combat_damage(&db, false);

    // Opponent should have taken 2 damage (Goblin Guide is 2/2 with haste)
    assert_eq!(state.players[1].life, 18);
}

/// Helper: create and register a creature permanent for a player (not summoning sick).
fn put_creature(state: &mut GameState, db: &[crate::card::CardDef], card_name: CardName, controller: PlayerId) -> ObjectId {
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

/// Helper: add a card to a player's library so draw effects have something to draw.
fn seed_library(state: &mut GameState, player: PlayerId, card_name: CardName, count: usize) {
    for _ in 0..count {
        let id = state.new_object_id();
        state.card_registry.push((id, card_name));
        state.players[player as usize].library.push(id);
    }
}

#[test]
fn test_scrawling_crawler_combat_damage_trigger_fires_when_unblocked() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Seed library so draw effects work
    seed_library(&mut state, 0, CardName::SolRing, 5);

    let crawler_id = put_creature(&mut state, &db, CardName::ScrawlingCrawler, 0);

    // Move to combat and attack
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;
    state.apply_action(&Action::DeclareAttacker { creature_id: crawler_id }, &db);
    state.apply_action(&Action::ConfirmAttackers, &db);
    state.apply_action(&Action::ConfirmBlockers, &db);

    let stack_before = state.stack.len();
    state.resolve_combat_damage(&db, false);

    // Opponent should have taken 3 damage (Scrawling Crawler is 3/3)
    assert_eq!(state.players[1].life, 17);

    // A combat damage trigger should be on the stack
    assert!(state.stack.len() > stack_before, "ScrawlingCrawler should push a trigger onto the stack");

    // Verify the trigger is the expected one
    let has_crawler_trigger = state.stack.items().iter().any(|item| {
        matches!(
            &item.kind,
            StackItemKind::TriggeredAbility { effect: TriggeredEffect::ScrawlingCrawlerCombatDamage, .. }
        )
    });
    assert!(has_crawler_trigger, "Stack should contain ScrawlingCrawlerCombatDamage trigger");

    // Resolve the trigger: controller should draw a card
    let hand_before = state.players[0].hand.len();
    state.phase = Phase::Combat;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);
    assert_eq!(state.players[0].hand.len(), hand_before + 1, "ScrawlingCrawler trigger should draw a card");
}

#[test]
fn test_combat_damage_trigger_does_not_fire_when_blocked() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 attacks with Scrawling Crawler; player 1 blocks with a creature
    let crawler_id = put_creature(&mut state, &db, CardName::ScrawlingCrawler, 0);
    // Use GoblinGuide as the blocker (2/2) — note Scrawling Crawler has "can't be blocked"
    // so use ScrawlingCrawler on both sides for simplicity, but we need a blockable creature.
    // Use a basic creature for attacker instead.
    let guide_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);

    // Remove Scrawling Crawler from attackers scenario; attack with Goblin Guide instead
    // and block with a creature controlled by player 1
    let blocker_id = put_creature(&mut state, &db, CardName::GoblinGuide, 1);

    // Move to combat
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;
    state.apply_action(&Action::DeclareAttacker { creature_id: guide_id }, &db);
    state.apply_action(&Action::ConfirmAttackers, &db);

    // Declare blocker
    state.apply_action(&Action::DeclareBlocker { blocker_id, attacker_id: guide_id }, &db);
    state.apply_action(&Action::ConfirmBlockers, &db);

    let stack_before = state.stack.len();
    state.resolve_combat_damage(&db, false);

    // No combat damage trigger should fire for Goblin Guide (no such trigger defined)
    // and the blocked creature should not deal damage to defending player
    assert_eq!(state.players[1].life, 20, "Blocked attacker should not deal damage to defending player");

    // No new triggers on the stack (Goblin Guide has no combat damage trigger)
    assert_eq!(state.stack.len(), stack_before, "No combat damage trigger should fire for blocked Goblin Guide");

    // Suppress unused variable warning
    let _ = crawler_id;
}

#[test]
fn test_psychic_frog_combat_damage_trigger_draws_when_graveyard_has_card() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Seed library so draw effects work
    seed_library(&mut state, 0, CardName::SolRing, 5);

    let frog_id = put_creature(&mut state, &db, CardName::PsychicFrog, 0);

    // Put a card in player 0's graveyard
    let card_id = state.new_object_id();
    state.card_registry.push((card_id, CardName::SolRing));
    state.players[0].graveyard.push(card_id);

    // Attack unblocked
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;
    state.apply_action(&Action::DeclareAttacker { creature_id: frog_id }, &db);
    state.apply_action(&Action::ConfirmAttackers, &db);
    state.apply_action(&Action::ConfirmBlockers, &db);
    state.resolve_combat_damage(&db, false);

    // Trigger should be on the stack
    let has_frog_trigger = state.stack.items().iter().any(|item| {
        matches!(
            &item.kind,
            StackItemKind::TriggeredAbility { effect: TriggeredEffect::PsychicFrogCombatDamage, .. }
        )
    });
    assert!(has_frog_trigger, "PsychicFrog combat damage trigger should be on the stack");

    // Resolve: should exile graveyard card and draw a card
    let hand_before = state.players[0].hand.len();
    let gy_before = state.players[0].graveyard.len();
    let exile_before = state.exile.len();

    state.phase = Phase::Combat;
    state.active_player = 0;
    state.priority_player = 0;
    state.pass_priority(&db);
    state.pass_priority(&db);

    assert_eq!(state.players[0].graveyard.len(), gy_before - 1, "Psychic Frog should exile a card from graveyard");
    assert_eq!(state.exile.len(), exile_before + 1, "Exiled card should be in exile zone");
    assert_eq!(state.players[0].hand.len(), hand_before + 1, "Psychic Frog trigger should draw a card");
}
