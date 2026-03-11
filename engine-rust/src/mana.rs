/// Mana system: pools, costs, and payment logic.
/// Designed for compact representation and fast cloning.

use crate::types::Color;

/// A mana pool holding available mana.
///
/// `workshop` tracks mana produced by Mishra's Workshop (and similar restricted sources).
/// Workshop mana may only be spent to cast artifact spells — it is excluded from `total()`
/// and `can_pay()` so that the engine will not offer it for non-artifact spells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ManaPool {
    pub white: u8,
    pub blue: u8,
    pub black: u8,
    pub red: u8,
    pub green: u8,
    pub colorless: u8,
    /// Mana that may only be spent on artifact spells (Mishra's Workshop).
    pub workshop: u8,
}

impl ManaPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, color: Option<Color>, amount: u8) {
        match color {
            Some(Color::White) => self.white += amount,
            Some(Color::Blue) => self.blue += amount,
            Some(Color::Black) => self.black += amount,
            Some(Color::Red) => self.red += amount,
            Some(Color::Green) => self.green += amount,
            None => self.colorless += amount,
        }
    }

    /// Add mana restricted to artifact spells (Mishra's Workshop).
    pub fn add_workshop(&mut self, amount: u8) {
        self.workshop += amount;
    }

    /// Total freely-spendable mana (excludes restricted workshop mana).
    pub fn total(&self) -> u16 {
        self.white as u16
            + self.blue as u16
            + self.black as u16
            + self.red as u16
            + self.green as u16
            + self.colorless as u16
    }

    /// Total mana available when casting an artifact spell (includes workshop mana).
    pub fn total_for_artifact(&self) -> u16 {
        self.total() + self.workshop as u16
    }

    pub fn empty(&mut self) {
        *self = Self::default();
    }

    pub fn get(&self, color: Option<Color>) -> u8 {
        match color {
            Some(Color::White) => self.white,
            Some(Color::Blue) => self.blue,
            Some(Color::Black) => self.black,
            Some(Color::Red) => self.red,
            Some(Color::Green) => self.green,
            None => self.colorless,
        }
    }

    pub fn remove(&mut self, color: Option<Color>, amount: u8) {
        match color {
            Some(Color::White) => self.white = self.white.saturating_sub(amount),
            Some(Color::Blue) => self.blue = self.blue.saturating_sub(amount),
            Some(Color::Black) => self.black = self.black.saturating_sub(amount),
            Some(Color::Red) => self.red = self.red.saturating_sub(amount),
            Some(Color::Green) => self.green = self.green.saturating_sub(amount),
            None => self.colorless = self.colorless.saturating_sub(amount),
        }
    }

    /// Check if this pool can pay a given mana cost for a non-artifact spell.
    /// Workshop mana is NOT counted — it is restricted to artifacts.
    pub fn can_pay(&self, cost: &ManaCost) -> bool {
        self.can_pay_inner(cost, false)
    }

    /// Check if this pool can pay a given mana cost for an artifact spell.
    /// Workshop mana IS counted as generic colorless mana.
    pub fn can_pay_for_artifact(&self, cost: &ManaCost) -> bool {
        self.can_pay_inner(cost, true)
    }

    fn can_pay_inner(&self, cost: &ManaCost, include_workshop: bool) -> bool {
        // First check colored requirements
        if self.white < cost.white
            || self.blue < cost.blue
            || self.black < cost.black
            || self.red < cost.red
            || self.green < cost.green
        {
            return false;
        }
        // Remaining mana after colored costs
        let remaining = (self.white - cost.white) as u16
            + (self.blue - cost.blue) as u16
            + (self.black - cost.black) as u16
            + (self.red - cost.red) as u16
            + (self.green - cost.green) as u16;
        // Colorless mana can only pay generic or colorless costs
        let colorless_remaining = self.colorless + if include_workshop { self.workshop } else { 0 };
        // Colorless-specific cost
        if colorless_remaining < cost.colorless {
            return false;
        }
        let colorless_after = (colorless_remaining - cost.colorless) as u16;
        // Generic can be paid by any remaining
        remaining + colorless_after >= cost.generic as u16
    }

    /// Pay a given amount of generic (any-color) mana. Returns true if successful.
    pub fn pay_generic(&mut self, amount: u32) -> bool {
        if self.total() < amount as u16 {
            return false;
        }
        let mut remaining = amount as u8;
        // Pay with colorless first, then colors
        let from_colorless = remaining.min(self.colorless);
        self.colorless -= from_colorless;
        remaining -= from_colorless;
        let pools = [&mut self.white, &mut self.blue, &mut self.black, &mut self.red, &mut self.green];
        for pool in pools {
            if remaining == 0 {
                break;
            }
            let from_pool = remaining.min(*pool);
            *pool -= from_pool;
            remaining -= from_pool;
        }
        true
    }

    /// Pay a mana cost from this pool for a non-artifact spell.
    /// Workshop mana cannot be used. Returns true if successful.
    pub fn pay(&mut self, cost: &ManaCost) -> bool {
        self.pay_inner(cost, false)
    }

    /// Pay a mana cost from this pool for an artifact spell.
    /// Workshop mana is used as generic colorless mana. Returns true if successful.
    pub fn pay_for_artifact(&mut self, cost: &ManaCost) -> bool {
        self.pay_inner(cost, true)
    }

    fn pay_inner(&mut self, cost: &ManaCost, use_workshop: bool) -> bool {
        if !self.can_pay_inner(cost, use_workshop) {
            return false;
        }
        self.white -= cost.white;
        self.blue -= cost.blue;
        self.black -= cost.black;
        self.red -= cost.red;
        self.green -= cost.green;
        self.colorless -= cost.colorless;

        // Pay generic cost from remaining mana.
        // Order: workshop (artifact-only, least flexible) → colorless → colors.
        let mut generic_remaining = cost.generic;

        if use_workshop {
            let from_workshop = generic_remaining.min(self.workshop);
            self.workshop -= from_workshop;
            generic_remaining -= from_workshop;
        }

        // Then from free colorless
        let from_colorless = generic_remaining.min(self.colorless);
        self.colorless -= from_colorless;
        generic_remaining -= from_colorless;

        // Then from each color
        let pools = [&mut self.white, &mut self.blue, &mut self.black, &mut self.red, &mut self.green];
        for pool in pools {
            if generic_remaining == 0 {
                break;
            }
            let from_pool = generic_remaining.min(*pool);
            *pool -= from_pool;
            generic_remaining -= from_pool;
        }
        true
    }
}

