# Mage — Magic: The Gathering Engine in Rust

A high-performance Magic: The Gathering game engine written in Rust, optimized for game tree search. The current focus is implementing the **Vintage Supreme Draft** format.

## Overview

Mage is a Rust engine (`engine-rust/`) that models Magic: The Gathering gameplay with full rules enforcement, designed for fast simulation and AI-driven game tree search. The card pool in `cards/` is organized by color and guild, targeting the Vintage Supreme Draft card list.

## Project Structure

```
engine-rust/       # Rust game engine
  src/
    game.rs        # Game state management
    card.rs        # Card representation
    combat.rs      # Combat system
    mana.rs        # Mana system
    movegen.rs     # Move generation
    search.rs      # Game tree search
    stack.rs       # Stack and spell resolution
    player.rs      # Player state
    permanent.rs   # Battlefield permanents
    action.rs      # Player actions
    types.rs       # Shared type definitions
cards/             # Card pool organized by color/guild
```

## Building

```bash
cd engine-rust
cargo build --release
```

## Current Focus: Vintage Supreme Draft

The engine targets the most recent Vintage Supreme Draft format — a curated draft environment drawing from Magic's full card history. The `cards/` directory contains the draft pool organized by color (white, blue, black, red, green), multicolor, guild pairs (azorius, dimir, rakdos, gruul, selesnya, orzhov, izzet, golgari, boros, simic), colorless, and lands.
