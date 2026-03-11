/// Choice resolution: handling pending choices (card selection, number, color).

use crate::action::*;
use crate::card::*;
use crate::game::*;
use crate::permanent::Permanent;
use crate::types::*;

impl GameState {
    pub(crate) fn resolve_choice(&mut self, choice: PendingChoice, card_id: ObjectId, db: &[CardDef]) {
        match choice.kind {
            ChoiceKind::ChooseFromList { reason, .. } => {
                match reason {
                    ChoiceReason::DemonicTutorSearch => {
                        // Move chosen card from library to hand
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            self.players[pid].hand.push(card_id);
                        }
                    }
                    ChoiceReason::VampiricTutorSearch | ChoiceReason::MysticalTutorSearch => {
                        // Move chosen card to top of library
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            self.players[pid].library.push(card_id); // push = top of library
                        }
                    }
                    ChoiceReason::EntombSearch => {
                        // Move chosen card from library to graveyard
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            self.players[pid].graveyard.push(card_id);
                        }
                    }
                    ChoiceReason::ThoughtseizeDiscard => {
                        // Discard chosen card from opponent's hand
                        let target_player = self.opponent(choice.player) as usize;
                        if let Some(pos) = self.players[target_player].hand.iter().position(|&id| id == card_id) {
                            self.players[target_player].hand.remove(pos);
                            self.players[target_player].graveyard.push(card_id);
                        }
                    }
                    ChoiceReason::GenericSearch => {
                        // Fetch land: put onto battlefield, or apply replacement effects.
                        let pid = choice.player as usize;
                        // Check if the card is coming from library (Natural Order puts creature onto battlefield).
                        let from_library = self.players[pid].library.iter().any(|&id| id == card_id);
                        let cage_active = self.grafdiggers_cage_active();
                        let priest_active = self.containment_priest_active();
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            let card_name = self.card_name_for_id(card_id);
                            if let Some(cn) = card_name {
                                if let Some(def) = find_card(db, cn) {
                                    let is_creature = def.card_types.contains(&CardType::Creature);
                                    // Grafdigger's Cage: creature cards from libraries can't enter the battlefield.
                                    // Containment Priest: nontoken creatures that weren't cast are exiled instead.
                                    if from_library && is_creature && (cage_active || priest_active) {
                                        // Card is exiled instead of entering
                                        self.exile.push((card_id, cn, choice.player));
                                    } else {
                                        let mut perm = Permanent::new(
                                            card_id, cn, choice.player, choice.player,
                                            def.power, def.toughness, def.loyalty, def.keywords, def.card_types,
                                        );
                                        if def.is_changeling {
                                            perm.creature_types = crate::types::CreatureType::ALL.to_vec();
                                        } else {
                                            perm.creature_types = def.creature_types.to_vec();
                                        }
                                        perm.colors = def.color_identity.to_vec();
                                        self.battlefield.push(perm);
                                        self.handle_etb(cn, card_id, choice.player);
                                    }
                                }
                            }
                        }
                    }
                    ChoiceReason::MyrRetrieverReturn => {
                        // Return chosen artifact from graveyard to hand
                        let pid = choice.player as usize;
                        if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == card_id) {
                            self.players[pid].graveyard.remove(pos);
                            self.players[pid].hand.push(card_id);
                        }
                    }
                    ChoiceReason::EdictSacrifice => {
                        // The chosen player sacrifices the chosen creature
                        self.destroy_permanent(card_id);
                    }
                    ChoiceReason::CloneTarget { clone_id, is_metamorph } => {
                        // Copy the chosen permanent's characteristics onto the clone.
                        // Collect the data we need from the target before mutating.
                        let copy_data = self.find_permanent(card_id).map(|target| {
                            (
                                target.card_name,
                                target.base_power,
                                target.base_toughness,
                                target.keywords,
                                target.card_types.clone(),
                                target.creature_types.clone(),
                                target.colors.clone(),
                            )
                        });
                        if let Some((
                            copied_name,
                            copied_power,
                            copied_toughness,
                            copied_keywords,
                            mut copied_types,
                            copied_creature_types,
                            copied_colors,
                        )) = copy_data
                        {
                            // Phyrexian Metamorph is always an artifact in addition to other types.
                            if is_metamorph && !copied_types.contains(&CardType::Artifact) {
                                copied_types.push(CardType::Artifact);
                            }
                            if let Some(clone_perm) = self.find_permanent_mut(clone_id) {
                                clone_perm.card_name = copied_name;
                                clone_perm.base_power = copied_power;
                                clone_perm.base_toughness = copied_toughness;
                                clone_perm.keywords = copied_keywords;
                                clone_perm.card_types = copied_types;
                                clone_perm.creature_types = copied_creature_types;
                                clone_perm.colors = copied_colors;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub(crate) fn resolve_number_choice(&mut self, choice: PendingChoice, n: u32) {
        match choice.kind {
            ChoiceKind::ChooseNumber { reason, .. } => {
                match reason {
                    ChoiceReason::ShockLandETB { card_id } => {
                        if n == 0 {
                            // Enter tapped
                            if let Some(perm) = self.find_permanent_mut(card_id) {
                                perm.tapped = true;
                            }
                        } else {
                            // Pay 2 life, enter untapped
                            self.players[choice.player as usize].life -= 2;
                        }
                    }
                    ChoiceReason::CavernOfSoulsETB { cavern_id } => {
                        // n is an index into CreatureType::ALL
                        let all_types = crate::types::CreatureType::ALL;
                        if (n as usize) < all_types.len() {
                            let chosen_type = all_types[n as usize];
                            if let Some(perm) = self.find_permanent_mut(cavern_id) {
                                perm.cavern_creature_type = Some(chosen_type);
                            }
                        }
                    }
                    ChoiceReason::TrueNameNemesisETB { permanent_id } => {
                        // n is the chosen player id; grant protection from that player.
                        let chosen_player = n as PlayerId;
                        if let Some(perm) = self.find_permanent_mut(permanent_id) {
                            perm.protections.push(Protection::FromPlayer(chosen_player));
                        }
                    }
                    ChoiceReason::SurveilLandShock { card_id } => {
                        if n == 0 {
                            // Enter tapped
                            if let Some(perm) = self.find_permanent_mut(card_id) {
                                perm.tapped = true;
                            }
                        } else {
                            // Pay 2 life, enter untapped
                            self.players[choice.player as usize].life -= 2;
                        }
                        // After the tapped/life choice, surveil 1 (no draw after).
                        self.surveil(choice.player, 1, false);
                    }
                    ChoiceReason::SurveilCard { draw_after } => {
                        // n == 0: keep the top card on top (do nothing)
                        // n == 1: put the top card into the graveyard
                        let pid = choice.player as usize;
                        if let Some(card_id) = self.players[pid].library.pop() {
                            if n == 1 {
                                // Send to graveyard (respecting Rest in Peace etc.)
                                let card_name = self.card_name_for_id(card_id)
                                    .unwrap_or(crate::card::CardName::Plains);
                                self.send_to_graveyard(card_id, card_name, choice.player);
                            } else {
                                // Put back on top
                                self.players[pid].library.push(card_id);
                            }
                        }
                        if draw_after {
                            self.draw_cards(choice.player, 1);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub(crate) fn resolve_color_choice(&mut self, choice: PendingChoice, color: Color) {
        match choice.kind {
            ChoiceKind::ChooseColor { reason } => {
                match reason {
                    ChoiceReason::BlackLotusColor => {
                        self.players[choice.player as usize]
                            .mana_pool
                            .add(Some(color), 3);
                    }
                    ChoiceReason::LotusPetalColor => {
                        self.players[choice.player as usize]
                            .mana_pool
                            .add(Some(color), 1);
                    }
                    ChoiceReason::TreasureSacrificeColor => {
                        // Add 1 mana of the chosen color
                        self.players[choice.player as usize]
                            .mana_pool
                            .add(Some(color), 1);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
