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
                self.check_state_based_actions();
            }

            Action::PlayLand(card_id) => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if self.players[player_id as usize].remove_from_hand(*card_id) {
                    self.players[player_id as usize].land_plays_remaining -= 1;
                    if let Some(cn) = card_name {
                        if let Some(def) = find_card(db, cn) {
                            let perm = Permanent::new(
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
                            self.battlefield.push(perm);
                            self.handle_etb(cn, *card_id, player_id);
                        }
                    }
                }
            }

            Action::CastSpell { card_id, targets } => {
                let player_id = self.priority_player;
                let card_name = self.card_name_for_id(*card_id);
                if let Some(cn) = card_name {
                    if let Some(def) = find_card(db, cn) {
                        let cost = self.effective_cost(def, player_id);
                        if self.players[player_id as usize].mana_pool.pay(&cost) {
                            self.players[player_id as usize].remove_from_hand(*card_id);
                            let uncounterable = is_uncounterable(cn);
                            self.stack.push_with_flags(
                                StackItemKind::Spell {
                                    card_name: cn,
                                    card_id: *card_id,
                                },
                                player_id,
                                targets.clone(),
                                uncounterable,
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
                if let Some(perm) = self.find_permanent_mut(*creature_id) {
                    if !perm.keywords.has(Keyword::Vigilance) {
                        perm.tapped = true;
                    }
                    perm.attacked_this_turn = true;
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
                    self.resolve_number_choice(choice, *n);
                }
            }
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

}
