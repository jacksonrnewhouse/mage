# Engine Foundations Phase 1: Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement foundational engine features that are prerequisites for higher-level card mechanics: bug fixes, simple flags, static ability enforcement, counterspell variants, enters-tapped, and leaves/dies triggers.

**Architecture:** Extend existing patterns in game.rs and movegen.rs. Static effects follow Thalia's tax pattern (iterate battlefield in effective_cost/generate_priority_actions). New flags added to Permanent/StackItem structs. Triggered effects extend existing TriggeredEffect enum.

**Tech Stack:** Rust, cargo test

---

## Chunk 1: Bug Fixes and Simple Flags

### Task 1: Fix fetch land targets to include shock and survey lands (#50)

**Files:**
- Modify: `engine-rust/src/movegen.rs` (the `is_fetchable` function, ~line 1435)
- Test: `engine-rust/src/lib.rs` (add test)

- [ ] **Step 1: Write failing test for fetching a shock land**

```rust
#[test]
fn test_fetch_finds_shock_lands() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Deck with Flooded Strand and Hallowed Fountain
    let deck: Vec<CardName> = vec![
        CardName::FloodedStrand,
        CardName::HallowedFountain,
    ]
    .into_iter()
    .chain(std::iter::repeat(CardName::Island).take(38))
    .collect();
    state.load_deck(0, &deck, &db);
    state.load_deck(1, &(vec![CardName::Mountain; 40]), &db);
    state.start_game();

    state.phase = Phase::PreCombatMain;
    state.step = None;

    // Play Flooded Strand
    let strand_id = state.players[0]
        .hand
        .iter()
        .find(|&&id| state.card_name_for_id(id) == Some(CardName::FloodedStrand))
        .copied()
        .unwrap();
    state.apply_action(&Action::PlayLand(strand_id), &db);

    // Activate fetch
    let perm_id = state.permanents_controlled_by(0).next().unwrap().id;
    let actions = state.legal_actions(&db);
    let activate = actions.iter().find(|a| matches!(a, Action::ActivateAbility { permanent_id, .. } if *permanent_id == perm_id));
    assert!(activate.is_some(), "Should be able to activate fetch land");
}
```

- [ ] **Step 2: Run test to verify current behavior**

Run: `cd engine-rust && cargo test test_fetch_finds_shock_lands -- --nocapture`

- [ ] **Step 3: Add shock lands and survey lands to is_fetchable**

In `movegen.rs`, find the `is_fetchable` function and add all shock lands and survey lands to the appropriate fetch land match arms. Shock lands have these subtypes:
- Hallowed Fountain = Plains Island (fetchable by Flooded Strand, Marsh Flats, Arid Mesa, Windswept Heath, Misty Rainforest, etc.)
- Watery Grave = Island Swamp
- Blood Crypt = Swamp Mountain
- Stomping Ground = Mountain Forest
- Temple Garden = Forest Plains
- Godless Shrine = Plains Swamp
- Steam Vents = Island Mountain
- Overgrown Tomb = Swamp Forest
- Sacred Foundry = Mountain Plains
- Breeding Pool = Forest Island

Survey lands:
- Meticulous Archive = Plains Island
- Undercity Sewers = Island Swamp
- Thundering Falls = Mountain Forest (verify actual subtypes)
- Hedge Maze = Forest Plains (verify actual subtypes)

For each fetch land, add the shock/survey lands that share at least one subtype with the fetch's search types.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd engine-rust && cargo test test_fetch_finds_shock_lands -- --nocapture`

- [ ] **Step 5: Run full test suite**

