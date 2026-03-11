use crate::action::{Action, ActionContext};
use crate::card::{build_card_db, find_card, CardName};
use crate::game::GameState;
use crate::permanent::Permanent;
use crate::types::*;

/// Helper: create and register a creature permanent for a player (not summoning sick).
fn put_creature(
    state: &mut GameState,
    db: &[crate::card::CardDef],
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
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);
    id
}

// ============================================================
// Protection prevents damage in combat
// ============================================================

/// Auriok Champion (protection from red) should not take damage from a red attacker.
#[test]
fn test_protection_prevents_damage_from_red() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0: Goblin Guide (red) attacks
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);
    // Player 1: Auriok Champion (pro red) defends
    let champion_id = put_creature(&mut state, &db, CardName::AuriokChampion, 1);
    // Fire ETB to set protections (pro black and pro red)
    state.handle_etb(CardName::AuriokChampion, champion_id, 1);

    // Set up combat: GG attacks, Champion blocks
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;
    state.apply_action(&Action::DeclareAttacker { creature_id: gg_id }, &db);
    state.apply_action(&Action::ConfirmAttackers, &db);

    // Manually declare Champion as blocker (bypassing protection for block declaration
    // — the protection rule for "can't be blocked" applies to the attacker's protections,
    // not the blocker's protections)
    state.blockers.push((champion_id, gg_id));
    state.apply_action(&Action::ConfirmBlockers, &db);

    let champion_damage_before = state
        .find_permanent(champion_id)
        .map(|p| p.damage)
        .unwrap_or(0);

    state.resolve_combat_damage(&db, false);

    let champion_damage_after = state
        .find_permanent(champion_id)
        .map(|p| p.damage)
        .unwrap_or(0);

    assert_eq!(
        champion_damage_after, champion_damage_before,
        "Auriok Champion has protection from red and should take no damage from Goblin Guide"
    );
}

/// Kor Firewalker (protection from red) should not take damage from a red attacker.
#[test]
fn test_kor_firewalker_protection_prevents_damage() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0: Goblin Guide (2/2 red) attacks
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);
    // Player 1: Kor Firewalker (2/2, pro red)
    let firewalker_id = put_creature(&mut state, &db, CardName::KorFirewalker, 1);
    // Fire ETB to set protection from red
    state.handle_etb(CardName::KorFirewalker, firewalker_id, 1);

    // Set up combat
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;
    state.apply_action(&Action::DeclareAttacker { creature_id: gg_id }, &db);
    state.apply_action(&Action::ConfirmAttackers, &db);

    // Force Kor Firewalker as blocker
    state.blockers.push((firewalker_id, gg_id));
    state.apply_action(&Action::ConfirmBlockers, &db);

    let fw_damage_before = state.find_permanent(firewalker_id).map(|p| p.damage).unwrap_or(0);

    state.resolve_combat_damage(&db, false);

    let fw_damage_after = state.find_permanent(firewalker_id).map(|p| p.damage).unwrap_or(0);

    assert_eq!(
        fw_damage_after, fw_damage_before,
        "Kor Firewalker has protection from red and should take no damage from Goblin Guide"
    );
}

/// A creature without protection should take normal damage (attacker deals unblocked damage to player).
#[test]
fn test_no_protection_damage_applies_normally() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0: Goblin Guide (red) attacks player 1, no blockers
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);

    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareAttackers);
    state.action_context = ActionContext::DeclareAttackers;
    state.apply_action(&Action::DeclareAttacker { creature_id: gg_id }, &db);
    state.apply_action(&Action::ConfirmAttackers, &db);
    state.apply_action(&Action::ConfirmBlockers, &db);

    let life_before = state.players[1].life;
    state.resolve_combat_damage(&db, false);
    let life_after = state.players[1].life;

    assert!(
        life_after < life_before,
        "Goblin Guide should deal damage to the player (no protection on defending player)"
    );
}

// ============================================================
// Protection prevents blocking
// ============================================================