/// A mana cost for casting a spell or activating an ability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ManaCost {
    pub white: u8,
    pub blue: u8,
    pub black: u8,
    pub red: u8,
    pub green: u8,
    pub colorless: u8, // {C} - specifically colorless (e.g. Eldrazi)
    pub generic: u8,   // {N} - payable with any color
}

impl ManaCost {
    pub const ZERO: ManaCost = ManaCost {
        white: 0,
        blue: 0,
        black: 0,
        red: 0,
        green: 0,
        colorless: 0,
        generic: 0,
    };

    pub fn cmc(&self) -> u8 {
        self.white + self.blue + self.black + self.red + self.green + self.colorless + self.generic
    }

    /// Create a cost with just generic mana.
    pub const fn generic(n: u8) -> Self {
        ManaCost {
            generic: n,
            ..Self::ZERO
        }
    }

    /// Create a cost with one colored mana.
    pub const fn color(color_idx: usize, n: u8) -> Self {
        let mut cost = Self::ZERO;
        match color_idx {
            0 => cost.white = n,
            1 => cost.blue = n,
            2 => cost.black = n,
            3 => cost.red = n,
            4 => cost.green = n,
            _ => {}
        }
        cost
    }

    /// Convenience constructors
    pub const fn w(n: u8) -> Self {
        ManaCost {
            white: n,
            ..Self::ZERO
        }
    }
    pub const fn u(n: u8) -> Self {
        ManaCost {
            blue: n,
            ..Self::ZERO
        }
    }
    pub const fn b(n: u8) -> Self {
        ManaCost {
            black: n,
            ..Self::ZERO
        }
    }
    pub const fn r(n: u8) -> Self {
        ManaCost {
            red: n,
            ..Self::ZERO
        }
    }
    pub const fn g(n: u8) -> Self {
        ManaCost {
            green: n,
            ..Self::ZERO
        }
    }
}

/// Parse a mana cost string like "{2}{U}{U}" into a ManaCost.
pub fn parse_mana_cost(s: &str) -> ManaCost {
    let mut cost = ManaCost::ZERO;
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let end = bytes.iter().skip(i).position(|&b| b == b'}').unwrap() + i;
            let symbol = &s[i + 1..end];
            match symbol {
                "W" => cost.white += 1,
                "U" => cost.blue += 1,
                "B" => cost.black += 1,
                "R" => cost.red += 1,
                "G" => cost.green += 1,
                "C" => cost.colorless += 1,
                n => {
                    if let Ok(v) = n.parse::<u8>() {
                        cost.generic += v;
                    }
                }
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mana_pool_pay() {
        let mut pool = ManaPool {
            blue: 2,
            red: 1,
            ..Default::default()
        };
        let cost = ManaCost {
            blue: 1,
            generic: 1,
            ..ManaCost::ZERO
        };
        assert!(pool.can_pay(&cost));
        assert!(pool.pay(&cost));
        // After paying {U} + {1}: blue goes from 2→1 for the {U},
        // then generic {1} is paid from blue (first available color in order), so blue→0
        assert_eq!(pool.blue, 0);
        assert_eq!(pool.red, 1); // red untouched since blue covered generic
    }

    #[test]
    fn test_cannot_pay() {
        let pool = ManaPool {
            red: 1,
            ..Default::default()
        };
        let cost = ManaCost {
            blue: 1,
            ..ManaCost::ZERO
        };
        assert!(!pool.can_pay(&cost));
    }

    #[test]
    fn test_parse_mana_cost() {
        let cost = parse_mana_cost("{2}{U}{U}");
        assert_eq!(cost.generic, 2);
        assert_eq!(cost.blue, 2);
        assert_eq!(cost.cmc(), 4);
    }

    #[test]
    fn test_zero_cost() {
        let pool = ManaPool::new();
        assert!(pool.can_pay(&ManaCost::ZERO));
    }
}
