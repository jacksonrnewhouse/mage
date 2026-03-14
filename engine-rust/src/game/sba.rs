/// State-based actions and legend rule.

use crate::card::*;
use crate::game::{Emblem, GameState};
use crate::permanent::*;
use crate::types::*;

impl GameState {
    pub fn check_state_based_actions(&mut self, db: &[CardDef]) {
        let mut changes = true;
        while changes {
            changes = false;

            // Player loses if life <= 0
            // Exception: Gideon of the Trials emblem — "As long as you control a Gideon
            // planeswalker, you can't lose the game and your opponents can't win the game."
            for i in 0..self.num_players as usize {
                if self.players[i].life <= 0 && !self.players[i].has_lost {
                    let pid = i as PlayerId;
                    let gideon_protected = self.has_emblem(pid, Emblem::GideonOfTheTrials)
                        && self.battlefield.iter().any(|p| {
                            p.controller == pid && p.is_planeswalker()
                                && matches!(p.card_name, CardName::GideonOfTheTrials)
                        });
                    if !gideon_protected {
                        self.players[i].has_lost = true;
                        changes = true;
                    }
                }
            }

            // Player loses if they tried to draw from empty library
            // (Handled when draw happens)

            // Creatures with 0 or less toughness die
            let mut to_die = Vec::new();
            for perm in &self.battlefield {
                if perm.is_creature() {
                    let toughness = self.effective_toughness(perm.id, db);
                    let has_lethal = perm.damage >= toughness && !perm.keywords.has(Keyword::Indestructible);
                    if toughness <= 0 || has_lethal {
                        to_die.push(perm.id);
                    }
                }
            }
            for id in to_die {
                if self.destroy_permanent(id).is_some() {
                    changes = true;
                }
            }

            // Planeswalkers with 0 or less loyalty die
            let mut pw_to_die = Vec::new();
            for perm in &self.battlefield {
                if perm.is_planeswalker() && perm.loyalty <= 0 {
                    pw_to_die.push(perm.id);
                }
            }
            for id in pw_to_die {
                if self.destroy_permanent(id).is_some() {
                    changes = true;
                }
            }

            // Legend rule: if a player controls 2+ legendaries with the same name,
            // they choose one to keep (for simplicity, keep the newer one)
            let mut legend_names: Vec<(CardName, PlayerId, ObjectId)> = Vec::new();
            let mut legend_to_remove = Vec::new();
            for perm in &self.battlefield {
                if perm.card_types.contains(&CardType::Planeswalker)
                    || self.is_legendary(perm)
                {
                    if let Some(existing) = legend_names.iter().find(|(n, c, _)| {
                        *n == perm.card_name && *c == perm.controller
                    }) {
                        legend_to_remove.push(existing.2); // Remove older one
                    }
                    legend_names.push((perm.card_name, perm.controller, perm.id));
                }
            }
            for id in legend_to_remove {
                if self.destroy_permanent(id).is_some() {
                    changes = true;
                }
            }
        }

        // Check for game over
        let alive_count = self.players.iter().filter(|p| !p.has_lost).count();
        if alive_count <= 1 {
            if let Some(winner) = self.players.iter().find(|p| !p.has_lost) {
                self.result = GameResult::Win(winner.id);
            } else {
                self.result = GameResult::Draw;
            }
        }
    }

    pub fn is_legendary(&self, perm: &Permanent) -> bool {
        matches!(
            perm.card_name,
            // Artifacts
            CardName::MoxOpal | CardName::BolassCitadel | CardName::Shadowspear
            | CardName::TheOneRing | CardName::UnderworldBreach
            // Lands
            | CardName::TolarianAcademy | CardName::GaeasCradle
            | CardName::UrborgTombOfYawgmoth | CardName::YavimayaCradleOfGrowth
            | CardName::Karakas | CardName::OtawaraSoaringCity | CardName::BoseijuWhoEndures
            | CardName::TalonGatesOfMadara
            // White creatures
            | CardName::ThaliaGuardianOfThraben | CardName::AjaniNacatlPariah
            | CardName::KatakiWarsWage | CardName::OswaldFiddlebender
            | CardName::PheliaExuberantShepherd | CardName::SamwiseTheStouthearted
            | CardName::BoromirWardenOfTheTower
            | CardName::LoranOfTheThirdPath
            // Blue creatures
            | CardName::TamiyoInquisitiveStudent | CardName::EmryLurkerOfTheLoch
            | CardName::PlagonLordOfTheBeach
            // Black creatures
            | CardName::SheoldredTheApocalypse | CardName::Griselbrand
            | CardName::MaiScornfulStriker
            // Red creatures
            | CardName::RagavanNimblePilferer | CardName::ZhaoTheMoonSlayer
            | CardName::GutTrueSoulZealot | CardName::SqueeGoblinNabob
            // Green creatures
            | CardName::HogaakArisenNecropolis
            // Colorless creatures
            | CardName::GolosTirelessPilgrim | CardName::KarnSilverGolem
            | CardName::EmrakulTheAeonsTorn
            // Multicolor creatures
            | CardName::LaviniaAzoriusRenegade | CardName::MakdeeAndItlaSkysnarers
            | CardName::LurrusOfTheDreamDen | CardName::NaduWingedWisdom
            | CardName::LeovoldEmissaryOfTrest | CardName::AtraxaGrandUnifier
            // Planeswalkers (all legendary)
            | CardName::JaceTheMindSculptor | CardName::TeferiTimeRaveler
            | CardName::DackFayden | CardName::NarsetParterOfVeils
            | CardName::GideonOfTheTrials | CardName::KarnTheGreatCreator
            | CardName::TezzeretCruelCaptain | CardName::WrennAndSix
            | CardName::MinscAndBooTimelessHeroes | CardName::KayaOrzhovUsurper
            | CardName::OkoThiefOfCrowns | CardName::CometStellarPup
            | CardName::DovinHandOfControl
            // Enchantments
            | CardName::FableOfTheMirrorBreaker | CardName::ReflectionOfKikiJiki
            | CardName::HidetsuguConsumesAll
            | CardName::VesselOfTheAllConsuming
        )
    }
}