/// An attacker with protection from a color cannot be blocked by creatures of that color.
#[test]
fn test_protection_prevents_blocking() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has an Auriok Champion (pro black/red) — attack
    let champion_id = put_creature(&mut state, &db, CardName::AuriokChampion, 0);
    // Fire ETB to set protections (pro black and pro red)
    state.handle_etb(CardName::AuriokChampion, champion_id, 0);

    // Player 1 has a Goblin Guide (red creature) to block
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 1);

    // Set up blocker generation phase
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareBlockers);
    state.action_context = ActionContext::DeclareBlockers;
    state.active_player = 0;

    // Declare champion as attacker
    state.attackers.push((champion_id, 1));

    let actions = state.legal_actions(&db);

    // There should be no DeclareBlocker action for Goblin Guide blocking champion
    // (because champion has protection from red)
    let red_blocker_action = actions.iter().any(|a| {
        matches!(a, Action::DeclareBlocker { blocker_id, attacker_id }
            if *blocker_id == gg_id && *attacker_id == champion_id)
    });

    assert!(
        !red_blocker_action,
        "Goblin Guide (red) should not be able to block Auriok Champion (protection from red)"
    );
}

/// A non-protected attacker should be blockable normally.
#[test]
fn test_no_protection_allows_blocking() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0 has a Goblin Guide (no protection)
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);
    // Player 1 has a Goblin Guide to block
    let blocker_id = put_creature(&mut state, &db, CardName::GoblinGuide, 1);

    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareBlockers);
    state.action_context = ActionContext::DeclareBlockers;
    state.active_player = 0;
    state.attackers.push((gg_id, 1));

    let actions = state.legal_actions(&db);

    let can_block = actions.iter().any(|a| {
        matches!(a, Action::DeclareBlocker { blocker_id: bid, attacker_id }
            if *bid == blocker_id && *attacker_id == gg_id)
    });

    assert!(can_block, "Goblin Guide should be able to block another Goblin Guide (no protection)");
}

// ============================================================
// Protection prevents targeting
// ============================================================

/// A creature with protection from red cannot be targeted by Lightning Bolt.
#[test]
fn test_protection_prevents_targeting() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 mana and a Lightning Bolt in hand
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].hand.push(bolt_id);
    state.players[0].mana_pool.add(Some(Color::Red), 1);

    // Player 1 has a Kor Firewalker (protection from red) on the battlefield
    let firewalker_id = put_creature(&mut state, &db, CardName::KorFirewalker, 1);
    // Fire ETB to set protection from red
    state.handle_etb(CardName::KorFirewalker, firewalker_id, 1);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let actions = state.legal_actions(&db);

    // There should be no CastSpell action targeting Kor Firewalker with Lightning Bolt
    let can_bolt_firewalker = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, targets, .. }
            if *card_id == bolt_id && targets.iter().any(|t| *t == Target::Object(firewalker_id)))
    });

    assert!(
        !can_bolt_firewalker,
        "Lightning Bolt (red) should not be able to target Kor Firewalker (protection from red)"
    );
}

/// A creature without protection can be targeted by Lightning Bolt.
#[test]
fn test_no_protection_allows_targeting() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Give player 0 mana and a Lightning Bolt
    let bolt_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.players[0].hand.push(bolt_id);
    state.players[0].mana_pool.add(Some(Color::Red), 1);

    // Player 1 has a True-Name Nemesis (no protection set yet)
    let tnn_id = put_creature(&mut state, &db, CardName::TrueNameNemesis, 1);

    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let actions = state.legal_actions(&db);

    let can_bolt_tnn = actions.iter().any(|a| {
        matches!(a, Action::CastSpell { card_id, targets, .. }
            if *card_id == bolt_id && targets.iter().any(|t| *t == Target::Object(tnn_id)))
    });

    assert!(
        can_bolt_tnn,
        "Lightning Bolt should be able to target True-Name Nemesis (no protection set)"
    );
}

// ============================================================
// True-Name Nemesis: ETB player choice and protection from chosen player
// ============================================================

