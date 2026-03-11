use crate::card::*;
use crate::game::*;
use crate::types::*;

mod alt_cost;
mod coin_flip;
mod annihilator_extra_turn;
mod delayed_triggers;
mod basic;
mod modal_spells;
mod dredge;
mod emblem;
mod clone;
mod protection;
mod devotion_metalcraft;
mod combat;
mod monarch;
mod cost_reduction;
mod cycling;
mod dynamic_pt;
mod equipment;
mod exile_linked;
mod flashback;
mod land_type_modification;
mod library_top;
mod replacement_effects;
mod show_and_tell;
mod spells;
mod statics;
mod surveil;
mod triggers;
mod tribal;
mod one_ring;
mod imprint;
mod madness;

pub(crate) fn setup_simple_game() -> (GameState, Vec<CardDef>) {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Player 0: interleave Mountains with spells so hand has both
    // Library is LIFO, so last cards added are drawn first
    let p0_deck: Vec<CardName> = std::iter::repeat(CardName::GoblinGuide)
        .take(10)
        .chain(std::iter::repeat(CardName::LightningBolt).take(10))
        .chain(std::iter::repeat(CardName::Mountain).take(4))
        .chain(std::iter::repeat(CardName::LightningBolt).take(3))
        .chain(std::iter::repeat(CardName::Mountain).take(13))
        .collect();
    state.load_deck(0, &p0_deck, &db);

    // Player 1: same approach
    let p1_deck: Vec<CardName> = std::iter::repeat(CardName::AncestralRecall)
        .take(10)
        .chain(std::iter::repeat(CardName::Counterspell).take(10))
        .chain(std::iter::repeat(CardName::Island).take(4))
        .chain(std::iter::repeat(CardName::Counterspell).take(3))
        .chain(std::iter::repeat(CardName::Island).take(13))
        .collect();
    state.load_deck(1, &p1_deck, &db);

    state.start_game();
    // Hand now has: 4 Mountains + 3 Lightning Bolts for P0
    //               4 Islands + 3 Counterspells for P1
    (state, db)
}
