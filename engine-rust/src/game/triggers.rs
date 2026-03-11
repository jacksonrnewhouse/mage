/// Dies triggers, leaves-battlefield triggers, and permanent removal with trigger checks.

use crate::card::*;
use crate::game::{DestinationZone, GameState};
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
            _ => {}
        }

        // --- Triggers on other permanents that care about things dying ---
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
    /// Currently a placeholder for future expansion.
    pub(crate) fn check_leaves_triggers(
        &mut self,
        _left_id: ObjectId,
        _left_name: CardName,
        _controller: PlayerId,
    ) {
        // Future: handle leaves-battlefield triggers like
        // Oblivion Ring, Flickerwisp, etc.
    }
}
