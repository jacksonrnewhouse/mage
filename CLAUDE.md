# Mage — Project Instructions

## What This Is

A Rust game engine for Magic: The Gathering, focused on the Vintage Supreme Draft format. The engine lives in `engine-rust/` and the card pool is in `cards/`.

## Building & Testing

```bash
cd engine-rust
cargo build
cargo test
```

## Project Structure

- `engine-rust/src/` — Rust engine source code
- `cards/` — Card pool text files organized by color/guild (one card name per line)

## Code Conventions

- Rust edition 2021
- Release builds use `opt-level = 3`, LTO, and `codegen-units = 1` for maximum performance
- The engine is designed for game tree search — keep data structures cache-friendly and minimize allocations

## Domain Context

- **Vintage Supreme Draft**: A curated draft format using cards from across Magic's history
- Card files in `cards/` are organized by color identity: mono-colors (white, blue, black, red, green), guild pairs (azorius, dimir, etc.), multicolor, colorless, and lands
- Each card file contains one card name per line

## Workflow

- **Commit after each task**: When completing a discrete unit of work (bug fix, feature, audit batch), commit immediately before moving on to the next task
