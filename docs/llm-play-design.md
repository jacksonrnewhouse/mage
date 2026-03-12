# LLM Play for Mage: Design Document

## The Core Challenge

Magic: The Gathering is one of the hardest games for AI due to:
- **Branching factor**: Turns can have 50-200+ legal actions (land, cast spell with different targets, activate abilities, pass)
- **Hidden information**: Opponent's hand and library order are unknown in real play
- **Long-horizon strategy**: Decisions on turn 1 (mulligan, land choice) determine wins on turn 8
- **Compositional complexity**: Cards interact in emergent ways — knowing individual cards isn't enough
- **Sequential decisions within a turn**: A turn is 5-20+ sequential action choices, not one move

The engine currently exposes perfect information (both hands visible, library order known). This is fine for game tree search but unrealistic for competitive play. We'll need to address this.

---

## Architecture: LLM as Action Selector

### Option A: Pure LLM (Simplest)

```
Loop:
  1. Serialize game state to text
  2. Serialize legal_actions to text
  3. LLM picks action index
  4. apply_action()
```

**Pros**: Simple, leverages LLM's MTG knowledge from training data.
**Cons**: Slow (API call per action, ~10-50 per turn), expensive, no lookahead.

### Option B: LLM + Tree Search (Hybrid, Recommended)

```
Loop:
  1. LLM evaluates game state → position assessment + strategy
  2. LLM scores top-N candidate actions (policy prior)
  3. MCTS uses LLM scores as priors for tree expansion
  4. Best action selected by MCTS visit count
  5. apply_action()
```

**Pros**: LLM provides strategic reasoning; MCTS provides tactical accuracy. Fewer API calls (1-2 per turn instead of 10-50). This is the AlphaGo pattern applied to MTG.
**Cons**: More complex to implement. Requires tuning exploration vs. LLM prior weight.

### Option C: LLM for Strategy, Engine for Tactics

```
Each turn:
  1. LLM receives game state, outputs a strategic plan:
     "Play Thalia, hold up Spell Pierce, attack with Ragavan"
  2. Engine translates plan into action sequence using heuristics
  3. Engine handles priority passes and micro-decisions automatically
```

**Pros**: Minimizes API calls (1 per turn). Natural language plan is interpretable.
**Cons**: Hard to translate plans to actions. Loses nuance in complex stack interactions.

**Recommendation**: Start with Option A for prototyping, graduate to Option B for competitive play.

---

## Game State Serialization

The LLM needs a text representation of the game state. This is the most critical design decision — if the LLM can't understand the board, it can't play well.

### Proposed Format

```
Turn 4 | Phase: Pre-Combat Main | Priority: You
You: 18 life | Hand: 4 | Library: 29 | Graveyard: 2
Opponent: 20 life | Hand: 7 | Library: 26 | Graveyard: 0

=== YOUR HAND ===
[A] Thalia, Guardian of Thraben (1W, 2/1, First Strike)
[B] Swords to Plowshares (W, Instant)
[C] Spell Pierce (U, Instant)
[D] Flooded Strand (Land)

=== BATTLEFIELD ===
You control:
  Tundra (untapped) — Land
  Monastery Mentor (2W, 2/2, Prowess) — tapped, attacked this turn
  Mox Pearl (0) — Artifact, untapped

Opponent controls:
  Underground Sea (untapped) — Land
  Underground Sea (tapped) — Land
  Dark Confidant (1B, 2/1) — Creature, untapped

=== GRAVEYARD ===
Your graveyard: Ponder, Gitaxian Probe
Opponent graveyard: (empty)

=== STACK ===
(empty)

=== MANA AVAILABLE ===
You can produce: WU (Tundra) + W (Mox Pearl) = {W}{W}{U}

=== LEGAL ACTIONS ===
[1] Play land: Flooded Strand
[2] Cast: Thalia, Guardian of Thraben (cost: 1W)
[3] Cast: Swords to Plowshares targeting Dark Confidant (cost: W)
[4] Cast: Spell Pierce (no valid targets)
[5] Pass priority
```