Run: `cd engine-rust && cargo test`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "fix: add shock and survey lands to fetch land targets (#50)"
```

---

### Task 2: Fix Crop Rotation resolution (#51)

**Files:**
- Modify: `engine-rust/src/game.rs` (resolve_card_effect, find CropRotation in the match)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_crop_rotation_searches_for_land() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let forest_id = state.new_object_id();
    let crop_id = state.new_object_id();
    state.card_registry.push((forest_id, CardName::Forest));
    state.card_registry.push((crop_id, CardName::CropRotation));
    state.players[0].hand.push(crop_id);

    // Put a Forest on the battlefield to sacrifice
    let def = find_card(&db, CardName::Forest).unwrap();
    let perm = crate::permanent::Permanent::new(
        forest_id, CardName::Forest, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Put some lands in library to find
    let gaea_id = state.new_object_id();
    state.card_registry.push((gaea_id, CardName::GaeasCradle));
    state.players[0].library.push(gaea_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Add green mana
    state.players[0].mana_pool.green = 1;

    // Cast Crop Rotation targeting the Forest to sacrifice
    state.apply_action(
        &Action::CastSpell {
            card_id: crop_id,
            targets: vec![Target::Object(forest_id)],
        },
        &db,
    );

    // Resolve
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Forest should be gone (sacrificed), and we should have a pending choice
    // to search for a land, OR Gaea's Cradle should be on battlefield
    let has_cradle = state.battlefield.iter().any(|p| p.card_name == CardName::GaeasCradle);
    let has_forest = state.battlefield.iter().any(|p| p.card_name == CardName::Forest);
    assert!(!has_forest, "Forest should have been sacrificed");
    // Either cradle is on battlefield or there's a pending choice to find it
    assert!(
        has_cradle || state.pending_choice.is_some(),
        "Should have searched for a land or have pending choice"
    );
}
```

- [ ] **Step 2: Run test to see current behavior**

Run: `cd engine-rust && cargo test test_crop_rotation -- --nocapture`

- [ ] **Step 3: Fix Crop Rotation resolution**

In `game.rs` `resolve_card_effect()`, move `CardName::CropRotation` out of the destroy-target group. Implement it as:
1. Sacrifice the targeted land (remove from battlefield, move to graveyard)
2. Create a PendingChoice for the controller to search library for any land card
3. On choice resolution, put the chosen land onto the battlefield

- [ ] **Step 4: Run test**

Run: `cd engine-rust && cargo test test_crop_rotation -- --nocapture`

- [ ] **Step 5: Run full suite**

Run: `cd engine-rust && cargo test`

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "fix: correct Crop Rotation resolution to sacrifice + search (#51)"
```

---

### Task 3: Add "can't be countered" flag (#48)

**Files:**
- Modify: `engine-rust/src/stack.rs` (add field to StackItem)
- Modify: `engine-rust/src/game.rs` (check flag in counterspell resolution)
- Modify: `engine-rust/src/movegen.rs` (set flag when casting uncounterable spells)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_abrupt_decay_cant_be_countered() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // P0 has Abrupt Decay, P1 has Counterspell
    let decay_id = state.new_object_id();
    let counter_id = state.new_object_id();
    state.card_registry.push((decay_id, CardName::AbruptDecay));
    state.card_registry.push((counter_id, CardName::Counterspell));
    state.players[0].hand.push(decay_id);
    state.players[1].hand.push(counter_id);

    // Target: an artifact on battlefield
    let target_id = state.new_object_id();
    state.card_registry.push((target_id, CardName::SolRing));
    let def = find_card(&db, CardName::SolRing).unwrap();
    let perm = crate::permanent::Permanent::new(
        target_id, CardName::SolRing, 1, 1,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.black = 1;
    state.players[0].mana_pool.green = 1;

    // Cast Abrupt Decay targeting Sol Ring
    state.apply_action(
        &Action::CastSpell {
            card_id: decay_id,
            targets: vec![Target::Object(target_id)],
        },
        &db,
    );
    assert_eq!(state.stack.len(), 1);

    // P0 passes priority
    state.pass_priority(&db);

    // P1 tries to counter - give them mana
    state.players[1].mana_pool.blue = 2;
    // Counterspell should NOT be able to target Abrupt Decay
    let actions = state.legal_actions(&db);
    let can_counter = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == counter_id));
    // If we allow targeting but fizzle, that's also acceptable
    // The key test: after both pass, Sol Ring should be destroyed
    state.pass_priority(&db); // P1 passes (can't effectively counter)

    // Abrupt Decay resolves, Sol Ring should be gone
    let has_sol_ring = state.battlefield.iter().any(|p| p.card_name == CardName::SolRing);
    assert!(!has_sol_ring, "Sol Ring should have been destroyed by Abrupt Decay");
}
```

