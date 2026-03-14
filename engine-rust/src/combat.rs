/// Combat system: damage assignment and resolution.

use crate::card::{CardDef, CardName};
use crate::game::GameState;
use crate::stack::{StackItemKind, TriggeredEffect};
use crate::types::*;

impl GameState {
    /// Resolve combat damage for the current combat step.
    pub fn resolve_combat_damage(&mut self, db: &[CardDef], first_strike_only: bool) {
        if self.combat_damage_dealt && !first_strike_only {
            return;
        }

        let mut damage_to_players: Vec<(PlayerId, i32)> = Vec::new();
        let mut damage_to_creatures: Vec<(ObjectId, i16)> = Vec::new();
        // Collect initiative-steal events to apply after the loop (avoids borrow conflict).
        let mut initiative_taken_by: Option<PlayerId> = None;

        for &(attacker_id, defending_player) in &self.attackers {
            let attacker = match self.find_permanent(attacker_id) {
                Some(p) => p,
                None => continue,
            };

            let has_first_strike = attacker.keywords.has(Keyword::FirstStrike)
                || attacker.keywords.has(Keyword::DoubleStrike);
            let has_regular_strike = !attacker.keywords.has(Keyword::FirstStrike)
                || attacker.keywords.has(Keyword::DoubleStrike);

            let should_deal_damage = if first_strike_only {
                has_first_strike
            } else {
                has_regular_strike
            };

            if !should_deal_damage {
                continue;
            }

            let attacker_power = self.effective_power(attacker_id, db);
            let attacker = match self.find_permanent(attacker_id) {
                Some(p) => p,
                None => continue,
            };
            let attacker_has_trample = attacker.keywords.has(Keyword::Trample);
            let attacker_has_deathtouch = attacker.keywords.has(Keyword::Deathtouch);
            let attacker_has_lifelink = attacker.keywords.has(Keyword::Lifelink);
            let attacker_controller = attacker.controller;
            let attacker_colors = attacker.colors.clone();

            if attacker_power <= 0 {
                continue;
            }

            // Find blockers for this attacker
            let blockers_for_attacker: Vec<ObjectId> = self
                .blockers
                .iter()
                .filter(|(_, aid)| *aid == attacker_id)
                .map(|(bid, _)| *bid)
                .collect();

            if blockers_for_attacker.is_empty() {
                // Unblocked - damage goes to defending player
                damage_to_players.push((defending_player, attacker_power as i32));

                // Fire combat-damage-to-player triggers for unblocked attackers
                let trigger_effect = match attacker.card_name {
                    CardName::RagavanNimblePilferer => Some(TriggeredEffect::RagavanCombatDamage),
                    CardName::ScrawlingCrawler => Some(TriggeredEffect::ScrawlingCrawlerCombatDamage),
                    CardName::PsychicFrog => Some(TriggeredEffect::PsychicFrogCombatDamage),
                    CardName::Barrowgoyf => Some(TriggeredEffect::BarrowgoyfCombatDamage { damage: attacker_power }),
                    CardName::VesselOfTheAllConsuming => Some(TriggeredEffect::VesselDealsDamage { vessel_id: attacker_id }),
                    _ => None,
                };
                if let Some(effect) = trigger_effect {
                    let attacker_name = attacker.card_name;
                    let attacker_ctrl = attacker_controller;
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: attacker_id,
                            source_name: attacker_name,
                            effect,
                        },
                        attacker_ctrl,
                        vec![],
                    );
                }

                // Monarch: if the defending player is the monarch, the attacker's
                // controller becomes the new monarch.
                if self.monarch == Some(defending_player) {
                    self.monarch = Some(attacker_controller);
                }
                // Initiative: if the defending player has the initiative, the attacker's
                // controller takes the initiative.
                if self.initiative == Some(defending_player) {
                    initiative_taken_by = Some(attacker_controller);
                }
            } else {
                // Blocked - assign damage to blockers
                let mut remaining_damage = attacker_power;

                for &blocker_id in &blockers_for_attacker {
                    if remaining_damage <= 0 {
                        break;
                    }
                    let blocker_toughness = self.effective_toughness(blocker_id, db);
                    let blocker_power = self.effective_power(blocker_id, db);
                    if let Some(blocker) = self.find_permanent(blocker_id) {
                        // Check if blocker has protection from the attacker
                        // (attacker's damage is prevented if blocker has protection from attacker's quality)
                        let blocker_protected = blocker.is_protected_from(&attacker_colors, attacker_controller)
                            || blocker.has_protection_from_creatures();

                        if !blocker_protected {
                            let lethal = if attacker_has_deathtouch {
                                1
                            } else {
                                (blocker_toughness - blocker.damage).max(0)
                            };
                            let assigned = remaining_damage.min(lethal);
                            damage_to_creatures.push((blocker_id, assigned));
                            remaining_damage -= assigned;
                        }

                        // Blocker deals damage back to attacker
                        // Check if attacker has protection from the blocker
                        let blocker_colors = blocker.colors.clone();
                        let blocker_controller = blocker.controller;
                        let attacker_protected = {
                            // We need attacker's protections; re-fetch attacker
                            self.find_permanent(attacker_id)
                                .map(|a| a.is_protected_from(&blocker_colors, blocker_controller)
                                    || a.has_protection_from_creatures())
                                .unwrap_or(false)
                        };
                        if blocker_power > 0 && !attacker_protected {
                            damage_to_creatures.push((attacker_id, blocker_power));
                            if blocker.keywords.has(Keyword::Lifelink) {
                                damage_to_players.push((blocker.controller, blocker_power as i32));
                            }
                        }
                    }
                }

                // Trample: excess damage goes to defending player
                if attacker_has_trample && remaining_damage > 0 {
                    damage_to_players.push((defending_player, remaining_damage as i32));
                    // Monarch: trample damage reaching the defending player steals the monarchy
                    if self.monarch == Some(defending_player) {
                        self.monarch = Some(attacker_controller);
                    }
                    // Initiative: trample damage reaching the initiative holder steals it
                    if self.initiative == Some(defending_player) {
                        initiative_taken_by = Some(attacker_controller);
                    }
                }
            }

            // Lifelink
            if attacker_has_lifelink {
                damage_to_players.push((attacker_controller, attacker_power as i32));
            }
        }

        // Apply damage
        for (creature_id, damage) in damage_to_creatures {
            if let Some(perm) = self.find_permanent_mut(creature_id) {
                perm.damage += damage;
            }
        }

        for (player_id, damage) in damage_to_players {
            // Prevent all damage to a player with protection from everything
            // (e.g. The One Ring ETB effect until their next turn).
            // Note: lifelink entries for the lifelink-granting player are also stored here;
            // we only prevent damage entries (positive damage targeting defending players).
            // To avoid breaking lifelink, we only skip entries where the player has protection
            // AND the damage is a positive damage-to-player scenario.
            // Lifelink entries use the attacker's controller as the target — they gain life.
            // Check if this entry corresponds to an attack against the protected player.
            let player_has_protection = self.players[player_id as usize].protection_from_everything;
            if player_has_protection {
                // Skip damage to this player entirely; lifelink will still be handled
                // since the lifelink player is the attacker_controller, not the defender.
                continue;
            }
            self.players[player_id as usize].life -= damage;
        }

        // Apply deferred initiative steal (from combat damage to the initiative holder).
        if let Some(new_holder) = initiative_taken_by {
            self.take_initiative(new_holder);
        }

        if !first_strike_only {
            self.combat_damage_dealt = true;
        }

        self.check_state_based_actions(db);
    }
}