### Key Design Principles

1. **Card names are enough context**: LLMs know MTG cards from training data. Don't repeat full oracle text — just name + key stats.
2. **Actions as a numbered list**: The LLM outputs a single number. Parsing is trivial.
3. **Mana summary**: Show available mana explicitly. Don't make the LLM calculate it.
4. **Group by zone**: Hand, battlefield (yours/theirs), graveyard, stack, exile.
5. **Tapped/untapped matters**: Always show tap state for permanents.

### Hidden Information Mode

For realistic play, hide the opponent's hand and library order:

```
Opponent: 20 life | Hand: 7 cards | Library: 26
```

The LLM can still reason about likely cards from the draft format and cards it's seen.

---

## Implementation Plan

### Phase 1: Text Interface (Rust side)

Add to the engine:

```rust
// New module: engine-rust/src/llm.rs

/// Serialize game state to text from a player's perspective.
/// If `hide_opponent` is true, opponent's hand contents are hidden.
pub fn serialize_game_state(
    state: &GameState,
    db: &[CardDef],
    perspective: PlayerId,
    hide_opponent: bool,
) -> String

/// Serialize legal actions as a numbered list.
/// Returns (text, Vec<Action>) so the LLM's choice index maps to an action.
pub fn serialize_legal_actions(
    state: &GameState,
    db: &[CardDef],
) -> (String, Vec<Action>)

/// Parse LLM response to an action index.
pub fn parse_action_choice(response: &str, num_actions: usize) -> Option<usize>
```

### Phase 2: Python Bridge

The LLM API calls happen in Python. We need a bridge:

**Option 1: PyO3 bindings** (Preferred)
- Expose `GameState`, `legal_actions`, `apply_action`, `serialize_*` to Python
- Zero-copy where possible, Rust speed for game logic
- Python handles LLM API calls and orchestration

**Option 2: JSON over stdin/stdout**
- Rust binary reads actions from stdin, writes state to stdout
- Python subprocess manages the Rust process
- Simpler but slower, good for prototyping

**Option 3: HTTP API**
- Rust binary serves HTTP (e.g., with axum)
- Python client calls endpoints
- Most flexible, good for distributed play

### Phase 3: LLM Integration (Python side)

```python
class LLMPlayer:
    def __init__(self, model: str = "claude-sonnet-4-6"):
        self.client = anthropic.Anthropic()
        self.system_prompt = SYSTEM_PROMPT  # MTG rules + VSD format knowledge
        self.conversation = []  # Track game history for context

    def choose_action(self, game_state_text: str, legal_actions_text: str) -> int:
        """Ask the LLM to pick an action. Returns action index."""
        message = f"{game_state_text}\n\n{legal_actions_text}\n\nChoose an action number."
        response = self.client.messages.create(
            model=self.model,
            system=self.system_prompt,
            messages=self.conversation + [{"role": "user", "content": message}],
            max_tokens=100,
        )
        return parse_action_number(response.content[0].text)
```

### Phase 4: Making It Play Well

#### 4a. System Prompt Engineering

The system prompt is critical. It should include:
- VSD format metagame knowledge (common archetypes, key cards)
- Strategic heuristics ("don't walk into counterspells", "sequence land drops carefully")
- Phase-specific guidance ("during combat, consider removal before blocks")
- Priority pass heuristics ("pass priority on an empty stack if you have no instants")

#### 4b. Action Filtering / Grouping

Legal actions can number 100+. Most are mana ability permutations or equivalent.

**Smart grouping** reduces cognitive load:
- Collapse mana ability variants into "Tap lands for mana" (engine auto-pays)
- Group "Cast X targeting Y" by spell, not target
- Separate strategic actions (cast spell, attack) from automatic ones (pass, mana)
- Present top-5 "interesting" actions prominently, rest as "other options"

#### 4c. MCTS with LLM Prior (Option B)