- [ ] **Step 2: Add `cant_be_countered: bool` to StackItem**

In `stack.rs`, add field:
```rust
pub struct StackItem {
    pub id: ObjectId,
    pub kind: StackItemKind,
    pub controller: PlayerId,
    pub targets: Vec<Target>,
    pub cant_be_countered: bool, // NEW
}
```

Update `GameStack::push()` to accept and set this field (default false).

- [ ] **Step 3: Set the flag for uncounterable spells in movegen.rs**

When generating CastSpell actions or when applying CastSpell in game.rs, check if the card is uncounterable (AbruptDecay, Emrakul, etc.) and set the flag on the StackItem.

Create a helper function:
```rust
fn is_uncounterable(name: CardName) -> bool {
    matches!(name, CardName::AbruptDecay)
    // Add more as needed
}
```

- [ ] **Step 4: Check the flag in counterspell resolution**

In `game.rs` counterspell resolution, before removing the targeted spell from the stack, check `cant_be_countered`. If true, the counterspell fizzles (resolves but doesn't remove the target).

- [ ] **Step 5: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): add 'can't be countered' spell flag (#48)"
```

---

### Task 4: Add "doesn't untap" tracking (#32)

**Files:**
- Modify: `engine-rust/src/permanent.rs` (add field)
- Modify: `engine-rust/src/game.rs` (check in untap step, set on ETB for Mana Vault/Grim Monolith)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_mana_vault_doesnt_untap() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Mana Vault on battlefield, tapped
    let vault_id = state.new_object_id();
    state.card_registry.push((vault_id, CardName::ManaVault));
    let def = find_card(&db, CardName::ManaVault).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        vault_id, CardName::ManaVault, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.tapped = true;
    perm.doesnt_untap = true; // This field doesn't exist yet
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Advance through untap step
    state.phase = Phase::Beginning;
    state.step = Some(Step::Untap);
    state.active_player = 0;
    state.untap_step();

    // Mana Vault should still be tapped
    let vault = state.find_permanent(vault_id).unwrap();
    assert!(vault.tapped, "Mana Vault should NOT untap during untap step");
}
```

- [ ] **Step 2: Add `doesnt_untap: bool` to Permanent**

In `permanent.rs`:
```rust
pub doesnt_untap: bool,
```
Default to `false` in `Permanent::new()`.

- [ ] **Step 3: Check the flag in untap step**

In `game.rs`, find the untap step logic and skip permanents where `doesnt_untap == true`.

- [ ] **Step 4: Set the flag for Mana Vault, Grim Monolith, Time Vault on ETB**

In `game.rs` `resolve_spell()` or `handle_etb()`, when these cards enter the battlefield, set `doesnt_untap = true`.

