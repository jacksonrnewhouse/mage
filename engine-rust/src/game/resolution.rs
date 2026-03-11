/// Spell and ability resolution logic.

use crate::action::*;
use crate::card::*;
use super::card_name_for_token;
use crate::game::{ChoiceKind, ChoiceReason, DestinationZone, GameState, PendingChoice};
use crate::permanent::*;
use crate::stack::*;
use crate::types::*;

impl GameState {
    pub fn resolve_top(&mut self, db: &[CardDef]) {
        if let Some(item) = self.stack.pop() {
            match item.kind {
                StackItemKind::Spell { card_name, card_id, cast_via_evoke } => {
                    self.resolve_spell(card_name, card_id, item.controller, &item.targets, item.x_value, item.cast_from_graveyard, cast_via_evoke, db);
                }
                StackItemKind::TriggeredAbility { effect, .. } => {
                    self.resolve_triggered(effect, item.controller, &item.targets);
                }
                StackItemKind::ActivatedAbility { effect, .. } => {
                    self.resolve_activated(effect, item.controller, &item.targets);
                }
            }
            self.check_state_based_actions(db);
        }
    }

    fn resolve_spell(
        &mut self,
        card_name: CardName,
        card_id: ObjectId,
        controller: PlayerId,
        targets: &[Target],
        x_value: u8,
        cast_from_graveyard: bool,
        cast_via_evoke: bool,
        db: &[CardDef],
    ) {
        let card_def = find_card(db, card_name);
        let is_permanent = card_def
            .map(|c| {
                c.card_types.iter().any(|t| matches!(t,
                    CardType::Creature | CardType::Artifact | CardType::Enchantment
                    | CardType::Planeswalker | CardType::Land
                ))
            })
            .unwrap_or(false);

        if is_permanent {
            // Put permanent onto battlefield
            if let Some(def) = card_def {
                let perm = Permanent::new(
                    card_id,
                    card_name,
                    controller,
                    controller,
                    def.power,
                    def.toughness,
                    def.loyalty,
                    def.keywords,
                    def.card_types,
                );
                self.battlefield.push(perm);
                // ETB triggers
                self.handle_etb_with_x(card_name, card_id, controller, x_value);
                // Handle ETB effects that need the cast targets (e.g. Snapcaster Mage)
                self.handle_etb_with_cast_targets(card_name, card_id, controller, targets);
                // Evoke sacrifice trigger: when cast via evoke, the creature is sacrificed
                // after ETB triggers resolve (goes on stack under ETB triggers, resolves last).
                if cast_via_evoke {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::EvokeSacrifice { permanent_id: card_id },
                        },
                        controller,
                        vec![],
                    );
                }
            }
        } else {
            // Instant/sorcery: resolve effect, then place in appropriate zone.
            // If cast via flashback (or via Yawgmoth's Will), exile instead of going to graveyard.
            self.resolve_card_effect_with_x(card_name, controller, targets, x_value, db);
            if cast_from_graveyard {
                // Exile the card (flashback / Yawgmoth's Will rule: if it would go to graveyard, exile it)
                // The card was already removed from graveyard when cast; just push to exile.
                self.exile.push((card_id, card_name, controller));
            } else {
                // Apply graveyard-replacement effects (Rest in Peace).
                self.send_to_graveyard(card_id, card_name, controller);
            }
        }
    }

    fn resolve_card_effect_with_x(
        &mut self,
        card_name: CardName,
        controller: PlayerId,
        targets: &[Target],
        _x_value: u8,
        db: &[CardDef],
    ) {
        self.resolve_card_effect(card_name, controller, targets, db);
    }

    fn resolve_card_effect(
        &mut self,
        card_name: CardName,
        controller: PlayerId,
        targets: &[Target],
        db: &[CardDef],
    ) {
        match card_name {
            // === Draw spells ===
            CardName::AncestralRecall => {
                let target_player = match targets.first() {
                    Some(Target::Player(p)) => *p,
                    _ => controller,
                };
                self.draw_cards(target_player, 3);
            }

            // === Counterspells ===
            CardName::Counterspell | CardName::ForceOfWill | CardName::ManaDrain
            | CardName::ForceOfNegation | CardName::MindbreakTrap => {
                if let Some(Target::Object(spell_id)) = targets.first() {
                    // Check if the targeted spell can't be countered
                    let is_uncounterable = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .map(|item| item.cant_be_countered)
                        .unwrap_or(false);
                    if !is_uncounterable {
                        if let Some(item) = self.stack.remove(*spell_id) {
                            self.route_countered_spell(item);
                        }
                    }
                }
            }
            CardName::MemoryLapse => {
                // Counter target spell; its owner puts it on top of their library
                if let Some(Target::Object(spell_id)) = targets.first() {
                    let is_uncounterable = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .map(|item| item.cant_be_countered)
                        .unwrap_or(false);
                    if !is_uncounterable {
                        if let Some(item) = self.stack.remove(*spell_id) {
                            if let crate::stack::StackItemKind::Spell { card_id, .. } = item.kind {
                                // Put on top of owner's library (top = last element)
                                self.players[item.controller as usize].library.push(card_id);
                            }
                        }
                    }
                }
            }
            CardName::Remand => {
                // Counter target spell; return it to owner's hand, controller draws 1
                if let Some(Target::Object(spell_id)) = targets.first() {
                    let is_uncounterable = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .map(|item| item.cant_be_countered)
                        .unwrap_or(false);
                    if !is_uncounterable {
                        if let Some(item) = self.stack.remove(*spell_id) {
                            if let crate::stack::StackItemKind::Spell { card_id, .. } = item.kind {
                                // Return spell to its owner's hand
                                self.players[item.controller as usize].hand.push(card_id);
                            }
                        }
                    }
                }
                // Remand controller draws a card regardless of whether the spell was countered
                self.draw_cards(controller, 1);
            }
            CardName::MentalMisstep | CardName::Flusterstorm | CardName::Daze
            | CardName::ManaLeak
            | CardName::SpellPierce | CardName::MysticalDispute | CardName::ConsignToMemory
            | CardName::SinkIntoStupor => {
                // Counter unless controller pays - simplified: just counter
                // Also respects can't-be-countered flag
                if let Some(Target::Object(spell_id)) = targets.first() {
                    let is_uncounterable = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .map(|item| item.cant_be_countered)
                        .unwrap_or(false);
                    if !is_uncounterable {
                        if let Some(item) = self.stack.remove(*spell_id) {
                            self.route_countered_spell(item);
                        }
                    }
                }
            }
            CardName::Stifle => {
                // Counter target activated or triggered ability
                if let Some(Target::Object(ability_id)) = targets.first() {
                    self.stack.remove(*ability_id);
                }
            }

            // === Damage spells ===
            CardName::LightningBolt | CardName::ChainLightning => {
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, 3, controller);
                }
            }
            CardName::Abrade => {
                if let Some(target) = targets.first() {
                    match target {
                        Target::Object(id) => {
                            // Either deal 3 to creature OR destroy artifact
                            self.destroy_permanent(*id);
                        }
                        _ => {}
                    }
                }
            }
            CardName::ShrapnelBlast => {
                // targets[0] = artifact to sacrifice (additional cost), targets[1] = damage target
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                if let Some(damage_target) = targets.get(1) {
                    self.deal_damage_to_target(*damage_target, 5, controller);
                }
            }
            CardName::RedirectLightning => {
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, 4, controller);
                }
            }

            // === Removal ===
            CardName::SwordsToPlowshares => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    // Need power before removal for life gain
                    let power = self.find_permanent(*creature_id).map(|p| p.power()).unwrap_or(0);
                    if let Some(perm) = self.remove_permanent_to_zone(*creature_id, DestinationZone::Exile) {
                        self.players[perm.controller as usize].life += power as i32;
                    }
                }
            }
            CardName::PathToExile | CardName::Dismember => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    self.remove_permanent_to_zone(*creature_id, DestinationZone::Exile);
                }
            }
            // Bounce spells
            CardName::ChainOfVapor | CardName::IntoTheFloodMaw | CardName::HurkylsRecall
            | CardName::Commandeer | CardName::Misdirection => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }

            // === Mana generation ===
            CardName::DarkRitual => {
                self.players[controller as usize].mana_pool.add(Some(Color::Black), 3);
            }

            // === Sacrifice-cost draw spells ===
            // Village Rites: sacrifice a creature (targets[0]), draw 2 cards.
            CardName::VillageRites => {
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                self.draw_cards(controller, 2);
            }

            // Deadly Dispute: sacrifice an artifact or creature (targets[0]), draw 2, create Treasure.
            CardName::DeadlyDispute => {
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                self.draw_cards(controller, 2);
                self.create_treasure_token(controller);
            }

            // === Extra turns ===
            CardName::TimeWalk => {
                self.players[controller as usize].extra_turns += 1;
            }

            // === Tutors ===
            CardName::DemonicTutor => {
                // In a real implementation, this would present a choice.
                // For search engine: the choice is an action the AI makes.
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::DemonicTutorSearch,
                        },
                    });
                }
            }
            CardName::VampiricTutor => {
                self.players[controller as usize].life -= 2;
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::VampiricTutorSearch,
                        },
                    });
                }
            }
            CardName::MysticalTutor => {
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::MysticalTutorSearch,
                        },
                    });
                }
            }
            CardName::Entomb => {
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::EntombSearch,
                        },
                    });
                }
            }

            // === More Tutors ===
            CardName::EnlightenedTutor | CardName::ImperialSeal | CardName::MerchantScroll => {
                // Search library, put on top
                let options: Vec<ObjectId> = self.players[controller as usize].library.clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::MysticalTutorSearch,
                        },
                    });
                }
                if card_name == CardName::ImperialSeal {
                    self.players[controller as usize].life -= 2;
                }
            }
            CardName::DemonicConsultation => {
                // Exile top 6, then find named card - simplified: tutor to hand
                let options: Vec<ObjectId> = self.players[controller as usize].library.clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::DemonicTutorSearch,
                        },
                    });
                }
            }
            CardName::BeseechTheMirror => {
                let options: Vec<ObjectId> = self.players[controller as usize].library.clone();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::DemonicTutorSearch,
                        },
                    });
                }
            }

            // === Mana generation ===
            CardName::CabalRitual => {
                // Add BBB (or BBBBB with threshold)
                let gy_count = self.players[controller as usize].graveyard.len();
                let amount = if gy_count >= 7 { 5 } else { 3 };
                self.players[controller as usize].mana_pool.add(Some(Color::Black), amount);
            }

            // === Discard ===
            CardName::Duress | CardName::InquisitionOfKozilek | CardName::Thoughtseize => {
                self.players[controller as usize].life -= 2;
                if let Some(Target::Player(target_player)) = targets.first() {
                    let options: Vec<ObjectId> = self.players[*target_player as usize]
                        .hand
                        .clone();
                    if !options.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options,
                                reason: ChoiceReason::ThoughtseizeDiscard,
                            },
                        });
                    }
                }
            }
            CardName::HymnToTourach => {
                if let Some(Target::Player(target_player)) = targets.first() {
                    let pid = *target_player as usize;
                    let owner = *target_player;
                    // Discard 2 at random - for deterministic search, pick last 2
                    let count = 2.min(self.players[pid].hand.len());
                    let mut discarded = Vec::new();
                    for _ in 0..count {
                        if let Some(id) = self.players[pid].hand.pop() {
                            discarded.push(id);
                        }
                    }
                    for id in discarded {
                        let cn = self.card_name_for_id(id).unwrap_or(CardName::Plains);
                        self.send_to_graveyard(id, cn, owner);
                    }
                }
            }

            CardName::Unmask => {
                // May exile black card instead of paying mana
                if let Some(Target::Player(target_player)) = targets.first() {
                    let options: Vec<ObjectId> = self.players[*target_player as usize]
                        .hand.clone();
                    if !options.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options,
                                reason: ChoiceReason::ThoughtseizeDiscard,
                            },
                        });
                    }
                }
            }
            CardName::MindTwist => {
                // Target player discards X at random
                if let Some(Target::Player(target_player)) = targets.first() {
                    let pid = *target_player as usize;
                    // X is part of the cost - simplified: discard 3
                    let count = 3.min(self.players[pid].hand.len());
                    for _ in 0..count {
                        if let Some(id) = self.players[pid].hand.pop() {
                            self.players[pid].graveyard.push(id);
                        }
                    }
                }
            }

            // === Wheel effects ===
            CardName::WheelOfFortune | CardName::Timetwister | CardName::Windfall
            | CardName::EchoOfEons => {
                for pid in 0..self.num_players as usize {
                    // Discard hand
                    let hand = std::mem::take(&mut self.players[pid].hand);
                    if card_name == CardName::Timetwister {
                        // Shuffle hand + graveyard into library
                        self.players[pid].library.extend(hand);
                        let gy = std::mem::take(&mut self.players[pid].graveyard);
                        self.players[pid].library.extend(gy);
                    } else {
                        self.players[pid].graveyard.extend(hand);
                    }
                    // Draw 7
                    self.draw_cards(pid as PlayerId, 7);
                }
            }

            // === Draw spells ===
            CardName::CarefulStudy => {
                self.draw_cards(controller, 2);
                // Discard 2 - simplified: discard last 2
                let pid = controller as usize;
                let count = 2.min(self.players[pid].hand.len());
                for _ in 0..count {
                    if let Some(id) = self.players[pid].hand.pop() {
                        self.players[pid].graveyard.push(id);
                    }
                }
            }
            CardName::TreasureCruise | CardName::StockUp | CardName::LorienRevealed => {
                self.draw_cards(controller, 3);
            }
            CardName::DigThroughTime => {
                // Look at top 7, take 2 - simplified: draw 2
                self.draw_cards(controller, 2);
            }
            CardName::GiftsUngiven => {
                // Search for 4, opponent picks 2 for graveyard - simplified: draw 2
                self.draw_cards(controller, 2);
            }
            CardName::Thoughtcast => {
                self.draw_cards(controller, 2);
            }
            CardName::ParadoxicalOutcome => {
                // Bounce own permanents, draw that many - simplified: draw 2
                self.draw_cards(controller, 2);
            }
            CardName::Gush => {
                // Return 2 Islands or pay mana, draw 2
                self.draw_cards(controller, 2);
            }
            CardName::ShowAndTell => {
                // Each player may put a permanent from hand - simplified: no-op
            }
            CardName::Flash => {
                // Put creature from hand onto battlefield - simplified
            }
            CardName::GitaxianProbe => {
                // Look at opponent's hand, draw a card
                self.draw_cards(controller, 1);
            }
            CardName::SurgicalExtraction => {
                // Exile all copies of target card from all zones (simplified: exile target from graveyard)
                if let Some(Target::Object(target_id)) = targets.first() {
                    for pid in 0..self.num_players as usize {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let card = self.players[pid].graveyard.remove(pos);
                            let card_name = self.card_name_for_id(card).unwrap_or(CardName::Plains);
                            self.exile.push((card, card_name, pid as PlayerId));
                            break;
                        }
                    }
                }
            }
            CardName::NoxiousRevival => {
                // Put target card from graveyard on top of library
                if let Some(Target::Object(target_id)) = targets.first() {
                    for pid in 0..self.num_players as usize {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let card = self.players[pid].graveyard.remove(pos);
                            self.players[pid].library.push(card);
                            break;
                        }
                    }
                }
            }
            CardName::VeilOfSummer => {
                // Draw a card if opponent cast blue or black, hexproof from blue/black
                self.draw_cards(controller, 1);
            }
            CardName::OnceUponATime => {
                // Look at top 5, take creature or land - simplified: draw 1
                self.draw_cards(controller, 1);
            }

            // === Cantrips ===
            CardName::Brainstorm => {
                self.draw_cards(controller, 3);
                // Need to put 2 back - this becomes a pending choice
                // Simplified: put last 2 drawn back
                let hand = &mut self.players[controller as usize].hand;
                if hand.len() >= 2 {
                    let c1 = hand.pop().unwrap();
                    let c2 = hand.pop().unwrap();
                    self.players[controller as usize].library.push(c2);
                    self.players[controller as usize].library.push(c1);
                }
            }
            CardName::Ponder => {
                // Simplified: draw a card
                self.draw_cards(controller, 1);
            }
            CardName::Preordain => {
                // Simplified: draw a card
                self.draw_cards(controller, 1);
            }

            // === Board wipes ===
            CardName::Balance => {
                // Find minimum land/creature/hand counts
                let min_lands = (0..self.num_players as usize)
                    .map(|p| self.lands_controlled_by(p as PlayerId).count())
                    .min()
                    .unwrap_or(0);
                let min_creatures = (0..self.num_players as usize)
                    .map(|p| self.creatures_controlled_by(p as PlayerId).count())
                    .min()
                    .unwrap_or(0);
                let min_hand = (0..self.num_players as usize)
                    .map(|p| self.players[p].hand.len())
                    .min()
                    .unwrap_or(0);

                // Sacrifice excess lands
                for pid in 0..self.num_players as usize {
                    let current = self.lands_controlled_by(pid as PlayerId).count();
                    if current > min_lands {
                        let to_sac: Vec<ObjectId> = self
                            .lands_controlled_by(pid as PlayerId)
                            .take(current - min_lands)
                            .map(|p| p.id)
                            .collect();
                        for id in to_sac {
                            self.destroy_permanent(id);
                        }
                    }
                }

                // Sacrifice excess creatures
                for pid in 0..self.num_players as usize {
                    let current = self.creatures_controlled_by(pid as PlayerId).count();
                    if current > min_creatures {
                        let to_sac: Vec<ObjectId> = self
                            .creatures_controlled_by(pid as PlayerId)
                            .take(current - min_creatures)
                            .map(|p| p.id)
                            .collect();
                        for id in to_sac {
                            self.destroy_permanent(id);
                        }
                    }
                }

                // Discard to min hand size
                for pid in 0..self.num_players as usize {
                    while self.players[pid].hand.len() > min_hand {
                        if let Some(id) = self.players[pid].hand.pop() {
                            self.players[pid].graveyard.push(id);
                        }
                    }
                }
            }

            CardName::Armageddon => {
                let lands: Vec<ObjectId> = self
                    .battlefield
                    .iter()
                    .filter(|p| p.is_land())
                    .map(|p| p.id)
                    .collect();
                for id in lands {
                    self.destroy_permanent(id);
                }
            }

            CardName::ToxicDeluge => {
                // Need X life payment - simplified version
                let x = 3i16;
                for perm in &mut self.battlefield {
                    if perm.is_creature() {
                        perm.toughness_mod -= x;
                        perm.power_mod -= x;
                    }
                }
            }
            CardName::BrotherhoodsEnd => {
                // Deal 3 to each creature and planeswalker OR destroy artifacts CMC<=3
                let to_remove: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_creature() || p.is_planeswalker())
                    .map(|p| p.id)
                    .collect();
                for id in to_remove {
                    self.deal_damage_to_target(Target::Object(id), 3, controller);
                }
            }
            CardName::WrathOfTheSkies => {
                // Destroy each creature and non-Aura enchantment with MV <= X
                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_creature() || p.is_enchantment())
                    .map(|p| p.id)
                    .collect();
                for id in to_destroy {
                    self.destroy_permanent(id);
                }
            }
            CardName::Meltdown => {
                // Destroy artifacts with MV <= X - simplified: destroy all
                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_artifact() && !p.is_creature())
                    .map(|p| p.id)
                    .collect();
                for id in to_destroy {
                    self.destroy_permanent(id);
                }
            }
            CardName::SeedsOfInnocence => {
                // Destroy all artifacts
                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_artifact())
                    .map(|p| p.id)
                    .collect();
                for id in to_destroy {
                    self.destroy_permanent(id);
                }
            }
            CardName::ForceOfVigor => {
                // Destroy up to 2 artifacts/enchantments
                for target in targets.iter().take(2) {
                    if let Target::Object(id) = target {
                        self.destroy_permanent(*id);
                    }
                }
            }

            CardName::Disenchant | CardName::NaturesClaim | CardName::Fragmentize
            | CardName::AbruptDecay | CardName::AncientGrudge | CardName::ShatteringSpree
            | CardName::Vandalblast | CardName::Suplex
            | CardName::MoltenCollapse | CardName::PrismaticEnding | CardName::FatalPush
            | CardName::BitterTriumph | CardName::SnuffOut
            | CardName::UntimellyMalfunction | CardName::Crash | CardName::CouncilsJudgment
            | CardName::MarchOfOtherworldlyLight | CardName::SunderingEruption
            | CardName::PestControl => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
                // Nature's Claim: controller gains 4 life
                if card_name == CardName::NaturesClaim {
                    // target's controller already handled
                }
            }

            // === Edict effects: target player sacrifices a creature ===
            CardName::SheoldredsEdict => {
                // Sheoldred's Edict: choose one — each opponent sacrifices a nontoken creature,
                // or a creature token, or a planeswalker.
                // Simplified: force opponent to sacrifice a creature (player chooses which).
                if let Some(Target::Player(target_player)) = targets.first() {
                    let opp = *target_player;
                    let creatures: Vec<ObjectId> = self.battlefield.iter()
                        .filter(|p| p.controller == opp && p.is_creature())
                        .map(|p| p.id)
                        .collect();
                    if !creatures.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: opp,
                            kind: ChoiceKind::ChooseFromList {
                                options: creatures,
                                reason: ChoiceReason::EdictSacrifice,
                            },
                        });
                    }
                }
            }

            // Crop Rotation: sacrifice a land you control, search your library for any land,
            // put it onto the battlefield.
            CardName::CropRotation => {
                // Sacrifice the targeted land
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
                // Search library for any land card
                let searchable: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .iter()
                    .filter(|&&id| {
                        self.card_name_for_id(id)
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| def.card_types.contains(&CardType::Land))
                            .unwrap_or(false)
                    })
                    .copied()
                    .collect();
                if !searchable.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options: searchable,
                            reason: ChoiceReason::GenericSearch,
                        },
                    });
                }
            }

            // Natural Order: sacrifice a green creature (targets[0]), tutor a green creature.
            CardName::NaturalOrder => {
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                let searchable: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .iter()
                    .filter(|&&id| {
                        self.card_name_for_id(id)
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| {
                                def.card_types.contains(&CardType::Creature)
                                    && def.color_identity.contains(&Color::Green)
                            })
                            .unwrap_or(false)
                    })
                    .copied()
                    .collect();
                if !searchable.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options: searchable,
                            reason: ChoiceReason::GenericSearch,
                        },
                    });
                }
            }

            // === Color hosers ===
            CardName::Pyroblast | CardName::RedElementalBlast => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Counter if on stack, destroy if permanent - simplified
                    if self.stack.remove(*target_id).is_none() {
                        self.destroy_permanent(*target_id);
                    }
                }
            }

            // === Reanimation ===
            CardName::Reanimate => {
                // Grafdigger's Cage: creature cards from graveyards can't enter the battlefield.
                // Containment Priest: nontoken creatures that weren't cast are exiled instead.
                let cage_active = self.grafdiggers_cage_active();
                let priest_active = self.containment_priest_active();
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Find card in any graveyard
                    for pid in 0..self.num_players as usize {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let card_id = self.players[pid].graveyard.remove(pos);
                            let card_name = self.card_name_for_id(card_id);
                            if let Some(cn) = card_name {
                                if cage_active || priest_active {
                                    // Grafdigger's Cage / Containment Priest: exile the card instead
                                    self.exile.push((card_id, cn, pid as PlayerId));
                                } else {
                                    // TODO: look up proper stats from db
                                    let perm = Permanent::new(
                                        card_id, cn, controller, pid as PlayerId,
                                        Some(0), Some(0), None, Keywords::empty(), &[CardType::Creature],
                                    );
                                    self.battlefield.push(perm);
                                    self.handle_etb(cn, card_id, controller);
                                    // Lose life equal to CMC - simplified
                                    self.players[controller as usize].life -= 5;
                                }
                            }
                            break;
                        }
                    }
                }
            }

            // === Yawgmoth's Will ===
            CardName::YawgmothsWill => {
                // Until end of turn, you may play lands and cast spells from your graveyard.
                // If a card would be put into your graveyard from anywhere this turn, exile it instead.
                // We model this as a flag on the player; the exile-instead-of-graveyard rule
                // for Yawgmoth's Will is not yet fully implemented (engine-wide complexity).
                self.players[controller as usize].yawgmoth_will_active = true;
            }

            // === Storm ===
            CardName::TendrillsOfAgony => {
                // Base effect: target loses 2, you gain 2
                if let Some(target) = targets.first() {
                    match target {
                        Target::Player(p) => {
                            self.players[*p as usize].life -= 2;
                            self.players[controller as usize].life += 2;
                        }
                        _ => {}
                    }
                }
                // Storm copies
                let storm = self.storm_count;
                for _ in 0..storm {
                    if let Some(target) = targets.first() {
                        match target {
                            Target::Player(p) => {
                                self.players[*p as usize].life -= 2;
                                self.players[controller as usize].life += 2;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // === Channel ===
            CardName::Channel => {
                // Until end of turn, pay 1 life to add {C}.
                // Simplified: convert all life except 1 to colorless mana
                let life = self.players[controller as usize].life;
                if life > 1 {
                    let mana_to_add = (life - 1) as u8;
                    self.players[controller as usize].mana_pool.colorless += mana_to_add;
                    self.players[controller as usize].life = 1;
                }
            }

            // === Storm spells ===
            CardName::BrainFreeze => {
                // Target player mills 3 cards. Storm.
                if let Some(Target::Player(p)) = targets.first() {
                    let mill_count = 3 * (1 + self.storm_count as usize);
                    for _ in 0..mill_count {
                        if let Some(id) = self.players[*p as usize].library.pop() {
                            self.players[*p as usize].graveyard.push(id);
                        }
                    }
                }
            }
            CardName::MindsDesire => {
                // Exile top card, play it free. Storm.
                let copies = 1 + self.storm_count as usize;
                for _ in 0..copies {
                    if let Some(id) = self.players[controller as usize].library.pop() {
                        self.exile.push((id, self.card_name_for_id(id).unwrap_or(CardName::Plains), controller));
                        // Simplified: put in hand instead
                        self.players[controller as usize].hand.push(id);
                    }
                }
            }

            // === Reanimation ===
            CardName::Exhume => {
                // Each player puts a creature from graveyard onto battlefield
                for pid in 0..self.num_players as usize {
                    if let Some(pos) = self.players[pid].graveyard.iter().position(|_| true) {
                        let card_id = self.players[pid].graveyard.remove(pos);
                        let card_name = self.card_name_for_id(card_id);
                        if let Some(cn) = card_name {
                            let perm = Permanent::new(
                                card_id, cn, pid as PlayerId, pid as PlayerId,
                                Some(0), Some(0), None, Keywords::empty(), &[CardType::Creature],
                            );
                            self.battlefield.push(perm);
                        }
                    }
                }
            }

            // === Extra turns ===
            CardName::ExpressiveIteration => {
                // Look at top 3, put one in hand, exile one (play this turn), bottom one
                self.draw_cards(controller, 1); // Simplified
            }
            CardName::ForthEorlingas => {
                // Create two 2/2 Human Knight tokens with trample and haste
                for _ in 0..2 {
                    let token_id = self.new_object_id();
                    let mut kws = Keywords::empty();
                    kws.add(Keyword::Trample);
                    kws.add(Keyword::Haste);
                    let mut token = Permanent::new(
                        token_id, CardName::ForthEorlingas, controller, controller,
                        Some(2), Some(2), None, kws, &[CardType::Creature],
                    );
                    token.is_token = true;
                    self.battlefield.push(token);
                }
            }

            // === Doomsday ===
            CardName::Doomsday => {
                // Lose half life, search for 5 cards
                let life = self.players[controller as usize].life;
                self.players[controller as usize].life = (life + 1) / 2; // Rounded up loss
                // Simplified: don't actually search
            }

            // === Life from the Loam ===
            CardName::LifeFromTheLoam => {
                // Return up to 3 lands from graveyard to hand
                let mut count = 0;
                let gy = &self.players[controller as usize].graveyard;
                let land_indices: Vec<usize> = gy.iter().enumerate()
                    .filter(|(_, &id)| {
                        self.card_name_for_id(id).map_or(false, |_| true) // Simplified
                    })
                    .map(|(i, _)| i)
                    .take(3)
                    .collect();
                for &idx in land_indices.iter().rev() {
                    let id = self.players[controller as usize].graveyard.remove(idx);
                    self.players[controller as usize].hand.push(id);
                    count += 1;
                    if count >= 3 { break; }
                }
            }

            // === Regrowth and similar ===
            CardName::Regrowth | CardName::MemorysJourney => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    let gy = &mut self.players[controller as usize].graveyard;
                    if let Some(pos) = gy.iter().position(|&id| id == *target_id) {
                        let card = gy.remove(pos);
                        self.players[controller as usize].hand.push(card);
                    }
                }
            }

            _ => {
                // Unimplemented card effect - no-op
            }
        }
    }

    /// Handle ETB for spells cast without an X value (e.g., lands played directly).
    pub(crate) fn handle_etb(&mut self, card_name: CardName, card_id: ObjectId, controller: PlayerId) {
        self.handle_etb_with_x(card_name, card_id, controller, 0);
    }

    /// Handle ETB effects that require the original CastSpell targets (e.g. Snapcaster Mage).
    /// Called after handle_etb_with_x for creatures.
    pub(crate) fn handle_etb_with_cast_targets(
        &mut self,
        card_name: CardName,
        _card_id: ObjectId,
        _controller: PlayerId,
        targets: &[Target],
    ) {
        match card_name {
            CardName::SnapcasterMage => {
                // targets[0] is the graveyard card that gains flashback until end of turn.
                if let Some(Target::Object(target_card_id)) = targets.first() {
                    self.snapcaster_flashback_cards.push(*target_card_id);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_etb_with_x(&mut self, card_name: CardName, _card_id: ObjectId, controller: PlayerId, x_value: u8) {
        match card_name {
            // Orcish Bowmasters: amass 1 and deal 1
            CardName::OrcishBowmasters => {
                let opp = self.opponent(controller);
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::OrcishBowmastersETB,
                    },
                    controller,
                    vec![Target::Player(opp)],
                );
            }
            // Grief: target opponent reveals hand, discard nonland
            CardName::Grief => {
                let opp = self.opponent(controller);
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::GriefETB,
                    },
                    controller,
                    vec![Target::Player(opp)],
                );
            }
            // Solitude: exile creature
            CardName::Solitude => {
                let targets: Vec<_> = self.battlefield.iter()
                    .filter(|p| p.is_creature() && p.id != _card_id)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = targets.first() {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::SolitudeETB,
                        },
                        controller,
                        vec![Target::Object(target_id)],
                    );
                }
            }
            // Archon of Cruelty: drain + discard + sac
            CardName::ArchonOfCruelty => {
                let opp = self.opponent(controller);
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::ArchonOfCrueltyTrigger,
                    },
                    controller,
                    vec![Target::Player(opp)],
                );
            }
            // Thought Monitor: draw 2
            CardName::ThoughtMonitor => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::DrawCards(2),
                    },
                    controller,
                    vec![],
                );
            }
            // Snapcaster Mage: ETB handled separately in handle_etb_with_cast_targets
            // because it needs the targets from the CastSpell action.
            CardName::SnapcasterMage => {}
            // Stoneforge Mystic: search for equipment
            CardName::StoneforgeMystic => {}
            // Palace Jailer: become monarch, exile creature
            CardName::PalaceJailer => {}
            // Manglehorn: destroy artifact
            CardName::Manglehorn => {
                let targets: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_artifact() && p.controller != controller)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = targets.first() {
                    self.destroy_permanent(target_id);
                }
            }
            // Mana Vault / Grim Monolith / Time Vault: set doesnt_untap flag
            CardName::ManaVault | CardName::GrimMonolith | CardName::TimeVault => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.doesnt_untap = true;
                }
            }
            // Shock lands: player chooses to pay 2 life (enter untapped) or enter tapped
            CardName::HallowedFountain
            | CardName::WateryGrave
            | CardName::BloodCrypt
            | CardName::StompingGround
            | CardName::TempleGarden
            | CardName::GodlessShrine
            | CardName::SteamVents
            | CardName::OvergrownTomb
            | CardName::SacredFoundry
            | CardName::BreedingPool => {
                self.pending_choice = Some(PendingChoice {
                    player: controller,
                    kind: ChoiceKind::ChooseNumber {
                        min: 0,
                        max: 1,
                        reason: ChoiceReason::ShockLandETB { card_id: _card_id },
                    },
                });
            }
            // Generous Plunderer: each player creates a Treasure token
            CardName::GenerousPlunderer => {
                let num_players = self.num_players;
                for pid in 0..num_players {
                    self.create_treasure_token(pid);
                }
            }
            // Loran of the Third Path: destroy artifact or enchantment
            CardName::LoranOfTheThirdPath => {
                let targets: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| (p.is_artifact() || p.is_enchantment()) && p.controller != controller)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = targets.first() {
                    self.destroy_permanent(target_id);
                }
            }
            // Agent of Treachery: gain control of target permanent on ETB
            CardName::AgentOfTreachery => {
                let opp = self.opponent(controller);
                let opp_perms: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.controller == opp && p.id != _card_id)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = opp_perms.first() {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::GainControlOfPermanent,
                        },
                        controller,
                        vec![Target::Object(target_id)],
                    );
                }
            }
            // Gilded Drake: exchange control with target creature an opponent controls
            CardName::GildedDrake => {
                let opp = self.opponent(controller);
                let opp_creatures: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.controller == opp && p.is_creature() && p.id != _card_id)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = opp_creatures.first() {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::GildedDrakeExchange { drake_id: _card_id },
                        },
                        controller,
                        vec![Target::Object(target_id)],
                    );
                }
            }

            // Walking Ballista: enters with X +1/+1 counters (X chosen when cast, {X}{X} cost)
            CardName::WalkingBallista => {
                if x_value > 0 {
                    if let Some(perm) = self.find_permanent_mut(_card_id) {
                        perm.counters.add(CounterType::PlusOnePlusOne, x_value as i16);
                    }
                }
            }

            // Stonecoil Serpent: enters with X +1/+1 counters (X chosen when cast, {X} cost)
            CardName::StonecoilSerpent => {
                if x_value > 0 {
                    if let Some(perm) = self.find_permanent_mut(_card_id) {
                        perm.counters.add(CounterType::PlusOnePlusOne, x_value as i16);
                    }
                }
            }

            // Chalice of the Void: enters with X charge counters
            CardName::ChaliceOfTheVoid => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.counters.add(CounterType::Charge, x_value as i16);
                }
            }

            // Engineered Explosives: enters with X charge counters (sunburst; simplified as X)
            CardName::EngineeredExplosives => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.counters.add(CounterType::Charge, x_value as i16);
                }
            }

            // Rest in Peace: exile all graveyards when it enters the battlefield.
            // Its static replacement effect (cards go to exile instead of graveyard) is
            // applied at the point of send_to_graveyard / remove_permanent_to_zone.
            CardName::RestInPeace => {
                let num_players = self.num_players as usize;
                for pid in 0..num_players {
                    let graveyard = std::mem::take(&mut self.players[pid].graveyard);
                    for card_id in graveyard {
                        let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                        self.exile.push((card_id, card_name, pid as PlayerId));
                    }
                }
            }

            _ => {}
        }
    }

    fn resolve_triggered(&mut self, effect: TriggeredEffect, controller: PlayerId, targets: &[Target]) {
        match effect {
            TriggeredEffect::ManaCryptUpkeep => {
                // Flip coin - simplified: 50% chance of 3 damage
                // For deterministic search: always deal damage (worst case)
                self.players[controller as usize].life -= 3;
            }
            TriggeredEffect::DealDamage(amount) => {
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, amount, controller);
                }
            }
            TriggeredEffect::DrawCards(n) => {
                self.draw_cards(controller, n as usize);
            }
            TriggeredEffect::GainLife(amount) => {
                self.players[controller as usize].life += amount;
            }
            TriggeredEffect::LoseLife(amount) => {
                self.players[controller as usize].life -= amount;
            }
            TriggeredEffect::SheoldredDraw => {
                self.players[controller as usize].life += 2;
            }
            TriggeredEffect::SheoldredOpponentDraw => {
                let opp = self.opponent(controller);
                self.players[opp as usize].life -= 2;
            }
            TriggeredEffect::DarkConfidantUpkeep => {
                // Reveal top card, lose life equal to CMC
                if let Some(id) = self.players[controller as usize].library.pop() {
                    self.players[controller as usize].hand.push(id);
                    // Lose life equal to CMC - simplified: lose 2
                    self.players[controller as usize].life -= 2;
                }
            }
            TriggeredEffect::YoungPyromancerCast | TriggeredEffect::MonasteryMentorCast => {
                // Create 1/1 token
                let token_id = self.new_object_id();
                let mut kws = Keywords::empty();
                if matches!(effect, TriggeredEffect::MonasteryMentorCast) {
                    kws.add(Keyword::Prowess);
                }
                let mut token = Permanent::new(
                    token_id,
                    card_name_for_token(),
                    controller,
                    controller,
                    Some(1),
                    Some(1),
                    None,
                    kws,
                    &[CardType::Creature],
                );
                token.is_token = true;
                self.battlefield.push(token);
            }
            TriggeredEffect::CreateTokens { power, toughness, count } => {
                for _ in 0..count {
                    let token_id = self.new_object_id();
                    let mut token = Permanent::new(
                        token_id,
                        card_name_for_token(),
                        controller,
                        controller,
                        Some(power),
                        Some(toughness),
                        None,
                        Keywords::empty(),
                        &[CardType::Creature],
                    );
                    token.is_token = true;
                    self.battlefield.push(token);
                }
            }
            TriggeredEffect::CreateTreasures { count } => {
                for _ in 0..count {
                    self.create_treasure_token(controller);
                }
            }
            TriggeredEffect::RagavanCombatDamage => {
                // Create a Treasure token for Ragavan's controller
                self.create_treasure_token(controller);
            }
            TriggeredEffect::ScrawlingCrawlerCombatDamage => {
                // Scrawling Crawler deals combat damage to a player: draw a card
                self.draw_cards(controller, 1);
            }
            TriggeredEffect::PsychicFrogCombatDamage => {
                // Psychic Frog deals combat damage to a player:
                // you may exile a card from your graveyard; if you do, draw a card.
                // Simplified: if the controller has a card in their graveyard, exile one and draw.
                let pid = controller as usize;
                if !self.players[pid].graveyard.is_empty() {
                    let exiled_id = self.players[pid].graveyard.pop().unwrap();
                    let exiled_name = self.card_name_for_id(exiled_id).unwrap_or(CardName::Plains);
                    self.exile.push((exiled_id, exiled_name, controller));
                    self.draw_cards(controller, 1);
                }
            }
            TriggeredEffect::MaiCombatDamage => {
                // Mai, Scornful Striker deals combat damage to a player:
                // you may cast a creature card from a graveyard.
                // Simplified: draw a card to represent the card advantage from the ability.
                // Full implementation requires choosing from any graveyard; model as draw for now.
                self.draw_cards(controller, 1);
            }
            TriggeredEffect::OrcishBowmastersETB | TriggeredEffect::OrcishBowmastersOpponentDraw => {
                // Deal 1 damage to any target and amass Orcs 1 (create 1/1 token)
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, 1, controller);
                }
                let token_id = self.new_object_id();
                let mut token = Permanent::new(
                    token_id, card_name_for_token(), controller, controller,
                    Some(1), Some(1), None, Keywords::empty(), &[CardType::Creature],
                );
                token.is_token = true;
                self.battlefield.push(token);
            }
            TriggeredEffect::GriefETB => {
                // Target opponent reveals hand, you choose nonland to discard
                if let Some(Target::Player(opp)) = targets.first() {
                    let pid = *opp as usize;
                    if !self.players[pid].hand.is_empty() {
                        let options = self.players[pid].hand.clone();
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options,
                                reason: ChoiceReason::ThoughtseizeDiscard,
                            },
                        });
                    }
                }
            }
            TriggeredEffect::SolitudeETB => {
                // Exile target creature - opponent gains life equal to its power
                if let Some(Target::Object(creature_id)) = targets.first() {
                    let power = self.find_permanent(*creature_id).map(|p| p.power()).unwrap_or(0);
                    if let Some(perm) = self.remove_permanent_to_zone(*creature_id, DestinationZone::Exile) {
                        self.players[perm.controller as usize].life += power as i32;
                    }
                }
            }
            TriggeredEffect::ArchonOfCrueltyTrigger => {
                // Opponent: sacrifice creature, discard, lose 3
                // You: draw, gain 3, create Treasure
                if let Some(Target::Player(opp)) = targets.first() {
                    let opid = *opp as usize;
                    self.players[opid].life -= 3;
                    if let Some(id) = self.players[opid].hand.pop() {
                        self.players[opid].graveyard.push(id);
                    }
                    self.draw_cards(controller, 1);
                    self.players[controller as usize].life += 3;
                    self.create_treasure_token(controller);
                }
            }
            TriggeredEffect::WurmcoilDeath => {
                // Create two tokens: 3/3 with lifelink and 3/3 with deathtouch
                for kw in [Keyword::Lifelink, Keyword::Deathtouch] {
                    let token_id = self.new_object_id();
                    let mut kws = Keywords::empty();
                    kws.add(kw);
                    let mut token = Permanent::new(
                        token_id,
                        card_name_for_token(),
                        controller,
                        controller,
                        Some(3),
                        Some(3),
                        None,
                        kws,
                        &[CardType::Creature, CardType::Artifact],
                    );
                    token.is_token = true;
                    self.battlefield.push(token);
                }
            }
            TriggeredEffect::SkullclampDeath => {
                // Draw 2 cards
                self.draw_cards(controller, 2);
            }
            TriggeredEffect::MyrRetrieverDeath => {
                // Return another target artifact card from your graveyard to your hand.
                // Present as a choice: pick an artifact from graveyard.
                let artifacts_in_gy: Vec<ObjectId> = self.players[controller as usize]
                    .graveyard
                    .iter()
                    .copied()
                    .collect();
                if !artifacts_in_gy.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options: artifacts_in_gy,
                            reason: ChoiceReason::MyrRetrieverReturn,
                        },
                    });
                }
            }
            TriggeredEffect::GainControlOfPermanent => {
                // Agent of Treachery: gain control of target permanent
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.gain_control(*target_id, controller);
                }
            }
            TriggeredEffect::GildedDrakeExchange { drake_id } => {
                // Gilded Drake: exchange control of drake and target creature
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.exchange_control(drake_id, *target_id);
                }
            }
            TriggeredEffect::EvokeSacrifice { permanent_id } => {
                // Evoke sacrifice: sacrifice the evoked creature.
                // The creature's card goes to the graveyard (owner's zone).
                self.destroy_permanent(permanent_id);
            }
            _ => {}
        }
    }

    fn resolve_activated(&mut self, effect: ActivatedEffect, controller: PlayerId, targets: &[Target]) {
        match effect {
            ActivatedEffect::SacrificeForMana { amount: _ } => {
                // Handled at activation time (mana already added, permanent already sacrificed)
            }
            ActivatedEffect::JaceBrainstorm => {
                self.draw_cards(controller, 3);
                // Put 2 back - simplified
                let hand = &mut self.players[controller as usize].hand;
                if hand.len() >= 2 {
                    let c1 = hand.pop().unwrap();
                    let c2 = hand.pop().unwrap();
                    self.players[controller as usize].library.push(c2);
                    self.players[controller as usize].library.push(c1);
                }
            }
            ActivatedEffect::JaceBounce => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    self.remove_permanent_to_zone(*creature_id, DestinationZone::Hand);
                }
            }
            ActivatedEffect::JaceFateseal => {
                // +2: Look at top of target player's library, may put on bottom
                // Simplified: no-op for now (hidden info)
            }
            ActivatedEffect::TeferiBounce => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::DrawCards(n) => {
                self.draw_cards(controller, n as usize);
            }
            ActivatedEffect::BazaarDraw => {
                // Draw 2, discard 3
                self.draw_cards(controller, 2);
                let pid = controller as usize;
                for _ in 0..3 {
                    if let Some(id) = self.players[pid].hand.pop() {
                        self.players[pid].graveyard.push(id);
                    }
                }
            }
            ActivatedEffect::TopLook => {
                // Sensei's Divining Top: look at top 3, rearrange
                // Simplified: no-op (hidden information)
            }
            ActivatedEffect::TopDraw => {
                // Sensei's Divining Top: draw a card, put Top on top of library
                // Find Top on the battlefield and return it to top of library
                let top_id = self.battlefield.iter()
                    .find(|p| p.card_name == CardName::SenseisDiviningTop && p.controller == controller)
                    .map(|p| p.id);
                if let Some(id) = top_id {
                    self.remove_permanent_to_zone(id, DestinationZone::Library);
                }
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::UntapArtifact => {
                // Voltaic Key / Manifold Key: untap target artifact
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.tapped = false;
                    }
                }
            }
            ActivatedEffect::KarakasBounce => {
                // Bounce target legendary creature to owner's hand
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }
            ActivatedEffect::GhostQuarterDestroy => {
                // Destroy target land (opponent may search for basic)
                // Simplified: just destroy the land
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }
            ActivatedEffect::NarsetMinus => {
                // Narset -2: look at top 4, may reveal noncreature/nonland
                // Simplified: draw 1 card (approximation)
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::OkoFood => {
                // Oko +2: create a Food token (artifact)
                let token_id = self.new_object_id();
                let token = Permanent {
                    id: token_id,
                    card_name: CardName::Plains, // placeholder for token
                    controller,
                    owner: controller,
                    tapped: false,
                    base_power: 0,
                    base_toughness: 0,
                    power_mod: 0,
                    toughness_mod: 0,
                    damage: 0,
                    keywords: Keywords::empty(),
                    counters: Counters::default(),
                    entered_this_turn: true,
                    attacked_this_turn: false,
                    doesnt_untap: false,
                    loyalty: 0,
                    loyalty_activated_this_turn: false,
                    card_types: vec![CardType::Artifact],
                    is_token: true,
                    attached_to: None,
                    attachments: Vec::new(),
                };
                self.battlefield.push(token);
            }
            ActivatedEffect::OkoElkify => {
                // Oko +1: target artifact or creature becomes a 3/3 Elk
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.base_power = 3;
                        perm.base_toughness = 3;
                        perm.power_mod = 0;
                        perm.toughness_mod = 0;
                        // Becomes a creature, loses other types except artifact
                        if !perm.card_types.contains(&CardType::Creature) {
                            perm.card_types.push(CardType::Creature);
                        }
                        // Remove all abilities (simplified: clear keywords)
                        perm.keywords = Keywords::empty();
                    }
                }
            }
            ActivatedEffect::WrennReturn => {
                // Wrenn and Six +1: return target land from graveyard to hand
                if let Some(Target::Object(target_id)) = targets.first() {
                    let pid = controller as usize;
                    if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                        let id = self.players[pid].graveyard.remove(pos);
                        self.players[pid].hand.push(id);
                    }
                }
            }
            ActivatedEffect::WrennPing => {
                // Wrenn and Six -1: deal 1 damage to any target
                if let Some(&target) = targets.first() {
                    self.deal_damage_to_target(target, 1, controller);
                }
            }
            ActivatedEffect::KarnAnimate => {
                // Karn +1: animate target noncreature artifact (simplified: no-op)
            }
            ActivatedEffect::KarnWish => {
                // Karn -2: wish for artifact from sideboard/exile
                // Simplified: no-op (sideboard not modeled)
            }
            ActivatedEffect::GideonCreature => {
                // Gideon 0: becomes a 4/4 creature until end of turn
                // Simplified: find Gideon and make it a creature
                if let Some(perm) = self.battlefield.iter_mut()
                    .find(|p| p.card_name == CardName::GideonOfTheTrials && p.controller == controller)
                {
                    if !perm.card_types.contains(&CardType::Creature) {
                        perm.card_types.push(CardType::Creature);
                    }
                    perm.base_power = 4;
                    perm.base_toughness = 4;
                    perm.keywords.add(Keyword::Indestructible);
                }
            }
            ActivatedEffect::GideonPrevent => {
                // Gideon +1: prevent all damage from a source (simplified: no-op)
            }
            ActivatedEffect::KayaExile => {
                // Kaya +1: exile target card from a graveyard
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Check both players' graveyards
                    for pid in 0..self.players.len() {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let id = self.players[pid].graveyard.remove(pos);
                            let card_name = self.card_name_for_id(id).unwrap_or(CardName::Plains);
                            self.exile.push((id, card_name, pid as PlayerId));
                            break;
                        }
                    }
                }
            }
            ActivatedEffect::KayaMinus => {
                // Kaya -1: exile target nonland permanent, owner gains 2 life
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.remove_permanent_to_zone(*target_id, DestinationZone::Exile) {
                        let owner = perm.owner as usize;
                        self.players[owner].life += 2;
                    }
                }
            }
            ActivatedEffect::PlaneswalkerAbility { .. } => {
                // Generic planeswalker ability - handled by specific variants above
            }
            ActivatedEffect::EquipCreature { equipment_id } => {
                // Attach equipment to target creature
                if let Some(Target::Object(creature_id)) = targets.first() {
                    self.do_attach_equipment(equipment_id, *creature_id);
                }
            }
            ActivatedEffect::BatterskullBounce => {
                // Return Batterskull to owner's hand
                let batterskull_id = self.battlefield.iter()
                    .find(|p| p.card_name == CardName::Batterskull && p.controller == controller)
                    .map(|p| p.id);
                if let Some(id) = batterskull_id {
                    self.detach_and_remove(id, DestinationZone::Hand);
                }
            }
            ActivatedEffect::CyclingDraw => {
                // Discard already happened at activation; just draw a card.
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::SharkTyphoonCycling { x_value } => {
                // Discard already happened at activation; create an X/X Shark with flying, then draw.
                let token_id = self.new_object_id();
                let mut kw = Keywords::empty();
                kw.add(Keyword::Flying);
                let token = Permanent {
                    id: token_id,
                    card_name: CardName::SharkToken,
                    controller,
                    owner: controller,
                    tapped: false,
                    base_power: x_value as i16,
                    base_toughness: x_value as i16,
                    power_mod: 0,
                    toughness_mod: 0,
                    damage: 0,
                    keywords: kw,
                    counters: Counters::default(),
                    entered_this_turn: true,
                    attacked_this_turn: false,
                    doesnt_untap: false,
                    loyalty: 0,
                    loyalty_activated_this_turn: false,
                    card_types: vec![CardType::Creature],
                    is_token: true,
                    attached_to: None,
                    attachments: Vec::new(),
                };
                self.battlefield.push(token);
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::BoseijuChannel => {
                // Destroy target artifact, enchantment, or nonbasic land (opponent controls).
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }
            ActivatedEffect::OtawaraChannel => {
                // Return target artifact, creature, or planeswalker to owner's hand.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }
        }
    }

    /// Detach from old host, attach equipment to creature, apply bonuses.
    pub(crate) fn do_attach_equipment(&mut self, equip_id: ObjectId, creature_id: ObjectId) {
        // First, detach from current host (if any)
        let old_host = self.find_permanent(equip_id).and_then(|p| p.attached_to);
        if let Some(old_host_id) = old_host {
            self.remove_equipment_bonuses(equip_id, old_host_id);
            // Remove equip_id from old host's attachments
            if let Some(host) = self.find_permanent_mut(old_host_id) {
                host.attachments.retain(|&id| id != equip_id);
            }
        }
        // Update attached_to on the equipment
        if let Some(equip) = self.find_permanent_mut(equip_id) {
            equip.attached_to = Some(creature_id);
        }
        // Add to new host's attachments
        if let Some(host) = self.find_permanent_mut(creature_id) {
            if !host.attachments.contains(&equip_id) {
                host.attachments.push(equip_id);
            }
        }
        // Apply equipment bonuses to the new host
        self.apply_equipment_bonuses(equip_id, creature_id);
    }

    /// Apply equipment stat bonuses/keywords to the host creature.
    pub(crate) fn apply_equipment_bonuses(&mut self, equip_id: ObjectId, creature_id: ObjectId) {
        let equip_name = match self.find_permanent(equip_id) {
            Some(p) => p.card_name,
            None => return,
        };
        if let Some(bonus) = crate::card::equipment_bonus(equip_name) {
            if let Some(creature) = self.find_permanent_mut(creature_id) {
                creature.power_mod += bonus.power_mod;
                creature.toughness_mod += bonus.toughness_mod;
                let kw_bits = bonus.keywords.0;
                creature.keywords.0 |= kw_bits;
            }
        }
    }

    /// Remove equipment stat bonuses/keywords from the host creature (on detach/death).
    pub(crate) fn remove_equipment_bonuses(&mut self, equip_id: ObjectId, creature_id: ObjectId) {
        let equip_name = match self.find_permanent(equip_id) {
            Some(p) => p.card_name,
            None => return,
        };
        if let Some(bonus) = crate::card::equipment_bonus(equip_name) {
            if let Some(creature) = self.find_permanent_mut(creature_id) {
                creature.power_mod -= bonus.power_mod;
                creature.toughness_mod -= bonus.toughness_mod;
                let kw_bits = bonus.keywords.0;
                creature.keywords.0 &= !kw_bits;
            }
        }
    }

    /// Route a countered spell to its destination zone.
    /// Spells cast from graveyard (flashback / Yawgmoth's Will) go to exile when countered.
    /// Other spells go to their owner's graveyard.
    pub(crate) fn route_countered_spell(&mut self, item: crate::stack::StackItem) {
        if let crate::stack::StackItemKind::Spell { card_id, card_name, .. } = item.kind {
            if item.cast_from_graveyard {
                // Flashback / Yawgmoth's Will: exile instead of graveyard
                self.exile.push((card_id, card_name, item.controller));
            } else {
                // Normal: put in owner's graveyard
                self.players[item.controller as usize].graveyard.push(card_id);
            }
        }
        // Triggered/activated abilities on the stack have no card to route
    }

    /// Remove equipment bonuses and then remove the equipment from the battlefield.
    pub(crate) fn detach_and_remove(&mut self, equip_id: ObjectId, zone: DestinationZone) {
        let old_host = self.find_permanent(equip_id).and_then(|p| p.attached_to);
        if let Some(host_id) = old_host {
            self.remove_equipment_bonuses(equip_id, host_id);
            if let Some(host) = self.find_permanent_mut(host_id) {
                host.attachments.retain(|&id| id != equip_id);
            }
            if let Some(equip) = self.find_permanent_mut(equip_id) {
                equip.attached_to = None;
            }
        }
        self.remove_permanent_to_zone(equip_id, zone);
    }
}