```python
def llm_mcts(state, db, llm, iterations=200):
    actions = legal_actions(state, db)
    # LLM scores each action (0-1 probability)
    priors = llm.score_actions(state, actions)

    root = MCTSNode(state, actions, priors)
    for _ in range(iterations):
        node = root.select()          # UCB1 with LLM priors
        child = node.expand()
        result = child.rollout(db)    # Random or LLM-guided rollout
        child.backpropagate(result)

    return root.best_action()  # Most visited
```

#### 4d. Self-Play and Evaluation

- Run LLM vs. MCTS baseline (MaterialEvaluator)
- Run LLM vs. random play baseline
- Track win rate by archetype matchup
- Elo rating system across different LLM configurations

---

## Cost and Latency Estimates

| Approach | API calls/game | Tokens/game | Cost/game (Sonnet) | Latency/game |
|----------|---------------|-------------|--------------------|----|
| Pure LLM (Option A) | ~100-300 | ~300K-1M | $0.90-3.00 | 5-15 min |
| LLM + MCTS (Option B) | ~20-40 | ~60K-120K | $0.18-0.36 | 2-5 min |
| LLM Strategy (Option C) | ~10-20 | ~30K-60K | $0.09-0.18 | 1-3 min |

Haiku would cut costs ~10x with some quality loss. For self-play training, Haiku is likely sufficient.

---

## Key Technical Decisions

### 1. Determinism vs. Imperfect Information

The engine currently has **perfect information** (both players see everything). For LLM play:
- **Training/debugging**: Keep perfect info. LLM sees everything.
- **Competitive play**: Add `perspective: PlayerId` to `serialize_game_state()`. Hide opponent hand and library.
- **MCTS with hidden info**: Use Information Set MCTS (IS-MCTS) — sample possible opponent hands, run MCTS on each, aggregate.

### 2. Turn Compression

Many actions within a turn are mechanical (tap land, pay mana, pass priority on empty stack). We should:
- Auto-pass priority when player has no instant-speed plays and stack is empty
- Auto-tap mana when there's only one way to pay
- Bundle "cast spell + pay mana" into a single LLM decision
- Only ask the LLM when there's a real decision

This could reduce API calls from ~300/game to ~30/game.

### 3. Context Window Management

A full game can be 200+ turns of state serialization. The LLM context window fills fast.

**Solutions**:
- Only send current state + last 3-5 key events (not full history)
- Summarize earlier turns ("You've been trading creatures. You're ahead on cards.")
- Use prompt caching (Anthropic API) — the system prompt and early game state are cacheable

### 4. Draft Integration

The LLM should also draft! The 23-pack draft is a natural LLM task:

```python
def draft_pick(pack_contents: list[str], pool_so_far: list[str]) -> tuple[int, int]:
    """LLM picks 2 cards from a 15-card pack given current pool."""
    prompt = f"Pool: {pool_so_far}\nPack: {pack_contents}\nPick 2 cards (indices)."
    ...
```

Draft decisions are high-level strategic choices that LLMs should excel at.

---

## Minimum Viable Implementation

**Week 1**: `serialize_game_state()` and `serialize_legal_actions()` in Rust. JSON output mode.

**Week 2**: Python script that plays a game via subprocess (JSON over stdin/stdout). Pure LLM (Option A) with Claude Sonnet.

**Week 3**: Auto-priority-pass and mana auto-tap to reduce API calls. Measure win rate vs. random.

**Week 4**: Add draft support. LLM drafts a deck, then plays a game. End-to-end VSD experience.

---

## Open Questions

1. **Structured output vs. free text?** Tool use / JSON mode guarantees valid action indices. Free text is more natural but needs parsing.
2. **Memory across games?** Should the LLM remember metagame patterns from prior games?
3. **Multi-game matches?** Sideboarding between games is a rich strategic decision.
4. **Fine-tuning?** Could fine-tune a smaller model on game transcripts from strong play. Requires generating training data first.
5. **Evaluation function learning?** Instead of LLM per-action, train a neural eval function (like AlphaZero) and use with alpha-beta.
