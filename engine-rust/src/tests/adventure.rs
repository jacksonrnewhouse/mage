/// Tests for adventure cards (Bonecrusher Giant / Stomp, Brazen Borrower / Petty Theft).
/// Adventure cards have two halves: a creature and an instant/sorcery "adventure."
/// Cast the adventure from hand → it goes to exile → cast the creature from exile.

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::stack::{StackItemKind, TriggeredEffect};
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

fn add_creature(
    state: &mut GameState,
    controller: u8,
    card_name: CardName,
    power: i16,
    toughness: i16,
) -> ObjectId {
    let id = state.new_object_id();
    state.card_registry.push((id, card_name));
    let perm = Permanent::new(
        id,
        card_name,
        controller,
        controller,
        Some(power),
        Some(toughness),
        None,
        Keywords::empty(),
        &[CardType::Creature],
    );
    state.battlefield.push(perm);
    id
}

// ── Test 1: Can cast adventure from hand ─────────────────────────────────────

/// Bonecrusher Giant in hand with {1}{R} mana should generate a CastAdventure action.
#[test]
fn test_can_cast_stomp_adventure_from_hand() {
    let (mut state, db) = setup_base();

    // Put Bonecrusher Giant in hand
    let giant_id = state.new_object_id();
    state.card_registry.push((giant_id, CardName::BonecrusherGiant));
    state.players[0].hand.push(giant_id);

    // Give enough mana for Stomp ({1}{R})
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;

    // Add a target (player 1 can be targeted by Stomp)
    let actions = state.legal_actions(&db);
    let has_adventure = actions.iter().any(|a| {
        matches!(a, Action::CastAdventure { card_id, .. } if *card_id == giant_id)
    });
    assert!(
        has_adventure,
        "Should generate CastAdventure action for Bonecrusher Giant (Stomp) when {{1}}{{R}} available"
    );
}

/// Brazen Borrower in hand with {1}{U} should generate CastAdventure for Petty Theft.
#[test]
fn test_can_cast_petty_theft_adventure_from_hand() {
    let (mut state, db) = setup_base();

    // Put Brazen Borrower in hand
    let borrower_id = state.new_object_id();
    state.card_registry.push((borrower_id, CardName::BrazenBorrower));
    state.players[0].hand.push(borrower_id);

    // Give enough mana for Petty Theft ({1}{U})
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 1;

    // Add a target permanent for opponent
    let _ = add_creature(&mut state, 1, CardName::GoblinGuide, 2, 2);

    let actions = state.legal_actions(&db);
    let has_adventure = actions.iter().any(|a| {
        matches!(a, Action::CastAdventure { card_id, .. } if *card_id == borrower_id)
    });
    assert!(
        has_adventure,
        "Should generate CastAdventure action for Brazen Borrower (Petty Theft) when {{1}}{{U}} available"
    );
}

/// Without sufficient mana, no adventure action should be generated.
#[test]
fn test_cannot_cast_adventure_without_mana() {
    let (mut state, db) = setup_base();

    let giant_id = state.new_object_id();
    state.card_registry.push((giant_id, CardName::BonecrusherGiant));
    state.players[0].hand.push(giant_id);

    // No mana at all
    let actions = state.legal_actions(&db);
    let has_adventure = actions.iter().any(|a| {
        matches!(a, Action::CastAdventure { card_id, .. } if *card_id == giant_id)
    });
    assert!(
        !has_adventure,
        "Should NOT generate CastAdventure when player can't afford the adventure cost"
    );
}

// ── Test 2: Adventure goes to exile after resolving ───────────────────────────

/// After Stomp resolves, Bonecrusher Giant should be in exile (not graveyard or hand).
#[test]
fn test_stomp_exiles_card_after_resolving() {
    let (mut state, db) = setup_base();

    let giant_id = state.new_object_id();
    state.card_registry.push((giant_id, CardName::BonecrusherGiant));
    state.players[0].hand.push(giant_id);

    // Give mana for Stomp ({1}{R})
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;

    // Cast Stomp targeting player 1
    state.apply_action(
        &Action::CastAdventure {
            card_id: giant_id,
            targets: vec![Target::Player(1)],
        },
        &db,
    );

    // Card should be on the stack, not in hand
    assert!(
        !state.players[0].hand.contains(&giant_id),
        "Bonecrusher Giant should be removed from hand when Stomp is cast"
    );
    assert_eq!(state.stack.len(), 1, "Stomp should be on the stack");

    // Resolve: both players pass priority
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Card should now be in exile
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == giant_id);
    let in_graveyard = state.players[0].graveyard.contains(&giant_id);
    let in_hand = state.players[0].hand.contains(&giant_id);

    assert!(in_exile, "Bonecrusher Giant should be in exile after Stomp resolves");
    assert!(!in_graveyard, "Bonecrusher Giant should NOT be in graveyard after adventure");
    assert!(!in_hand, "Bonecrusher Giant should NOT return to hand after adventure");

    // adventure_exiled should track the card
    let in_adventure_exiled = state.adventure_exiled.iter().any(|(id, _)| *id == giant_id);
    assert!(
        in_adventure_exiled,
        "adventure_exiled should track Bonecrusher Giant after Stomp resolves"
    );
}