- [ ] **Step 5: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): add doesnt_untap tracking for Mana Vault/Grim Monolith/Time Vault (#32)"
```

---

### Task 5: Enters-tapped choice for shock lands (#12)

**Files:**
- Modify: `engine-rust/src/game.rs` (in resolve_spell/permanent ETB, add shock land choice)
- Modify: `engine-rust/src/game.rs` (add ChoiceReason variant for shock land)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_shock_land_enters_tapped_if_no_life_paid() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let fountain_id = state.new_object_id();
    state.card_registry.push((fountain_id, CardName::HallowedFountain));
    state.players[0].hand.push(fountain_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    // Play the shock land
    state.apply_action(&Action::PlayLand(fountain_id), &db);

    // Should have a pending choice: pay 2 life or enter tapped
    assert!(state.pending_choice.is_some(), "Should have pending choice for shock land");
}
```

- [ ] **Step 2: Add ChoiceReason::ShockLandETB**

In `game.rs` `ChoiceReason` enum:
```rust
ShockLandETB, // Choose: pay 2 life (number 1) or enter tapped (number 0)
```

- [ ] **Step 3: Create pending choice when shock land enters**

In the land-play path or permanent ETB path, check if the played land is a shock land. If so, create a `PendingChoice` with `ChooseNumber { min: 0, max: 1, reason: ShockLandETB }` where 0 = enter tapped, 1 = pay 2 life.

- [ ] **Step 4: Handle the choice resolution**

When the ChooseNumber resolves for ShockLandETB:
- If 0: set the permanent's `tapped = true`
- If 1: subtract 2 life from the player

- [ ] **Step 5: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): add enters-tapped choice for shock lands (#12)"
```

---

## Chunk 2: Static Ability Enforcement

### Task 6: Enforce draw-limit statics (#44 partial)

**Files:**
- Modify: `engine-rust/src/player.rs` (add `draws_this_turn: u8`)
- Modify: `engine-rust/src/game.rs` (track draws, check limits before drawing)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_spirit_of_the_labyrinth_limits_draws() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Spirit of the Labyrinth on battlefield for P0
    let spirit_id = state.new_object_id();
    state.card_registry.push((spirit_id, CardName::SpiritOfTheLabyrinth));
    let def = find_card(&db, CardName::SpiritOfTheLabyrinth).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        spirit_id, CardName::SpiritOfTheLabyrinth, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Give P1 some cards in library
    for _ in 0..10 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[1].library.push(id);
    }

    // P1 draws first card (should work)
    let hand_before = state.players[1].hand.len();
    state.draw_cards(1, 1);
    assert_eq!(state.players[1].hand.len(), hand_before + 1, "First draw should succeed");

    // P1 tries to draw again (should be blocked)
    let hand_after_first = state.players[1].hand.len();
    state.draw_cards(1, 1);
    assert_eq!(state.players[1].hand.len(), hand_after_first, "Second draw should be blocked by Spirit");
}
```

- [ ] **Step 2: Add `draws_this_turn: u8` to Player**

In `player.rs`, add field, default 0, reset in `reset_for_turn()`.

- [ ] **Step 3: Implement draw-limit check in draw_cards()**

In `game.rs` `draw_cards()`, before each individual card draw:
1. Check battlefield for Spirit of the Labyrinth / Narset / Leovold
2. If a draw-limiter is present and the player has already drawn this turn, skip the draw
3. Increment `draws_this_turn` on successful draw

- [ ] **Step 4: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 5: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): enforce draw-limit statics (Spirit of the Labyrinth, Narset, Leovold) (#44)"
```

---

### Task 7: Enforce cast-restriction statics (#44 partial)

**Files:**
- Modify: `engine-rust/src/player.rs` (add `nonartifact_spells_cast: u16`, `noncreature_spells_cast: u16`)
- Modify: `engine-rust/src/movegen.rs` (check restrictions in generate_priority_actions)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test for Ethersworn Canonist**

```rust
#[test]
fn test_ethersworn_canonist_limits_nonartifact_spells() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Ethersworn Canonist on battlefield for P0
    let canonist_id = state.new_object_id();
    state.card_registry.push((canonist_id, CardName::EtherswornCanonist));
    let def = find_card(&db, CardName::EtherswornCanonist).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        canonist_id, CardName::EtherswornCanonist, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Give P1 two Lightning Bolts and mana
    let bolt1_id = state.new_object_id();
    let bolt2_id = state.new_object_id();
    state.card_registry.push((bolt1_id, CardName::LightningBolt));
    state.card_registry.push((bolt2_id, CardName::LightningBolt));
    state.players[1].hand.push(bolt1_id);
    state.players[1].hand.push(bolt2_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 1;
    state.priority_player = 1;
    state.players[1].mana_pool.red = 2;

    // P1 casts first bolt (should work)
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt1_id,
            targets: vec![Target::Player(0)],
        },
        &db,
    );

    // Resolve bolt
    state.pass_priority(&db);
    state.pass_priority(&db);

    // P1 should NOT be able to cast second bolt
    let actions = state.legal_actions(&db);
    let can_cast_second = actions.iter().any(|a| matches!(a, Action::CastSpell { card_id, .. } if *card_id == bolt2_id));
    assert!(!can_cast_second, "Canonist should prevent second nonartifact spell");
}
```

- [ ] **Step 2: Add tracking fields to Player**

```rust
pub nonartifact_spells_cast_this_turn: u16,
pub noncreature_spells_cast_this_turn: u16,
pub total_spells_cast_this_turn: u16,
```

Increment appropriately when spells are cast. Reset in `reset_for_turn()`.

- [ ] **Step 3: Add restriction checks in generate_priority_actions**

In `movegen.rs` `generate_priority_actions()`, before adding a CastSpell action, check:
- **Ethersworn Canonist**: if opponent controls Canonist and player has cast a nonartifact spell, skip nonartifact spells
- **Deafening Silence**: if opponent controls it and player has cast a noncreature spell, skip noncreature spells
- **Archon of Emeria**: if opponent controls it and player has cast any spell, skip all spells

- [ ] **Step 4: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 5: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): enforce cast-restriction statics (Canonist, Deafening Silence, Archon) (#44)"
```