/// True-Name Nemesis ETB triggers a pending choice; after choosing player 0,
/// it gains protection from player 0 (can't be blocked by player 0's creatures).
#[test]
fn test_true_name_nemesis_etb_sets_protection() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place TNN on the battlefield for player 1 and fire ETB
    let tnn_id = state.new_object_id();
    state.card_registry.push((tnn_id, CardName::TrueNameNemesis));
    let def = find_card(&db, CardName::TrueNameNemesis).unwrap();
    let mut perm = Permanent::new(
        tnn_id,
        CardName::TrueNameNemesis,
        1, // controller = player 1
        1,
        def.power,
        def.toughness,
        None,
        def.keywords,
        def.card_types,
    );
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);

    // Fire ETB — should create a pending choice for player selection
    state.handle_etb(CardName::TrueNameNemesis, tnn_id, 1);

    assert!(
        state.pending_choice.is_some(),
        "True-Name Nemesis ETB should set a pending choice for player selection"
    );

    // Resolve: choose player 0
    state.apply_action(&Action::ChooseNumber(0), &db);

    // TNN should now have protection from player 0
    let tnn = state.find_permanent(tnn_id).expect("TNN should still be on battlefield");
    assert!(
        tnn.has_protection_from_player(0),
        "True-Name Nemesis should have protection from chosen player 0"
    );
}

/// After True-Name Nemesis gains protection from player 0, player 0's red creatures
/// cannot block it.
#[test]
fn test_true_name_nemesis_cannot_be_blocked_by_protected_player() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Place TNN on battlefield for player 1, choose protection from player 0
    let tnn_id = state.new_object_id();
    state.card_registry.push((tnn_id, CardName::TrueNameNemesis));
    let def = find_card(&db, CardName::TrueNameNemesis).unwrap();
    let mut perm = Permanent::new(
        tnn_id, CardName::TrueNameNemesis, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.colors = def.color_identity.to_vec();
    perm.entered_this_turn = false;
    // Grant protection from player 0 directly
    perm.protections.push(Protection::FromPlayer(0));
    state.battlefield.push(perm);

    // Player 0 has a Goblin Guide as potential blocker
    let gg_id = put_creature(&mut state, &db, CardName::GoblinGuide, 0);

    // Set up blocker generation: TNN attacks (player 1 active), player 0 blocks
    state.phase = Phase::Combat;
    state.step = Some(Step::DeclareBlockers);
    state.action_context = ActionContext::DeclareBlockers;
    state.active_player = 1;
    state.attackers.push((tnn_id, 0));

    let actions = state.legal_actions(&db);

    let gg_can_block_tnn = actions.iter().any(|a| {
        matches!(a, Action::DeclareBlocker { blocker_id, attacker_id }
            if *blocker_id == gg_id && *attacker_id == tnn_id)
    });

    assert!(
        !gg_can_block_tnn,
        "Goblin Guide (controlled by player 0) should not block True-Name Nemesis (protection from player 0)"
    );
}

/// Auriok Champion has protection from both black AND red.
#[test]
fn test_auriok_champion_has_both_protections() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let champion_id = state.new_object_id();
    state.card_registry.push((champion_id, CardName::AuriokChampion));
    let def = find_card(&db, CardName::AuriokChampion).unwrap();
    let mut perm = Permanent::new(
        champion_id, CardName::AuriokChampion, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.colors = def.color_identity.to_vec();
    state.battlefield.push(perm);

    // Fire ETB to set protections
    state.handle_etb(CardName::AuriokChampion, champion_id, 0);

    let champion = state.find_permanent(champion_id).unwrap();
    assert!(
        champion.has_protection_from_color(Color::Black),
        "Auriok Champion should have protection from black"
    );
    assert!(
        champion.has_protection_from_color(Color::Red),
        "Auriok Champion should have protection from red"
    );
    assert!(
        !champion.has_protection_from_color(Color::Blue),
        "Auriok Champion should NOT have protection from blue"
    );
}
