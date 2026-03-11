/// Player state: life total, mana pool, zones (hand, library, graveyard).
/// Designed for fast cloning - all data is owned, no references.

use crate::mana::ManaPool;
use crate::types::*;

/// Runtime state for a single player.
#[derive(Debug, Clone)]
pub struct Player {
    pub id: PlayerId,
    pub life: i32,
    pub mana_pool: ManaPool,
    pub hand: Vec<ObjectId>,
    pub library: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
    pub land_plays_remaining: u8,
    pub land_plays_per_turn: u8,
    pub has_drawn_this_turn: bool,
    /// Number of cards drawn this turn (for draw-limiter statics like Spirit of the Labyrinth)
    pub draws_this_turn: u8,
    pub poison_counters: u8,
    pub has_lost: bool,
    pub has_won: bool,
    /// Number of spells cast this turn (for storm count, etc.)
    pub spells_cast_this_turn: u16,
    /// Number of nonartifact spells cast this turn (for Ethersworn Canonist)
    pub nonartifact_spells_cast_this_turn: u16,
    /// Number of noncreature spells cast this turn (for Deafening Silence)
    pub noncreature_spells_cast_this_turn: u16,
    /// Extra turns queued
    pub extra_turns: u8,
}

impl Player {
    pub fn new(id: PlayerId) -> Self {
        Player {
            id,
            life: 20,
            mana_pool: ManaPool::new(),
            hand: Vec::with_capacity(10),
            library: Vec::with_capacity(60),
            graveyard: Vec::with_capacity(20),
            land_plays_remaining: 1,
            land_plays_per_turn: 1,
            has_drawn_this_turn: false,
            draws_this_turn: 0,
            poison_counters: 0,
            has_lost: false,
            has_won: false,
            spells_cast_this_turn: 0,
            nonartifact_spells_cast_this_turn: 0,
            noncreature_spells_cast_this_turn: 0,
            extra_turns: 0,
        }
    }

    pub fn draw_card(&mut self) -> Option<ObjectId> {
        self.library.pop()
    }

    pub fn draw_cards(&mut self, n: usize) -> Vec<ObjectId> {
        let mut drawn = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(id) = self.library.pop() {
                drawn.push(id);
                self.hand.push(id);
            }
        }
        drawn
    }

    pub fn reset_for_turn(&mut self) {
        self.land_plays_remaining = self.land_plays_per_turn;
        self.has_drawn_this_turn = false;
        self.draws_this_turn = 0;
        self.spells_cast_this_turn = 0;
        self.nonartifact_spells_cast_this_turn = 0;
        self.noncreature_spells_cast_this_turn = 0;
        self.mana_pool.empty();
    }

    pub fn has_card_in_hand(&self, id: ObjectId) -> bool {
        self.hand.contains(&id)
    }

    pub fn remove_from_hand(&mut self, id: ObjectId) -> bool {
        if let Some(pos) = self.hand.iter().position(|&c| c == id) {
            self.hand.swap_remove(pos);
            true
        } else {
            false
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.has_lost && !self.has_won
    }
}
