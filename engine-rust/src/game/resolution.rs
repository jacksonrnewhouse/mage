/// Spell and ability resolution logic.

use crate::action::*;
use crate::card::*;
use super::card_name_for_token;
use crate::game::{ChoiceKind, ChoiceReason, DestinationZone, Emblem, GameState, PendingChoice};
use crate::permanent::*;
use crate::stack::*;
use crate::types::*;

impl GameState {
    pub fn resolve_top(&mut self, db: &[CardDef]) {
        if let Some(item) = self.stack.pop() {
            let is_copy = item.is_copy;
            let cast_as_adventure = item.cast_as_adventure;
            match item.kind {
                StackItemKind::Spell { card_name, card_id, cast_via_evoke } => {
                    self.resolve_spell(card_name, card_id, item.controller, &item.targets, item.x_value, item.cast_from_graveyard, cast_as_adventure, cast_via_evoke, &item.modes, is_copy, db);
                }
                StackItemKind::TriggeredAbility { effect, .. } => {
                    self.resolve_triggered(effect, item.controller, &item.targets, db);
                }
                StackItemKind::ActivatedAbility { effect, .. } => {
                    self.resolve_activated(effect, item.controller, &item.targets, db);
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
        cast_as_adventure: bool,
        cast_via_evoke: bool,
        modes: &[u8],
        is_copy: bool,
        db: &[CardDef],
    ) {
        let card_def = find_card(db, card_name);
        // When cast as an adventure, the card resolves as the adventure spell (instant/sorcery),
        // NOT as the permanent (creature). So treat it as non-permanent in that case.
        let is_permanent = if cast_as_adventure {
            false
        } else {
            card_def
                .map(|c| {
                    c.card_types.iter().any(|t| matches!(t,
                        CardType::Creature | CardType::Artifact | CardType::Enchantment
                        | CardType::Planeswalker | CardType::Land
                    ))
                })
                .unwrap_or(false)
        };

        if is_permanent && !is_copy {
            // Put permanent onto battlefield (copies of permanent spells don't create permanents
            // unless specifically handled; for simplicity copies of permanents are not supported).
            if let Some(def) = card_def {
                let mut perm = Permanent::new(
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
                // Set creature types from card definition (or all types if changeling)
                if def.is_changeling {
                    perm.creature_types = crate::types::CreatureType::ALL.to_vec();
                } else {
                    perm.creature_types = def.creature_types.to_vec();
                }
                // Store color identity for protection checks
                perm.colors = def.color_identity.to_vec();
                self.battlefield.push(perm);
                // Apply static abilities that cause permanents to enter tapped
                self.apply_enters_tapped_statics(card_id, controller);
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
        } else if !is_permanent {
            // Instant/sorcery: resolve effect, then place in appropriate zone.
            // If cast via flashback (or via Yawgmoth's Will), exile instead of going to graveyard.
            // If cast as an adventure, exile the card (so creature half can be cast from exile).
            self.resolve_card_effect_with_x(card_name, controller, targets, x_value, modes, is_copy, db);
            if is_copy {
                // Copies of spells cease to exist after resolving — no card to zone-route.
            } else if cast_from_graveyard {
                // Exile the card (flashback / Yawgmoth's Will rule: if it would go to graveyard, exile it)
                // The card was already removed from graveyard when cast; just push to exile.
                self.exile.push((card_id, card_name, controller));
            } else if cast_as_adventure {
                // Adventure rule: after the adventure resolves, exile the card.
                // The card was already removed from hand when cast.
                // Mark it in adventure_exiled so the creature half can be cast later.
                self.exile.push((card_id, card_name, controller));
                self.adventure_exiled.push((card_id, controller));
            } else if card_name == CardName::GreenSunsZenith {
                // Green Sun's Zenith: shuffle into library instead of going to graveyard.
                // Approximation: put on bottom of library.
                self.players[controller as usize].library.insert(0, card_id);
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
        x_value: u8,
        modes: &[u8],
        is_copy: bool,
        db: &[CardDef],
    ) {
        self.resolve_card_effect(card_name, controller, targets, x_value, modes, is_copy, db);
    }

    fn resolve_card_effect(
        &mut self,
        card_name: CardName,
        controller: PlayerId,
        targets: &[Target],
        x_value: u8,
        modes: &[u8],
        is_copy: bool,
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
            CardName::Counterspell | CardName::ForceOfWill | CardName::ForceOfNegation => {
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
            CardName::ManaDrain => {
                if let Some(Target::Object(spell_id)) = targets.first() {
                    // Get the mana value of the targeted spell before removing it
                    let mana_value = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .and_then(|item| {
                            if let crate::stack::StackItemKind::Spell { card_name, .. } = &item.kind {
                                find_card(db, *card_name).map(|d| d.mana_cost.cmc())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    let is_uncounterable = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .map(|item| item.cant_be_countered)
                        .unwrap_or(false);
                    if !is_uncounterable {
                        if let Some(item) = self.stack.remove(*spell_id) {
                            self.route_countered_spell(item);
                        }
                        // Add colorless mana equal to countered spell's mana value
                        self.players[controller as usize].mana_pool.add(None, mana_value as u8);
                    }
                }
            }
            CardName::MindbreakTrap => {
                // Exile any number of target spells from the stack (not counter — bypasses "can't be countered")
                for target in targets.iter() {
                    if let Target::Object(spell_id) = target {
                        if let Some(item) = self.stack.remove(*spell_id) {
                            if let crate::stack::StackItemKind::Spell { card_id, card_name, .. } = item.kind {
                                self.exile.push((card_id, card_name, item.controller));
                            }
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
            CardName::MentalMisstep => {
                // Hard counter: counter target spell with mana value 1
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
            CardName::SpellPierce | CardName::ManaLeak | CardName::Daze
            | CardName::MysticalDispute | CardName::Flusterstorm => {
                // Soft counters: "counter unless controller pays {X}"
                // Simplified for game tree search: auto-pay if opponent has enough mana, else counter.
                let tax = match card_name {
                    CardName::SpellPierce => 2u16,
                    CardName::ManaLeak => 3,
                    CardName::MysticalDispute => 3,
                    CardName::Daze => 1,
                    CardName::Flusterstorm => 1,
                    _ => unreachable!(),
                };
                if let Some(Target::Object(spell_id)) = targets.first() {
                    let spell_info = self.stack.items()
                        .iter()
                        .find(|item| item.id == *spell_id)
                        .map(|item| (item.cant_be_countered, item.controller));
                    if let Some((is_uncounterable, spell_controller)) = spell_info {
                        if !is_uncounterable {
                            // Count opponent's available mana: mana pool + untapped lands
                            let pool_mana = self.players[spell_controller as usize].mana_pool.total();
                            let untapped_land_count = self.battlefield.iter()
                                .filter(|p| p.controller == spell_controller && !p.tapped && p.is_land())
                                .count() as u16;
                            let available = pool_mana + untapped_land_count;
                            if available < tax {
                                // Can't pay — counter the spell
                                if let Some(item) = self.stack.remove(*spell_id) {
                                    self.route_countered_spell(item);
                                }
                            }
                            // If they can pay, the spell is NOT countered (auto-pay simplification)
                        }
                    }
                }
                // Storm: Flusterstorm has storm — push copies
                if card_name == CardName::Flusterstorm && !is_copy {
                    let storm = self.storm_count;
                    let template = crate::stack::StackItem {
                        id: 0,
                        kind: crate::stack::StackItemKind::Spell {
                            card_name: CardName::Flusterstorm,
                            card_id: 0,
                            cast_via_evoke: false,
                        },
                        controller,
                        targets: targets.to_vec(),
                        cant_be_countered: false,
                        x_value: 0,
                        cast_from_graveyard: false,
                        cast_as_adventure: false,
                        modes: vec![],
                        is_copy: false,
                    };
                    for _ in 0..storm {
                        self.stack.push_copy(&template);
                    }
                }
            }
            CardName::ConsignToMemory => {
                // Hard counter with Storm
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
                // Storm: push storm_count copies onto the stack
                if !is_copy {
                    let storm = self.storm_count;
                    let template = crate::stack::StackItem {
                        id: 0,
                        kind: crate::stack::StackItemKind::Spell {
                            card_name: CardName::ConsignToMemory,
                            card_id: 0,
                            cast_via_evoke: false,
                        },
                        controller,
                        targets: targets.to_vec(),
                        cant_be_countered: false,
                        x_value: 0,
                        cast_from_graveyard: false,
                        cast_as_adventure: false,
                        modes: vec![],
                        is_copy: false,
                    };
                    for _ in 0..storm {
                        self.stack.push_copy(&template);
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
            // === Adventure spells ===
            // Stomp (adventure of Bonecrusher Giant): deal 2 damage to any target.
            // Note: "damage can't be prevented" — not modeled yet, treated as normal damage.
            CardName::BonecrusherGiant => {
                if let Some(target) = targets.first() {
                    self.deal_damage_to_target(*target, 2, controller);
                }
            }
            // Petty Theft (adventure of Brazen Borrower): return target nonland permanent
            // an opponent controls to its owner's hand.
            CardName::BrazenBorrower => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }
            CardName::Abrade => {
                if let Some(target) = targets.first() {
                    match target {
                        Target::Object(id) => {
                            // Check if target is an artifact - if so, destroy; otherwise deal 3 damage
                            let is_artifact = self.find_permanent(*id)
                                .map(|p| p.card_types.contains(&CardType::Artifact))
                                .unwrap_or(false);
                            if is_artifact {
                                self.destroy_permanent(*id);
                            } else {
                                self.deal_damage_to_target(Target::Object(*id), 3, controller);
                            }
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
                    let controls_artifact = self.battlefield.iter().any(|p| p.controller == controller && p.card_types.contains(&CardType::Artifact));
                    let damage = if controls_artifact { 5 } else { 4 };
                    self.deal_damage_to_target(*target, damage, controller);
                }
            }
            // Shatterskull Smashing: deals X damage divided among up to two targets.
            // If X is 6 or more, deals twice X damage instead.
            // Simplified: deal all damage to a single target.
            CardName::ShatterskullSmashing => {
                if let Some(target) = targets.first() {
                    let damage = if x_value >= 6 { (x_value as u16) * 2 } else { x_value as u16 };
                    self.deal_damage_to_target(*target, damage, controller);
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
            CardName::PathToExile => {
                if let Some(Target::Object(creature_id)) = targets.first() {
                    self.remove_permanent_to_zone(*creature_id, DestinationZone::Exile);
                }
            }
            CardName::Dismember => {
                // -5/-5 until end of turn
                if let Some(Target::Object(creature_id)) = targets.first() {
                    self.add_temporary_effect(TemporaryEffect::ModifyPT {
                        target: *creature_id,
                        power: -5,
                        toughness: -5,
                    });
                }
            }
            // Bounce spells (permanents only)
            CardName::ChainOfVapor | CardName::IntoTheFloodMaw => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }
            // Sink into Stupor: return target spell or nonland permanent an opponent controls to hand
            CardName::SinkIntoStupor => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Check if target is a spell on the stack
                    if self.stack.items().iter().any(|item| item.id == *target_id) {
                        if let Some(item) = self.stack.remove(*target_id) {
                            if let crate::stack::StackItemKind::Spell { card_id, .. } = item.kind {
                                self.players[item.controller as usize].hand.push(card_id);
                            }
                        }
                    } else {
                        // Target is a permanent on the battlefield
                        self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                    }
                }
            }
            // Step Through: return two target creatures to their owners' hands
            CardName::StepThrough => {
                for target in targets.iter().take(2) {
                    if let Target::Object(target_id) = target {
                        self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                    }
                }
            }
            // Commandeer: counter target noncreature spell (simplified from gain control)
            // Misdirection: counter target spell with a single target (simplified from redirect)
            CardName::Commandeer | CardName::Misdirection => {
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
            CardName::HurkylsRecall => {
                // Return ALL artifacts target player owns to their hand
                if let Some(Target::Player(target_player)) = targets.first() {
                    let artifact_ids: Vec<ObjectId> = self.battlefield.iter()
                        .filter(|p| p.owner == *target_player && p.is_artifact())
                        .map(|p| p.id)
                        .collect();
                    for id in artifact_ids {
                        self.remove_permanent_to_zone(id, DestinationZone::Hand);
                    }
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
            CardName::TimeWalk | CardName::TemporalMastery => {
                self.players[controller as usize].extra_turns += 1;
            }

            // === Tutors ===
            CardName::DemonicTutor => {
                // Leonin Arbiter / Opposition Agent: search is restricted
                if !self.library_search_restricted(controller) {
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
            }
            CardName::VampiricTutor => {
                self.players[controller as usize].life -= 2;
                if !self.library_search_restricted(controller) {
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
            }
            CardName::MysticalTutor => {
                if !self.library_search_restricted(controller) {
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
            }
            CardName::Entomb => {
                if !self.library_search_restricted(controller) {
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
            }

            // === More Tutors ===
            CardName::EnlightenedTutor => {
                // Search library for an artifact or enchantment card, put on top
                if !self.library_search_restricted(controller) {
                    let options: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .and_then(|cn| find_card(db, cn))
                                .map(|def| {
                                    def.card_types.contains(&CardType::Artifact)
                                        || def.card_types.contains(&CardType::Enchantment)
                                })
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect();
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
            }
            CardName::ImperialSeal | CardName::MerchantScroll => {
                if card_name == CardName::ImperialSeal {
                    self.players[controller as usize].life -= 2;
                }
                if !self.library_search_restricted(controller) {
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
                }
            }
            CardName::DemonicConsultation => {
                if !self.library_search_restricted(controller) {
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
            }
            CardName::BeseechTheMirror => {
                if !self.library_search_restricted(controller) {
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
                if card_name == CardName::Thoughtseize {
                    self.players[controller as usize].life -= 2;
                }
                if let Some(Target::Player(target_player)) = targets.first() {
                    let options: Vec<ObjectId> = self.players[*target_player as usize]
                        .hand
                        .iter()
                        .copied()
                        .filter(|&card_id| {
                            if let Some(cn) = self.card_name_for_id(card_id) {
                                if let Some(def) = crate::card::find_card(db, cn) {
                                    let is_land = def.card_types.contains(&CardType::Land);
                                    let is_creature = def.card_types.contains(&CardType::Creature);
                                    match card_name {
                                        CardName::Duress => !is_land && !is_creature,
                                        CardName::Thoughtseize => !is_land,
                                        CardName::InquisitionOfKozilek => !is_land && def.mana_cost.cmc() <= 3,
                                        _ => true,
                                    }
                                } else {
                                    true
                                }
                            } else {
                                true
                            }
                        })
                        .collect();
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
                        self.discard_card(id, owner, db);
                    }
                }
            }

            CardName::Unmask => {
                // Unmask: target player reveals hand, choose a nonland card to discard
                if let Some(Target::Player(target_player)) = targets.first() {
                    let options: Vec<ObjectId> = self.players[*target_player as usize]
                        .hand
                        .iter()
                        .copied()
                        .filter(|&card_id| {
                            if let Some(cn) = self.card_name_for_id(card_id) {
                                if let Some(def) = crate::card::find_card(db, cn) {
                                    !def.card_types.contains(&CardType::Land)
                                } else {
                                    true
                                }
                            } else {
                                true
                            }
                        })
                        .collect();
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
                    let owner = *target_player;
                    let count = (x_value as usize).min(self.players[pid].hand.len());
                    let mut discarded = Vec::new();
                    for _ in 0..count {
                        if let Some(id) = self.players[pid].hand.pop() {
                            discarded.push(id);
                        }
                    }
                    for id in discarded {
                        self.discard_card(id, owner, db);
                    }
                    self.check_emrakul_graveyard_shuffle(*target_player);
                }
            }

            // === Wheel effects ===
            CardName::WheelOfFortune | CardName::Timetwister | CardName::EchoOfEons => {
                for pid in 0..self.num_players as usize {
                    // Discard hand
                    let hand = std::mem::take(&mut self.players[pid].hand);
                    if card_name == CardName::Timetwister || card_name == CardName::EchoOfEons {
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

            // Windfall: each player discards, then draws equal to greatest number discarded
            CardName::Windfall => {
                let mut max_discarded = 0usize;
                for pid in 0..self.num_players as usize {
                    let hand = std::mem::take(&mut self.players[pid].hand);
                    let discarded = hand.len();
                    if discarded > max_discarded {
                        max_discarded = discarded;
                    }
                    self.players[pid].graveyard.extend(hand);
                }
                for pid in 0..self.num_players as usize {
                    self.draw_cards(pid as PlayerId, max_discarded);
                }
            }

            // === Draw spells ===
            CardName::CarefulStudy => {
                self.draw_cards(controller, 2);
                // Discard 2 - simplified: discard last 2
                let pid = controller as usize;
                let count = 2.min(self.players[pid].hand.len());
                let mut to_discard = Vec::with_capacity(count);
                for _ in 0..count {
                    if let Some(id) = self.players[pid].hand.pop() {
                        to_discard.push(id);
                    }
                }
                for id in to_discard {
                    self.discard_card(id, controller, db);
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
                // Bounce any number of nonland, nontoken permanents you control, draw that many.
                // Simplified: bounce all nonland nontoken permanents you control, then draw that many.
                let to_bounce: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.controller == controller && !p.is_land() && !p.is_token)
                    .map(|p| p.id)
                    .collect();
                let count = to_bounce.len();
                for id in to_bounce {
                    self.remove_permanent_to_zone(id, DestinationZone::Hand);
                }
                self.draw_cards(controller, count);
            }
            CardName::Gush => {
                // Return 2 Islands or pay mana, draw 2
                self.draw_cards(controller, 2);
            }
            CardName::ShowAndTell => {
                // Each player may put an artifact, creature, enchantment, planeswalker, or land
                // from their hand onto the battlefield simultaneously.
                // We resolve one player at a time: active player first, then opponent.
                // Valid permanent types: Artifact, Creature, Enchantment, Planeswalker, Land (not Instant/Sorcery).
                let valid_options: Vec<ObjectId> = self.players[controller as usize]
                    .hand
                    .iter()
                    .copied()
                    .filter(|&id| {
                        if let Some(cn) = self.card_name_for_id(id) {
                            if let Some(def) = find_card(db, cn) {
                                return def.card_types.iter().any(|t| matches!(t,
                                    CardType::Artifact | CardType::Creature
                                    | CardType::Enchantment | CardType::Planeswalker
                                    | CardType::Land
                                ));
                            }
                        }
                        false
                    })
                    .collect();
                let opponent = self.opponent(controller);
                self.pending_choice = Some(PendingChoice {
                    player: controller,
                    kind: ChoiceKind::ChooseFromList {
                        options: valid_options,
                        reason: ChoiceReason::ShowAndTellChoose {
                            next_player: Some(opponent),
                        },
                    },
                });
            }
            CardName::Flash => {
                // Put a creature from hand onto battlefield
                let creature_options: Vec<ObjectId> = self.players[controller as usize]
                    .hand
                    .iter()
                    .copied()
                    .filter(|&id| {
                        if let Some(cn) = self.card_name_for_id(id) {
                            if let Some(def) = find_card(db, cn) {
                                return def.card_types.contains(&CardType::Creature);
                            }
                        }
                        false
                    })
                    .collect();
                if !creature_options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options: creature_options,
                            reason: ChoiceReason::FlashPutCreature,
                        },
                    });
                }
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
                // Draw a card if an opponent has cast a blue or black spell this turn.
                // Simplified: check if opponent controls any blue or black permanents as a proxy
                // for having cast blue/black spells. In a game tree search this is a reasonable heuristic.
                let opp = self.opponent(controller);
                let opp_has_blue_or_black = self.battlefield.iter().any(|p| {
                    p.controller == opp && p.colors.iter().any(|c| matches!(c, Color::Blue | Color::Black))
                });
                if opp_has_blue_or_black {
                    self.draw_cards(controller, 1);
                }
                // Your spells can't be countered this turn.
                // You and permanents you control gain hexproof from blue and from black until end of turn.
                self.players[controller as usize].veil_of_summer_active = true;
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
            // Consider: surveil 1, then draw 1.
            // The surveil sets a pending binary choice; once resolved, the draw fires.
            CardName::Consider => {
                self.surveil(controller, 1, true);
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
                    self.check_emrakul_graveyard_shuffle(pid as PlayerId);
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
                match modes.first().copied().unwrap_or(0) {
                    0 => {
                        // Mode 0: Deal 3 damage to each creature and each planeswalker
                        let to_damage: Vec<ObjectId> = self.battlefield.iter()
                            .filter(|p| p.is_creature() || p.is_planeswalker())
                            .map(|p| p.id)
                            .collect();
                        for id in to_damage {
                            self.deal_damage_to_target(Target::Object(id), 3, controller);
                        }
                    }
                    _ => {
                        // Mode 1: Destroy all artifacts with mana value 3 or less
                        let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                            .filter(|p| {
                                p.is_artifact() && {
                                    let cmc = find_card(db, p.card_name).map(|d| d.mana_cost.cmc()).unwrap_or(0);
                                    cmc <= 3
                                }
                            })
                            .map(|p| p.id)
                            .collect();
                        for id in to_destroy {
                            self.destroy_permanent(id);
                        }
                    }
                }
            }
            CardName::WrathOfTheSkies => {
                // "You get X {E}, then you may pay any amount of {E}. Destroy each artifact,
                // creature, and enchantment with mana value less than or equal to the amount
                // of {E} paid this way." Simplified: treat X as the amount of energy paid.
                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| {
                        (p.is_creature() || p.is_enchantment() || p.is_artifact()) && {
                            let cmc = find_card(db, p.card_name).map(|d| d.mana_cost.cmc()).unwrap_or(0);
                            cmc <= x_value
                        }
                    })
                    .map(|p| p.id)
                    .collect();
                for id in to_destroy {
                    self.destroy_permanent(id);
                }
            }
            CardName::Meltdown => {
                // Destroy each noncreature artifact with MV <= X
                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| {
                        p.is_artifact() && !p.is_creature() && {
                            let cmc = find_card(db, p.card_name).map(|d| d.mana_cost.cmc()).unwrap_or(0);
                            cmc <= x_value
                        }
                    })
                    .map(|p| p.id)
                    .collect();
                for id in to_destroy {
                    self.destroy_permanent(id);
                }
            }
            CardName::SeedsOfInnocence => {
                // Destroy all artifacts. Controller of each gains life equal to its mana value.
                let to_destroy: Vec<(ObjectId, PlayerId, i32)> = self.battlefield.iter()
                    .filter(|p| p.is_artifact())
                    .map(|p| {
                        let mv = find_card(db, p.card_name).map(|d| d.mana_cost.cmc() as i32).unwrap_or(0);
                        (p.id, p.controller, mv)
                    })
                    .collect();
                for (id, artifact_controller, mv) in &to_destroy {
                    self.destroy_permanent(*id);
                    self.players[*artifact_controller as usize].life += *mv;
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

            CardName::Vandalblast => {
                if targets.is_empty() {
                    // Overloaded: destroy each artifact opponents control
                    let opponent = self.opponent(controller);
                    let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                        .filter(|p| p.controller == opponent && p.is_artifact())
                        .map(|p| p.id)
                        .collect();
                    for id in to_destroy {
                        self.destroy_permanent(id);
                    }
                } else if let Some(Target::Object(target_id)) = targets.first() {
                    // Single target: destroy target artifact you don't control
                    self.destroy_permanent(*target_id);
                }
            }

            CardName::Disenchant | CardName::NaturesClaim | CardName::Fragmentize
            | CardName::AbruptDecay | CardName::AncientGrudge | CardName::ShatteringSpree
            | CardName::MoltenCollapse | CardName::FatalPush
            | CardName::BitterTriumph | CardName::SnuffOut
            | CardName::UntimelyMalfunction | CardName::Crash | CardName::CouncilsJudgment => {
                // Nature's Claim: destroyed permanent's controller gains 4 life
                let target_controller = if card_name == CardName::NaturesClaim {
                    if let Some(Target::Object(target_id)) = targets.first() {
                        self.find_permanent(*target_id).map(|p| p.controller)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Fragmentize: only destroy if MV <= 4
                    if card_name == CardName::Fragmentize {
                        let mv_ok = self.find_permanent(*target_id)
                            .and_then(|p| find_card(db, p.card_name))
                            .map(|d| d.mana_cost.cmc() <= 4)
                            .unwrap_or(false);
                        if mv_ok {
                            self.destroy_permanent(*target_id);
                        }
                    } else if card_name == CardName::FatalPush {
                        // Fatal Push: destroy if MV <= 2 (or MV <= 4 with revolt).
                        // Revolt not yet tracked; approximate as MV <= 4.
                        let mv_ok = self.find_permanent(*target_id)
                            .and_then(|p| find_card(db, p.card_name))
                            .map(|d| d.mana_cost.cmc() <= 4)
                            .unwrap_or(false);
                        if mv_ok {
                            self.destroy_permanent(*target_id);
                        }
                    } else {
                        self.destroy_permanent(*target_id);
                    }
                }
                if let Some(tc) = target_controller {
                    self.players[tc as usize].life += 4;
                }
            }

            CardName::SunderingEruption => {
                // Destroy target artifact or enchantment, then deal 3 damage to each opponent.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
                for pid in 0..self.num_players {
                    if pid != controller {
                        self.players[pid as usize].life -= 3;
                    }
                }
            }

            // === Exile-based removal ===
            CardName::PrismaticEnding | CardName::MarchOfOtherworldlyLight => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Exile);
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
                if !self.library_search_restricted(controller) {
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
            }

            // Tinker: sacrifice an artifact (targets[0]), search for an artifact and put it onto the battlefield.
            CardName::Tinker => {
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                if !self.library_search_restricted(controller) {
                    let searchable: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .and_then(|cn| find_card(db, cn))
                                .map(|def| def.card_types.contains(&CardType::Artifact))
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
            }

            // Transmute Artifact: sacrifice an artifact (targets[0]), search library for
            // an artifact card, put it onto the battlefield (simplified: any artifact).
            CardName::TransmuteArtifact => {
                // Sacrifice the targeted artifact
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                // Search for any artifact in library
                if !self.library_search_restricted(controller) {
                    let searchable: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .and_then(|cn| find_card(db, cn))
                                .map(|def| def.card_types.contains(&CardType::Artifact))
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
            }

            // Natural Order: sacrifice a green creature (targets[0]), tutor a green creature.
            CardName::NaturalOrder => {
                if let Some(Target::Object(sac_id)) = targets.first() {
                    self.destroy_permanent(*sac_id);
                }
                if !self.library_search_restricted(controller) {
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
            }

            // Green Sun's Zenith: search library for a green creature card with MV <= X,
            // put it onto the battlefield. GSZ is then shuffled into the library (not graveyard).
            // The shuffle-into-library is handled in resolve_spell via the gsz_shuffle_back flag.
            CardName::GreenSunsZenith => {
                if !self.library_search_restricted(controller) {
                    let searchable: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .and_then(|cn| find_card(db, cn))
                                .map(|def| {
                                    def.card_types.contains(&CardType::Creature)
                                        && def.color_identity.contains(&Color::Green)
                                        && def.mana_cost.cmc() <= x_value
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
                            self.check_kishla_skimmer_trigger(pid as PlayerId);
                            let card_name = self.card_name_for_id(card_id);
                            if let Some(cn) = card_name {
                                if cage_active || priest_active {
                                    // Grafdigger's Cage / Containment Priest: exile the card instead
                                    self.exile.push((card_id, cn, pid as PlayerId));
                                } else {
                                    let card_def = find_card(db, cn);
                                    let (power, toughness, loyalty, keywords, card_types) = if let Some(def) = card_def {
                                        (def.power, def.toughness, def.loyalty, def.keywords, def.card_types)
                                    } else {
                                        (Some(0), Some(0), None, Keywords::empty(), &[CardType::Creature][..])
                                    };
                                    let perm = Permanent::new(
                                        card_id, cn, controller, pid as PlayerId,
                                        power, toughness, loyalty, keywords, card_types,
                                    );
                                    self.battlefield.push(perm);
                                    self.handle_etb(cn, card_id, controller);
                                    // Lose life equal to mana value
                                    let mana_value = card_def.map(|d| d.mana_cost.cmc()).unwrap_or(0) as i32;
                                    self.players[controller as usize].life -= mana_value;
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
            CardName::TendrilsOfAgony => {
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
                // Storm: push storm_count copies onto the stack as individual items
                if !is_copy {
                    let storm = self.storm_count;
                    let template = crate::stack::StackItem {
                        id: 0,
                        kind: crate::stack::StackItemKind::Spell {
                            card_name: CardName::TendrilsOfAgony,
                            card_id: 0,
                            cast_via_evoke: false,
                        },
                        controller,
                        targets: targets.to_vec(),
                        cant_be_countered: false,
                        x_value: 0,
                        cast_from_graveyard: false,
                        cast_as_adventure: false,
                        modes: vec![],
                        is_copy: false,
                    };
                    for _ in 0..storm {
                        self.stack.push_copy(&template);
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
                // Each copy (including the original) mills exactly 3.
                if let Some(Target::Player(p)) = targets.first() {
                    for _ in 0..3 {
                        if let Some(id) = self.players[*p as usize].library.pop() {
                            self.players[*p as usize].graveyard.push(id);
                        }
                    }
                    self.check_emrakul_graveyard_shuffle(*p);
                }
                // Storm: push storm_count copies onto the stack as individual items
                if !is_copy {
                    let storm = self.storm_count;
                    let template = crate::stack::StackItem {
                        id: 0,
                        kind: crate::stack::StackItemKind::Spell {
                            card_name: CardName::BrainFreeze,
                            card_id: 0,
                            cast_via_evoke: false,
                        },
                        controller,
                        targets: targets.to_vec(),
                        cant_be_countered: false,
                        x_value: 0,
                        cast_from_graveyard: false,
                        cast_as_adventure: false,
                        modes: vec![],
                        is_copy: false,
                    };
                    for _ in 0..storm {
                        self.stack.push_copy(&template);
                    }
                }
            }
            CardName::MindsDesire => {
                // Shuffle library, then exile top card. Until end of turn, you may play
                // that card without paying its mana cost. Storm.
                // Simplified: put top card directly into hand (approximates
                // "cast for free from exile" without needing exile-cast infrastructure).
                // Shuffle is omitted since library order is treated as random in search.
                if let Some(id) = self.players[controller as usize].library.pop() {
                    self.players[controller as usize].hand.push(id);
                }
                // Storm: push storm_count copies onto the stack as individual items
                if !is_copy {
                    let storm = self.storm_count;
                    let template = crate::stack::StackItem {
                        id: 0,
                        kind: crate::stack::StackItemKind::Spell {
                            card_name: CardName::MindsDesire,
                            card_id: 0,
                            cast_via_evoke: false,
                        },
                        controller,
                        targets: targets.to_vec(),
                        cant_be_countered: false,
                        x_value: 0,
                        cast_from_graveyard: false,
                        cast_as_adventure: false,
                        modes: vec![],
                        is_copy: false,
                    };
                    for _ in 0..storm {
                        self.stack.push_copy(&template);
                    }
                }
            }

            // === Reanimation ===
            CardName::Exhume => {
                // Each player puts a creature card from their graveyard onto the battlefield.
                // The caster's chosen creature is in the target; opponent gets first creature card found.
                let cage_active = self.grafdiggers_cage_active();
                let priest_active = self.containment_priest_active();

                // Caster's creature: use the target if provided
                if let Some(Target::Object(target_id)) = targets.first() {
                    for pid in 0..self.num_players as usize {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let card_id = self.players[pid].graveyard.remove(pos);
                            if let Some(cn) = self.card_name_for_id(card_id) {
                                if cage_active || priest_active {
                                    self.exile.push((card_id, cn, pid as PlayerId));
                                } else {
                                    let card_def = find_card(db, cn);
                                    let (power, toughness, loyalty, keywords, card_types) = if let Some(def) = card_def {
                                        (def.power, def.toughness, def.loyalty, def.keywords, def.card_types)
                                    } else {
                                        (Some(0), Some(0), None, Keywords::empty(), &[CardType::Creature][..])
                                    };
                                    let perm = Permanent::new(
                                        card_id, cn, pid as PlayerId, pid as PlayerId,
                                        power, toughness, loyalty, keywords, card_types,
                                    );
                                    self.battlefield.push(perm);
                                    self.handle_etb(cn, card_id, pid as PlayerId);
                                }
                            }
                            break;
                        }
                    }
                }

                // Each other player: pick first creature card from their graveyard
                for pid in 0..self.num_players as usize {
                    if pid as PlayerId == controller {
                        continue;
                    }
                    // Find first creature card in this player's graveyard
                    let creature_pos = self.players[pid].graveyard.iter().position(|&id| {
                        self.card_name_for_id(id)
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| def.card_types.contains(&CardType::Creature))
                            .unwrap_or(false)
                    });
                    if let Some(pos) = creature_pos {
                        let card_id = self.players[pid].graveyard.remove(pos);
                        if let Some(cn) = self.card_name_for_id(card_id) {
                            if cage_active || priest_active {
                                self.exile.push((card_id, cn, pid as PlayerId));
                            } else {
                                let card_def = find_card(db, cn);
                                let (power, toughness, loyalty, keywords, card_types) = if let Some(def) = card_def {
                                    (def.power, def.toughness, def.loyalty, def.keywords, def.card_types)
                                } else {
                                    (Some(0), Some(0), None, Keywords::empty(), &[CardType::Creature][..])
                                };
                                let perm = Permanent::new(
                                    card_id, cn, pid as PlayerId, pid as PlayerId,
                                    power, toughness, loyalty, keywords, card_types,
                                );
                                self.battlefield.push(perm);
                                self.handle_etb(cn, card_id, pid as PlayerId);
                            }
                        }
                    }
                }
            }

            // === Extra turns ===
            CardName::ExpressiveIteration => {
                // Look at top 3, put 1 in hand, put 1 on bottom of library
                // Simplified: draw 2 (put 1 in hand + exile 1 playable this turn ~ 2 cards of advantage)
                self.draw_cards(controller, 2);
            }
            CardName::FlameOfAnor => {
                // Choose one (or two if you control a Wizard):
                //   0: Destroy target artifact
                //   1: Draw two cards
                //   2: Deal 5 damage to target creature
                // Simplified: if no modes given, default to draw 2
                if modes.is_empty() {
                    self.draw_cards(controller, 2);
                } else {
                    let mut target_idx = 0usize;
                    for &mode in modes {
                        match mode {
                            0 => {
                                if let Some(Target::Object(id)) = targets.get(target_idx) {
                                    self.destroy_permanent(*id);
                                }
                                target_idx += 1;
                            }
                            1 => {
                                self.draw_cards(controller, 2);
                            }
                            2 => {
                                if let Some(Target::Object(id)) = targets.get(target_idx) {
                                    self.deal_damage_to_target(Target::Object(*id), 5, controller);
                                }
                                target_idx += 1;
                            }
                            _ => {}
                        }
                    }
                }
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
                // You become the monarch.
                self.become_monarch(controller);
            }

            // === Doomsday ===
            CardName::Doomsday => {
                // Lose half life rounded up, search for 5 cards
                let life = self.players[controller as usize].life;
                let loss = (life + 1) / 2;
                self.players[controller as usize].life -= loss;
                // Simplified: don't actually search
            }

            // === Life from the Loam ===
            CardName::LifeFromTheLoam => {
                // Return up to 3 land cards from graveyard to hand
                let gy = &self.players[controller as usize].graveyard;
                let land_indices: Vec<usize> = gy.iter().enumerate()
                    .filter(|(_, &id)| {
                        self.card_name_for_id(id).map_or(false, |cn| {
                            find_card(db, cn).map_or(false, |d| d.card_types.contains(&CardType::Land))
                        })
                    })
                    .map(|(i, _)| i)
                    .take(3)
                    .collect();
                for &idx in land_indices.iter().rev() {
                    let id = self.players[controller as usize].graveyard.remove(idx);
                    self.players[controller as usize].hand.push(id);
                }
            }

            // === Regrowth and similar ===
            CardName::Regrowth => {
                if let Some(Target::Object(target_id)) = targets.first() {
                    let gy = &mut self.players[controller as usize].graveyard;
                    if let Some(pos) = gy.iter().position(|&id| id == *target_id) {
                        let card = gy.remove(pos);
                        self.players[controller as usize].hand.push(card);
                    }
                }
            }
            CardName::MemorysJourney => {
                // Target player shuffles up to three target cards from their graveyard into their library.
                // targets[0] = Target::Player(pid), targets[1..] = Target::Object(card_id)
                let target_player = match targets.first() {
                    Some(Target::Player(pid)) => *pid as usize,
                    _ => controller as usize,
                };
                for target in targets.iter().skip(1).take(3) {
                    if let Target::Object(target_id) = target {
                        let gy = &mut self.players[target_player].graveyard;
                        if let Some(pos) = gy.iter().position(|&id| id == *target_id) {
                            let card = gy.remove(pos);
                            // Put into target player's library (shuffled in)
                            self.players[target_player].library.push(card);
                        }
                    }
                }
            }

            // === Modal spells ===
            CardName::KolaghanCommand => {
                // Choose two — modes:
                //   0: Return target creature card from your graveyard to your hand
                //   1: Target player discards a card
                //   2: Destroy target artifact
                //   3: Kolaghan's Command deals 2 damage to any target
                // targets layout (ordered by mode):
                //   mode 0 -> Target::Object(graveyard_creature_id)
                //   mode 1 -> Target::Player(discard_player)
                //   mode 2 -> Target::Object(artifact_id)
                //   mode 3 -> Target::Object(creature_or_player) or Target::Player(...)
                let mut target_idx = 0usize;
                for &mode in modes {
                    match mode {
                        0 => {
                            // Return creature from graveyard to hand
                            if let Some(Target::Object(card_id)) = targets.get(target_idx) {
                                let pid = controller as usize;
                                if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *card_id) {
                                    self.players[pid].graveyard.remove(pos);
                                    self.players[pid].hand.push(*card_id);
                                }
                            }
                            target_idx += 1;
                        }
                        1 => {
                            // Target player discards a card (simplified: discard last)
                            if let Some(Target::Player(p)) = targets.get(target_idx) {
                                let discard_player = *p;
                                if let Some(id) = self.players[discard_player as usize].hand.pop() {
                                    self.discard_card(id, discard_player, db);
                                }
                            }
                            target_idx += 1;
                        }
                        2 => {
                            // Destroy target artifact
                            if let Some(Target::Object(artifact_id)) = targets.get(target_idx) {
                                self.destroy_permanent(*artifact_id);
                            }
                            target_idx += 1;
                        }
                        3 => {
                            // Deal 2 damage to any target
                            if let Some(tgt) = targets.get(target_idx) {
                                self.deal_damage_to_target(*tgt, 2, controller);
                            }
                            target_idx += 1;
                        }
                        _ => {}
                    }
                }
            }

            CardName::KozileksCommand => {
                // Choose two — X spell modes:
                //   0: Target player creates X 0/1 Eldrazi Spawn tokens
                //   1: Target player scries X, then draws a card
                //   2: Exile target creature with mana value X or less
                //   3: Exile up to X target cards from graveyards
                let x = x_value;
                let mut target_idx = 0usize;
                for &mode in modes {
                    match mode {
                        0 => {
                            // Create X 0/1 Eldrazi Spawn tokens for target player
                            let target_player = if let Some(Target::Player(p)) = targets.get(target_idx) {
                                *p
                            } else {
                                controller
                            };
                            target_idx += 1;
                            for _ in 0..x {
                                let token_id = self.new_object_id();
                                let mut token = crate::permanent::Permanent::new(
                                    token_id,
                                    CardName::EldraziSpawnToken,
                                    target_player,
                                    target_player,
                                    Some(0),
                                    Some(1),
                                    None,
                                    Keywords::empty(),
                                    &[CardType::Creature],
                                );
                                token.is_token = true;
                                self.battlefield.push(token);
                            }
                        }
                        1 => {
                            // Target player scries X, then draws a card
                            // Simplified: just draw a card (scry is hidden info)
                            let target_player = if let Some(Target::Player(p)) = targets.get(target_idx) {
                                *p
                            } else {
                                controller
                            };
                            target_idx += 1;
                            self.draw_cards(target_player, 1);
                        }
                        2 => {
                            // Exile target creature with mana value X or less
                            if let Some(Target::Object(id)) = targets.get(target_idx) {
                                self.remove_permanent_to_zone(*id, DestinationZone::Exile);
                            }
                            target_idx += 1;
                        }
                        3 => {
                            // Exile up to X target cards from graveyards
                            for _ in 0..x {
                                if let Some(Target::Object(id)) = targets.get(target_idx) {
                                    for pid in 0..self.num_players as usize {
                                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&gid| gid == *id) {
                                            let card = self.players[pid].graveyard.remove(pos);
                                            let cn = self.card_name_for_id(card).unwrap_or(CardName::Plains);
                                            self.exile.push((card, cn, pid as PlayerId));
                                            break;
                                        }
                                    }
                                }
                                target_idx += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }

            CardName::PestControl => {
                // Choose one:
                //   Mode 0: Destroy target artifact or enchantment
                //   Mode 1: Create two 1/1 black and green Pest creature tokens
                match modes.first().copied().unwrap_or(0) {
                    0 => {
                        if let Some(Target::Object(target_id)) = targets.first() {
                            self.destroy_permanent(*target_id);
                        }
                    }
                    1 => {
                        for _ in 0..2 {
                            let token_id = self.new_object_id();
                            let mut token = crate::permanent::Permanent::new(
                                token_id,
                                CardName::PestToken,
                                controller,
                                controller,
                                Some(1),
                                Some(1),
                                None,
                                Keywords::empty(),
                                &[CardType::Creature],
                            );
                            token.is_token = true;
                            token.creature_types = vec![CreatureType::Pest];
                            token.colors = vec![Color::Black, Color::Green];
                            self.battlefield.push(token);
                        }
                    }
                    _ => {}
                }
            }

            CardName::Suplex => {
                // Choose one:
                //   Mode 0: Deal 3 damage to target creature. If it would die this turn, exile instead.
                //   Mode 1: Exile target artifact.
                match modes.first().copied().unwrap_or(0) {
                    0 => {
                        if let Some(Target::Object(target_id)) = targets.first() {
                            self.deal_damage_to_target(Target::Object(*target_id), 3, controller);
                            // Check if the creature has lethal damage and exile it instead of letting it die
                            let should_exile = self.find_permanent(*target_id)
                                .map(|p| p.has_lethal_damage())
                                .unwrap_or(false);
                            if should_exile {
                                self.remove_permanent_to_zone(*target_id, DestinationZone::Exile);
                            }
                        }
                    }
                    1 => {
                        if let Some(Target::Object(target_id)) = targets.first() {
                            self.remove_permanent_to_zone(*target_id, DestinationZone::Exile);
                        }
                    }
                    _ => {}
                }
            }

            // === Copy-spell: Twincast ===
            CardName::Twincast => {
                // Copy target instant or sorcery spell on the stack.
                // targets[0] is the Target::Object(stack_item_id) of the spell to copy.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.stack.copy_spell(*target_id);
                }
            }

            // === Storm: Galvanic Relay ===
            CardName::GalvanicRelay => {
                // Base effect: exile top card of library; simplified as putting it in hand.
                if let Some(id) = self.players[controller as usize].library.pop() {
                    self.players[controller as usize].hand.push(id);
                }
                // Storm: push storm_count copies onto the stack.
                // Copies are marked is_copy=true and will only execute the base effect above
                // (no recursive copy-pushing) because is_copy is true.
                if !is_copy {
                    let storm = self.storm_count;
                    let template = crate::stack::StackItem {
                        id: 0,
                        kind: crate::stack::StackItemKind::Spell {
                            card_name: CardName::GalvanicRelay,
                            card_id: 0,
                            cast_via_evoke: false,
                        },
                        controller,
                        targets: targets.to_vec(),
                        cant_be_countered: false,
                        x_value: 0,
                        cast_from_graveyard: false,
                        cast_as_adventure: false,
                        modes: vec![],
                        is_copy: false, // push_copy will override this to true
                    };
                    for _ in 0..storm {
                        self.stack.push_copy(&template);
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
        // Torpor Orb: creatures entering the battlefield don't cause abilities to trigger.
        let torpor_orb_active = self.battlefield.iter().any(|p| p.card_name == CardName::TorporOrb);
        if torpor_orb_active {
            let is_creature = self.find_permanent(_card_id)
                .map(|p| p.is_creature())
                .unwrap_or(false);
            if is_creature {
                return;
            }
        }
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
        // Torpor Orb: creatures entering the battlefield don't cause abilities to trigger.
        let torpor_orb_active = self.battlefield.iter().any(|p| p.card_name == CardName::TorporOrb);
        if torpor_orb_active {
            let is_creature = self.find_permanent(_card_id)
                .map(|p| p.is_creature())
                .unwrap_or(false);
            if is_creature {
                // Still handle non-trigger ETB effects (counters from X spells like Chalice, Walking Ballista, etc.)
                // These are replacement effects, not triggered abilities.
                match card_name {
                    CardName::ChaliceOfTheVoid => {
                        // Chalice is not a creature, so this won't actually fire here.
                        // But keep the pattern for future X-cost creatures.
                    }
                    CardName::WalkingBallista | CardName::StonecoilSerpent => {
                        if x_value > 0 {
                            if let Some(perm) = self.find_permanent_mut(_card_id) {
                                perm.counters.add(CounterType::PlusOnePlusOne, x_value as i16);
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
        }
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
            // Skyclave Apparition: exile target nonland nontoken permanent MV <= 4 an opponent controls
            CardName::SkyclaveApparition => {
                let opp = self.opponent(controller);
                let targets: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| {
                        p.controller == opp
                            && !p.is_land()
                            && !p.is_token
                    })
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = targets.first() {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::SkyclaveApparitionETB,
                        },
                        controller,
                        vec![Target::Object(target_id)],
                    );
                }
            }
            // Thassa's Oracle: ETB win condition (put on stack so it resolves with db access)
            CardName::ThassasOracle => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::ThassasOracleETB,
                    },
                    controller,
                    vec![],
                );
            }
            // Fury: deal 4 damage divided among any number of target creatures/planeswalkers
            // Simplified: deal 4 to the first opponent creature
            CardName::Fury => {
                let target: Option<ObjectId> = self.battlefield.iter()
                    .find(|p| p.is_creature() && p.controller != controller)
                    .map(|p| p.id);
                if let Some(tid) = target {
                    self.deal_damage_to_target(Target::Object(tid), 4, controller);
                }
            }
            // Dark Confidant: register a recurring upkeep trigger
            CardName::DarkConfidant => {
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfUpkeep {
                        player: controller,
                    },
                    effect: TriggeredEffect::DarkConfidantUpkeep,
                    controller,
                    fires_once: false,
                    source_id: Some(_card_id),
                });
            }
            // Portable Hole: exile target nonland permanent an opponent controls with MV 2 or less
            CardName::PortableHole => {
                let opp = self.opponent(controller);
                let targets: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.controller == opp && !p.is_land() && !p.is_token)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = targets.first() {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::PortableHoleETB { hole_id: _card_id },
                        },
                        controller,
                        vec![Target::Object(target_id)],
                    );
                }
            }
            // Talon Gates of Madara: "When this land enters, up to one target creature phases out."
            // Approximated as exile-and-return-at-next-end-step since phasing is not implemented.
            CardName::TalonGatesOfMadara => {
                // Find target creature (prefer opponent's creatures)
                let opp = self.opponent(controller);
                let target: Option<ObjectId> = self.battlefield.iter()
                    .find(|p| p.is_creature() && p.controller == opp)
                    .or_else(|| self.battlefield.iter().find(|p| p.is_creature() && p.id != _card_id))
                    .map(|p| p.id);
                if let Some(target_id) = target {
                    if let Some(perm) = self.remove_permanent_to_zone(target_id, DestinationZone::Exile) {
                        let owner = perm.owner;
                        // Set up delayed trigger to return the creature at next end step
                        self.add_delayed_trigger(crate::types::DelayedTrigger {
                            condition: crate::types::DelayedTriggerCondition::AtBeginningOfNextEndStep,
                            effect: TriggeredEffect::ExileLinkedReturn { card_id: target_id, card_owner: owner },
                            controller,
                            fires_once: true,
                            source_id: Some(_card_id),
                        });
                    }
                }
            }
            // Argentum Masticore: protection from multicolored + upkeep trigger
            CardName::ArgentumMasticore => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.protections.push(Protection::FromMulticolored);
                }
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfUpkeep {
                        player: controller,
                    },
                    effect: TriggeredEffect::ArgentumMasticoreUpkeep { masticore_id: _card_id },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }
            // The Mightstone and Weakstone: ETB modal (draw 2 or -5/-5)
            CardName::TheMightstoneAndWeakstone => {
                // Mode 0: draw 2 cards (no target needed)
                // Mode 1: target creature gets -5/-5 until end of turn
                // Check if any opponent creature exists for mode 1
                let opp = self.opponent(controller);
                let enemy_creatures: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_creature() && p.controller == opp)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = enemy_creatures.first() {
                    // Push with target for -5/-5 mode (AI can choose)
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::MightstoneWeakstoneETB { permanent_id: _card_id },
                        },
                        controller,
                        vec![Target::Object(target_id)],
                    );
                } else {
                    // No valid creature target — just draw 2
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::MightstoneWeakstoneETB { permanent_id: _card_id },
                        },
                        controller,
                        vec![],
                    );
                }
            }
            // Golos, Tireless Pilgrim: ETB search for a land
            CardName::GolosTirelessPilgrim => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::GolosETB,
                    },
                    controller,
                    vec![],
                );
            }
            // Soul-Guide Lantern: ETB exile target card from a graveyard
            CardName::SoulGuideLantern => {
                // Find a card in any graveyard to target
                let mut target_card: Option<(ObjectId, PlayerId)> = None;
                for pid in 0..self.players.len() {
                    if let Some(&card_id) = self.players[pid].graveyard.last() {
                        target_card = Some((card_id, pid as PlayerId));
                        break;
                    }
                }
                if let Some((card_id, _)) = target_card {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: _card_id,
                            source_name: card_name,
                            effect: TriggeredEffect::SoulGuideLanternETB,
                        },
                        controller,
                        vec![Target::Object(card_id)],
                    );
                }
            }
            // Satyr Wayfinder: ETB reveal top 4, may put a land into hand, rest to graveyard.
            CardName::SatyrWayfinder => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::SatyrWayfinderETB,
                    },
                    controller,
                    vec![],
                );
            }
            // Coveted Jewel: draw 3 cards on ETB
            CardName::CovetedJewel => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::CovetedJewelETB,
                    },
                    controller,
                    vec![],
                );
            }
            // Thundertrap Trainer: ETB — look at top 4 cards, take noncreature nonland
            // Simplified: draw a card (approximation of the impulse-like effect)
            CardName::ThundertrapTrainer => {
                self.draw_cards(controller, 1);
            }
            // Plagon, Lord of the Beach: ETB — draw a card for each creature you control
            // with toughness greater than its power
            CardName::PlagonLordOfTheBeach => {
                let count = self.battlefield.iter()
                    .filter(|p| p.controller == controller && p.is_creature() && p.toughness() > p.power())
                    .count();
                if count > 0 {
                    self.draw_cards(controller, count);
                }
            }
            // Kappa Cannoneer: ETB — put a +1/+1 counter on it
            CardName::KappaCannoneer => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::KappaCannoneerTrigger { cannoneer_id: _card_id },
                    },
                    controller,
                    vec![],
                );
            }
            // Mai, Scornful Striker: each player mills 2 cards on ETB
            CardName::MaiScornfulStriker => {
                for pid in 0..self.players.len() {
                    for _ in 0..2 {
                        if let Some(id) = self.players[pid].library.pop() {
                            self.players[pid].graveyard.push(id);
                        }
                    }
                }
            }
            // Emry, Lurker of the Loch: mill 4 cards on ETB
            CardName::EmryLurkerOfTheLoch => {
                for _ in 0..4 {
                    if let Some(id) = self.players[controller as usize].library.pop() {
                        self.players[controller as usize].graveyard.push(id);
                    }
                }
            }
            // Master of Death: ETB surveil 2, register recurring upkeep trigger
            CardName::MasterOfDeath => {
                self.surveil(controller, 2, false);
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfUpkeep {
                        player: controller,
                    },
                    effect: TriggeredEffect::MasterOfDeathUpkeep { owner: controller },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }
            // Kishla Skimmer: flying is handled via keywords.
            // The "leaves graveyard" trigger is tracked via kishla_skimmer_triggered_this_turn.
            CardName::KishlaSkimmer => {}
            // Snapcaster Mage: ETB handled separately in handle_etb_with_cast_targets
            // because it needs the targets from the CastSpell action.
            CardName::SnapcasterMage => {}
            // Stoneforge Mystic: search for equipment
            CardName::StoneforgeMystic => {}
            // Auriok Champion: protection from black and from red (set on ETB)
            CardName::AuriokChampion => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.protections.push(Protection::FromColor(Color::Black));
                    perm.protections.push(Protection::FromColor(Color::Red));
                }
            }
            // Kor Firewalker: protection from red (set on ETB)
            CardName::KorFirewalker => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.protections.push(Protection::FromColor(Color::Red));
                }
            }
            // True-Name Nemesis: choose a player on ETB, gain protection from that player
            CardName::TrueNameNemesis => {
                // The controller chooses a player (0 or 1 in a 2-player game).
                self.pending_choice = Some(PendingChoice {
                    player: controller,
                    kind: ChoiceKind::ChooseNumber {
                        min: 0,
                        max: (self.num_players - 1) as u32,
                        reason: ChoiceReason::TrueNameNemesisETB { permanent_id: _card_id },
                    },
                });
            }
            // Palace Jailer: become monarch, exile target opponent's creature
            CardName::PalaceJailer => {
                self.become_monarch(controller);
                // Exile target creature an opponent controls (simplified: pick first opponent creature)
                let target_id = self.battlefield.iter()
                    .find(|p| p.controller != controller && p.is_creature())
                    .map(|p| p.id);
                if let Some(tid) = target_id {
                    // Track the exile link: when Palace Jailer leaves, the creature returns.
                    self.exile_linked.push((_card_id, tid));
                    self.remove_permanent_to_zone(tid, DestinationZone::Exile);
                }
            }
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
            // Ajani, Nacatl Pariah: create a 2/1 white Cat Warrior token
            CardName::AjaniNacatlPariah => {
                let token_id = self.new_object_id();
                let mut token = Permanent::new(
                    token_id, CardName::AjaniNacatlPariah, controller, controller,
                    Some(2), Some(1), None, Keywords::empty(), &[CardType::Creature],
                );
                token.is_token = true;
                token.creature_types = vec![CreatureType::Cat, CreatureType::Warrior];
                self.battlefield.push(token);
            }
            // Voice of Victory: create a 1/1 white Human creature token
            CardName::VoiceOfVictory => {
                let token_id = self.new_object_id();
                let mut token = Permanent::new(
                    token_id, CardName::VoiceOfVictory, controller, controller,
                    Some(1), Some(1), None, Keywords::empty(), &[CardType::Creature],
                );
                token.is_token = true;
                token.creature_types = vec![CreatureType::Human];
                self.battlefield.push(token);
            }
            // White Orchid Phantom: destroy target nonbasic land opponent controls
            CardName::WhiteOrchidPhantom => {
                let target_id = self.battlefield.iter()
                    .find(|p| p.controller != controller && p.is_land() && !matches!(p.card_name,
                        CardName::Plains | CardName::Island | CardName::Swamp | CardName::Mountain | CardName::Forest))
                    .map(|p| p.id);
                if let Some(tid) = target_id {
                    self.destroy_permanent(tid);
                }
            }
            // Seasoned Pyromancer: discard 2, draw 2, create tokens for nonland discards
            CardName::SeasonedPyromancer => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::SeasonedPyromancerETB,
                    },
                    controller,
                    vec![],
                );
            }
            // Caves of Chaos Adventurer: take the initiative on ETB
            CardName::CavesOfChaosAdventurer => {
                self.take_initiative(controller);
            }
            // Undermountain Adventurer: take the initiative on ETB
            CardName::UndermountainAdventurer => {
                self.take_initiative(controller);
            }
            // Broadside Bombardiers: boast ability (not yet implemented), no ETB
            CardName::BroadsideBombardiers => {}
            // Avalanche of Sector 7: no ETB (triggered ability handled in triggers.rs)
            CardName::AvalancheOfSector7 => {}
            // Mana Vault / Grim Monolith / Time Vault: set doesnt_untap flag
            CardName::ManaVault | CardName::GrimMonolith | CardName::TimeVault => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.doesnt_untap = true;
                }
            }
            // Shatterskull, the Hammer Pass (MDFC back face): enters tapped
            // (simplified: always enters tapped, skip the "pay 3 life" option)
            CardName::ShatterskullTheHammerPass => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.tapped = true;
                }
            }
            // Starting Town: enters tapped unless it's turn 1-3
            CardName::StartingTown => {
                if self.turn_number > 3 {
                    if let Some(perm) = self.find_permanent_mut(_card_id) {
                        perm.tapped = true;
                    }
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
            // Survey lands: always enter tapped, then surveil 1.
            CardName::MeticulousArchive
            | CardName::UndercitySewers
            | CardName::ThunderingFalls
            | CardName::HedgeMaze => {
                // Always enter tapped
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.tapped = true;
                }
                // Surveil 1
                self.surveil(controller, 1, false);
            }
            // Generous Plunderer: each player creates a Treasure token
            CardName::GenerousPlunderer => {
                let num_players = self.num_players;
                for pid in 0..num_players {
                    self.create_treasure_token(pid);
                }
            }
            // Loran of the Third Path / Witch Enchanter: destroy artifact or enchantment an opponent controls
            CardName::LoranOfTheThirdPath | CardName::WitchEnchanter => {
                let targets: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| (p.is_artifact() || p.is_enchantment()) && p.controller != controller)
                    .map(|p| p.id)
                    .collect();
                if let Some(&target_id) = targets.first() {
                    self.destroy_permanent(target_id);
                }
            }
            // Samwise the Stouthearted: return target permanent card from your graveyard to your hand
            CardName::SamwiseTheStouthearted => {
                let pid = controller as usize;
                let db_local = crate::card::build_card_db();
                // Find a permanent card in our graveyard to return to hand
                // (Simplified: ignores the "put there from the battlefield this turn" restriction)
                let target: Option<usize> = self.players[pid].graveyard.iter().enumerate()
                    .find(|(_, &id)| {
                        self.card_name_for_id(id).map_or(false, |cn| {
                            crate::card::find_card(&db_local, cn).map_or(false, |d| {
                                d.card_types.iter().any(|t| matches!(t,
                                    CardType::Creature | CardType::Artifact | CardType::Enchantment
                                    | CardType::Planeswalker | CardType::Land))
                            })
                        })
                    })
                    .map(|(i, _)| i);
                if let Some(pos) = target {
                    let card_id = self.players[pid].graveyard.remove(pos);
                    self.players[pid].hand.push(card_id);
                    self.check_kishla_skimmer_trigger(controller);
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
            // Also has protection from multicolored (static ability set on ETB).
            CardName::StonecoilSerpent => {
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    if x_value > 0 {
                        perm.counters.add(CounterType::PlusOnePlusOne, x_value as i16);
                    }
                    perm.protections.push(Protection::FromMulticolored);
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

            // Cavern of Souls: player chooses a creature type when it enters.
            // The chosen type is stored on the permanent and used for mana abilities.
            CardName::CavernOfSouls => {
                self.pending_choice = Some(PendingChoice {
                    player: controller,
                    kind: ChoiceKind::ChooseNumber {
                        min: 0,
                        max: (crate::types::CreatureType::ALL.len() as u32).saturating_sub(1),
                        reason: ChoiceReason::CavernOfSoulsETB { cavern_id: _card_id },
                    },
                });
            }

            // Phyrexian Metamorph: enter as a copy of any artifact or creature on the battlefield.
            // The controller chooses a target from all artifacts and creatures (excluding itself).
            // The clone is always an artifact in addition to any other types it copies.
            CardName::PhyrexianMetamorph => {
                let options: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.id != _card_id && (p.is_artifact() || p.is_creature()))
                    .map(|p| p.id)
                    .collect();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::CloneTarget { clone_id: _card_id, is_metamorph: true },
                        },
                    });
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

            // The One Ring: ETB triggers protection from everything until next turn,
            // and registers a recurring upkeep trigger (lose life per burden counter,
            // then add a burden counter).
            CardName::TheOneRing => {
                // Push the ETB protection trigger onto the stack.
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::TheOneRing,
                        effect: TriggeredEffect::TheOneRingETB { ring_id: _card_id },
                    },
                    controller,
                    vec![],
                );
                // Register a recurring upkeep trigger for the controller.
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfUpkeep {
                        player: controller,
                    },
                    effect: TriggeredEffect::TheOneRingUpkeep { ring_id: _card_id },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Chrome Mox: imprint a nonartifact, nonland card from hand on ETB
            CardName::ChromeMox => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::ChromeMox,
                        effect: TriggeredEffect::ChromeMoxETB { mox_id: _card_id },
                    },
                    controller,
                    vec![],
                );
            }

