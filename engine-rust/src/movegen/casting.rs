/// Action application: casting spells, activating abilities, and related logic.

use super::is_uncounterable;
use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::mana::*;
use crate::permanent::Permanent;
use crate::stack::*;
use crate::types::*;

impl GameState {
    pub fn apply_action(&mut self, action: &Action, db: &[CardDef]) {
        match action {
            Action::PassPriority => {
                self.pass_priority(db);
            }

            Action::Concede => {
                self.players[self.priority_player as usize].has_lost = true;
                self.check_state_based_actions(db);
            }

            Action::PlayLand(card_id) => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if self.players[player_id as usize].remove_from_hand(*card_id) {
                    self.players[player_id as usize].land_plays_remaining -= 1;
                    if let Some(cn) = card_name {
                        if let Some(def) = find_card(db, cn) {
                            let mut perm = Permanent::new(
                                *card_id,
                                cn,
                                player_id,
                                player_id,
                                def.power,
                                def.toughness,
                                def.loyalty,
                                def.keywords,
                                def.card_types,
                            );
                            perm.colors = def.color_identity.to_vec();
                            self.battlefield.push(perm);
                            self.handle_etb(cn, *card_id, player_id);
                        }
                    }
                }
            }

            Action::PlayLandFromTop(card_id) => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                // The top of the library must be this card
                let is_top = self.players[player_id as usize].library.last() == Some(card_id);
                if !is_top {
                    return;
                }
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        if def.card_types.contains(&CardType::Land) {
                            // Remove from top of library
                            self.players[player_id as usize].library.pop();
                            self.players[player_id as usize].land_plays_remaining -= 1;
                            let mut perm = Permanent::new(
                                *card_id,
                                cn,
                                player_id,
                                player_id,
                                def.power,
                                def.toughness,
                                def.loyalty,
                                def.keywords,
                                def.card_types,
                            );
                            perm.colors = def.color_identity.to_vec();
                            self.battlefield.push(perm);
                            self.handle_etb(cn, *card_id, player_id);
                        }
                    }
                }
            }

            Action::CastSpell { card_id, targets, x_value, from_graveyard, from_library_top, alt_cost, modes } => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        // Determine whether we're paying an alternate cost or normal mana cost.
                        let is_artifact = def.card_types.contains(&CardType::Artifact);
                        let paid = if let Some(alt) = alt_cost {
                            self.pay_alt_cost(player_id, *card_id, alt)
                        } else if *from_graveyard {
                            // Flashback / Yawgmoth's Will: pay the flashback cost.
                            let base_cost = def.flashback_cost.unwrap_or(def.mana_cost);
                            let taxed = self.effective_cost_with_base(def, player_id, base_cost);
                            if is_artifact {
                                self.players[player_id as usize].mana_pool.pay_for_artifact(&taxed)
                            } else {
                                self.players[player_id as usize].mana_pool.pay(&taxed)
                            }
                        } else if *from_library_top {
                            // Casting from top of library.
                            // Check if the card is actually on top of the library.
                            let is_top = self.players[player_id as usize].library.last() == Some(card_id);
                            if !is_top {
                                false
                            } else {
                                // Check if Bolas's Citadel is on the battlefield (controlled by this player).
                                let citadel_active = self.battlefield.iter().any(|p| {
                                    p.card_name == CardName::BolassCitadel && p.controller == player_id
                                });
                                if citadel_active {
                                    // Pay life equal to the card's mana value instead of mana.
                                    let life_cost = def.mana_cost.cmc() as i32;
                                    if self.players[player_id as usize].life > life_cost {
                                        self.players[player_id as usize].life -= life_cost;
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    // Future Sight / Mystic Forge / Experimental Frenzy: pay normal mana cost.
                                    let mut cost = self.effective_cost(def, player_id);
                                    if def.has_x_cost {
                                        let x_cost = (*x_value as u16) * (def.x_multiplier as u16);
                                        cost.generic = cost.generic.saturating_add(x_cost as u8);
                                    }
                                    if is_artifact {
                                        self.players[player_id as usize].mana_pool.pay_for_artifact(&cost)
                                    } else {
                                        self.players[player_id as usize].mana_pool.pay(&cost)
                                    }
                                }
                            }
                        } else {
                            // Normal mana cost (with tax effects applied).
                            let mut cost = self.effective_cost(def, player_id);
                            // For X spells, add X * x_multiplier to the generic cost.
                            if def.has_x_cost {
                                let x_cost = (*x_value as u16) * (def.x_multiplier as u16);
                                cost.generic = cost.generic.saturating_add(x_cost as u8);
                            }
                            if is_artifact {
                                self.players[player_id as usize].mana_pool.pay_for_artifact(&cost)
                            } else {
                                self.players[player_id as usize].mana_pool.pay(&cost)
                            }
                        };

                        if paid {
                            // Remove the card from the appropriate zone.
                            if *from_graveyard {
                                // Remove from graveyard
                                let gyd = &mut self.players[player_id as usize].graveyard;
                                if let Some(pos) = gyd.iter().position(|&id| id == *card_id) {
                                    gyd.swap_remove(pos);
                                } else {
                                    // Card not found in graveyard — bail out
                                    return;
                                }
                            } else if *from_library_top {
                                // Remove from top of library
                                let lib = &mut self.players[player_id as usize].library;
                                if lib.last() == Some(card_id) {
                                    lib.pop();
                                } else {
                                    return;
                                }
                            } else {
                                self.players[player_id as usize].remove_from_hand(*card_id);
                            }
                            // Check static can't-be-countered (e.g., Abrupt Decay)
                            let mut uncounterable = is_uncounterable(cn);
                            // Cavern of Souls: creature spells of the named type can't be countered.
                            if !uncounterable && def.card_types.contains(&CardType::Creature) {
                                uncounterable = self.cavern_makes_uncounterable(player_id, def, cn);
                            }
                            // Mark evoke-cast spells so resolution can apply the sacrifice trigger.
                            let is_evoke = matches!(alt_cost, Some(AltCost::Evoke { .. }));
                            self.stack.push_with_flags(
                                StackItemKind::Spell {
                                    card_name: cn,
                                    card_id: *card_id,
                                    cast_via_evoke: is_evoke,
                                },
                                player_id,
                                targets.clone(),
                                uncounterable,
                                *x_value,
                                *from_graveyard,
                                modes.clone(),
                            );
                            self.players[player_id as usize].spells_cast_this_turn += 1;
                            if !def.card_types.contains(&CardType::Artifact) {
                                self.players[player_id as usize].nonartifact_spells_cast_this_turn += 1;
                            }
                            let is_noncreature = !def.card_types.contains(&CardType::Creature);
                            if is_noncreature {
                                self.players[player_id as usize].noncreature_spells_cast_this_turn += 1;
                            }
                            self.storm_count += 1;
                            if is_noncreature {
                                self.check_noncreature_cast_triggers(player_id);
                            }
                            // Emrakul, the Aeons Torn: when cast, take an extra turn after this one.
                            if cn == CardName::EmrakulTheAeonsTorn {
                                self.stack.push(
                                    crate::stack::StackItemKind::TriggeredAbility {
                                        source_id: *card_id,
                                        source_name: cn,
                                        effect: crate::stack::TriggeredEffect::EmrakulCast,
                                    },
                                    player_id,
                                    vec![],
                                );
                            }
                            // Dack Fayden emblem: "Whenever you cast a spell that targets one or
                            // more permanents, gain control of those permanents."
                            let permanent_targets: Vec<Target> = targets.iter()
                                .filter(|t| matches!(t, Target::Object(_)))
                                .copied()
                                .collect();
                            if !permanent_targets.is_empty()
                                && self.has_emblem(player_id, crate::game::Emblem::DackFayden)
                            {
                                for &tgt in &permanent_targets {
                                    self.stack.push(
                                        crate::stack::StackItemKind::TriggeredAbility {
                                            source_id: 0,
                                            source_name: crate::card::CardName::Plains,
                                            effect: crate::stack::TriggeredEffect::DackEmblemControl,
                                        },
                                        player_id,
                                        vec![tgt],
                                    );
                                }
                            }
                            // Tezzeret, Cruel Captain emblem: "Whenever you cast an artifact spell,
                            // search your library for an artifact card, put it onto the battlefield."
                            if def.card_types.contains(&CardType::Artifact)
                                && self.has_emblem(player_id, crate::game::Emblem::TezzeretCruelCaptain)
                            {
                                self.stack.push(
                                    crate::stack::StackItemKind::TriggeredAbility {
                                        source_id: 0,
                                        source_name: crate::card::CardName::Plains,
                                        effect: crate::stack::TriggeredEffect::TezzeretEmblemArtifact,
                                    },
                                    player_id,
                                    vec![],
                                );
                            }
                            self.reset_priority_passes();
                        }
                    }
                }
            }

            Action::ActivateManaAbility {
                permanent_id,
                color_choice,
            } => {
                self.activate_mana_ability(*permanent_id, *color_choice);
            }

            Action::ActivateAbility {
                permanent_id,
                ability_index,
                targets,
            } => {
                self.apply_activated_ability(*permanent_id, *ability_index, targets, db);
            }

            Action::DeclareAttacker { creature_id } => {
                let defending_player = self.opponent(self.active_player);
                self.attackers.push((*creature_id, defending_player));
                let card_name = self.find_permanent(*creature_id).map(|p| p.card_name);
                if let Some(perm) = self.find_permanent_mut(*creature_id) {
                    if !perm.keywords.has(Keyword::Vigilance) {
                        perm.tapped = true;
                    }
                    perm.attacked_this_turn = true;
                }
                // Annihilator N: when this creature attacks, the defending player sacrifices N permanents.
                if let Some(cn) = card_name {
                    let n = crate::card::annihilator_value(cn);
                    if n > 0 {
                        self.trigger_annihilator(defending_player, n);
                    }
                }
            }

            Action::ConfirmAttackers => {
                self.action_context = ActionContext::Priority;
                self.advance_phase(); // Move to declare blockers step
            }

            Action::DeclareBlocker {
                blocker_id,
                attacker_id,
            } => {
                self.blockers.push((*blocker_id, *attacker_id));
            }

            Action::ConfirmBlockers => {
                self.action_context = ActionContext::Priority;
                self.advance_phase(); // Move to combat damage
            }

            Action::ChooseCard(card_id) => {
                if let Some(choice) = self.pending_choice.take() {
                    self.resolve_choice(choice, *card_id, db);
                }
            }

            Action::ChooseColor(color) => {
                if let Some(choice) = self.pending_choice.take() {
                    self.resolve_color_choice(choice, *color);
                }
            }

            Action::ChooseNumber(n) => {
                if let Some(choice) = self.pending_choice.take() {
                    self.resolve_number_choice(choice, *n, db);
                }
            }

            Action::ActivateFromHand { card_id, ability_index, targets, x_value } => {
                self.apply_activate_from_hand(*card_id, *ability_index, targets, *x_value, db);
            }

            Action::CompanionFromSideboard => {
                let player_id = self.priority_player;
                let companion_cost = ManaCost { generic: 3, ..ManaCost::ZERO };
                let player = &self.players[player_id as usize];
                if let Some(companion_id) = player.companion {
                    if player.mana_pool.can_pay(&companion_cost) {
                        // Pay the {3} cost
                        self.players[player_id as usize].mana_pool.pay(&companion_cost);
                        // Move companion from "outside the game" into hand
                        self.players[player_id as usize].companion = None;
                        self.players[player_id as usize].hand.push(companion_id);
                        self.reset_priority_passes();
                    }
                }
            }
        }
    }

    fn apply_activate_from_hand(
        &mut self,
        card_id: ObjectId,
        ability_index: u8,
        targets: &[Target],
        x_value: u8,
        _db: &[CardDef],
    ) {
        let player_id = self.priority_player;
        let card_name = match self.card_name_for_id(card_id) {
            Some(cn) => cn,
            None => return,
        };

        match ability_index {
            // 0 = Cycling
            0 => {
                if let Some((cycling_cost, cycling_kind)) = crate::card::cycling_ability(card_name) {
                    // Street Wraith: pay 2 life instead of mana
                    if card_name == CardName::StreetWraith {
                        self.players[player_id as usize].life -= 2;
                    } else {
                        // Pay X mana for Shark Typhoon, or flat mana for normal cycling
                        let mut cost = cycling_cost;
                        if matches!(cycling_kind, CyclingKind::SharkTyphoon) {
                            cost.generic = cost.generic.saturating_add(x_value);
                        }
                        if !self.players[player_id as usize].mana_pool.pay(&cost) {
                            return;
                        }
                    }
                    // Discard the card (cycling counts as a discard for Hollow One)
                    if !self.players[player_id as usize].remove_from_hand(card_id) {
                        return; // Card not in hand
                    }
                    self.players[player_id as usize].graveyard.push(card_id);
                    self.players[player_id as usize].cards_discarded_this_turn += 1;

                    // Push cycling effect to stack
                    let effect = match cycling_kind {
                        CyclingKind::Basic => ActivatedEffect::CyclingDraw,
                        CyclingKind::SharkTyphoon => ActivatedEffect::SharkTyphoonCycling { x_value },
                    };
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: card_id,
                            source_name: card_name,
                            effect,
                        },
                        player_id,
                        vec![],
                    );
                    self.reset_priority_passes();
                }
            }
            // 1 = Channel
            1 => {
                if let Some((channel_cost, channel_kind)) = crate::card::channel_ability(card_name) {
                    if !self.players[player_id as usize].mana_pool.pay(&channel_cost) {
                        return;
                    }
                    // Discard the card
                    if !self.players[player_id as usize].remove_from_hand(card_id) {
                        return;
                    }
                    self.players[player_id as usize].graveyard.push(card_id);

                    // Push channel effect to stack
                    let effect = match channel_kind {
                        ChannelKind::Boseiju => ActivatedEffect::BoseijuChannel,
                        ChannelKind::Otawara => ActivatedEffect::OtawaraChannel,
                    };
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: card_id,
                            source_name: card_name,
                            effect,
                        },
                        player_id,
                        targets.to_vec(),
                    );
                    self.reset_priority_passes();
                }
            }
            _ => {}
        }
    }

    fn apply_activated_ability(
        &mut self,
        permanent_id: ObjectId,
        ability_index: u8,
        targets: &[Target],
        _db: &[CardDef],
    ) {
        let perm = match self.find_permanent(permanent_id) {
            Some(p) => p,
            None => return,
        };
        let card_name = perm.card_name;
        let controller = perm.controller;

        match card_name {
            CardName::BlackLotus => {
                // Sacrifice + add 3 mana of any color
                // Color is chosen via follow-up ChooseColor action
                if self.destroy_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::BlackLotusColor,
                        },
                    });
                }
            }

            CardName::LotusPetal => {
                if self.destroy_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::LotusPetalColor,
                        },
                    });
                }
            }

            CardName::LionEyeDiamond => {
                // Discard hand, sacrifice, add 3 mana of any color
                let hand = std::mem::take(&mut self.players[controller as usize].hand);
                self.players[controller as usize].graveyard.extend(hand);
                if self.destroy_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::BlackLotusColor, // Same effect
                        },
                    });
                }
            }

            // Treasure token: Sacrifice to add one mana of any color
            CardName::TreasureToken => {
                if self.remove_permanent(permanent_id).is_some() {
                    self.pending_choice = Some(PendingChoice {
                        player: controller,
                        kind: ChoiceKind::ChooseColor {
                            reason: ChoiceReason::TreasureSacrificeColor,
                        },
                    });
                }
            }

            // Fetch lands
            CardName::FloodedStrand
            | CardName::PollutedDelta
            | CardName::BloodstainedMire
            | CardName::WoodedFoothills
            | CardName::WindsweptHeath
            | CardName::MistyRainforest
            | CardName::ScaldingTarn
            | CardName::VerdantCatacombs
            | CardName::AridMesa
            | CardName::MarshFlats => {
                self.players[controller as usize].life -= 1;
                self.destroy_permanent(permanent_id);
                // Search for appropriate land - present as choice
                let searchable: Vec<ObjectId> = self.players[controller as usize]
                    .library
                    .iter()
                    .filter(|&&id| {
                        self.card_name_for_id(id)
                            .map(|cn| self.is_fetchable(card_name, cn))
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

            // Strip Mine: destroy target land
            CardName::StripMine if ability_index == 1 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                // Sacrifice Strip Mine
                self.destroy_permanent(permanent_id);
                // Destroy target land
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }

            // Wasteland: destroy target nonbasic land
            CardName::Wasteland if ability_index == 1 => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.tapped = true;
                }
                self.destroy_permanent(permanent_id);
                if let Some(Target::Object(target_id)) = targets.first() {
                    self.destroy_permanent(*target_id);
                }
            }

            // Isochron Scepter: {2},{T} — copy and cast imprinted instant for free.
            CardName::IsochronScepter if ability_index == 0 => {
                // Pay {2} mana cost
                let cost = crate::mana::ManaCost::generic(2);
                if !self.players[controller as usize].mana_pool.pay(&cost) {
                    return;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::IsochronScepterActivated { scepter_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Shelldock Isle: {T} — cast the hidden card for free (ability_index 1).
            // Condition (library <=20) is already verified in activatable_abilities.
            CardName::ShelldockIsle if ability_index == 1 => {
                if let Some(perm_mut) = self.find_permanent_mut(permanent_id) {
                    perm_mut.tapped = true;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::HideawayActivated { land_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // Mosswort Bridge: {T} — cast the hidden card for free (ability_index 1).
            // Condition (control creature with power >= 10) is already verified in activatable_abilities.
            CardName::MosswortBridge if ability_index == 1 => {
                if let Some(perm_mut) = self.find_permanent_mut(permanent_id) {
                    perm_mut.tapped = true;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::HideawayActivated { land_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
                self.reset_priority_passes();
            }

            // The One Ring: {T}: Put a burden counter, then draw cards equal to burden counters.
            CardName::TheOneRing if ability_index == 0 => {
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::TheOneRingDraw { ring_id: permanent_id },
                    },
                    controller,
                    vec![],
                );
            }

            // Planeswalker abilities
            CardName::JaceTheMindSculptor => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 2;
                            // Fateseal - simplified no-op
                        }
                        1 => {
                            // Brainstorm
                            let effect = ActivatedEffect::JaceBrainstorm;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 1;
                            let effect = ActivatedEffect::JaceBounce;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::TeferiTimeRaveler => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            // +1: sorceries have flash - would need continuous effect
                        }
                        1 => {
                            perm.loyalty -= 3;
                            let effect = ActivatedEffect::TeferiBounce;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::DackFayden => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DackDraw,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DackSteal,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 6;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::DackUltimate,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::WrennAndSix => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::WrennReturn,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::WrennPing,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        2 => {
                            perm.loyalty -= 7;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::WrennUltimate,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::TezzeretCruelCaptain => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::TezzeretDraw,
                                },
                                controller,
                                vec![],
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::TezzeretThopter,
                                },
                                controller,
                                vec![],
                            );
                        }
                        2 => {
                            perm.loyalty -= 7;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::TezzeretUltimate,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::GideonOfTheTrials => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::GideonPrevent,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            // 0: Become 4/4 creature (no loyalty change)
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::GideonCreature,
                                },
                                controller,
                                vec![],
                            );
                        }
                        2 => {
                            // +0: Create emblem (no loyalty change)
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::GideonEmblem,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::NarsetParterOfVeils => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::NarsetMinus,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::OkoThiefOfCrowns => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::OkoFood,
                                },
                                controller,
                                vec![],
                            );
                        }
                        1 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::OkoElkify,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::KarnTheGreatCreator => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KarnAnimate,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 2;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KarnWish,
                                },
                                controller,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            CardName::KayaOrzhovUsurper => {
                if let Some(perm) = self.find_permanent_mut(permanent_id) {
                    perm.loyalty_activated_this_turn = true;
                    match ability_index {
                        0 => {
                            perm.loyalty += 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KayaExile,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        1 => {
                            perm.loyalty -= 1;
                            self.stack.push(
                                StackItemKind::ActivatedAbility {
                                    source_id: permanent_id,
                                    source_name: card_name,
                                    effect: ActivatedEffect::KayaMinus,
                                },
                                controller,
                                targets.to_vec(),
                            );
                        }
                        _ => {}
                    }
                }
            }

            // Equipment: equip ability (ability_index == 20) — attach to target creature
            _ if ability_index == 20 && crate::card::equip_cost(card_name).is_some() => {
                // Pay equip cost (generic mana)
                let equip_generic = crate::card::equip_cost(card_name).unwrap();
                let paid = self.players[controller as usize].mana_pool.pay_generic(equip_generic as u32);
                if !paid {
                    return; // Can't afford equip
                }
                // Attach to the target creature
                if let Some(Target::Object(creature_id)) = targets.first() {
                    let creature_id = *creature_id;
                    // Push the equip as a triggered-like activated ability to the stack
                    self.stack.push(
                        StackItemKind::ActivatedAbility {
                            source_id: permanent_id,
                            source_name: card_name,
                            effect: ActivatedEffect::EquipCreature { equipment_id: permanent_id },
                        },
                        controller,
                        vec![Target::Object(creature_id)],
                    );
                }
            }

            // Batterskull bounce ability (ability_index == 21): {3}: Return to hand
            CardName::Batterskull if ability_index == 21 => {
                let paid = self.players[controller as usize].mana_pool.pay_generic(3);
                if !paid {
                    return;
                }
                self.stack.push(
                    StackItemKind::ActivatedAbility {
                        source_id: permanent_id,
                        source_name: card_name,
                        effect: ActivatedEffect::BatterskullBounce,
                    },
                    controller,
                    vec![],
                );
            }

            _ => {}
        }
    }

    fn is_fetchable(&self, fetch: CardName, target: CardName) -> bool {
        match fetch {
            // FloodedStrand fetches Plains or Island lands
            CardName::FloodedStrand => matches!(target,
                // Basic lands
                CardName::Plains | CardName::Island
                // Dual lands (Alpha/Beta)
                | CardName::Tundra | CardName::Savannah | CardName::Scrubland
                | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::TropicalIsland
                // Shock lands with Plains subtype
                | CardName::HallowedFountain | CardName::TempleGarden | CardName::GodlessShrine | CardName::SacredFoundry
                // Shock lands with Island subtype
                | CardName::WateryGrave | CardName::SteamVents | CardName::BreedingPool
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive | CardName::HedgeMaze
                // Survey lands with Island subtype
                | CardName::UndercitySewers
            ),
            // PollutedDelta fetches Island or Swamp lands
            CardName::PollutedDelta => matches!(target,
                // Basic lands
                CardName::Island | CardName::Swamp
                // Dual lands
                | CardName::UndergroundSea | CardName::TropicalIsland | CardName::VolcanicIsland
                | CardName::Badlands | CardName::Bayou | CardName::Tundra
                // Shock lands with Island subtype
                | CardName::WateryGrave | CardName::SteamVents | CardName::BreedingPool
                | CardName::HallowedFountain
                // Shock lands with Swamp subtype
                | CardName::BloodCrypt | CardName::OvergrownTomb | CardName::GodlessShrine
                // Survey lands with Island subtype
                | CardName::UndercitySewers | CardName::MeticulousArchive
                // Survey lands with Swamp subtype (none currently)
            ),
            // BloodstainedMire fetches Swamp or Mountain lands
            CardName::BloodstainedMire => matches!(target,
                // Basic lands
                CardName::Swamp | CardName::Mountain
                // Dual lands
                | CardName::Badlands | CardName::UndergroundSea | CardName::Bayou
                | CardName::VolcanicIsland | CardName::Plateau | CardName::Taiga
                // Shock lands with Swamp subtype
                | CardName::BloodCrypt | CardName::OvergrownTomb | CardName::WateryGrave | CardName::GodlessShrine
                // Shock lands with Mountain subtype
                | CardName::StompingGround | CardName::SteamVents | CardName::SacredFoundry
                // Survey lands with Swamp subtype (none currently)
                // Survey lands with Mountain subtype
                | CardName::ThunderingFalls
                // Survey lands with Island subtype that also have Swamp (UndercitySewers = Island+Swamp)
                | CardName::UndercitySewers
            ),
            // WoodedFoothills fetches Mountain or Forest lands
            CardName::WoodedFoothills => matches!(target,
                // Basic lands
                CardName::Mountain | CardName::Forest
                // Dual lands
                | CardName::Taiga | CardName::VolcanicIsland | CardName::Plateau
                | CardName::Badlands | CardName::Bayou | CardName::Savannah | CardName::TropicalIsland
                // Shock lands with Mountain subtype
                | CardName::StompingGround | CardName::SteamVents | CardName::SacredFoundry | CardName::BloodCrypt
                // Shock lands with Forest subtype
                | CardName::TempleGarden | CardName::OvergrownTomb | CardName::BreedingPool
                // Survey lands with Mountain subtype
                | CardName::ThunderingFalls
                // Survey lands with Forest subtype
                | CardName::HedgeMaze
            ),
            // WindsweptHeath fetches Forest or Plains lands
            CardName::WindsweptHeath => matches!(target,
                // Basic lands
                CardName::Forest | CardName::Plains
                // Dual lands
                | CardName::Savannah | CardName::TropicalIsland | CardName::Bayou
                | CardName::Taiga | CardName::Tundra | CardName::Plateau | CardName::Scrubland
                // Shock lands with Forest subtype
                | CardName::TempleGarden | CardName::OvergrownTomb | CardName::BreedingPool | CardName::StompingGround
                // Shock lands with Plains subtype
                | CardName::HallowedFountain | CardName::GodlessShrine | CardName::SacredFoundry
                // Survey lands with Forest subtype
                | CardName::HedgeMaze | CardName::ThunderingFalls
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive
            ),
            // MistyRainforest fetches Forest or Island lands
            CardName::MistyRainforest => matches!(target,
                // Basic lands
                CardName::Forest | CardName::Island
                // Dual lands
                | CardName::TropicalIsland | CardName::Bayou | CardName::Savannah
                | CardName::Taiga | CardName::UndergroundSea | CardName::VolcanicIsland | CardName::Tundra
                // Shock lands with Forest subtype
                | CardName::BreedingPool | CardName::OvergrownTomb | CardName::TempleGarden
                | CardName::StompingGround
                // Shock lands with Island subtype
                | CardName::HallowedFountain | CardName::WateryGrave | CardName::SteamVents
                // Survey lands with Forest subtype
                | CardName::HedgeMaze | CardName::ThunderingFalls
                // Survey lands with Island subtype
                | CardName::MeticulousArchive | CardName::UndercitySewers
            ),
            // ScaldingTarn fetches Island or Mountain lands
            CardName::ScaldingTarn => matches!(target,
                // Basic lands
                CardName::Island | CardName::Mountain
                // Dual lands
                | CardName::VolcanicIsland | CardName::UndergroundSea | CardName::TropicalIsland
                | CardName::Tundra | CardName::Badlands | CardName::Plateau | CardName::Taiga
                // Shock lands with Island subtype
                | CardName::SteamVents | CardName::HallowedFountain | CardName::WateryGrave | CardName::BreedingPool
                // Shock lands with Mountain subtype
                | CardName::StompingGround | CardName::SacredFoundry | CardName::BloodCrypt
                // Survey lands with Island subtype
                | CardName::MeticulousArchive | CardName::UndercitySewers
                // Survey lands with Mountain subtype
                | CardName::ThunderingFalls
            ),
            // VerdantCatacombs fetches Swamp or Forest lands
            CardName::VerdantCatacombs => matches!(target,
                // Basic lands
                CardName::Swamp | CardName::Forest
                // Dual lands
                | CardName::Bayou | CardName::UndergroundSea | CardName::Badlands
                | CardName::TropicalIsland | CardName::Savannah | CardName::Taiga
                // Shock lands with Swamp subtype
                | CardName::OvergrownTomb | CardName::BloodCrypt | CardName::WateryGrave | CardName::GodlessShrine
                // Shock lands with Forest subtype
                | CardName::TempleGarden | CardName::BreedingPool | CardName::StompingGround
                // Survey lands with Swamp subtype (UndercitySewers = Island+Swamp)
                | CardName::UndercitySewers
                // Survey lands with Forest subtype
                | CardName::HedgeMaze | CardName::ThunderingFalls
            ),
            // AridMesa fetches Mountain or Plains lands
            CardName::AridMesa => matches!(target,
                // Basic lands
                CardName::Mountain | CardName::Plains
                // Dual lands
                | CardName::Plateau | CardName::VolcanicIsland | CardName::Badlands
                | CardName::Taiga | CardName::Tundra | CardName::Savannah | CardName::Scrubland
                // Shock lands with Mountain subtype
                | CardName::SacredFoundry | CardName::StompingGround | CardName::SteamVents | CardName::BloodCrypt
                // Shock lands with Plains subtype
                | CardName::HallowedFountain | CardName::GodlessShrine | CardName::TempleGarden
                // Survey lands with Mountain subtype
                | CardName::ThunderingFalls
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive | CardName::HedgeMaze
            ),
            // MarshFlats fetches Plains or Swamp lands
            CardName::MarshFlats => matches!(target,
                // Basic lands
                CardName::Plains | CardName::Swamp
                // Dual lands
                | CardName::Scrubland | CardName::Tundra | CardName::Savannah
                | CardName::Plateau | CardName::UndergroundSea | CardName::Badlands | CardName::Bayou
                // Shock lands with Plains subtype
                | CardName::GodlessShrine | CardName::HallowedFountain | CardName::TempleGarden | CardName::SacredFoundry
                // Shock lands with Swamp subtype
                | CardName::BloodCrypt | CardName::OvergrownTomb | CardName::WateryGrave
                // Survey lands with Plains subtype
                | CardName::MeticulousArchive | CardName::HedgeMaze
                // Survey lands with Swamp subtype (UndercitySewers = Island+Swamp)
                | CardName::UndercitySewers
            ),
            _ => false,
        }
    }

    /// Pay an alternate cost for a spell being cast.
    /// Returns true if the cost was paid successfully (all required resources were present and consumed).
    /// On success, the exiled card(s) and any life payment are consumed from the player's resources.
    /// On failure, no resources are consumed.
    pub(crate) fn pay_alt_cost(&mut self, player_id: PlayerId, _spell_id: ObjectId, alt: &AltCost) -> bool {
        match alt {
            AltCost::ForceOfWill { exile_id } => {
                // Cost: pay 1 life and exile a blue card from hand.
                let player = &self.players[player_id as usize];
                // Verify the exile card is in hand and is blue.
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand || player.life <= 1 {
                    return false;
                }
                // Pay life and exile the card.
                self.players[player_id as usize].life -= 1;
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::ForceOfNegation { exile_id } => {
                // Cost: exile a blue card from hand (no life payment).
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::Misdirection { exile_id } => {
                // Cost: exile a blue card from hand.
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::Commandeer { exile_id1, exile_id2 } => {
                // Cost: exile two blue cards from hand.
                let player = &self.players[player_id as usize];
                let has_both = player.hand.contains(exile_id1) && player.hand.contains(exile_id2)
                    && exile_id1 != exile_id2;
                if !has_both {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id1);
                self.players[player_id as usize].remove_from_hand(*exile_id2);
                let name1 = self.card_name_for_id(*exile_id1).unwrap_or(CardName::Plains);
                let name2 = self.card_name_for_id(*exile_id2).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id1, name1, player_id));
                self.exile.push((*exile_id2, name2, player_id));
                true
            }
            AltCost::Evoke { exile_id } => {
                // Cost: exile a card of the matching color from hand.
                let player = &self.players[player_id as usize];
                let exile_in_hand = player.hand.contains(exile_id);
                if !exile_in_hand {
                    return false;
                }
                self.players[player_id as usize].remove_from_hand(*exile_id);
                let exile_name = self.card_name_for_id(*exile_id).unwrap_or(CardName::Plains);
                self.exile.push((*exile_id, exile_name, player_id));
                true
            }
            AltCost::PhyrexianMana { life_paid, normal_cost } => {
                // Pay `life_paid` life and `normal_cost` mana from the pool.
                let player = &self.players[player_id as usize];
                // Ensure player has enough life (must survive paying it: life > life_paid)
                if player.life <= *life_paid as i32 {
                    return false;
                }
                // Ensure player can pay the remaining mana cost
                if !player.mana_pool.can_pay(normal_cost) {
                    return false;
                }
                self.players[player_id as usize].life -= *life_paid as i32;
                self.players[player_id as usize].mana_pool.pay(normal_cost);
                true
            }
        }
    }

}