---

### Task 8: Enforce artifact ability lockdown (Null Rod, Stony Silence) (#44 partial)

**Files:**
- Modify: `engine-rust/src/movegen.rs` (check for Null Rod/Stony Silence alongside Collector Ouphe)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_null_rod_prevents_artifact_abilities() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Null Rod on battlefield
    let rod_id = state.new_object_id();
    state.card_registry.push((rod_id, CardName::NullRod));
    let def = find_card(&db, CardName::NullRod).unwrap();
    let perm = crate::permanent::Permanent::new(
        rod_id, CardName::NullRod, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    state.battlefield.push(perm);

    // Put Sol Ring on battlefield for P0
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    let def2 = find_card(&db, CardName::SolRing).unwrap();
    let mut perm2 = crate::permanent::Permanent::new(
        ring_id, CardName::SolRing, 0, 0,
        def2.power, def2.toughness, None, def2.keywords, def2.card_types,
    );
    perm2.entered_this_turn = false;
    state.battlefield.push(perm2);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;

    let actions = state.legal_actions(&db);
    let can_tap_ring = actions.iter().any(|a| matches!(a, Action::ActivateManaAbility { permanent_id, .. } if *permanent_id == ring_id));
    assert!(!can_tap_ring, "Null Rod should prevent Sol Ring activation");
}
```

- [ ] **Step 2: Add Null Rod and Stony Silence to the Collector Ouphe check**

In `movegen.rs`, find where `CollectorOuphe` is checked for blocking artifact abilities. Add `NullRod` and `StonySilence` to the same check.

- [ ] **Step 3: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 4: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): enforce Null Rod and Stony Silence artifact ability lockdown (#44)"
```

---

## Chunk 3: Counterspell Variants and Leaves/Dies Triggers

### Task 9: Counterspell resolution variants (#25)

**Files:**
- Modify: `engine-rust/src/game.rs` (resolve_card_effect, counterspell section)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test for Memory Lapse**

```rust
#[test]
fn test_memory_lapse_puts_on_top() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let bolt_id = state.new_object_id();
    let lapse_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.card_registry.push((lapse_id, CardName::MemoryLapse));
    state.players[0].hand.push(bolt_id);
    state.players[1].hand.push(lapse_id);

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.red = 1;
    state.players[1].mana_pool.blue = 2;

    // P0 casts Bolt
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
        },
        &db,
    );
    let bolt_stack_id = state.stack.top().unwrap().id;

    // P0 passes
    state.pass_priority(&db);

    // P1 casts Memory Lapse targeting the bolt
    state.apply_action(
        &Action::CastSpell {
            card_id: lapse_id,
            targets: vec![Target::Object(bolt_stack_id)],
        },
        &db,
    );

    // Both pass to resolve Memory Lapse
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bolt should be on top of P0's library, not in graveyard
    let top_of_library = state.players[0].library.last().copied();
    assert_eq!(
        state.card_name_for_id(top_of_library.unwrap()),
        Some(CardName::LightningBolt),
        "Memory Lapse should put countered spell on top of library"
    );
    assert!(
        !state.players[0].graveyard.contains(&bolt_id),
        "Countered spell should NOT be in graveyard"
    );
}
```