            // Isochron Scepter: imprint an instant with MV <= 2 from hand on ETB
            CardName::IsochronScepter => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::IsochronScepter,
                        effect: TriggeredEffect::IsochronScepterETB { scepter_id: _card_id },
                    },
                    controller,
                    vec![],
                );
            }

            // Hideaway lands: enter tapped, then look at top N cards, exile one face-down.
            // ShelldockIsle: hideaway 4, enters tapped
            CardName::ShelldockIsle => {
                // Enter tapped
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.tapped = true;
                }
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::ShelldockIsle,
                        effect: TriggeredEffect::HideawayETB { land_id: _card_id, n: 4 },
                    },
                    controller,
                    vec![],
                );
            }

            // MosswortBridge: hideaway 4, enters tapped
            CardName::MosswortBridge => {
                // Enter tapped
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.tapped = true;
                }
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::MosswortBridge,
                        effect: TriggeredEffect::HideawayETB { land_id: _card_id, n: 4 },
                    },
                    controller,
                    vec![],
                );
            }

            // Mana Crypt: register a recurring "at the beginning of your upkeep, flip a coin"
            // trigger. The trigger fires every upkeep for the controller and creates a
            // PendingChoice so both outcomes can be explored by the search tree.
            CardName::ManaCrypt => {
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfUpkeep {
                        player: controller,
                    },
                    effect: TriggeredEffect::ManaCryptUpkeep,
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Emperor of Bones: register a recurring "at the beginning of combat on your turn"
            // trigger that exiles a card from a graveyard (simplified: auto-exile best creature).
            CardName::EmperorOfBones => {
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfCombat {
                        player: controller,
                    },
                    effect: TriggeredEffect::EmperorOfBonesExile { emperor_id: _card_id },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Urza's Saga: a Saga enchantment land with 3 chapters.
            // When it enters, add a lore counter and trigger Chapter I.
            // At the beginning of each of the controller's subsequent precombat main phases,
            // add another lore counter and trigger the corresponding chapter.
            // After Chapter III resolves, the saga is sacrificed.
            //
            // Chapter I:   Urza's Saga gains "{T}: Add {C}." (this is a static ability —
            //               the engine handles this implicitly via land activated abilities;
            //               for chapter I we push an empty trigger to mark chapter resolution).
            // Chapter II:  Create a 0/0 colorless Construct artifact creature token that gets
            //               +1/+1 for each artifact you control. (We use a triggered effect.)
            // Chapter III: Search your library for an artifact with MV 0 or 1, put it onto
            //               the battlefield, then shuffle.
            CardName::UrzasSaga => {
                // Add the first lore counter immediately as it enters.
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.counters.add(CounterType::Lore, 1);
                }
                // Push Chapter I trigger onto the stack.
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::UrzasSaga,
                        effect: TriggeredEffect::SagaChapter { saga_id: _card_id, chapter: 1 },
                    },
                    controller,
                    vec![],
                );
                // Register a recurring precombat-main trigger to advance lore counters.
                // It fires every precombat main phase for the saga's controller.
                // The trigger adds a lore counter and fires the next chapter.
                // The saga's sacrifice is handled inside resolve_triggered after Chapter III.
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfPreCombatMain {
                        player: controller,
                    },
                    effect: TriggeredEffect::SagaChapter { saga_id: _card_id, chapter: 0 }, // 0 = advance
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Hidetsugu Consumes All: a Saga enchantment with 3 chapters.
            // Same pattern as Urza's Saga: add a lore counter on ETB, register recurring advance trigger.
            // Chapter I:   Destroy each nonland permanent with mana value 1 or less.
            // Chapter II:  Exile all graveyards.
            // Chapter III: Exile this Saga, then return it to the battlefield transformed.
            CardName::HidetsuguConsumesAll => {
                // Add the first lore counter immediately as it enters.
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.counters.add(CounterType::Lore, 1);
                }
                // Push Chapter I trigger onto the stack.
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::HidetsuguConsumesAll,
                        effect: TriggeredEffect::SagaChapter { saga_id: _card_id, chapter: 1 },
                    },
                    controller,
                    vec![],
                );
                // Register a recurring precombat-main trigger to advance lore counters.
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfPreCombatMain {
                        player: controller,
                    },
                    effect: TriggeredEffect::SagaChapter { saga_id: _card_id, chapter: 0 },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Fable of the Mirror-Breaker: a Saga enchantment with 3 chapters.
            // Same pattern as Urza's Saga / Hidetsugu Consumes All.
            // Chapter I:   Create a 2/2 red Goblin Shaman creature token with
            //              "Whenever this token attacks, create a Treasure token."
            // Chapter II:  You may discard up to two cards. If you do, draw that many cards.
            // Chapter III: Exile this Saga, then return it to the battlefield transformed.
            CardName::FableOfTheMirrorBreaker => {
                // Add the first lore counter immediately as it enters.
                if let Some(perm) = self.find_permanent_mut(_card_id) {
                    perm.counters.add(CounterType::Lore, 1);
                }
                // Push Chapter I trigger onto the stack.
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: CardName::FableOfTheMirrorBreaker,
                        effect: TriggeredEffect::SagaChapter { saga_id: _card_id, chapter: 1 },
                    },
                    controller,
                    vec![],
                );
                // Register a recurring precombat-main trigger to advance lore counters.
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfPreCombatMain {
                        player: controller,
                    },
                    effect: TriggeredEffect::SagaChapter { saga_id: _card_id, chapter: 0 },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Delver of Secrets: register a recurring upkeep trigger to check the top card.
            // At the beginning of your upkeep, reveal the top card of your library.
            // If it's an instant or sorcery, transform Delver of Secrets.
            CardName::DelverOfSecrets => {
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfUpkeep {
                        player: controller,
                    },
                    effect: TriggeredEffect::DelverUpkeep { delver_id: _card_id },
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // White Plume Adventurer: take the initiative on ETB + opponent upkeep untap trigger
            CardName::WhitePlumeAdventurer => {
                self.take_initiative(controller);
                // Register delayed trigger: at the beginning of each opponent's upkeep,
                // untap a creature you control (or all if dungeon completed).
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfOpponentUpkeep {
                        controller,
                    },
                    effect: TriggeredEffect::WhitePlumeAdventurerUntap,
                    controller,
                    fires_once: false,
                    source_id: Some(_card_id),
                });
            }

            // Seasoned Dungeoneer: take the initiative on ETB
            CardName::SeasonedDungeoneer => {
                self.take_initiative(controller);
            }

            // Endurance: target player shuffles their graveyard into their library.
            // Simplified: move all cards from opponent's graveyard to the bottom of their library.
            CardName::Endurance => {
                let target_player = self.opponent(controller);
                let graveyard = std::mem::take(&mut self.players[target_player as usize].graveyard);
                // Insert at the bottom of the library (index 0 = bottom)
                for card_id in graveyard {
                    self.players[target_player as usize].library.insert(0, card_id);
                }
            }

            // Atraxa, Grand Unifier: reveal top 10, put one of each card type into hand, rest on bottom.
            // Simplified for game tree search: draw 4 cards (typical yield is 3-5 cards).
            CardName::AtraxaGrandUnifier => {
                self.draw_cards(controller, 4);
            }

            // Necropotence: set the necropotence_active flag on the controller.
            // This skips their draw step and exiles discards instead of sending to graveyard.
            CardName::Necropotence => {
                self.players[controller as usize].necropotence_active = true;
            }

            // Animate Dead: return target creature from any graveyard to the battlefield
            // under your control with -1/-0. Simplified as a Reanimate variant.
            CardName::AnimateDead => {
                // Find the best creature in any graveyard to reanimate
                let db_local = crate::card::build_card_db();
                let mut target_id: Option<ObjectId> = None;
                let mut target_pid: Option<usize> = None;
                for pid in 0..self.num_players as usize {
                    for &gid in &self.players[pid].graveyard {
                        if let Some(cn) = self.card_name_for_id(gid) {
                            if let Some(def) = find_card(&db_local, cn) {
                                if def.card_types.iter().any(|&t| t == CardType::Creature) {
                                    target_id = Some(gid);
                                    target_pid = Some(pid);
                                    break;
                                }
                            }
                        }
                    }
                    if target_id.is_some() { break; }
                }
                if let (Some(card_id), Some(pid)) = (target_id, target_pid) {
                    let cage_active = self.grafdiggers_cage_active();
                    let priest_active = self.containment_priest_active();
                    if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == card_id) {
                        self.players[pid].graveyard.remove(pos);
                        let cn = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                        if cage_active || priest_active {
                            self.exile.push((card_id, cn, pid as PlayerId));
                        } else if let Some(def) = find_card(&db_local, cn) {
                            let mut perm = Permanent::new(
                                card_id, cn, controller, pid as PlayerId,
                                def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
                            );
                            perm.creature_types = def.creature_types.to_vec();
                            perm.colors = def.color_identity.to_vec();
                            // Apply -1/-0 from Animate Dead
                            perm.power_mod -= 1;
                            self.battlefield.push(perm);
                            self.handle_etb(cn, card_id, controller);
                        }
                    }
                }
            }

            // Mystic Remora: no ETB effect itself; the cast trigger is handled in triggers.rs.
            // Just register it on the battlefield (already done by the spell resolution above).
            CardName::MysticRemora => {}

            // Dress Down: draw a card on ETB, and sacrifice at the beginning of the next end step.
            CardName::DressDown => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::DressDownETB,
                    },
                    controller,
                    vec![],
                );
                // Register delayed trigger to sacrifice at next end step
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfNextEndStep,
                    effect: TriggeredEffect::DressDownSacrifice { permanent_id: _card_id },
                    controller,
                    fires_once: true,
                    source_id: None,
                });
            }

            // Underworld Breach: at the beginning of the end step, sacrifice this enchantment.
            // Register a delayed trigger on ETB to sacrifice at the next end step.
            CardName::UnderworldBreach => {
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfNextEndStep,
                    effect: TriggeredEffect::UnderworldBreachSacrifice { permanent_id: _card_id },
                    controller,
                    fires_once: true,
                    source_id: None,
                });
            }

            // Cryogen Relic: when this artifact enters the battlefield, draw a card.
            CardName::CryogenRelic => {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: _card_id,
                        source_name: card_name,
                        effect: TriggeredEffect::DrawCards(1),
                    },
                    controller,
                    vec![],
                );
            }

            // Roiling Vortex: register recurring upkeep trigger to deal 1 damage to the active player.
            // Fires at the beginning of *each* player's upkeep, not just the controller's.
            CardName::RoilingVortex => {
                self.add_delayed_trigger(crate::types::DelayedTrigger {
                    condition: crate::types::DelayedTriggerCondition::AtBeginningOfNextUpkeep,
                    effect: TriggeredEffect::RoilingVortexUpkeep,
                    controller,
                    fires_once: false,
                    source_id: None,
                });
            }

            // Phyrexian Dreadnought: sacrifice unless you sacrifice 12 power of creatures.
            // Simplified: just sacrifice it (the combo with Stifle/Show and Tell is handled elsewhere).
            CardName::PhyrexianDreadnought => {
                self.destroy_permanent(_card_id);
            }

            // Ichor Wellspring: draw a card on ETB
            CardName::IchorWellspring => {
                self.draw_cards(controller, 1);
            }

            // Pinnacle Emissary: deal 3 damage to target creature or planeswalker
            CardName::PinnacleEmissary => {
                // Find the best target: prefer opponent's creature/planeswalker
                let opp = self.opponent(controller);
                let target = self.battlefield.iter()
                    .filter(|p| (p.is_creature() || p.is_planeswalker()) && p.controller == opp)
                    .min_by_key(|p| p.toughness())
                    .map(|p| p.id);
                if let Some(target_id) = target {
                    self.deal_damage_to_target(Target::Object(target_id), 3, controller);
                }
            }

            // Portal to Phyrexia: each opponent sacrifices three creatures
            CardName::PortalToPhyrexia => {
                for pid in 0..self.num_players {
                    if pid != controller {
                        for _ in 0..3 {
                            let creature = self.battlefield.iter()
                                .filter(|p| p.controller == pid && p.is_creature())
                                .min_by_key(|p| p.power())
                                .map(|p| p.id);
                            if let Some(cid) = creature {
                                self.destroy_permanent(cid);
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    fn resolve_triggered(&mut self, effect: TriggeredEffect, controller: PlayerId, targets: &[Target], db: &[CardDef]) {
        match effect {
            TriggeredEffect::ManaCryptUpkeep => {
                // Create a coin-flip pending choice so the search tree can explore both outcomes.
                // 0 = heads (win the flip, no damage)
                // 1 = tails (lose the flip, Mana Crypt deals 3 damage to you)
                self.pending_choice = Some(PendingChoice {
                    player: controller,
                    kind: ChoiceKind::ChooseNumber {
                        min: 0,
                        max: 1,
                        reason: ChoiceReason::CoinFlip,
                    },
                });
            }
            TriggeredEffect::DelverUpkeep { delver_id } => {
                // Look at the top card of controller's library.
                // If it is an instant or sorcery card, transform Delver of Secrets.
                // Only transform if the permanent is still on the battlefield and not already transformed.
                let still_on_field = self.battlefield.iter().any(|p| p.id == delver_id && !p.transformed);
                if still_on_field {
                    let is_instant_or_sorcery = self.players[controller as usize]
                        .library
                        .last()
                        .and_then(|&top_id| self.card_name_for_id(top_id))
                        .and_then(|cn| find_card(db, cn))
                        .map(|def| def.card_types.iter().any(|&t| matches!(t, CardType::Instant | CardType::Sorcery)))
                        .unwrap_or(false);
                    if is_instant_or_sorcery {
                        self.transform_permanent(delver_id, db);
                    }
                }
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
            TriggeredEffect::RazorkinNeedleheadOpponentDraw => {
                let opp = self.opponent(controller);
                self.players[opp as usize].life -= 1;
            }
            TriggeredEffect::DarkConfidantUpkeep => {
                // Reveal top card, put it in hand, lose life equal to its mana value
                if let Some(id) = self.players[controller as usize].library.pop() {
                    self.players[controller as usize].hand.push(id);
                    // Lose life equal to CMC of the revealed card
                    let life_loss = self.card_name_for_id(id)
                        .and_then(|cn| find_card(db, cn))
                        .map(|def| def.mana_cost.cmc() as i32)
                        .unwrap_or(0);
                    self.players[controller as usize].life -= life_loss;
                }
            }
            TriggeredEffect::MasterOfDeathUpkeep { owner } => {
                // At the beginning of your upkeep, if Master of Death is in your graveyard,
                // pay 1 life and return it to your hand.
                // For game tree search simplicity, we always pay the life if it's in the graveyard.
                let pid = owner as usize;
                if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| {
                    self.card_name_for_id(id) == Some(CardName::MasterOfDeath)
                }) {
                    let card_id = self.players[pid].graveyard.remove(pos);
                    self.players[pid].hand.push(card_id);
                    self.players[pid].life -= 1;
                    self.check_kishla_skimmer_trigger(owner);
                }
            }
            TriggeredEffect::KishlaSkimmerLeavesGraveyard => {
                // Draw a card when a card leaves your graveyard during your turn.
                self.draw_cards(controller, 1);
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
            TriggeredEffect::BarrowgoyfCombatDamage { damage } => {
                // Barrowgoyf deals combat damage to a player: mill that many cards.
                // If you do, you may put a creature card from among them into your hand.
                let pid = controller as usize;
                let mut milled_ids = Vec::new();
                for _ in 0..damage {
                    if let Some(id) = self.players[pid].library.pop() {
                        self.players[pid].graveyard.push(id);
                        milled_ids.push(id);
                    }
                }
                // Find a creature card among the milled cards and put it into hand.
                let creature_id = milled_ids.iter().find(|&&id| {
                    if let Some(cn) = self.card_name_for_id(id) {
                        if let Some(def) = crate::card::find_card(db, cn) {
                            return def.card_types.contains(&CardType::Creature);
                        }
                    }
                    false
                }).copied();
                if let Some(cid) = creature_id {
                    if let Some(pos) = self.players[pid].graveyard.iter().rposition(|&gid| gid == cid) {
                        let card = self.players[pid].graveyard.remove(pos);
                        self.players[pid].hand.push(card);
                    }
                }
                self.check_emrakul_graveyard_shuffle(controller);
            }
            TriggeredEffect::VesselDealsDamage { vessel_id } => {
                // Vessel of the All-Consuming deals damage: put a +1/+1 counter on it.
                if let Some(perm) = self.find_permanent_mut(vessel_id) {
                    perm.counters.add(CounterType::PlusOnePlusOne, 1);
                }
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
                // Record exile link so the creature returns when Solitude leaves
                if let Some(Target::Object(creature_id)) = targets.first() {
                    let creature_id = *creature_id;
                    let power = self.find_permanent(creature_id).map(|p| p.power()).unwrap_or(0);
                    // source_id is the Solitude permanent id (same as card_id when it entered)
                    // We need the Solitude id: find it on battlefield
                    let solitude_id = self.battlefield.iter()
                        .find(|p| p.card_name == CardName::Solitude)
                        .map(|p| p.id);
                    if let Some(perm) = self.remove_permanent_to_zone(creature_id, DestinationZone::Exile) {
                        self.players[perm.controller as usize].life += power as i32;
                        // Record exile link: when Solitude leaves, this card returns
                        if let Some(sol_id) = solitude_id {
                            self.exile_linked.push((sol_id, perm.id));
                        }
                    }
                }
            }
            TriggeredEffect::SkyclaveApparitionETB => {
                // Exile target nonland nontoken permanent MV <= 4 an opponent controls
                // Record exile link and token MV for when Apparition leaves
                if let Some(Target::Object(target_id)) = targets.first() {
                    let target_id = *target_id;
                    // Find the Skyclave Apparition on battlefield controlled by the resolver
                    let apparition_id = self.battlefield.iter()
                        .find(|p| p.card_name == CardName::SkyclaveApparition && p.controller == controller)
                        .map(|p| p.id);
                    let exiled_card_name = self.find_permanent(target_id).map(|p| p.card_name);
                    if let Some(perm) = self.remove_permanent_to_zone(target_id, DestinationZone::Exile) {
                        if let Some(app_id) = apparition_id {
                            // Record exile link
                            self.exile_linked.push((app_id, perm.id));
                            // Record the MV from db (we store the exiled card name in card_registry)
                            // Use 0 as fallback; the token MV is resolved later in the leaves trigger
                            let mv = exiled_card_name
                                .and_then(|cn| {
                                    db.iter().find(|d| d.name == cn).map(|d| d.mana_cost.cmc() as u32)
                                })
                                .unwrap_or(0);
                            self.skyclave_token_mv.push((app_id, mv));
                        }
                    }
                }
            }
            TriggeredEffect::ArchonOfCrueltyTrigger => {
                // Opponent: sacrifice creature, discard, lose 3
                // You: draw, gain 3
                if let Some(Target::Player(opp)) = targets.first() {
                    let opid = *opp as usize;
                    // Opponent sacrifices a creature (simplified: destroy first creature they control)
                    let creature_to_sac: Option<ObjectId> = self.battlefield.iter()
                        .find(|p| p.controller == *opp && p.is_creature())
                        .map(|p| p.id);
                    if let Some(cid) = creature_to_sac {
                        self.destroy_permanent(cid);
                    }
                    // Opponent discards a card
                    if let Some(id) = self.players[opid].hand.pop() {
                        self.players[opid].graveyard.push(id);
                        self.check_emrakul_graveyard_shuffle(opid as PlayerId);
                    }
                    // Opponent loses 3 life
                    self.players[opid].life -= 3;
                    // You draw a card, gain 3 life
                    self.draw_cards(controller, 1);
                    self.players[controller as usize].life += 3;
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
            TriggeredEffect::ExileLinkedReturn { card_id, card_owner } => {
                // Return the exiled card to the battlefield under its owner's control.
                // Remove from exile first.
                self.exile.retain(|(id, _, _)| *id != card_id);
                if let Some(card_name) = self.card_name_for_id(card_id) {
                    if let Some(card_def) = find_card(db, card_name) {
                        let is_creature = card_def.card_types.iter().any(|t| matches!(t, CardType::Creature));
                        let is_artifact = card_def.card_types.iter().any(|t| matches!(t, CardType::Artifact));
                        let is_enchantment = card_def.card_types.iter().any(|t| matches!(t, CardType::Enchantment));
                        let is_planeswalker = card_def.card_types.iter().any(|t| matches!(t, CardType::Planeswalker));
                        let is_land = card_def.card_types.iter().any(|t| matches!(t, CardType::Land));
                        let is_permanent_type = is_creature || is_artifact || is_enchantment || is_planeswalker || is_land;
                        if is_permanent_type {
                            let mut perm = Permanent::new(
                                card_id,
                                card_name,
                                card_owner,
                                card_owner,
                                card_def.power,
                                card_def.toughness,
                                card_def.loyalty,
                                card_def.keywords,
                                card_def.card_types,
                            );
                            perm.is_token = false;
                            self.battlefield.push(perm);
                        }
                    }
                }
            }
            TriggeredEffect::SkyclaveApparitionLeaves { opponent, token_mv, .. } => {
                // Create an X/X blue Illusion token for the opponent, where X = token_mv
                let x = token_mv as i16;
                if x > 0 {
                    let token_id = self.new_object_id();
                    let mut token = Permanent::new(
                        token_id,
                        CardName::SkyclaveApparition, // placeholder name for token
                        opponent,
                        opponent,
                        Some(x),
                        Some(x),
                        None,
                        Keywords::empty(),
                        &[CardType::Creature],
                    );
                    token.is_token = true;
                    self.battlefield.push(token);
                }
            }
            TriggeredEffect::MonarchEndStep => {
                // The monarch draws a card at the beginning of their end step.
                self.draw_cards(controller, 1);
            }
            TriggeredEffect::EmrakulCast => {
                // When you cast Emrakul, take an extra turn after this one.
                self.players[controller as usize].extra_turns += 1;
            }
            TriggeredEffect::DackEmblemControl => {
                // Dack Fayden emblem: gain control of target permanent.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.gain_control(*target_id, controller);
                }
            }
            TriggeredEffect::TezzeretEmblemArtifact => {
                // Tezzeret, Cruel Captain emblem: search library for an artifact, put it
                // onto the battlefield. Simplified: present as a search choice.
                if !self.library_search_restricted(controller) {
                    let options: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .and_then(|cn| crate::card::find_card(db, cn))
                                .map(|def| def.card_types.contains(&crate::types::CardType::Artifact))
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect();
                    if !options.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options,
                                reason: ChoiceReason::GenericSearch,
                            },
                        });
                    }
                }
            }
            TriggeredEffect::SacrificeTarget { permanent_id } => {
                // Sacrifice a specific permanent (e.g. Sneak Attack end-of-turn sacrifice).
                // Only sacrifice if it's still on the battlefield and still controlled by controller.
                let still_on_field = self.find_permanent(permanent_id)
                    .map(|p| p.controller == controller)
                    .unwrap_or(false);
                if still_on_field {
                    self.remove_permanent_to_zone(permanent_id, DestinationZone::Graveyard);
                }
            }
            TriggeredEffect::TheOneRingETB { ring_id: _ } => {
                // Grant the controller protection from everything until their next turn.
                // The protection_from_everything flag on the player is cleared in reset_for_turn,
                // which is called at the start of each new turn for the active player.
                self.players[controller as usize].protection_from_everything = true;
            }
            TriggeredEffect::TheOneRingUpkeep { ring_id } => {
                // At the beginning of your upkeep:
                // 1. Lose 1 life for each burden counter on The One Ring.
                // 2. Put a burden counter on The One Ring.
                // If the ring is no longer on the battlefield, do nothing.
                let ring_info = self.find_permanent(ring_id)
                    .map(|p| (p.controller, p.counters.get(CounterType::Burden)));
                if let Some((ring_controller, burden_count)) = ring_info {
                    // Only trigger if the ring is still controlled by its original controller.
                    if ring_controller != controller {
                        return;
                    }
                    let life_loss = burden_count as i32;
                    if life_loss > 0 {
                        self.players[controller as usize].life -= life_loss;
                    }
                    // Add a burden counter.
                    if let Some(perm_mut) = self.find_permanent_mut(ring_id) {
                        perm_mut.counters.add(CounterType::Burden, 1);
                    }
                }
            }
            TriggeredEffect::ChromeMoxETB { mox_id } => {
                // Imprint: the controller may exile a nonartifact, nonland card from their hand.
                // Collect eligible cards from hand.
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .hand
                    .iter()
                    .copied()
                    .filter(|&id| {
                        if let Some(cn) = self.card_name_for_id(id) {
                            if let Some(def) = find_card(db, cn) {
                                let is_artifact = def.card_types.contains(&crate::types::CardType::Artifact);
                                let is_land = def.card_types.contains(&crate::types::CardType::Land);
                                return !is_artifact && !is_land;
                            }
                        }
                        false
                    })
                    .collect();
                if !options.is_empty() {
                    // Present choice to controller; ChooseCard(0) = decline to imprint.
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::ChromeMoxImprint { mox_id },
                        },
                    });
                }
            }
            TriggeredEffect::IsochronScepterETB { scepter_id } => {
                // Imprint: the controller may exile an instant card with MV <= 2 from their hand.
                let options: Vec<ObjectId> = self.players[controller as usize]
                    .hand
                    .iter()
                    .copied()
                    .filter(|&id| {
                        if let Some(cn) = self.card_name_for_id(id) {
                            if let Some(def) = find_card(db, cn) {
                                let is_instant = def.card_types.contains(&crate::types::CardType::Instant);
                                let mv = def.mana_cost.cmc();
                                return is_instant && mv <= 2;
                            }
                        }
                        false
                    })
                    .collect();
                if !options.is_empty() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseFromList {
                            options,
                            reason: ChoiceReason::IsochronScepterImprint { scepter_id },
                        },
                    });
                }
            }

            TriggeredEffect::HideawayETB { land_id, n } => {
                // Look at the top N cards of the controller's library.
                // The controller chooses one to exile face-down (linked to the land).
                // The rest go on the bottom of the library.
                let pid = controller as usize;
                let lib_len = self.players[pid].library.len();
                let take = (n as usize).min(lib_len);
                if take == 0 {
                    // No cards to look at; do nothing.
                } else {
                    // Pop the top `take` cards (last elements = top of library)
                    let top_cards: Vec<ObjectId> = self.players[pid].library
                        .drain(lib_len - take..)
                        .rev() // reverse so index 0 = topmost card
                        .collect();

                    if top_cards.len() == 1 {
                        // Only one card: auto-exile it (no choice needed)
                        let card_id = top_cards[0];
                        let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                        self.exile.push((card_id, card_name, controller));
                        self.hideaway_exiled.push((land_id, card_id));
                    } else {
                        // Put all top_cards at the bottom of the library so the choice handler
                        // can remove the chosen one from the library and exile it.
                        // Insert at bottom (front of vector) preserving relative order.
                        for &card in top_cards.iter().rev() {
                            self.players[pid].library.insert(0, card);
                        }
                        // Present the choice: pick one to exile face-down.
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options: top_cards,
                                reason: ChoiceReason::HideawayExile { land_id },
                            },
                        });
                    }
                }
            }

            // Saga chapter advancement: `chapter == 0` is the recurring precombat-main trigger
            // that adds a lore counter and pushes the appropriate chapter effect.
            // Specific chapter numbers (1, 2, 3) are the actual chapter ability triggers.
            TriggeredEffect::SagaChapter { saga_id, chapter } => {
                match chapter {
                    // chapter == 0: recurring trigger — add a lore counter, then fire next chapter
                    0 => {
                        // Only proceed if the saga is still on the battlefield and still controlled
                        // by the same controller (it might have left since the trigger was registered).
                        let saga_card_name = self.find_permanent(saga_id)
                            .filter(|p| p.controller == controller)
                            .map(|p| p.card_name);
                        let saga_card_name = match saga_card_name {
                            Some(n) => n,
                            None => return,
                        };
                        // Add a lore counter.
                        let new_lore = {
                            let perm = self.find_permanent_mut(saga_id).unwrap();
                            perm.counters.add(CounterType::Lore, 1);
                            perm.counters.get(CounterType::Lore)
                        };
                        // Push the chapter trigger for this new lore count.
                        let chapter_to_fire = new_lore as u8;
                        self.stack.push(
                            StackItemKind::TriggeredAbility {
                                source_id: saga_id,
                                source_name: saga_card_name,
                                effect: TriggeredEffect::SagaChapter { saga_id, chapter: chapter_to_fire },
                            },
                            controller,
                            vec![],
                        );
                    }

                    // Chapters 1–3: dispatch based on which saga this is.
                    _ => {
                        // Determine which saga this is from the permanent or card registry.
                        let saga_name = self.find_permanent(saga_id)
                            .map(|p| p.card_name)
                            .or_else(|| self.card_name_for_id(saga_id))
                            .unwrap_or(CardName::UrzasSaga);

                        match (saga_name, chapter) {
                            // ---- Urza's Saga ----
                            // Chapter I: Urza's Saga gains "{T}: Add {C}."
                            (CardName::UrzasSaga, 1) => {
                                // Static-ability gain — no immediate effect to resolve.
                            }
                            // Chapter II: Create a 0/0 colorless Construct artifact creature token.
                            (CardName::UrzasSaga, 2) => {
                                let artifact_count = self.battlefield.iter()
                                    .filter(|p| p.controller == controller && p.is_artifact())
                                    .count() as i16;
                                let token_id = self.new_object_id();
                                let mut token = Permanent::new(
                                    token_id,
                                    card_name_for_token(),
                                    controller,
                                    controller,
                                    Some(0),
                                    Some(0),
                                    None,
                                    Keywords::empty(),
                                    &[CardType::Artifact, CardType::Creature],
                                );
                                token.creature_types.push(CreatureType::Construct);
                                token.power_mod += artifact_count;
                                token.toughness_mod += artifact_count;
                                token.is_token = true;
                                self.battlefield.push(token);
                            }
                            // Chapter III: Search for artifact with MV 0 or 1, put onto BF, sacrifice saga.
                            (CardName::UrzasSaga, 3) => {
                                if !self.library_search_restricted(controller) {
                                    let options: Vec<ObjectId> = self.players[controller as usize]
                                        .library
                                        .iter()
                                        .copied()
                                        .filter(|&id| {
                                            self.card_name_for_id(id)
                                                .and_then(|cn| find_card(db, cn))
                                                .map(|def| {
                                                    def.card_types.contains(&CardType::Artifact)
                                                        && def.mana_cost.cmc() <= 1
                                                })
                                                .unwrap_or(false)
                                        })
                                        .collect();
                                    if !options.is_empty() {
                                        self.pending_choice = Some(PendingChoice {
                                            player: controller,
                                            kind: ChoiceKind::ChooseFromList {
                                                options,
                                                reason: ChoiceReason::UrzasSagaChapterIII,
                                            },
                                        });
                                    }
                                }
                                self.stack.push(
                                    StackItemKind::TriggeredAbility {
                                        source_id: saga_id,
                                        source_name: CardName::UrzasSaga,
                                        effect: TriggeredEffect::SagaSacrifice { saga_id },
                                    },
                                    controller,
                                    vec![],
                                );
                            }

                            // ---- Hidetsugu Consumes All ----
                            // Chapter I: Destroy each nonland permanent with mana value 1 or less.
                            (CardName::HidetsuguConsumesAll, 1) => {
                                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                                    .filter(|p| {
                                        // Nonland permanent with mana value <= 1
                                        !p.is_land() && {
                                            let mv = find_card(db, p.card_name)
                                                .map(|def| def.mana_cost.cmc())
                                                .unwrap_or(0);
                                            mv <= 1
                                        }
                                    })
                                    .map(|p| p.id)
                                    .collect();
                                for id in to_destroy {
                                    self.destroy_permanent(id);
                                }
                            }
                            // Chapter II: Exile all graveyards.
                            (CardName::HidetsuguConsumesAll, 2) => {
                                for pid in 0..self.players.len() {
                                    let graveyard = std::mem::take(&mut self.players[pid].graveyard);
                                    for card_id in graveyard {
                                        let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                                        self.exile.push((card_id, card_name, pid as PlayerId));
                                    }
                                }
                            }
                            // Chapter III: Exile this Saga, then return it to the battlefield
                            // transformed under your control.
                            (CardName::HidetsuguConsumesAll, 3) => {
                                // Exile the saga
                                if self.find_permanent(saga_id).is_some() {
                                    self.remove_permanent_to_zone(saga_id, DestinationZone::Exile);
                                }
                                // Remove the recurring lore-counter delayed trigger
                                self.delayed_triggers.retain(|dt| {
                                    !matches!(dt.effect, TriggeredEffect::SagaChapter { saga_id: sid, chapter: 0 } if sid == saga_id)
                                });
                                // Return it to the battlefield transformed as Vessel of the All-Consuming
                                if let Some(target_def) = find_card(db, CardName::VesselOfTheAllConsuming) {
                                    let mut perm = Permanent::new(
                                        saga_id,
                                        CardName::VesselOfTheAllConsuming,
                                        controller,
                                        controller,
                                        target_def.power,
                                        target_def.toughness,
                                        target_def.loyalty,
                                        target_def.keywords,
                                        target_def.card_types,
                                    );
                                    perm.creature_types = target_def.creature_types.to_vec();
                                    perm.colors = target_def.color_identity.to_vec();
                                    perm.transformed = true;
                                    self.battlefield.push(perm);
                                    // Remove from exile (it was just exiled above)
                                    self.exile.retain(|(id, _, _)| *id != saga_id);
                                }
                            }

                            // ---- Fable of the Mirror-Breaker ----
                            // Chapter I: Create a 2/2 red Goblin Shaman creature token
                            // with "Whenever this token attacks, create a Treasure token."
                            (CardName::FableOfTheMirrorBreaker, 1) => {
                                let token_id = self.new_object_id();
                                let mut token = Permanent::new(
                                    token_id,
                                    CardName::FableGoblinToken,
                                    controller,
                                    controller,
                                    Some(2),
                                    Some(2),
                                    None,
                                    Keywords::empty(),
                                    &[CardType::Creature],
                                );
                                token.creature_types = vec![CreatureType::Goblin, CreatureType::Shaman];
                                token.colors = vec![Color::Red];
                                token.is_token = true;
                                self.battlefield.push(token);
                            }
                            // Chapter II: You may discard up to two cards. If you do,
                            // draw that many cards.
                            (CardName::FableOfTheMirrorBreaker, 2) => {
                                // Simplified: discard up to 2 (all available, up to 2), then draw that many.
                                let pid = controller as usize;
                                let discard_count = std::cmp::min(2, self.players[pid].hand.len());
                                for _ in 0..discard_count {
                                    if let Some(card_id) = self.players[pid].hand.pop() {
                                        self.players[pid].graveyard.push(card_id);
                                        self.check_emrakul_graveyard_shuffle(controller);
                                    }
                                }
                                if discard_count > 0 {
                                    self.draw_cards(controller, discard_count);
                                }
                            }
                            // Chapter III: Exile this Saga, then return it to the battlefield
                            // transformed under your control as Reflection of Kiki-Jiki.
                            (CardName::FableOfTheMirrorBreaker, 3) => {
                                // Exile the saga
                                if self.find_permanent(saga_id).is_some() {
                                    self.remove_permanent_to_zone(saga_id, DestinationZone::Exile);
                                }
                                // Remove the recurring lore-counter delayed trigger
                                self.delayed_triggers.retain(|dt| {
                                    !matches!(dt.effect, TriggeredEffect::SagaChapter { saga_id: sid, chapter: 0 } if sid == saga_id)
                                });
                                // Return it to the battlefield transformed as Reflection of Kiki-Jiki
                                if let Some(target_def) = find_card(db, CardName::ReflectionOfKikiJiki) {
                                    let mut perm = Permanent::new(
                                        saga_id,
                                        CardName::ReflectionOfKikiJiki,
                                        controller,
                                        controller,
                                        target_def.power,
                                        target_def.toughness,
                                        target_def.loyalty,
                                        target_def.keywords,
                                        target_def.card_types,
                                    );
                                    perm.creature_types = target_def.creature_types.to_vec();
                                    perm.colors = target_def.color_identity.to_vec();
                                    perm.transformed = true;
                                    self.battlefield.push(perm);
                                    // Remove from exile (it was just exiled above)
                                    self.exile.retain(|(id, _, _)| *id != saga_id);
                                }
                            }

                            // Unhandled saga/chapter combinations: no effect.
                            _ => {}
                        }
                    }
                }
            }

            // Initiative upkeep trigger: venture into the Undercity.
            TriggeredEffect::InitiativeUpkeep => {
                self.venture_into_undercity(controller);
            }

            // Undercity room effects.
            TriggeredEffect::UndercityRoom(room) => {
                use crate::types::UndercityRoom;
                match room {
                    UndercityRoom::Entrance => {
                        // Gain 1 life
                        self.players[controller as usize].life += 1;
                    }
                    UndercityRoom::Archives => {
                        // Create a Treasure token
                        self.create_treasure_token(controller);
                    }
                    UndercityRoom::LostWell => {
                        // Draw a card
                        self.draw_cards(controller, 1);
                    }
                    UndercityRoom::Forge => {
                        // Create a 4/1 red Devil creature token
                        let token_id = self.new_object_id();
                        let token_name = CardName::Plains; // placeholder
                        let mut token = Permanent::new(
                            token_id,
                            token_name,
                            controller,
                            controller,
                            Some(4),
                            Some(1),
                            None,
                            Keywords::empty(),
                            &[CardType::Creature],
                        );
                        token.is_token = true;
                        self.battlefield.push(token);
                    }
                    UndercityRoom::InnerSanctum => {
                        // Draw 3 cards (dungeon complete)
                        self.draw_cards(controller, 3);
                    }
                }
            }

            // Saga sacrifice: the saga's last chapter has resolved; sacrifice it.
            TriggeredEffect::SagaSacrifice { saga_id } => {
                // Remove the recurring lore-counter delayed trigger for this saga so it doesn't
                // fire again after the saga is gone.
                self.delayed_triggers.retain(|dt| {
                    !matches!(dt.effect, TriggeredEffect::SagaChapter { saga_id: sid, chapter: 0 } if sid == saga_id)
                });
                // Sacrifice the saga (send to graveyard).
                if self.find_permanent(saga_id).is_some() {
                    self.remove_permanent_to_zone(saga_id, DestinationZone::Graveyard);
                }
            }

            TriggeredEffect::ArgentumMasticoreUpkeep { masticore_id } => {
                // Sacrifice unless you discard a card.
                // Simplified: if hand is not empty, discard a card; otherwise sacrifice.
                if self.find_permanent(masticore_id).is_some() {
                    if !self.players[controller as usize].hand.is_empty() {
                        // Discard a card (simplified: discard last card in hand)
                        if let Some(card_id) = self.players[controller as usize].hand.pop() {
                            self.discard_card(card_id, controller, db);
                        }
                    } else {
                        // No cards to discard, sacrifice the Masticore
                        self.destroy_permanent(masticore_id);
                    }
                }
            }

            TriggeredEffect::ThassasOracleETB => {
                // If the number of cards in your library <= your devotion to blue, you win.
                let devotion = self.devotion_to(controller, Color::Blue, db);
                let lib_size = self.players[controller as usize].library.len() as u32;
                if devotion >= lib_size {
                    self.result = crate::types::GameResult::Win(controller);
                }
            }

            TriggeredEffect::CovetedJewelETB => {
                self.draw_cards(controller, 3);
            }

            TriggeredEffect::PortableHoleETB { hole_id } => {
                // Exile target nonland permanent an opponent controls with MV <= 2.
                if let Some(Target::Object(target_id)) = targets.first() {
                    let mv_ok = self.find_permanent(*target_id)
                        .and_then(|p| find_card(db, p.card_name))
                        .map(|d| d.mana_cost.cmc() <= 2)
                        .unwrap_or(false);
                    if mv_ok {
                        self.exile_linked.push((hole_id, *target_id));
                        self.remove_permanent_to_zone(*target_id, DestinationZone::Exile);
                    }
                }
            }

            TriggeredEffect::CindervinesDamage { target_player } => {
                // Deal 1 damage to the player who cast the noncreature spell
                self.players[target_player as usize].life -= 1;
            }

            TriggeredEffect::LaviniaCounter { spell_id } => {
                // Counter the spell (if it's still on the stack)
                if let Some(item) = self.stack.remove(spell_id) {
                    self.route_countered_spell(item);
                }
            }

            TriggeredEffect::ChaliceCounter { spell_id } => {
                // Counter the spell (if it's still on the stack and can be countered)
                if let Some(item) = self.stack.remove(spell_id) {
                    if !item.cant_be_countered {
                        self.route_countered_spell(item);
                    } else {
                        // Put it back — can't be countered
                        self.stack.push_with_flags(
                            item.kind,
                            item.controller,
                            item.targets,
                            item.cant_be_countered,
                            item.x_value,
                            item.cast_from_graveyard,
                            item.modes,
                        );
                    }
                }
            }

            TriggeredEffect::OkoExchange => {
                // Exchange control of two targets
                if targets.len() >= 2 {
                    if let (Target::Object(your_id), Target::Object(their_id)) =
                        (targets[0], targets[1])
                    {
                        // Swap controllers
                        let your_ctrl = self.find_permanent(your_id).map(|p| p.controller);
                        let their_ctrl = self.find_permanent(their_id).map(|p| p.controller);
                        if let (Some(yc), Some(tc)) = (your_ctrl, their_ctrl) {
                            if let Some(p) = self.find_permanent_mut(your_id) {
                                p.controller = tc;
                            }
                            if let Some(p) = self.find_permanent_mut(their_id) {
                                p.controller = yc;
                            }
                        }
                    }
                }
            }

            TriggeredEffect::EidolonDamage { target_player } => {
                // Eidolon of the Great Revel deals 2 damage to the player who cast the spell
                self.players[target_player as usize].life -= 2;
            }

            TriggeredEffect::HarshMentorDamage { target_player } => {
                // Harsh Mentor deals 2 damage to the opponent who activated the ability
                self.players[target_player as usize].life -= 2;
            }

            TriggeredEffect::AvalancheDamage { target_player } => {
                // Avalanche of Sector 7 deals 1 damage to the opponent who activated an artifact ability
                self.players[target_player as usize].life -= 1;
            }

            TriggeredEffect::MagebaneLizardDamage { target_player, damage } => {
                // Magebane Lizard deals damage equal to noncreature spells cast this turn
                self.players[target_player as usize].life -= damage as i32;
            }

            TriggeredEffect::AnimateDeadETB => {
                // Handled inline in handle_etb_with_x for AnimateDead
            }

            TriggeredEffect::MysticRemoraOpponentCast => {
                // Mystic Remora: draw a card (simplified — opponents rarely pay 4)
                self.draw_cards(controller, 1);
            }

            TriggeredEffect::DressDownETB => {
                // Dress Down ETB: draw a card
                self.draw_cards(controller, 1);
            }

            TriggeredEffect::DressDownSacrifice { permanent_id } => {
                // Dress Down: sacrifice at the beginning of the next end step
                if self.find_permanent(permanent_id).is_some() {
                    self.remove_permanent_to_zone(permanent_id, DestinationZone::Graveyard);
                }
            }

            TriggeredEffect::UnderworldBreachSacrifice { permanent_id } => {
                // Underworld Breach: sacrifice at the beginning of the end step
                if self.find_permanent(permanent_id).is_some() {
                    self.remove_permanent_to_zone(permanent_id, DestinationZone::Graveyard);
                }
            }

            TriggeredEffect::RoilingVortexUpkeep => {
                // Roiling Vortex: deal 1 damage to the player whose upkeep it is
                let active = self.active_player;
                self.players[active as usize].life -= 1;
            }

            TriggeredEffect::RoilingVortexFreeCast { target_player } => {
                // Roiling Vortex: deal 5 damage to the player who cast a spell without paying its mana cost
                self.players[target_player as usize].life -= 5;
            }

            TriggeredEffect::PatchworkAutomatonCast { automaton_id } => {
                // Patchwork Automaton: put a +1/+1 counter on it
                if let Some(perm) = self.find_permanent_mut(automaton_id) {
                    perm.counters.add(CounterType::PlusOnePlusOne, 1);
                }
            }

            TriggeredEffect::NaduTrigger => {
                // Nadu, Winged Wisdom: reveal the top card of your library.
                // If it's a land card, put it onto the battlefield.
                // Otherwise, put it into your hand.
                let top_card = self.players[controller as usize].library.last().copied();
                if let Some(card_id) = top_card {
                    let card_name_opt = self.card_name_for_id(card_id);
                    if let Some(cn) = card_name_opt {
                        let is_land = find_card(db, cn)
                            .map(|def| def.card_types.contains(&CardType::Land))
                            .unwrap_or(false);
                        // Remove from top of library
                        self.players[controller as usize].library.pop();
                        if is_land {
                            // Put it onto the battlefield (tapped, per the card text in the db)
                            if let Some(def) = find_card(db, cn) {
                                let mut perm = crate::permanent::Permanent::new(
                                    card_id,
                                    cn,
                                    controller,
                                    controller,
                                    def.power,
                                    def.toughness,
                                    def.loyalty,
                                    def.keywords,
                                    def.card_types,
                                );
                                perm.colors = def.color_identity.to_vec();
                                perm.tapped = true;
                                self.battlefield.push(perm);
                                self.handle_etb(cn, card_id, controller);
                            }
                        } else {
                            // Put it into your hand
                            self.players[controller as usize].hand.push(card_id);
                        }
                    }
                }
            }

            TriggeredEffect::DisplacerKittenBlink => {
                // Displacer Kitten: exile up to one target nonland permanent you control,
                // then return it to the battlefield under its owner's control.
                if let Some(Target::Object(target_id)) = targets.first() {
                    let target_id = *target_id;
                    // Get the card info before removing
                    let card_name_opt = self.find_permanent(target_id).map(|p| p.card_name);
                    let owner = self.find_permanent(target_id).map(|p| p.owner);
                    if let (Some(cn), Some(owner)) = (card_name_opt, owner) {
                        // Remove from battlefield (exile then return)
                        self.remove_permanent(target_id);
                        // Return to battlefield under owner's control
                        if let Some(def) = find_card(db, cn) {
                            let mut perm = crate::permanent::Permanent::new(
                                target_id,
                                cn,
                                owner,
                                owner,
                                def.power,
                                def.toughness,
                                def.loyalty,
                                def.keywords,
                                def.card_types,
                            );
                            perm.colors = def.color_identity.to_vec();
                            perm.creature_types = def.creature_types.to_vec();
                            perm.entered_this_turn = true;
                            self.battlefield.push(perm);
                            self.handle_etb(cn, target_id, owner);
                        }
                    }
                }
            }

            TriggeredEffect::KappaCannoneerTrigger { cannoneer_id } => {
                // Kappa Cannoneer: put a +1/+1 counter on it
                if let Some(perm) = self.find_permanent_mut(cannoneer_id) {
                    perm.counters.add(CounterType::PlusOnePlusOne, 1);
                }
            }

            TriggeredEffect::EmryETB => {
                // Emry: mill 4 cards (already handled inline in handle_etb_with_x)
                // This variant is for stack-based resolution if needed.
                for _ in 0..4 {
                    if let Some(id) = self.players[controller as usize].library.pop() {
                        self.players[controller as usize].graveyard.push(id);
                    }
                }
            }

            TriggeredEffect::ChromaticStarDeath => {
                // Chromatic Star: when put into graveyard from the battlefield, draw a card.
                self.draw_cards(controller, 1);
            }

            TriggeredEffect::ScrapTrawlerDeath => {
                // Scrap Trawler: return target artifact card in graveyard with lesser MV to hand.
                // targets[0] should be the artifact to return.
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Find the card in the graveyard and move it to hand
                    let pid = controller as usize;
                    if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                        let card_id = self.players[pid].graveyard.remove(pos);
                        self.players[pid].hand.push(card_id);
                        self.check_kishla_skimmer_trigger(controller);
                    }
                }
            }

            TriggeredEffect::MightstoneWeakstoneETB { permanent_id } => {
                // Modal: mode determined by targets.
                // If targets has a creature target, apply -5/-5 until end of turn.
                // Otherwise, draw 2 cards.
                if let Some(Target::Object(target_id)) = targets.first() {
                    // -5/-5 mode
                    self.add_temporary_effect(TemporaryEffect::ModifyPT {
                        target: *target_id,
                        power: -5,
                        toughness: -5,
                    });
                } else {
                    // Draw 2 mode
                    self.draw_cards(controller, 2);
                }
                let _ = permanent_id;
            }

            TriggeredEffect::GolosETB => {
                // Golos: search library for a land card, put it onto the battlefield tapped.
                // This uses the pending_choice system for land selection.
                if !self.library_search_restricted(controller) {
                    let searchable: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .map(|cn| crate::card::is_land_card(cn))
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect();
                    if !searchable.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options: searchable,
                                reason: ChoiceReason::GolosETBSearch,
                            },
                        });
                    }
                }
            }

            TriggeredEffect::SoulGuideLanternETB => {
                // Exile target card from a graveyard.
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Find and remove from any graveyard
                    for pid in 0..self.players.len() {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == *target_id) {
                            let card_id = self.players[pid].graveyard.remove(pos);
                            let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                            self.exile.push((card_id, card_name, pid as PlayerId));
                            break;
                        }
                    }
                }
            }

            TriggeredEffect::SatyrWayfinderETB => {
                // Satyr Wayfinder: reveal top 4 cards. May put a land into hand. Rest to graveyard.
                let pid = controller as usize;
                let lib_len = self.players[pid].library.len();
                let reveal_count = lib_len.min(4);
                let mut revealed: Vec<ObjectId> = Vec::with_capacity(reveal_count);
                for _ in 0..reveal_count {
                    if let Some(id) = self.players[pid].library.pop() {
                        revealed.push(id);
                    }
                }
                // Find first land among revealed cards and put it into hand
                let mut land_found = false;
                let mut remaining = Vec::with_capacity(reveal_count);
                for &id in &revealed {
                    if !land_found {
                        if let Some(cn) = self.card_name_for_id(id) {
                            if crate::card::is_land_card(cn) {
                                self.players[pid].hand.push(id);
                                land_found = true;
                                continue;
                            }
                        }
                    }
                    remaining.push(id);
                }
                // Put the rest into graveyard
                for id in remaining {
                    self.players[pid].graveyard.push(id);
                }
            }

            TriggeredEffect::HaywireMiteDeath => {
                // Haywire Mite dies: gain 2 life.
                self.players[controller as usize].life += 2;
            }

            TriggeredEffect::EmperorOfBonesExile { emperor_id } => {
                // Emperor of Bones: exile up to one target card from a graveyard.
                // Verify emperor is still on the battlefield.
                if self.find_permanent(emperor_id).is_none() {
                    return;
                }
                // Simplified: find the best creature card in any graveyard and exile it.
                // In a real implementation this would be a player choice.
                let db_local = crate::card::build_card_db();
                let mut best_card_id: Option<ObjectId> = None;
                let mut best_pid: Option<usize> = None;
                for pid in 0..self.num_players as usize {
                    for &gid in &self.players[pid].graveyard {
                        if let Some(cn) = self.card_name_for_id(gid) {
                            if let Some(def) = find_card(&db_local, cn) {
                                if def.card_types.iter().any(|&t| t == CardType::Creature) {
                                    best_card_id = Some(gid);
                                    best_pid = Some(pid);
                                    break;
                                }
                            }
                        }
                    }
                    if best_card_id.is_some() { break; }
                }
                if let (Some(card_id), Some(pid)) = (best_card_id, best_pid) {
                    if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == card_id) {
                        self.players[pid].graveyard.remove(pos);
                        let cn = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                        self.exile.push((card_id, cn, pid as PlayerId));
                        // Track that this card was exiled by this Emperor of Bones
                        self.exile_linked.push((emperor_id, card_id));
                    }
                }
            }

            TriggeredEffect::EmperorOfBonesReanimate { emperor_id } => {
                // Emperor of Bones: put a creature card exiled with this creature onto the
                // battlefield under your control with haste. Sacrifice it at beginning of
                // next end step.
                // Find a creature card exiled by this Emperor.
                let db_local = crate::card::build_card_db();
                let mut reanimate_card_id: Option<ObjectId> = None;
                let mut reanimate_card_name: Option<CardName> = None;
                let mut reanimate_owner: Option<PlayerId> = None;
                // Search exile_linked for cards linked to this emperor
                for &(source_id, exiled_id) in &self.exile_linked {
                    if source_id == emperor_id {
                        // Check if the card is still in exile and is a creature
                        if let Some(pos) = self.exile.iter().position(|&(id, _, _)| id == exiled_id) {
                            let (_, cn, owner) = self.exile[pos];
                            if let Some(def) = find_card(&db_local, cn) {
                                if def.card_types.iter().any(|&t| t == CardType::Creature) {
                                    reanimate_card_id = Some(exiled_id);
                                    reanimate_card_name = Some(cn);
                                    reanimate_owner = Some(owner);
                                    break;
                                }
                            }
                        }
                    }
                }
                if let (Some(card_id), Some(cn), Some(owner)) = (reanimate_card_id, reanimate_card_name, reanimate_owner) {
                    let cage_active = self.grafdiggers_cage_active();
                    let priest_active = self.containment_priest_active();
                    // Remove from exile
                    if let Some(pos) = self.exile.iter().position(|&(id, _, _)| id == card_id) {
                        self.exile.remove(pos);
                    }
                    // Remove the exile_linked entry
                    self.exile_linked.retain(|&(src, exiled)| !(src == emperor_id && exiled == card_id));
                    if cage_active || priest_active {
                        // Can't enter from exile — stays exiled
                        self.exile.push((card_id, cn, owner));
                    } else if let Some(def) = find_card(&db_local, cn) {
                        let mut perm = Permanent::new(
                            card_id, cn, controller, owner,
                            def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
                        );
                        // Grant haste
                        perm.keywords.add(Keyword::Haste);
                        perm.creature_types = def.creature_types.to_vec();
                        perm.colors = def.color_identity.to_vec();
                        self.battlefield.push(perm);
                        self.handle_etb(cn, card_id, controller);
                        // Set up delayed sacrifice at beginning of next end step
                        self.add_delayed_trigger(crate::types::DelayedTrigger {
                            condition: crate::types::DelayedTriggerCondition::AtBeginningOfNextEndStep,
                            effect: TriggeredEffect::SacrificeTarget { permanent_id: card_id },
                            controller,
                            fires_once: true,
                            source_id: None,
                        });
                    }
                }
            }

            TriggeredEffect::PheliaAttackExile { phelia_id } => {
                // Phelia attacks: exile up to one other target nonland permanent.
                // At the beginning of the next end step, return it under its owner's control.
                // If it entered under Phelia's controller, put a +1/+1 counter on Phelia.
                if let Some(Target::Object(target_id)) = targets.first() {
                    let target_id = *target_id;
                    let target_perm = self.find_permanent(target_id);
                    let is_token = target_perm.map(|p| p.is_token).unwrap_or(false);
                    let card_owner = target_perm.map(|p| p.owner).unwrap_or(controller);

                    if is_token {
                        // Tokens cease to exist when exiled; they won't return.
                        self.remove_permanent(target_id);
                    } else {
                        // Exile the target (not linked to Phelia leaving — uses delayed trigger instead)
                        let cn = self.find_permanent(target_id).map(|p| p.card_name).unwrap_or(CardName::Plains);
                        self.remove_permanent(target_id);
                        self.exile.push((target_id, cn, card_owner));

                        // Set up delayed trigger: at the beginning of the next end step, return it.
                        self.add_delayed_trigger(crate::types::DelayedTrigger {
                            condition: crate::types::DelayedTriggerCondition::AtBeginningOfNextEndStep,
                            effect: TriggeredEffect::PheliaEndStepReturn {
                                exiled_card_id: target_id,
                                card_owner,
                                phelia_controller: controller,
                                phelia_id,
                            },
                            controller,
                            fires_once: true,
                            source_id: Some(phelia_id),
                        });
                    }
                }
            }

            TriggeredEffect::PheliaEndStepReturn { exiled_card_id, card_owner: _, phelia_controller, phelia_id } => {
                // Return the exiled card to the battlefield under its owner's control.
                // If it entered under Phelia's controller, put a +1/+1 counter on Phelia.
                if let Some(pos) = self.exile.iter().position(|&(id, _, _)| id == exiled_card_id) {
                    let (card_id, cn, owner) = self.exile.remove(pos);
                    // Check Grafdigger's Cage / Containment Priest
                    let cage_active = self.battlefield.iter().any(|p| p.card_name == CardName::GrafdiggersCage);
                    let priest_active = self.battlefield.iter().any(|p| p.card_name == CardName::ContainmentPriest && p.controller != owner);
                    if cage_active || priest_active {
                        // Can't enter from exile — stays exiled
                        self.exile.push((card_id, cn, owner));
                    } else if let Some(def) = find_card(db, cn) {
                        let perm_id = card_id;
                        let mut perm = crate::permanent::Permanent::new(
                            perm_id,
                            cn,
                            owner,
                            owner,
                            def.power,
                            def.toughness,
                            def.loyalty,
                            def.keywords,
                            def.card_types,
                        );
                        perm.colors = def.color_identity.to_vec();
                        self.battlefield.push(perm);
                        self.handle_etb(cn, card_id, owner);

                        // If it entered under Phelia's controller, put a +1/+1 counter on Phelia.
                        if owner == phelia_controller {
                            if let Some(phelia_perm) = self.find_permanent_mut(phelia_id) {
                                phelia_perm.counters.add(CounterType::PlusOnePlusOne, 1);
                            }
                        }
                    }
                }
            }

            TriggeredEffect::SeasonedDungeoneerAttack => {
                // Target creature explores: reveal the top card of the library.
                // If it's a land card, put it into your hand.
                // Otherwise, put a +1/+1 counter on the creature, then put the card
                // back on top or into the graveyard.
                // Also: the creature gains protection from creatures until end of turn.
                if let Some(Target::Object(creature_id)) = targets.first() {
                    let creature_id = *creature_id;
                    // Explore
                    let pid = controller as usize;
                    if let Some(&top_card_id) = self.players[pid].library.last() {
                        let top_card_name = self.card_name_for_id(top_card_id);
                        let is_land = top_card_name
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| def.card_types.contains(&CardType::Land))
                            .unwrap_or(false);

                        if is_land {
                            // Put the land into hand
                            self.players[pid].library.pop();
                            self.players[pid].hand.push(top_card_id);
                        } else {
                            // Put a +1/+1 counter on the creature
                            if let Some(perm) = self.find_permanent_mut(creature_id) {
                                perm.counters.add(CounterType::PlusOnePlusOne, 1);
                            }
                            // Put the card into the graveyard (simplified: always bin it for search)
                            self.players[pid].library.pop();
                            self.players[pid].graveyard.push(top_card_id);
                            self.check_kishla_skimmer_trigger(controller);
                        }
                    }

                    // Grant protection from creatures until end of turn
                    if let Some(perm) = self.find_permanent_mut(creature_id) {
                        perm.protections.push(Protection::FromCreatures);
                    }
                }
            }

            TriggeredEffect::ClarionConquerorOpponentCast => {
                // Create a 1/1 white Soldier creature token.
                let token_id = self.new_object_id();
                let mut token = Permanent::new(
                    token_id,
                    card_name_for_token(),
                    controller,
                    controller,
                    Some(1),
                    Some(1),
                    None,
                    Keywords::empty(),
                    &[CardType::Creature],
                );
                token.is_token = true;
                token.creature_types = vec![CreatureType::Soldier];
                token.colors = vec![Color::White];
                self.battlefield.push(token);
            }

            TriggeredEffect::WhitePlumeAdventurerUntap => {
                // At the beginning of each opponent's upkeep, untap a creature you control.
                // If you've completed a dungeon (undercity_room >= 4), untap all creatures instead.
                let dungeon_completed = self.undercity_room[controller as usize] >= 4;
                if dungeon_completed {
                    // Untap all creatures you control
                    for perm in &mut self.battlefield {
                        if perm.is_creature() && perm.controller == controller {
                            perm.tapped = false;
                        }
                    }
                } else {
                    // Untap a single creature you control (pick the first tapped creature)
                    if let Some(perm) = self.battlefield.iter_mut()
                        .find(|p| p.is_creature() && p.controller == controller && p.tapped)
                    {
                        perm.tapped = false;
                    }
                }
            }

            TriggeredEffect::SeasonedPyromancerETB => {
                // Discard 2 cards, then draw 2. For each nonland card discarded, create a 1/1 Elemental.
                let pid = controller as usize;
                let mut nonland_count = 0u8;
                // Discard up to 2 cards
                for _ in 0..2 {
                    if let Some(card_id) = self.players[pid].hand.pop() {
                        let is_land = self.card_name_for_id(card_id)
                            .and_then(|cn| find_card(db, cn))
                            .map(|def| def.card_types.contains(&CardType::Land))
                            .unwrap_or(false);
                        if !is_land {
                            nonland_count += 1;
                        }
                        self.players[pid].graveyard.push(card_id);
                        self.check_emrakul_graveyard_shuffle(controller);
                    }
                }
                // Draw 2 cards
                self.draw_cards(controller, 2);
                // Create 1/1 red Elemental tokens for each nonland discarded
                for _ in 0..nonland_count {
                    let token_id = self.new_object_id();
                    let mut token = Permanent::new(
                        token_id,
                        card_name_for_token(),
                        controller,
                        controller,
                        Some(1),
                        Some(1),
                        None,
                        Keywords::empty(),
                        &[CardType::Creature],
                    );
                    token.is_token = true;
                    token.colors = vec![Color::Red];
                    self.battlefield.push(token);
                }
            }

            TriggeredEffect::BroadsideBombardiersDamage => {
                // Boast ability not yet implemented; this variant is kept for exhaustiveness.
            }

            TriggeredEffect::PyrogoyfDeath { power } => {
                // When Pyrogoyf dies, deal damage equal to its last-known power to any target.
                if power > 0 {
                    if let Some(target) = targets.first() {
                        self.deal_damage_to_target(*target, power as u16, controller);
                    }
                }
            }

            TriggeredEffect::GutAttackToken => {
                // Gut: sacrifice another creature or artifact to create a 4/1 skeleton token.
                // Simplified: sacrifice the first eligible permanent and create the token.
                let sac_target = self.battlefield.iter()
                    .find(|p| {
                        p.controller == controller
                            && (p.is_creature() || p.is_artifact())
                            && p.card_name != CardName::GutTrueSoulZealot
                    })
                    .map(|p| p.id);
                if let Some(sac_id) = sac_target {
                    self.destroy_permanent(sac_id);
                    // Create 4/1 black Skeleton token with menace, tapped and attacking
                    let token_id = self.new_object_id();
                    let mut kws = Keywords::empty();
                    kws.add(Keyword::Menace);
                    let mut token = Permanent::new(
                        token_id,
                        card_name_for_token(),
                        controller,
                        controller,
                        Some(4),
                        Some(1),
                        None,
                        kws,
                        &[CardType::Creature],
                    );
                    token.is_token = true;
                    token.colors = vec![Color::Black];
                    token.tapped = true;
                    self.battlefield.push(token);
                    // Also declare the token as attacking
                    let defending_player = self.opponent(controller);
                    self.attackers.push((token_id, defending_player));
                }
            }

            TriggeredEffect::CavesOfChaosAttackExile | TriggeredEffect::ZhaoAttackExile => {
                // Exile the top card of your library. You may play it this turn.
                // Simplified: draw a card (approximation of the exile-and-play effect).
                let pid = controller as usize;
                if let Some(card_id) = self.players[pid].library.pop() {
                    // Put into hand to approximate "may play this turn"
                    self.players[pid].hand.push(card_id);
                }
                // For Zhao: also put a +1/+1 counter when playing from exile
                // (simplified: we skip the counter since we're approximating as draw)
            }

            TriggeredEffect::BonecrusherGiantTargeted { target_player } => {
                // Bonecrusher Giant deals 2 damage to that spell's controller
                self.players[target_player as usize].life -= 2;
            }

            TriggeredEffect::LeovoldTargetDraw => {
                // Leovold, Emissary of Trest: you may draw a card.
                // Always draw (optimal in game tree search).
                self.draw_cards(controller, 1);
            }

            _ => {}
        }
        let _ = db; // suppress unused warning when db not used in all arms
    }

    fn resolve_activated(&mut self, effect: ActivatedEffect, controller: PlayerId, targets: &[Target], db: &[CardDef]) {
        match effect {
            ActivatedEffect::SacrificeForMana { amount: _ } => {
                // Handled at activation time (mana already added, permanent already sacrificed)
            }
            ActivatedEffect::GriselbrandDraw => {
                self.draw_cards(controller, 7);
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
                let count = 3.min(self.players[pid].hand.len());
                let mut to_discard = Vec::with_capacity(count);
                for _ in 0..count {
                    if let Some(id) = self.players[pid].hand.pop() {
                        to_discard.push(id);
                    }
                }
                for id in to_discard {
                    self.discard_card(id, controller, db);
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
                // Destroy target land. Its controller may search their library for a basic land card,
                // put it onto the battlefield, then shuffle.
                if let Some(Target::Object(target_id)) = targets.first() {
                    let land_controller = self.find_permanent(*target_id).map(|p| p.controller);
                    self.destroy_permanent(*target_id);
                    // The destroyed land's controller may search for a basic land
                    if let Some(land_owner) = land_controller {
                        if !self.library_search_restricted(land_owner) {
                            let searchable: Vec<ObjectId> = self.players[land_owner as usize]
                                .library
                                .iter()
                                .filter(|&&id| {
                                    self.card_name_for_id(id)
                                        .map(|cn| crate::card::is_basic_land_card(cn))
                                        .unwrap_or(false)
                                })
                                .copied()
                                .collect();
                            if !searchable.is_empty() {
                                self.pending_choice = Some(PendingChoice {
                                    player: land_owner,
                                    kind: ChoiceKind::ChooseFromList {
                                        options: searchable,
                                        reason: ChoiceReason::GenericSearch,
                                    },
                                });
                            }
                        }
                    }
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
                    creature_types: Vec::new(),
                    cavern_creature_type: None,
                    protections: Vec::new(),
                    colors: Vec::new(),
                    transformed: false,
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
            ActivatedEffect::OkoExchange => {
                // Oko -5: exchange control of target artifact/creature you control
                // and target creature opponent controls with power 3 or less
                if targets.len() >= 2 {
                    if let (Target::Object(your_id), Target::Object(their_id)) =
                        (targets[0], targets[1])
                    {
                        let your_ctrl = self.find_permanent(your_id).map(|p| p.controller);
                        let their_ctrl = self.find_permanent(their_id).map(|p| p.controller);
                        if let (Some(yc), Some(tc)) = (your_ctrl, their_ctrl) {
                            if let Some(p) = self.find_permanent_mut(your_id) {
                                p.controller = tc;
                            }
                            if let Some(p) = self.find_permanent_mut(their_id) {
                                p.controller = yc;
                            }
                        }
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
                        self.check_kishla_skimmer_trigger(controller);
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
                // Kaya +1: exile up to two cards from each graveyard.
                // You gain 2 life if at least one creature card was exiled.
                let mut exiled_creature = false;
                for pid in 0..self.players.len() {
                    let to_exile: Vec<ObjectId> = self.players[pid].graveyard.iter()
                        .rev()
                        .take(2)
                        .copied()
                        .collect();
                    for id in to_exile {
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&gid| gid == id) {
                            self.players[pid].graveyard.remove(pos);
                            let card_name = self.card_name_for_id(id).unwrap_or(CardName::Plains);
                            if let Some(def) = crate::card::find_card(db, card_name) {
                                if def.card_types.contains(&CardType::Creature) {
                                    exiled_creature = true;
                                }
                            }
                            self.exile.push((id, card_name, pid as PlayerId));
                        }
                    }
                }
                if exiled_creature {
                    self.players[controller as usize].life += 2;
                }
            }
            ActivatedEffect::KayaMinus => {
                // Kaya -1: exile target nonland permanent with mana value 1 or less
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Exile);
                }
            }
            ActivatedEffect::KayaUltimate => {
                // Kaya -5: deal damage to target player equal to cards they own in exile,
                // and you gain that much life.
                if let Some(Target::Player(target_player)) = targets.first() {
                    let cards_in_exile = self.exile.iter()
                        .filter(|&&(_, _, owner)| owner == *target_player)
                        .count() as i32;
                    self.players[*target_player as usize].life -= cards_in_exile;
                    self.players[controller as usize].life += cards_in_exile;
                }
            }
            ActivatedEffect::MinscCreateBoo => {
                // Minsc & Boo +1: Create Boo, a legendary 1/1 red Hamster with trample and haste.
                let token_id = self.new_object_id();
                let mut kw = Keywords::empty();
                kw.add(Keyword::Trample);
                kw.add(Keyword::Haste);
                let token = Permanent {
                    id: token_id,
                    card_name: card_name_for_token(),
                    controller,
                    owner: controller,
                    tapped: false,
                    base_power: 1,
                    base_toughness: 1,
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
                    creature_types: vec![CreatureType::Hamster],
                    cavern_creature_type: None,
                    protections: Vec::new(),
                    colors: vec![Color::Red],
                    transformed: false,
                    is_token: true,
                    attached_to: None,
                    attachments: Vec::new(),
                };
                self.battlefield.push(token);
            }
            ActivatedEffect::MinscPump => {
                // Minsc & Boo -2: Target creature gets +X/+0, trample, and haste until EOT, where X = its power.
                if let Some(Target::Object(target_id)) = targets.first() {
                    let power = self.find_permanent(*target_id).map(|p| p.power()).unwrap_or(0);
                    if power > 0 {
                        self.temporary_effects.push(TemporaryEffect::ModifyPT {
                            target: *target_id,
                            power: power,
                            toughness: 0,
                        });
                        if let Some(perm) = self.find_permanent_mut(*target_id) {
                            perm.power_mod += power;
                        }
                    }
                    self.temporary_effects.push(TemporaryEffect::GrantKeyword {
                        target: *target_id,
                        keyword: Keyword::Trample,
                    });
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.keywords.add(Keyword::Trample);
                    }
                    self.temporary_effects.push(TemporaryEffect::GrantKeyword {
                        target: *target_id,
                        keyword: Keyword::Haste,
                    });
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.keywords.add(Keyword::Haste);
                    }
                }
            }
            ActivatedEffect::MinscUltimate => {
                // Minsc & Boo -6: Sacrifice a creature (targets[0]), deal damage equal to its power,
                // draw that many cards.
                if let Some(Target::Object(target_id)) = targets.first() {
                    let power = self.find_permanent(*target_id).map(|p| p.power()).unwrap_or(0);
                    self.destroy_permanent(*target_id);
                    if power > 0 {
                        let opponent = self.opponent(controller);
                        self.players[opponent as usize].life -= power as i32;
                        self.draw_cards(controller, power as usize);
                    }
                }
            }
            ActivatedEffect::CometCreateTokens => {
                // Comet, Stellar Pup 0: Simplified — create two 1/1 tokens.
                for _ in 0..2 {
                    let token_id = self.new_object_id();
                    let token = Permanent {
                        id: token_id,
                        card_name: card_name_for_token(),
                        controller,
                        owner: controller,
                        tapped: false,
                        base_power: 1,
                        base_toughness: 1,
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
                        card_types: vec![CardType::Creature],
                        creature_types: Vec::new(),
                        cavern_creature_type: None,
                        protections: Vec::new(),
                        colors: Vec::new(),
                        transformed: false,
                        is_token: true,
                        attached_to: None,
                        attachments: Vec::new(),
                    };
                    self.battlefield.push(token);
                }
            }
            ActivatedEffect::DovinPrevent => {
                // Dovin, Hand of Control -1: Prevent damage from/to target permanent.
                // Simplified: no-op (damage prevention is hard to model).
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
            ActivatedEffect::CyclingSearchSwamp => {
                // Swampcycling: search library for a card with Swamp subtype, put it into hand.
                if !self.library_search_restricted(controller) {
                    let searchable: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .map(|cn| GameState::has_swamp_subtype(cn))
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect();
                    if !searchable.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options: searchable,
                                reason: ChoiceReason::DemonicTutorSearch,
                            },
                        });
                    }
                }
            }
            ActivatedEffect::CyclingSearchIsland => {
                // Islandcycling: search library for a card with Island subtype, put it into hand.
                if !self.library_search_restricted(controller) {
                    let searchable: Vec<ObjectId> = self.players[controller as usize]
                        .library
                        .iter()
                        .filter(|&&id| {
                            self.card_name_for_id(id)
                                .map(|cn| GameState::has_island_subtype(cn))
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect();
                    if !searchable.is_empty() {
                        self.pending_choice = Some(PendingChoice {
                            player: controller,
                            kind: ChoiceKind::ChooseFromList {
                                options: searchable,
                                reason: ChoiceReason::DemonicTutorSearch,
                            },
                        });
                    }
                }
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
                    creature_types: vec![CreatureType::Shark],
                    cavern_creature_type: None,
                    protections: Vec::new(),
                    colors: Vec::new(),
                    transformed: false,
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
                // Return target artifact, creature, enchantment, or planeswalker to owner's hand.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }

            // === Dack Fayden ===
            ActivatedEffect::DackDraw => {
                // +1: Target player draws 2 cards, then discards 2.
                let discard_player = if let Some(Target::Player(pid)) = targets.first() {
                    *pid
                } else {
                    controller
                };
                self.draw_cards(discard_player, 2);
                let count = 2.min(self.players[discard_player as usize].hand.len());
                let mut to_discard = Vec::with_capacity(count);
                for _ in 0..count {
                    if let Some(id) = self.players[discard_player as usize].hand.pop() {
                        to_discard.push(id);
                    }
                }
                for id in to_discard {
                    self.discard_card(id, discard_player, db);
                }
            }
            ActivatedEffect::DackSteal => {
                // -2: Gain control of target artifact.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.gain_control(*target_id, controller);
                }
            }
            ActivatedEffect::DackUltimate => {
                // -6: You get an emblem with "Whenever you cast a spell that targets one or more
                // permanents, gain control of those permanents."
                self.create_emblem(controller, Emblem::DackFayden);
            }

            // === Wrenn and Six ===
            ActivatedEffect::WrennUltimate => {
                // -7: You get an emblem with "Instant and sorcery cards in your graveyard have retrace."
                self.create_emblem(controller, Emblem::WrennAndSix);
            }

            // === Tezzeret, Cruel Captain ===
            ActivatedEffect::TezzeretDraw => {
                // +1: Draw a card if you control an artifact.
                let controls_artifact = self.battlefield.iter()
                    .any(|p| p.controller == controller && p.is_artifact());
                if controls_artifact {
                    self.draw_cards(controller, 1);
                }
            }
            ActivatedEffect::TezzeretThopter => {
                // -2: Create a 1/1 colorless Thopter artifact creature token with flying.
                let token_id = self.new_object_id();
                let mut kw = Keywords::empty();
                kw.add(Keyword::Flying);
                let token = Permanent {
                    id: token_id,
                    card_name: CardName::ThopterToken,
                    controller,
                    owner: controller,
                    tapped: false,
                    base_power: 1,
                    base_toughness: 1,
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
                    card_types: vec![CardType::Artifact, CardType::Creature],
                    creature_types: vec![CreatureType::Thopter],
                    cavern_creature_type: None,
                    protections: Vec::new(),
                    colors: Vec::new(),
                    transformed: false,
                    is_token: true,
                    attached_to: None,
                    attachments: Vec::new(),
                };
                self.battlefield.push(token);
            }
            ActivatedEffect::TezzeretUltimate => {
                // -7: You get an emblem with "Whenever you cast an artifact spell, search your
                // library for an artifact card, put it onto the battlefield, then shuffle."
                self.create_emblem(controller, Emblem::TezzeretCruelCaptain);
            }

            // === Gideon of the Trials ===
            ActivatedEffect::GideonEmblem => {
                // +0: You get an emblem with "As long as you control a Gideon planeswalker,
                // you can't lose the game and your opponents can't win the game."
                self.create_emblem(controller, Emblem::GideonOfTheTrials);
            }

            // The One Ring {T}: Put a burden counter on The One Ring, then draw a card for each
            // burden counter on it.
            ActivatedEffect::TheOneRingDraw { ring_id } => {
                // Tap the ring and add a burden counter.
                if let Some(perm) = self.find_permanent_mut(ring_id) {
                    perm.tapped = true;
                    perm.counters.add(CounterType::Burden, 1);
                }
                // Draw cards equal to burden counters after adding one.
                let burden = self.find_permanent(ring_id)
                    .map(|p| p.counters.get(CounterType::Burden))
                    .unwrap_or(0);
                if burden > 0 {
                    self.draw_cards(controller, burden as usize);
                }
            }
            // Isochron Scepter {2},{T}: copy and cast the imprinted instant for free.
            ActivatedEffect::IsochronScepterActivated { scepter_id } => {
                // Tap the scepter.
                if let Some(perm) = self.find_permanent_mut(scepter_id) {
                    perm.tapped = true;
                }
                // Find the imprinted card.
                let imprinted_id = self.imprinted.iter()
                    .find(|(perm_id, _)| *perm_id == scepter_id)
                    .map(|(_, card_id)| *card_id);
                if let Some(card_id) = imprinted_id {
                    // Get the card name and cast the effect directly (copy = cast without paying cost).
                    let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                    // Resolve the instant's effect directly (no mana cost).
                    // Simplified: directly resolve the card effect with no targets (basic instants).
                    // For targeted instants, we would need a pending choice; for now resolve inline.
                    let targets: Vec<Target> = Vec::new();
                    let modes: Vec<u8> = Vec::new();
                    self.resolve_card_effect(card_name, controller, &targets, 0, &modes, false, &[]);
                }
            }

            // Hideaway land {T}: cast the hidden card for free (condition already checked in movegen).
            ActivatedEffect::HideawayActivated { land_id } => {
                // Tap the land.
                if let Some(perm) = self.find_permanent_mut(land_id) {
                    perm.tapped = true;
                }
                // Find the exiled card linked to this land.
                let exiled_card_id = self.hideaway_exiled
                    .iter()
                    .find(|(lid, _)| *lid == land_id)
                    .map(|(_, card_id)| *card_id);

                if let Some(card_id) = exiled_card_id {
                    // Remove from hideaway_exiled tracking
                    self.hideaway_exiled.retain(|(lid, _)| *lid != land_id);
                    // Remove from exile
                    let card_name = if let Some(pos) = self.exile.iter().position(|(id, _, _)| *id == card_id) {
                        let (_, cn, _) = self.exile.swap_remove(pos);
                        cn
                    } else {
                        CardName::Plains // fallback
                    };
                    // Cast/play the card for free
                    if let Some(def) = find_card(db, card_name) {
                        let is_permanent = def.card_types.iter().any(|t| matches!(t,
                            CardType::Creature | CardType::Artifact
                            | CardType::Enchantment | CardType::Planeswalker
                            | CardType::Land
                        ));
                        if is_permanent {
                            // Permanents enter the battlefield directly
                            let mut perm = crate::permanent::Permanent::new(
                                card_id, card_name, controller, controller,
                                def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
                            );
                            if def.is_changeling {
                                perm.creature_types = crate::types::CreatureType::ALL.to_vec();
                            } else {
                                perm.creature_types = def.creature_types.to_vec();
                            }
                            perm.colors = def.color_identity.to_vec();
                            self.battlefield.push(perm);
                            self.handle_etb(card_name, card_id, controller);
                        } else {
                            // Instant/sorcery: push onto stack, cast without paying cost
                            let uncounterable = crate::movegen::is_uncounterable(card_name)
                                || self.players[controller as usize].veil_of_summer_active;
                            self.stack.push_with_flags(
                                crate::stack::StackItemKind::Spell {
                                    card_name,
                                    card_id,
                                    cast_via_evoke: false,
                                },
                                controller,
                                vec![],
                                uncounterable,
                                0,
                                false,
                                vec![],
                            );
                            self.players[controller as usize].spells_cast_this_turn += 1;
                            self.players[controller as usize].spells_cast_this_game += 1;
                            self.reset_priority_passes();
                        }
                    }
                }
            }

            // === Walking Ballista ===
            ActivatedEffect::WalkingBallistaAddCounter { ballista_id } => {
                // Put a +1/+1 counter on Walking Ballista
                if let Some(perm) = self.find_permanent_mut(ballista_id) {
                    perm.counters.add(CounterType::PlusOnePlusOne, 1);
                }
            }
            ActivatedEffect::WalkingBallistaPing { ballista_id: _ } => {
                // Deal 1 damage to target (counter already removed at activation)
                if let Some(&target) = targets.first() {
                    self.deal_damage_to_target(target, 1, controller);
                }
            }

            // === Time Vault ===
            ActivatedEffect::TimeVaultExtraTurn => {
                // Take an extra turn after this one
                self.players[controller as usize].extra_turns += 1;
            }
            ActivatedEffect::TimeVaultUntap { vault_id } => {
                // Untap Time Vault (skip turn cost already paid at activation)
                if let Some(perm) = self.find_permanent_mut(vault_id) {
                    perm.tapped = false;
                }
            }

            // === Krark-Clan Ironworks ===
            ActivatedEffect::KrarkClanIronworksSacrifice => {
                // Mana ability — resolved at activation time, nothing to do here
            }

            // === Engineered Explosives ===
            ActivatedEffect::EngineeredExplosivesDestroy { charge_counters } => {
                // Destroy each nonland permanent with mana value equal to charge_counters
                let to_destroy: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| {
                        if p.is_land() {
                            return false;
                        }
                        let mv = find_card(db, p.card_name)
                            .map(|d| d.mana_cost.cmc() as u32)
                            .unwrap_or(0);
                        mv == charge_counters
                    })
                    .map(|p| p.id)
                    .collect();
                for id in to_destroy {
                    self.destroy_permanent(id);
                }
            }

            ActivatedEffect::NecropotencePayLife => {
                // Necropotence: pay 1 life, draw a card (simplified approximation).
                // The actual card exiles from library and puts into hand at end step,
                // but for game tree search, drawing immediately is a reasonable model.
                self.draw_cards(controller, 1);
            }
            ActivatedEffect::MysticForgeExile => {
                // Mystic Forge: exile the top card of your library.
                let pid = controller as usize;
                if let Some(card_id) = self.players[pid].library.pop() {
                    let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                    self.exile.push((card_id, card_name, controller));
                }
            }
            ActivatedEffect::UntapArtifactOrCreature => {
                // Aphetto Alchemist: untap target artifact or creature
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.tapped = false;
                    }
                }
            }
            ActivatedEffect::EmryCastArtifact => {
                // Emry: handled at activation time (grants cast permission via emry_castable_artifacts)
            }

            // === Aether Spellbomb ===
            ActivatedEffect::AetherSpellbombBounce => {
                // Return target creature to its owner's hand
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Hand);
                }
            }
            ActivatedEffect::AetherSpellbombDraw => {
                // Draw a card
                self.draw_cards(controller, 1);
            }

            // === Cryogen Relic ===
            ActivatedEffect::CryogenRelicStun => {
                // Put a stun counter on up to one target tapped creature
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.counters.add(CounterType::Stun, 1);
                    }
                }
                // "up to one" — if no target, nothing happens
            }

            // === Tormod's Crypt ===
            ActivatedEffect::TormodsCryptExile => {
                // Exile target player's graveyard
                if let Some(Target::Player(target_player)) = targets.first() {
                    let pid = *target_player as usize;
                    let gy_cards: Vec<ObjectId> = self.players[pid].graveyard.drain(..).collect();
                    for card_id in gy_cards {
                        let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                        self.exile.push((card_id, card_name, *target_player));
                    }
                }
            }

            // === Soul-Guide Lantern ===
            ActivatedEffect::SoulGuideLanternExile => {
                // Exile each opponent's graveyard
                let opp = self.opponent(controller);
                let pid = opp as usize;
                let gy_cards: Vec<ObjectId> = self.players[pid].graveyard.drain(..).collect();
                for card_id in gy_cards {
                    let card_name = self.card_name_for_id(card_id).unwrap_or(CardName::Plains);
                    self.exile.push((card_id, card_name, opp));
                }
            }
            ActivatedEffect::SoulGuideLanternDraw => {
                // Draw a card
                self.draw_cards(controller, 1);
            }

            // === Manifold Key ===
            ActivatedEffect::ManifoldKeyUnblockable => {
                // Target creature can't be blocked this turn
                if let Some(Target::Object(target_id)) = targets.first() {
                    if let Some(perm) = self.find_permanent_mut(*target_id) {
                        perm.keywords.add(Keyword::CantBeBlocked);
                    }
                }
            }

            // === Haywire Mite ===
            ActivatedEffect::HaywireMiteExile => {
                // Exile target noncreature artifact or noncreature enchantment.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.remove_permanent_to_zone(*target_id, DestinationZone::Exile);
                }
            }

            // === Outland Liberator ===
            ActivatedEffect::OutlandLiberatorDestroy => {
                // Destroy target artifact or enchantment.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }

            // === Gorilla Shaman ===
            ActivatedEffect::GorillaShamanDestroy { target_mv } => {
                // Destroy target noncreature artifact with mana value X.
                if let Some(Target::Object(target_id)) = targets.first() {
                    // Verify target is still a noncreature artifact with the expected MV
                    let valid = self.find_permanent(*target_id).map(|p| {
                        p.is_artifact() && !p.is_creature()
                            && find_card(db, p.card_name)
                                .map(|d| d.mana_cost.cmc())
                                .unwrap_or(0) == target_mv
                    }).unwrap_or(false);
                    if valid {
                        self.destroy_permanent(*target_id);
                    }
                }
            }

            // === Hermit Druid ===
            ActivatedEffect::HermitDruidReveal => {
                // Reveal cards from top of library until a basic land is revealed.
                // Put that land into hand, all other revealed cards into graveyard.
                let pid = controller as usize;
                let mut found_basic = false;
                while let Some(id) = self.players[pid].library.pop() {
                    if let Some(cn) = self.card_name_for_id(id) {
                        if crate::card::is_basic_land_card(cn) {
                            // Put basic land into hand
                            self.players[pid].hand.push(id);
                            found_basic = true;
                            break;
                        }
                    }
                    // Not a basic land — put into graveyard
                    self.players[pid].graveyard.push(id);
                }
                let _ = found_basic;
            }

            // === Boromir, Warden of the Tower ===
            ActivatedEffect::BoromirSacrifice => {
                // Creatures you control gain indestructible until end of turn.
                let creature_ids: Vec<ObjectId> = self.battlefield.iter()
                    .filter(|p| p.is_creature() && p.controller == controller)
                    .map(|p| p.id)
                    .collect();
                for cid in creature_ids {
                    self.add_temporary_effect(TemporaryEffect::GrantKeyword {
                        target: cid,
                        keyword: Keyword::Indestructible,
                    });
                }
            }

            // === Sylvan Safekeeper ===
            ActivatedEffect::SylvanSafekeeperShroud => {
                // Target creature you control gains shroud until end of turn.
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.add_temporary_effect(TemporaryEffect::GrantKeyword {
                        target: *target_id,
                        keyword: Keyword::Shroud,
                    });
                }
            }

            // === Emperor of Bones Adapt 2 ===
            ActivatedEffect::EmperorOfBonesAdapt { emperor_id } => {
                // Adapt 2: if no +1/+1 counters, put two +1/+1 counters on it.
                if let Some(perm) = self.find_permanent_mut(emperor_id) {
                    if perm.counters.get(CounterType::PlusOnePlusOne) == 0 {
                        perm.counters.add(CounterType::PlusOnePlusOne, 2);
                        // Whenever +1/+1 counters are put on Emperor of Bones, trigger reanimate.
                        self.stack.push(
                            StackItemKind::TriggeredAbility {
                                source_id: emperor_id,
                                source_name: CardName::EmperorOfBones,
                                effect: TriggeredEffect::EmperorOfBonesReanimate { emperor_id },
                            },
                            controller,
                            vec![],
                        );
                    }
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
                self.check_emrakul_graveyard_shuffle(item.controller);
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

    /// Transform a double-faced permanent to its back face (or front face if already transformed).
    /// Updates card_name, base_power, base_toughness, keywords, creature_types, and the
    /// `transformed` flag. The permanent's ObjectId, counters, and controller are preserved.
    pub fn transform_permanent(&mut self, perm_id: ObjectId, db: &[CardDef]) {
        let (current_name, is_transformed) = match self.find_permanent(perm_id) {
            Some(p) => (p.card_name, p.transformed),
            None => return,
        };

        // Determine the target face: if already transformed, flip back to front face;
        // otherwise flip to the back face listed in the card definition.
        let target_name = if is_transformed {
            // Find the front face: look for a card whose back_face == current_name
            db.iter()
                .find(|def| def.back_face == Some(current_name))
                .map(|def| def.name)
        } else {
            find_card(db, current_name).and_then(|def| def.back_face)
        };

        let target_name = match target_name {
            Some(n) => n,
            None => return, // Not a DFC or no matching face found
        };

        if let Some(target_def) = find_card(db, target_name) {
            let power = target_def.power;
            let toughness = target_def.toughness;
            let keywords = target_def.keywords;
            let creature_types = target_def.creature_types.to_vec();
            let card_types = target_def.card_types.to_vec();
            let colors = target_def.color_identity.to_vec();

            if let Some(perm) = self.find_permanent_mut(perm_id) {
                perm.card_name = target_name;
                perm.base_power = power.unwrap_or(0);
                perm.base_toughness = toughness.unwrap_or(0);
                perm.keywords = keywords;
                perm.creature_types = creature_types;
                perm.card_types = card_types;
                perm.colors = colors;
                perm.transformed = !is_transformed;
            }
        }
    }
}