/// After Petty Theft resolves, Brazen Borrower should be in exile.
#[test]
fn test_petty_theft_exiles_card_after_resolving() {
    let (mut state, db) = setup_base();

    let borrower_id = state.new_object_id();
    state.card_registry.push((borrower_id, CardName::BrazenBorrower));
    state.players[0].hand.push(borrower_id);

    // Give mana for Petty Theft ({1}{U})
    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 1;

    // Add a target permanent for the opponent
    let goblin_id = add_creature(&mut state, 1, CardName::GoblinGuide, 2, 2);

    state.apply_action(
        &Action::CastAdventure {
            card_id: borrower_id,
            targets: vec![Target::Object(goblin_id)],
        },
        &db,
    );

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    let in_exile = state.exile.iter().any(|(id, _, _)| *id == borrower_id);
    assert!(in_exile, "Brazen Borrower should be in exile after Petty Theft resolves");

    let in_adventure_exiled = state.adventure_exiled.iter().any(|(id, _)| *id == borrower_id);
    assert!(
        in_adventure_exiled,
        "adventure_exiled should track Brazen Borrower after Petty Theft resolves"
    );
}

// ── Test 3: Can cast creature from exile ──────────────────────────────────────

/// After Stomp resolves (card in adventure exile), player can cast Bonecrusher Giant as creature.
#[test]
fn test_can_cast_creature_from_adventure_exile() {
    let (mut state, db) = setup_base();

    let giant_id = state.new_object_id();
    state.card_registry.push((giant_id, CardName::BonecrusherGiant));
    state.players[0].hand.push(giant_id);

    // Give mana for Stomp, then cast it
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;

    state.apply_action(
        &Action::CastAdventure {
            card_id: giant_id,
            targets: vec![Target::Player(1)],
        },
        &db,
    );

    // Resolve Stomp
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Now give mana for the creature ({2}{R})
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 2;

    // Should generate a CastCreatureFromAdventureExile action
    let actions = state.legal_actions(&db);
    let has_creature_cast = actions.iter().any(|a| {
        matches!(a, Action::CastCreatureFromAdventureExile { card_id } if *card_id == giant_id)
    });
    assert!(
        has_creature_cast,
        "Should generate CastCreatureFromAdventureExile for Bonecrusher Giant after Stomp resolved"
    );
}

/// After casting the creature from exile, it should enter the battlefield.
#[test]
fn test_creature_enters_battlefield_from_adventure_exile() {
    let (mut state, db) = setup_base();

    let giant_id = state.new_object_id();
    state.card_registry.push((giant_id, CardName::BonecrusherGiant));
    state.players[0].hand.push(giant_id);

    // Cast Stomp
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;
    state.apply_action(
        &Action::CastAdventure {
            card_id: giant_id,
            targets: vec![Target::Player(1)],
        },
        &db,
    );
    // Resolve Stomp
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Verify it's in adventure exile
    assert!(
        state.adventure_exiled.iter().any(|(id, _)| *id == giant_id),
        "Giant should be in adventure exile after Stomp"
    );

    // Give mana for Bonecrusher Giant ({2}{R})
    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 2;

    // Cast the creature half
    state.apply_action(
        &Action::CastCreatureFromAdventureExile { card_id: giant_id },
        &db,
    );

    // Giant should be on the stack
    assert_eq!(state.stack.len(), 1, "Bonecrusher Giant should be on the stack");

    // Not in exile anymore
    let in_exile = state.exile.iter().any(|(id, _, _)| *id == giant_id);
    assert!(!in_exile, "Giant should no longer be in exile once cast from adventure exile");

    // Not in adventure_exiled anymore
    let in_adventure_exiled = state.adventure_exiled.iter().any(|(id, _)| *id == giant_id);
    assert!(!in_adventure_exiled, "adventure_exiled should no longer track Giant once cast");

    // Resolve the creature spell
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Giant should now be on the battlefield
    let on_battlefield = state.battlefield.iter().any(|p| p.id == giant_id);
    assert!(on_battlefield, "Bonecrusher Giant should enter the battlefield after being cast from adventure exile");
}

// ── Test 4: Adventure effects ─────────────────────────────────────────────────