- [ ] **Step 2: Write failing test for Remand**

```rust
#[test]
fn test_remand_returns_to_hand_and_draws() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    let bolt_id = state.new_object_id();
    let remand_id = state.new_object_id();
    state.card_registry.push((bolt_id, CardName::LightningBolt));
    state.card_registry.push((remand_id, CardName::Remand));
    state.players[0].hand.push(bolt_id);
    state.players[1].hand.push(remand_id);

    // Give P1 library cards for the draw
    for _ in 0..5 {
        let id = state.new_object_id();
        state.card_registry.push((id, CardName::Island));
        state.players[1].library.push(id);
    }

    state.turn_number = 1;
    state.phase = Phase::PreCombatMain;
    state.step = None;
    state.active_player = 0;
    state.priority_player = 0;
    state.players[0].mana_pool.red = 1;
    state.players[1].mana_pool.blue = 1;
    state.players[1].mana_pool.colorless = 1;

    let p1_hand_size = state.players[1].hand.len();

    // P0 casts Bolt
    state.apply_action(
        &Action::CastSpell {
            card_id: bolt_id,
            targets: vec![Target::Player(1)],
        },
        &db,
    );
    let bolt_stack_id = state.stack.top().unwrap().id;
    state.pass_priority(&db);

    // P1 casts Remand
    state.apply_action(
        &Action::CastSpell {
            card_id: remand_id,
            targets: vec![Target::Object(bolt_stack_id)],
        },
        &db,
    );
    state.pass_priority(&db);
    state.pass_priority(&db);

    // Bolt should be back in P0's hand
    assert!(state.players[0].hand.contains(&bolt_id), "Remand should return spell to hand");
    // P1 should have drawn a card (hand size: original - remand + 1 draw)
    assert_eq!(state.players[1].hand.len(), p1_hand_size, "Remand controller should draw a card");
}
```

- [ ] **Step 3: Implement per-card counterspell resolution**

In `game.rs` `resolve_card_effect()`, in the counterspell section, instead of always removing to graveyard, dispatch per card:
- **MemoryLapse**: Remove from stack, put card_id on top of owner's library
- **Remand**: Remove from stack, put card_id back in owner's hand, controller draws 1
- **Default (Counterspell, FoW, etc.)**: Remove from stack, card to graveyard (existing behavior)

