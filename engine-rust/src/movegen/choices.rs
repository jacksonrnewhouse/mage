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
                            self.check_emrakul_graveyard_shuffle(choice.player);
                        }
                    }
                    ChoiceReason::ThoughtseizeDiscard => {
                        // Discard chosen card from opponent's hand (triggers madness)
                        let target_player = self.opponent(choice.player);
                        if let Some(pos) = self.players[target_player as usize].hand.iter().position(|&id| id == card_id) {
                            self.players[target_player as usize].hand.remove(pos);
                            self.discard_card(card_id, target_player, db);
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
                            // Note: In a real game, library would be shuffled after searching.
                            // For game tree search, the search algorithm handles randomization.
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
                    ChoiceReason::AnnihilatorSacrifice { remaining } => {
                        // The chosen player sacrifices the chosen permanent
                        self.destroy_permanent(card_id);
                        // If more sacrifices are required, queue the next one
                        if remaining > 0 {
                            self.trigger_annihilator(choice.player, remaining);
                        }
                    }
                    ChoiceReason::ShowAndTellChoose { next_player } => {
                        // card_id == 0 means the player passes (chooses not to put anything onto the battlefield).
                        if card_id != 0 {
                            let pid = choice.player as usize;
                            // Remove the chosen card from hand
                            if let Some(pos) = self.players[pid].hand.iter().position(|&id| id == card_id) {
                                self.players[pid].hand.remove(pos);
                                let card_name = self.card_name_for_id(card_id);
                                if let Some(cn) = card_name {
                                    if let Some(def) = find_card(db, cn) {
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
                        // Set up the next player's choice if there is one
                        if let Some(next) = next_player {
                            let valid_options: Vec<ObjectId> = self.players[next as usize]
                                .hand
                                .iter()
                                .copied()
                                .filter(|&id| {
                                    if let Some(cn) = self.card_name_for_id(id) {
                                        if let Some(def) = find_card(db, cn) {
                                            return def.card_types.iter().any(|t| matches!(t,
                                                CardType::Artifact | CardType::Creature
                                                | CardType::Enchantment | CardType::Planeswalker
                                            ));
                                        }
                                    }
                                    false
                                })
                                .collect();
                            self.pending_choice = Some(PendingChoice {
                                player: next,
                                kind: ChoiceKind::ChooseFromList {
                                    options: valid_options,
                                    reason: ChoiceReason::ShowAndTellChoose { next_player: None },
                                },
                            });
                        }
                    }
                    ChoiceReason::FlashPutCreature => {
                        // Flash: put chosen creature from hand onto battlefield
                        if card_id != 0 {
                            let pid = choice.player as usize;
                            if let Some(pos) = self.players[pid].hand.iter().position(|&id| id == card_id) {
                                self.players[pid].hand.remove(pos);
                                let card_name = self.card_name_for_id(card_id);
                                if let Some(cn) = card_name {
                                    if let Some(def) = find_card(db, cn) {
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
                    ChoiceReason::ChromeMoxImprint { mox_id } => {
                        // card_id == 0 means the player declined to imprint anything.
                        if card_id != 0 {
                            let pid = choice.player as usize;
                            // Remove the chosen card from hand and exile it.
                            if let Some(pos) = self.players[pid].hand.iter().position(|&id| id == card_id) {
                                self.players[pid].hand.remove(pos);
                                let card_name = self.card_name_for_id(card_id).unwrap_or(crate::card::CardName::Plains);
                                self.exile.push((card_id, card_name, choice.player));
                                // Record the imprint link: (mox_id, card_id)
                                self.imprinted.push((mox_id, card_id));
                            }
                        }
                    }
                    ChoiceReason::IsochronScepterImprint { scepter_id } => {
                        // card_id == 0 means the player declined to imprint anything.
                        if card_id != 0 {
                            let pid = choice.player as usize;
                            // Remove the chosen card from hand and exile it.
                            if let Some(pos) = self.players[pid].hand.iter().position(|&id| id == card_id) {
                                self.players[pid].hand.remove(pos);
                                let card_name = self.card_name_for_id(card_id).unwrap_or(crate::card::CardName::Plains);
                                self.exile.push((card_id, card_name, choice.player));
                                // Record the imprint link: (scepter_id, card_id)
                                self.imprinted.push((scepter_id, card_id));
                            }
                        }
                    }
                    ChoiceReason::HideawayExile { land_id } => {
                        // The player chooses one card from the top N to exile face-down.
                        // The rest (already inserted at the bottom of library) remain there.
                        let pid = choice.player as usize;
                        // Remove the chosen card from the bottom of the library (it was inserted there).
                        if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                            self.players[pid].library.remove(pos);
                            let card_name = self.card_name_for_id(card_id).unwrap_or(crate::card::CardName::Plains);
                            // Exile it face-down (linked to the hideaway land)
                            self.exile.push((card_id, card_name, choice.player));
                            self.hideaway_exiled.push((land_id, card_id));
                        }
                    }
                    ChoiceReason::UrzasSagaChapterIII => {
                        // Search library for an artifact with MV 0 or 1, put it onto the battlefield.
                        // card_id == 0 means no valid target (shouldn't happen if options were non-empty).
                        if card_id != 0 {
                            let pid = choice.player as usize;
                            if let Some(pos) = self.players[pid].library.iter().position(|&id| id == card_id) {
                                self.players[pid].library.remove(pos);
                                let card_name = self.card_name_for_id(card_id);
                                if let Some(cn) = card_name {
                                    if let Some(def) = find_card(db, cn) {
                                        let mut perm = Permanent::new(
                                            card_id, cn, choice.player, choice.player,
                                            def.power, def.toughness, def.loyalty,
                                            def.keywords, def.card_types,
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
                        // Library is "shuffled" after search (no-op in this deterministic engine).
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

    pub(crate) fn resolve_number_choice(&mut self, choice: PendingChoice, n: u32, db: &[CardDef]) {
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
                    ChoiceReason::CoinFlip => {
                        // 0 = heads: win the flip — no consequence.
                        // 1 = tails: lose the flip — Mana Crypt deals 3 damage to the controller.
                        if n == 1 {
                            self.players[choice.player as usize].life -= 3;
                        }
                    }
                    ChoiceReason::MadnessCast { card_id, madness_cost } => {
                        // n == 0: cast the card for its madness cost.
                        // n == 1: decline — move the card from exile to graveyard.

                        // Remove from exile and madness_exiled tracking regardless.
                        let owner_opt = if let Some(pos) = self.exile.iter().position(|(id, _, _)| *id == card_id) {
                            let (_, _cn, owner) = self.exile.swap_remove(pos);
                            Some(owner)
                        } else {
                            None
                        };
                        if let Some(mp) = self.madness_exiled.iter().position(|(id, _)| *id == card_id) {
                            self.madness_exiled.swap_remove(mp);
                        }

                        let owner = owner_opt.unwrap_or(choice.player);

                        if n == 1 {
                            // Declined: put in graveyard.
                            self.players[owner as usize].graveyard.push(card_id);
                            self.check_emrakul_graveyard_shuffle(owner);
                        } else {
                            // n == 0: cast for the madness cost.
                            // Attempt to pay the madness cost.
                            let paid = self.players[owner as usize].mana_pool.pay(&madness_cost);
                            if paid {
                                // Get the card name and def for stack push.
                                let card_name = self.card_name_for_id(card_id)
                                    .unwrap_or(crate::card::CardName::Plains);
                                // Push spell onto the stack. Use cast_from_graveyard=true so that
                                // after resolution the card is exiled (as per madness rules).
                                use crate::stack::StackItemKind;
                                let uncounterable = crate::movegen::is_uncounterable(card_name);
                                self.stack.push_with_flags(
                                    StackItemKind::Spell {
                                        card_name,
                                        card_id,
                                        cast_via_evoke: false,
                                    },
                                    owner,
                                    vec![],
                                    uncounterable,
                                    0,
                                    true, // cast_from_graveyard: exile after resolution
                                    vec![],
                                );
                                self.players[owner as usize].spells_cast_this_turn += 1;
                                if let Some(def) = self.card_name_for_id(card_id)
                                    .and_then(|cn| crate::card::find_card(db, cn))
                                {
                                    if !def.card_types.contains(&crate::types::CardType::Artifact) {
                                        self.players[owner as usize].nonartifact_spells_cast_this_turn += 1;
                                    }
                                    if !def.card_types.contains(&crate::types::CardType::Creature) {
                                        self.players[owner as usize].noncreature_spells_cast_this_turn += 1;
                                    }
                                }
                                self.storm_count += 1;
                                self.reset_priority_passes();
                            } else {
                                // Can't afford madness cost — put in graveyard instead.
                                self.players[owner as usize].graveyard.push(card_id);
                                self.check_emrakul_graveyard_shuffle(owner);
                            }
                        }
                    }
                    ChoiceReason::DredgeChoice { dredge_card_id, dredge_n, remaining_draws } => {
                        // n == 0: draw normally (don't dredge)
                        // n == 1: dredge — mill N cards, return the dredge card from graveyard to hand
                        if n == 1 {
                            // Dredge: mill dredge_n cards from the top of the library
                            let pid = choice.player as usize;
                            for _ in 0..dredge_n {
                                if let Some(milled_id) = self.players[pid].library.pop() {
                                    let milled_name = self.card_name_for_id(milled_id)
                                        .unwrap_or(crate::card::CardName::Plains);
                                    self.send_to_graveyard(milled_id, milled_name, choice.player);
                                }
                            }
                            // Return the dredge card from graveyard to hand
                            let pid = choice.player as usize;
                            if let Some(pos) = self.players[pid].graveyard.iter().position(|&id| id == dredge_card_id) {
                                self.players[pid].graveyard.remove(pos);
                                self.players[pid].hand.push(dredge_card_id);
                            }
                            // Dredging replaces the draw, so draws_this_turn does NOT increment.
                            // Continue with any remaining draws.
                            if remaining_draws > 0 {
                                self.draw_cards(choice.player, remaining_draws);
                            }
                        } else {
                            // Draw normally (n == 0): perform the draw that was pending.
                            let pid = choice.player as usize;
                            if let Some(id) = self.players[pid].library.pop() {
                                self.players[pid].hand.push(id);
                                self.players[pid].draws_this_turn += 1;
                                self.players[pid].has_drawn_this_turn = true;
                            } else {
                                self.players[pid].has_lost = true;
                            }
                            // Continue with any remaining draws.
                            if remaining_draws > 0 {
                                self.draw_cards(choice.player, remaining_draws);
                            }
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
