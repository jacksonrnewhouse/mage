/// Dies triggers, leaves-battlefield triggers, and permanent removal with trigger checks.

use crate::card::*;
use crate::game::GameState;
use crate::stack::*;
use crate::types::*;

impl GameState {
    /// Check for dies triggers on the permanent that just died and on other permanents
    /// that care about things dying.
    pub(crate) fn check_dies_triggers(
        &mut self,
        died_id: ObjectId,
        died_name: CardName,
        controller: PlayerId,
        is_artifact: bool,
        last_known_power: i16,
    ) {
        // --- Triggers on the dying permanent itself ---
        match died_name {
            CardName::WurmcoilEngine => {
                // Create two tokens: 3/3 lifelink and 3/3 deathtouch
                let trigger_id = self.new_object_id();
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: died_id,
                        source_name: died_name,
                        effect: TriggeredEffect::WurmcoilDeath,
                    },
                    controller,
                    vec![],
                );
                let _ = trigger_id;
            }
            CardName::MyrRetriever => {
                // Return another target artifact card from your graveyard to your hand.
                // Find artifacts in controller's graveyard (excluding Myr Retriever itself)
                let artifacts_in_gy: Vec<ObjectId> = self.players[controller as usize]
                    .graveyard
                    .iter()
                    .filter(|&&id| {
                        id != died_id
                            && self.card_name_for_id(id)
                                .and_then(|cn| {
                                    // Check if it's an artifact by looking at card registry + card db
                                    // For simplicity, use known artifact names or the card_types
                                    Some(cn)
                                })
                                .is_some()
                    })
                    .copied()
                    .collect();
                if !artifacts_in_gy.is_empty() {
                    // Put a triggered ability on the stack
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id: died_id,
                            source_name: died_name,
                            effect: TriggeredEffect::MyrRetrieverDeath,
                        },
                        controller,
                        vec![],
                    );
                }
            }
            CardName::ChromaticStar => {
                // When Chromatic Star is put into a graveyard from the battlefield, draw a card.
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: died_id,
                        source_name: died_name,
                        effect: TriggeredEffect::ChromaticStarDeath,
                    },
                    controller,
                    vec![],
                );
            }
            CardName::HaywireMite => {
                // When Haywire Mite dies, you gain 2 life.
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: died_id,
                        source_name: died_name,
                        effect: TriggeredEffect::HaywireMiteDeath,
                    },
                    controller,
                    vec![],
                );
            }
            // Broadside Bombardiers: boast ability (not yet implemented), no dies trigger
            CardName::BroadsideBombardiers => {}
            CardName::Pyrogoyf => {
                // When Pyrogoyf dies, it deals damage equal to its power to any target.
                // Use last-known power; Pyrogoyf is now in the graveyard so graveyard type
                // count has changed, but we use the power it had when it was last on the battlefield.
                let opp = self.opponent(controller);
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: died_id,
                        source_name: died_name,
                        effect: TriggeredEffect::PyrogoyfDeath { power: last_known_power },
                    },
                    controller,
                    vec![Target::Player(opp)],
                );
            }
            _ => {}
        }

        // --- Triggers on other permanents that care about things dying ---

        // Scrap Trawler: when this or another artifact dies, return a lesser-MV artifact from graveyard.
        if is_artifact {
            let scrap_trawlers: Vec<(ObjectId, PlayerId)> = self.battlefield.iter()
                .filter(|p| p.card_name == CardName::ScrapTrawler && p.controller == controller)
                .map(|p| (p.id, p.controller))
                .collect();

            // Also check if the dying permanent itself is Scrap Trawler (it triggers on its own death too)
            let died_is_scrap_trawler = died_name == CardName::ScrapTrawler;

            let trawler_sources: Vec<(ObjectId, PlayerId)> = if died_is_scrap_trawler {
                // Scrap Trawler itself dying — it triggers from the graveyard
                vec![(died_id, controller)]
            } else {
                scrap_trawlers
            };

            for (source_id, trawler_controller) in trawler_sources {
                // Find the MV of the dying artifact
                let db = crate::card::build_card_db();
                let dying_mv = self.card_name_for_id(died_id)
                    .and_then(|cn| find_card(&db, cn))
                    .map(|def| def.mana_cost.cmc())
                    .unwrap_or(0);

                // Find artifacts in graveyard with lesser MV
                let pid = trawler_controller as usize;
                let valid_targets: Vec<ObjectId> = self.players[pid].graveyard.iter()
                    .filter(|&&id| {
                        if id == died_id { return false; }
                        let is_valid = self.card_name_for_id(id)
                            .and_then(|cn| find_card(&db, cn))
                            .map(|def| {
                                def.card_types.contains(&CardType::Artifact) && def.mana_cost.cmc() < dying_mv
                            })
                            .unwrap_or(false);
                        is_valid
                    })
                    .copied()
                    .collect();

                if let Some(&target_id) = valid_targets.first() {
                    self.stack.push(
                        StackItemKind::TriggeredAbility {
                            source_id,
                            source_name: CardName::ScrapTrawler,
                            effect: TriggeredEffect::ScrapTrawlerDeath,
                        },
                        trawler_controller,
                        vec![Target::Object(target_id)],
                    );
                }
            }
        }

        // Skullclamp: when equipped creature dies, draw 2
        let skullclamp_controllers: Vec<PlayerId> = self.battlefield.iter()
            .filter(|p| p.card_name == CardName::SkullClamp)
            .map(|p| p.controller)
            .collect();
        // Note: Skullclamp triggers when the equipped creature dies.
        // For now, we skip equipment tracking - Skullclamp trigger would need
        // the dying creature to have been equipped. This is a placeholder for future work.
        let _ = skullclamp_controllers;
        let _ = is_artifact;
    }

    /// Check for leaves-battlefield triggers (bounce, exile, etc.).
    /// Handles exile-until-leaves effects: when the exiling permanent leaves,
    /// return the exiled card to the battlefield.
    pub(crate) fn check_leaves_triggers(
        &mut self,
        left_id: ObjectId,
        _left_name: CardName,
        controller: PlayerId,
    ) {
        // --- Exile-linked return triggers ---
        // Collect all cards exiled by this permanent
        let linked_cards: Vec<ObjectId> = self
            .exile_linked
            .iter()
            .filter(|(exiler_id, _)| *exiler_id == left_id)
            .map(|(_, exiled_id)| *exiled_id)
            .collect();

        if !linked_cards.is_empty() {
            // Remove the links for this exiler
            self.exile_linked.retain(|(exiler_id, _)| *exiler_id != left_id);

            for card_id in linked_cards {
                // Find the owner of the exiled card
                let card_owner = self.exile
                    .iter()
                    .find(|(id, _, _)| *id == card_id)
                    .map(|(_, _, owner)| *owner)
                    .unwrap_or(controller);

                // Push return trigger onto the stack
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: left_id,
                        source_name: _left_name,
                        effect: TriggeredEffect::ExileLinkedReturn {
                            card_id,
                            card_owner,
                        },
                    },
                    card_owner,
                    vec![],
                );
            }
        }

        // --- Skyclave Apparition leaves: create token for opponent ---
        let skyclave_mv: Option<u32> = self
            .skyclave_token_mv
            .iter()
            .find(|(app_id, _)| *app_id == left_id)
            .map(|(_, mv)| *mv);

        // --- Cryogen Relic leaves: draw a card ---
        if _left_name == CardName::CryogenRelic {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id: left_id,
                    source_name: _left_name,
                    effect: TriggeredEffect::DrawCards(1),
                },
                controller,
                vec![],
            );
        }

        if let Some(token_mv) = skyclave_mv {
            self.skyclave_token_mv.retain(|(app_id, _)| *app_id != left_id);
            let opponent = self.opponent(controller);
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id: left_id,
                    source_name: _left_name,
                    effect: TriggeredEffect::SkyclaveApparitionLeaves {
                        apparition_id: left_id,
                        token_mv,
                        opponent,
                    },
                },
                opponent,
                vec![],
            );
        }
    }

    /// Check for draw triggers (Sheoldred, Orcish Bowmasters).
    /// Called after each individual card draw.
    pub(crate) fn check_draw_triggers(&mut self, drawing_player: PlayerId) {
        // Sheoldred, the Apocalypse: controller gains 2 life on own draw,
        // opponent loses 2 life on opponent draw.
        let sheoldred_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::SheoldredTheApocalypse)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in sheoldred_triggers {
            if drawing_player == controller {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id,
                        source_name: CardName::SheoldredTheApocalypse,
                        effect: TriggeredEffect::SheoldredDraw,
                    },
                    controller,
                    vec![],
                );
            } else {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id,
                        source_name: CardName::SheoldredTheApocalypse,
                        effect: TriggeredEffect::SheoldredOpponentDraw,
                    },
                    controller,
                    vec![],
                );
            }
        }

        // Orcish Bowmasters: whenever an opponent draws a card, trigger
        let bowmasters_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::OrcishBowmasters)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in bowmasters_triggers {
            if drawing_player != controller {
                let opp = drawing_player;
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id,
                        source_name: CardName::OrcishBowmasters,
                        effect: TriggeredEffect::OrcishBowmastersOpponentDraw,
                    },
                    controller,
                    vec![Target::Player(opp)],
                );
            }
        }

        // Razorkin Needlehead: whenever an opponent draws a card, deal 1 damage to them
        let needlehead_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::RazorkinNeedlehead)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in needlehead_triggers {
            if drawing_player != controller {
                let opp = drawing_player;
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id,
                        source_name: CardName::RazorkinNeedlehead,
                        effect: TriggeredEffect::RazorkinNeedleheadOpponentDraw,
                    },
                    controller,
                    vec![Target::Player(opp)],
                );
            }
        }
    }

    /// Check for noncreature spell cast triggers.
    /// Called after a noncreature spell is pushed to the stack.
    /// Handles Young Pyromancer (1/1 red Elemental), Monastery Mentor (1/1 white Monk with prowess),
    /// and Cindervines (deal 1 damage to opponent when they cast noncreature spell).
    pub(crate) fn check_noncreature_cast_triggers(&mut self, caster: PlayerId) {
        // Collect triggers to fire: (source_id, source_name, effect, controller)
        let mut triggers: Vec<(ObjectId, CardName, TriggeredEffect, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.controller == caster)
            .filter_map(|p| match p.card_name {
                CardName::YoungPyromancer => Some((
                    p.id,
                    p.card_name,
                    TriggeredEffect::YoungPyromancerCast,
                    p.controller,
                )),
                CardName::MonasteryMentor => Some((
                    p.id,
                    p.card_name,
                    TriggeredEffect::MonasteryMentorCast,
                    p.controller,
                )),
                _ => None,
            })
            .collect();

        // Cindervines: whenever an opponent casts a noncreature spell, deal 1 damage to them
        let cindervines_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::Cindervines && p.controller != caster)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in cindervines_triggers {
            triggers.push((
                source_id,
                CardName::Cindervines,
                TriggeredEffect::CindervinesDamage { target_player: caster },
                controller,
            ));
        }

        // Mystic Remora: whenever an opponent casts a noncreature spell, draw a card
        // (simplified — opponents rarely pay the {4} cumulative upkeep tax)
        let remora_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::MysticRemora && p.controller != caster)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in remora_triggers {
            triggers.push((
                source_id,
                CardName::MysticRemora,
                TriggeredEffect::MysticRemoraOpponentCast,
                controller,
            ));
        }

        // Magebane Lizard: whenever a player casts a noncreature spell, deal damage to that
        // player equal to the number of noncreature spells they've cast this turn.
        let noncreature_count = self.players[caster as usize].noncreature_spells_cast_this_turn;
        let lizard_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::MagebaneLizard)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in lizard_triggers {
            triggers.push((
                source_id,
                CardName::MagebaneLizard,
                TriggeredEffect::MagebaneLizardDamage {
                    target_player: caster,
                    damage: noncreature_count,
                },
                controller,
            ));
        }

        // Displacer Kitten: whenever you cast a noncreature spell, exile up to one target
        // nonland permanent you control, then return it to the battlefield under its owner's control.
        let kitten_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::DisplacerKitten && p.controller == caster)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in kitten_triggers {
            // Find a nonland permanent the controller controls to blink (not the Kitten itself)
            let blink_targets: Vec<ObjectId> = self
                .battlefield
                .iter()
                .filter(|p| p.controller == controller && !p.is_land() && p.id != source_id)
                .map(|p| p.id)
                .collect();
            if let Some(&_target_id) = blink_targets.first() {
                triggers.push((
                    source_id,
                    CardName::DisplacerKitten,
                    TriggeredEffect::DisplacerKittenBlink,
                    controller,
                ));
                // We'll add the target below in the loop
                // For now, store the target and handle it in the push loop
            }
            // If no valid target, the "up to one" means we can still fire with no target (do nothing).
        }

        for (source_id, source_name, effect, controller) in &triggers {
            let target_vec = match effect {
                TriggeredEffect::DisplacerKittenBlink => {
                    // Find best nonland permanent to blink (first one found)
                    let blink_target: Option<ObjectId> = self
                        .battlefield
                        .iter()
                        .find(|p| p.controller == *controller && !p.is_land() && p.id != *source_id)
                        .map(|p| p.id);
                    match blink_target {
                        Some(tid) => vec![Target::Object(tid)],
                        None => vec![],
                    }
                }
                _ => vec![],
            };
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id: *source_id,
                    source_name: *source_name,
                    effect: effect.clone(),
                },
                *controller,
                target_vec,
            );
        }
    }

    /// Check Chalice of the Void triggered ability: whenever a player casts a spell,
    /// if its mana value equals the number of charge counters on Chalice, counter that spell.
    /// Called after any spell is pushed to the stack.
    pub(crate) fn check_chalice_trigger(&mut self, spell_id: ObjectId, spell_cmc: u8) {
        let chalice_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| {
                p.card_name == CardName::ChaliceOfTheVoid
                    && p.counters.get(crate::types::CounterType::Charge) == spell_cmc as i16
            })
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in chalice_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::ChaliceOfTheVoid,
                    effect: TriggeredEffect::ChaliceCounter { spell_id },
                },
                controller,
                vec![Target::Object(spell_id)],
            );
        }
    }

    /// Check Lavinia's and Boromir's triggered abilities: whenever an opponent casts a spell,
    /// if no mana was spent to cast it, counter that spell.
    /// Called after any spell is pushed to the stack.
    pub(crate) fn check_lavinia_trigger(&mut self, caster: PlayerId, spell_id: ObjectId, mana_spent: bool) {
        if mana_spent {
            return; // Lavinia, Boromir, and Roiling Vortex only trigger on free spells
        }
        // Lavinia, Azorius Renegade
        let lavinia_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::LaviniaAzoriusRenegade && p.controller != caster)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in lavinia_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::LaviniaAzoriusRenegade,
                    effect: TriggeredEffect::LaviniaCounter { spell_id },
                },
                controller,
                vec![Target::Object(spell_id)],
            );
        }
        // Boromir, Warden of the Tower
        let boromir_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::BoromirWardenOfTheTower && p.controller != caster)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in boromir_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::BoromirWardenOfTheTower,
                    effect: TriggeredEffect::LaviniaCounter { spell_id },
                },
                controller,
                vec![Target::Object(spell_id)],
            );
        }
        // Roiling Vortex: whenever a player casts a spell without paying its mana cost,
        // deal 5 damage to that player.
        let vortex_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::RoilingVortex)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in vortex_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::RoilingVortex,
                    effect: TriggeredEffect::RoilingVortexFreeCast { target_player: caster },
                },
                controller,
                vec![Target::Player(caster)],
            );
        }
    }

    /// Check Nadu, Winged Wisdom targeting triggers.
    /// Nadu grants all creatures you control: "Whenever this creature becomes the target
    /// of a spell or ability, reveal the top card of your library. If it's a land card,
    /// put it onto the battlefield. Otherwise, put it into your hand."
    /// This ability triggers only twice per creature per turn.
    ///
    /// `targeted_creature_ids` is the set of creature ObjectIds being targeted.
    pub(crate) fn check_nadu_targeting_triggers(&mut self, targeted_creature_ids: &[ObjectId]) {
        // Find all Nadu permanents on the battlefield
        let nadus: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::NaduWingedWisdom)
            .map(|p| (p.id, p.controller))
            .collect();

        if nadus.is_empty() {
            return;
        }

        for &(nadu_id, nadu_controller) in &nadus {
            for &creature_id in targeted_creature_ids {
                // The creature must be controlled by Nadu's controller
                let is_own_creature = self
                    .battlefield
                    .iter()
                    .any(|p| p.id == creature_id && p.controller == nadu_controller && p.is_creature());
                if !is_own_creature {
                    continue;
                }

                // Check the twice-per-turn limit for this creature
                let trigger_count = self
                    .nadu_triggers_this_turn
                    .iter()
                    .find(|(id, _)| *id == creature_id)
                    .map(|(_, count)| *count)
                    .unwrap_or(0);

                if trigger_count >= 2 {
                    continue;
                }

                // Increment the trigger count
                if let Some(entry) = self.nadu_triggers_this_turn.iter_mut().find(|(id, _)| *id == creature_id) {
                    entry.1 += 1;
                } else {
                    self.nadu_triggers_this_turn.push((creature_id, 1));
                }

                // Push the trigger onto the stack
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: nadu_id,
                        source_name: CardName::NaduWingedWisdom,
                        effect: TriggeredEffect::NaduTrigger,
                    },
                    nadu_controller,
                    vec![],
                );
            }
        }
    }

    /// Check Bonecrusher Giant targeting triggers.
    /// "Whenever this creature becomes the target of a spell, Bonecrusher Giant deals
    /// 2 damage to that spell's controller."
    ///
    /// `targeted_permanent_ids` is the set of permanent ObjectIds being targeted.
    /// `spell_controller` is the player who controls the spell targeting the creature.
    pub(crate) fn check_bonecrusher_targeting_triggers(
        &mut self,
        targeted_permanent_ids: &[ObjectId],
        spell_controller: PlayerId,
    ) {
        for &target_id in targeted_permanent_ids {
            // Check if the targeted permanent is a Bonecrusher Giant on the battlefield
            let bonecrusher = self
                .battlefield
                .iter()
                .find(|p| p.id == target_id && p.card_name == CardName::BonecrusherGiant);
            if let Some(perm) = bonecrusher {
                let bc_controller = perm.controller;
                let bc_id = perm.id;
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: bc_id,
                        source_name: CardName::BonecrusherGiant,
                        effect: TriggeredEffect::BonecrusherGiantTargeted {
                            target_player: spell_controller,
                        },
                    },
                    bc_controller,
                    vec![Target::Player(spell_controller)],
                );
            }
        }
    }

    /// Check Leovold, Emissary of Trest targeting triggers.
    /// "Whenever you or a permanent you control becomes the target of a spell or ability
    /// an opponent controls, you may draw a card."
    ///
    /// `targets` is the list of targets for the spell/ability.
    /// `source_controller` is the player who controls the spell/ability doing the targeting.
    pub(crate) fn check_leovold_targeting_triggers(
        &mut self,
        targets: &[Target],
        source_controller: PlayerId,
    ) {
        // Find all Leovold permanents on the battlefield
        let leovolds: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::LeovoldEmissaryOfTrest)
            .map(|p| (p.id, p.controller))
            .collect();

        if leovolds.is_empty() {
            return;
        }

        for &(leovold_id, leovold_controller) in &leovolds {
            // Only triggers when the source is controlled by an opponent
            if source_controller == leovold_controller {
                continue;
            }

            // Check if any target is Leovold's controller (player) or a permanent they control
            let mut triggered = false;
            for target in targets {
                match target {
                    Target::Player(pid) if *pid == leovold_controller => {
                        triggered = true;
                        break;
                    }
                    Target::Object(obj_id) => {
                        if self.find_permanent(*obj_id)
                            .map(|p| p.controller == leovold_controller)
                            .unwrap_or(false)
                        {
                            triggered = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if triggered {
                self.stack.push(
                    StackItemKind::TriggeredAbility {
                        source_id: leovold_id,
                        source_name: CardName::LeovoldEmissaryOfTrest,
                        effect: TriggeredEffect::LeovoldTargetDraw,
                    },
                    leovold_controller,
                    vec![],
                );
            }
        }
    }

    /// Check Clarion Conqueror triggered ability: whenever an opponent casts a spell
    /// during your turn, create a 1/1 white Soldier creature token.
    /// Called after any spell is pushed to the stack.
    pub(crate) fn check_clarion_conqueror_trigger(&mut self, caster: PlayerId) {
        // Only triggers when the caster is NOT the active player (i.e., opponent casting during your turn)
        if caster == self.active_player {
            return;
        }
        let conqueror_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| {
                p.card_name == CardName::ClarionConqueror
                    && p.controller == self.active_player
            })
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in conqueror_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::ClarionConqueror,
                    effect: TriggeredEffect::ClarionConquerorOpponentCast,
                },
                controller,
                vec![],
            );
        }
    }

    /// Check Eidolon of the Great Revel triggered ability: whenever a player casts a spell
    /// with mana value 3 or less, Eidolon deals 2 damage to that player.
    /// Called after any spell is pushed to the stack.
    pub(crate) fn check_eidolon_trigger(&mut self, caster: PlayerId, spell_cmc: u8) {
        if spell_cmc > 3 {
            return;
        }
        let eidolon_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::EidolonOfTheGreatRevel)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in eidolon_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::EidolonOfTheGreatRevel,
                    effect: TriggeredEffect::EidolonDamage { target_player: caster },
                },
                controller,
                vec![Target::Player(caster)],
            );
        }
    }

    /// Check Harsh Mentor triggered ability: whenever an opponent activates an ability of an
    /// artifact, creature, or land that isn't a mana ability, Harsh Mentor deals 2 damage to
    /// that player. Called after a non-mana activated ability is activated.
    pub(crate) fn check_harsh_mentor_trigger(&mut self, activator: PlayerId) {
        let mentor_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::HarshMentor)
            .filter(|p| p.controller != activator)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in mentor_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::HarshMentor,
                    effect: TriggeredEffect::HarshMentorDamage { target_player: activator },
                },
                controller,
                vec![Target::Player(activator)],
            );
        }
    }

    /// Check Avalanche of Sector 7 triggered ability: whenever an opponent activates an ability
    /// of an artifact they control, Avalanche of Sector 7 deals 1 damage to that player.
    /// Called after any artifact ability is activated (including mana abilities).
    pub(crate) fn check_avalanche_trigger(&mut self, activator: PlayerId) {
        let avalanche_triggers: Vec<(ObjectId, PlayerId)> = self
            .battlefield
            .iter()
            .filter(|p| p.card_name == CardName::AvalancheOfSector7)
            .filter(|p| p.controller != activator)
            .map(|p| (p.id, p.controller))
            .collect();
        for (source_id, controller) in avalanche_triggers {
            self.stack.push(
                StackItemKind::TriggeredAbility {
                    source_id,
                    source_name: CardName::AvalancheOfSector7,
                    effect: TriggeredEffect::AvalancheDamage { target_player: activator },
                },
                controller,
                vec![Target::Player(activator)],
            );
        }
    }
}