- [ ] **Step 4: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 5: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): implement counterspell resolution variants (Memory Lapse, Remand) (#25)"
```

---

### Task 10: Leaves-battlefield and dies triggers (#28)

**Files:**
- Modify: `engine-rust/src/game.rs` (add trigger check when permanents leave battlefield)
- Modify: `engine-rust/src/stack.rs` (add new TriggeredEffect variants)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_myr_retriever_dies_trigger() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put Myr Retriever on battlefield
    let myr_id = state.new_object_id();
    state.card_registry.push((myr_id, CardName::MyrRetriever));
    let def = find_card(&db, CardName::MyrRetriever).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        myr_id, CardName::MyrRetriever, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Put an artifact in P0's graveyard to retrieve
    let ring_id = state.new_object_id();
    state.card_registry.push((ring_id, CardName::SolRing));
    state.players[0].graveyard.push(ring_id);

    // Kill Myr Retriever (e.g., via lethal damage)
    state.destroy_permanent(myr_id);

    // Should have a triggered ability on the stack or pending choice
    assert!(
        !state.stack.is_empty() || state.pending_choice.is_some(),
        "Myr Retriever should trigger on death"
    );
}
```

- [ ] **Step 2: Create a centralized `remove_permanent` method**

In `game.rs`, create a method that handles permanent removal and checks for dies/leaves triggers:

```rust
pub fn remove_permanent_to_zone(&mut self, perm_id: ObjectId, destination: Zone) {
    // Find and remove the permanent
    if let Some(pos) = self.battlefield.iter().position(|p| p.id == perm_id) {
        let perm = self.battlefield.remove(pos);
        let card_name = perm.card_name;
        let controller = perm.controller;
        let owner = perm.owner;

        // Move to destination zone
        match destination {
            Zone::Graveyard => {
                self.players[owner as usize].graveyard.push(perm_id);
                // Check dies triggers
                self.check_dies_triggers(perm_id, card_name, controller);
            }
            Zone::Exile => {
                self.exile.push((perm_id, card_name, owner));
            }
            Zone::Hand => {
                self.players[owner as usize].hand.push(perm_id);
            }
            _ => {}
        }
        // Check leaves-battlefield triggers (fires for any zone change)
        self.check_leaves_triggers(perm_id, card_name, controller);
    }
}
```

- [ ] **Step 3: Implement check_dies_triggers and check_leaves_triggers**

```rust
fn check_dies_triggers(&mut self, died_id: ObjectId, died_name: CardName, controller: PlayerId) {
    match died_name {
        CardName::MyrRetriever => {
            // Return target artifact from GY to hand
            // Create pending choice from artifacts in graveyard
        }
        CardName::WurmcoilEngine => {
            // Already handled via WurmcoilDeath - keep existing
        }
        _ => {}
    }
    // Also check battlefield for permanents that care about other things dying
    // e.g., Skullclamp equipped creature dying
}
```

- [ ] **Step 4: Refactor existing removal code to use remove_permanent_to_zone**

Find all places in game.rs where permanents are removed from battlefield and route through the new method. This includes:
- SBA lethal damage removal
- Destroy effects (Disenchant, etc.)
- Sacrifice effects
- Bounce effects (these go to Hand, not GY)
- Exile effects (Swords to Plowshares)

- [ ] **Step 5: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): add leaves-battlefield and dies trigger framework (#28)"
```

---

## Chunk 4: Temporary Effects

### Task 11: Until-end-of-turn effects system (#21)

**Files:**
- Modify: `engine-rust/src/game.rs` (add temporary effects list, apply/remove in end step)
- Modify: `engine-rust/src/types.rs` (add TemporaryEffect type)
- Test: `engine-rust/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_temporary_pt_modification() {
    let db = build_card_db();
    let mut state = GameState::new_two_player();

    // Put a 2/2 creature on battlefield
    let creature_id = state.new_object_id();
    state.card_registry.push((creature_id, CardName::GoblinGuide));
    let def = find_card(&db, CardName::GoblinGuide).unwrap();
    let mut perm = crate::permanent::Permanent::new(
        creature_id, CardName::GoblinGuide, 0, 0,
        def.power, def.toughness, None, def.keywords, def.card_types,
    );
    perm.entered_this_turn = false;
    state.battlefield.push(perm);

    // Apply a temporary -1/-1 effect
    state.add_temporary_effect(TemporaryEffect::ModifyPT {
        target: creature_id,
        power: -1,
        toughness: -1,
    });

    // Check creature is now 1/1
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 1);
    assert_eq!(creature.toughness(), 1);

    // End of turn cleanup
    state.end_of_turn_cleanup();

    // Creature should be back to 2/2
    let creature = state.find_permanent(creature_id).unwrap();
    assert_eq!(creature.power(), 2);
    assert_eq!(creature.toughness(), 2);
}
```

- [ ] **Step 2: Define TemporaryEffect enum**

In `types.rs`:
```rust
#[derive(Debug, Clone)]
pub enum TemporaryEffect {
    ModifyPT { target: ObjectId, power: i16, toughness: i16 },
    GrantKeyword { target: ObjectId, keyword: Keyword },
    RemoveAllAbilities { target: ObjectId },
    // Add more as needed
}
```

- [ ] **Step 3: Add temporary_effects to GameState**

```rust
pub temporary_effects: Vec<TemporaryEffect>,
```

- [ ] **Step 4: Implement add/remove temporary effects**

- `add_temporary_effect()`: push to list, apply the effect (modify power_mod/toughness_mod on target)
- `end_of_turn_cleanup()`: iterate temporary_effects, reverse each one, clear the list

- [ ] **Step 5: Run tests**

Run: `cd engine-rust && cargo test`

- [ ] **Step 6: Commit**

```bash
cd engine-rust && git add -A && git commit -m "feat(engine): add temporary until-end-of-turn effects system (#21)"
```