/// Stomp deals 2 damage to a player.
#[test]
fn test_stomp_deals_2_damage_to_player() {
    let (mut state, db) = setup_base();

    let giant_id = state.new_object_id();
    state.card_registry.push((giant_id, CardName::BonecrusherGiant));
    state.players[0].hand.push(giant_id);

    state.players[0].mana_pool.red = 1;
    state.players[0].mana_pool.colorless = 1;

    // Stomp targets player 1
    state.apply_action(
        &Action::CastAdventure {
            card_id: giant_id,
            targets: vec![Target::Player(1)],
        },
        &db,
    );

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    assert_eq!(
        state.players[1].life,
        18,
        "Player 1 should take 2 damage from Stomp"
    );
}

/// Petty Theft bounces an opponent's permanent to their hand.
#[test]
fn test_petty_theft_bounces_permanent() {
    let (mut state, db) = setup_base();

    let borrower_id = state.new_object_id();
    state.card_registry.push((borrower_id, CardName::BrazenBorrower));
    state.players[0].hand.push(borrower_id);

    state.players[0].mana_pool.blue = 1;
    state.players[0].mana_pool.colorless = 1;

    // Add a creature for player 1
    let goblin_id = add_creature(&mut state, 1, CardName::GoblinGuide, 2, 2);

    state.apply_action(
        &Action::CastAdventure {
            card_id: borrower_id,
            targets: vec![Target::Object(goblin_id)],
        },
        &db,
    );

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Goblin should be back in player 1's hand
    let goblin_on_bf = state.battlefield.iter().any(|p| p.id == goblin_id);
    let goblin_in_hand = state.players[1].hand.contains(&goblin_id);

    assert!(!goblin_on_bf, "Goblin Guide should no longer be on battlefield after Petty Theft");
    assert!(goblin_in_hand, "Goblin Guide should be returned to player 1's hand by Petty Theft");
}

/// A non-adventure card in hand should NOT generate CastAdventure actions.
#[test]
fn test_non_adventure_card_has_no_adventure_action() {
    let (mut state, db) = setup_base();

    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].hand.push(bolt_id);
    state.players[0].mana_pool.red = 1;

    let actions = state.legal_actions(&db);
    let has_adventure = actions.iter().any(|a| matches!(a, Action::CastAdventure { .. }));
    assert!(
        !has_adventure,
        "Lightning Bolt has no adventure — should not generate CastAdventure action"
    );
}

// ── Test: Bonecrusher Giant when-targeted damage trigger ─────────────────────

/// When Bonecrusher Giant on the battlefield is targeted by a spell,
/// it deals 2 damage to that spell's controller.
#[test]
fn test_bonecrusher_giant_targeting_trigger() {
    let (mut state, db) = setup_base();

    // Put Bonecrusher Giant on the battlefield for player 0
    let giant_id = add_creature(&mut state, 0, CardName::BonecrusherGiant, 4, 3);

    // Simulate the targeting trigger by calling check_bonecrusher_targeting_triggers
    // Player 1 is the spell controller targeting Bonecrusher Giant
    state.check_bonecrusher_targeting_triggers(&[giant_id], 1);

    // Should have a BonecrusherGiantTargeted trigger on the stack
    assert_eq!(state.stack.len(), 1, "Bonecrusher targeting trigger should be on the stack");
    let top = state.stack.top().unwrap();
    assert!(
        matches!(
            &top.kind,
            StackItemKind::TriggeredAbility {
                effect: TriggeredEffect::BonecrusherGiantTargeted { target_player: 1 },
                ..
            }
        ),
        "Stack item should be BonecrusherGiantTargeted with target_player = 1"
    );

    // Resolve the trigger
    state.resolve_top(&db);

    // Player 1 (the spell's controller) should have taken 2 damage
    assert_eq!(
        state.players[1].life, 18,
        "Spell controller should take 2 damage from Bonecrusher Giant trigger"
    );
}

/// When a non-Bonecrusher creature is targeted, no trigger fires.
#[test]
fn test_bonecrusher_trigger_does_not_fire_for_other_creatures() {
    let (mut state, db) = setup_base();

    // Put a non-Bonecrusher creature on the battlefield
    let goblin_id = add_creature(&mut state, 0, CardName::GoblinGuide, 2, 2);

    // Also put Bonecrusher on the battlefield (it only triggers when IT is targeted)
    let _giant_id = add_creature(&mut state, 0, CardName::BonecrusherGiant, 4, 3);

    // Target the Goblin Guide, not the Bonecrusher
    state.check_bonecrusher_targeting_triggers(&[goblin_id], 1);

    // No trigger should fire
    assert_eq!(state.stack.len(), 0, "Targeting a non-Bonecrusher creature should not trigger");
    assert_eq!(state.players[1].life, 20, "No damage should be dealt");
}
