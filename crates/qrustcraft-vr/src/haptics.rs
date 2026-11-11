//! Haptic events the game raises; the XR layer turns them into controller pulses.

use crate::Hand;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    pub hand: Hand,
    /// 0..=1.
    pub amplitude: f32,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Tool contact while mining; `hardness` is the block's, scaled by the caller to 0..=1.
    MiningTick {
        hand: Hand,
        hardness: f32,
    },
    BlockBroken {
        hand: Hand,
    },
    Hit {
        hand: Hand,
    },
    Place {
        hand: Hand,
    },
    /// Bow draw tension 0..=1, retriggered each frame.
    BowDraw {
        hand: Hand,
        tension: f32,
    },
    BowRelease {
        hand: Hand,
    },
    Damage,
    ScreenOpen {
        hand: Hand,
    },
    UiHover {
        hand: Hand,
    },
}

pub fn pulses(e: Event) -> Vec<Pulse> {
    let p = |hand, amplitude: f32, duration_ms| Pulse {
        hand,
        amplitude: amplitude.clamp(0.0, 1.0),
        duration_ms,
    };
    match e {
        Event::MiningTick { hand, hardness } => vec![p(hand, 0.15 + 0.35 * hardness, 25)],
        Event::BlockBroken { hand } => vec![p(hand, 0.8, 70)],
        Event::Hit { hand } => vec![p(hand, 0.7, 60)],
        Event::Place { hand } => vec![p(hand, 0.4, 35)],
        Event::BowDraw { hand, tension } => vec![p(hand, 0.1 + 0.4 * tension, 20)],
        Event::BowRelease { hand } => vec![p(hand, 0.9, 80)],
        Event::Damage => vec![p(Hand::Left, 1.0, 120), p(Hand::Right, 1.0, 120)],
        Event::ScreenOpen { hand } => vec![p(hand, 0.3, 30)],
        Event::UiHover { hand } => vec![p(hand, 0.1, 10)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_buzzes_both_hands() {
        let v = pulses(Event::Damage);
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|x| x.hand == Hand::Left) && v.iter().any(|x| x.hand == Hand::Right));
    }

    #[test]
    fn harder_blocks_buzz_harder_and_amplitude_is_bounded() {
        let soft = pulses(Event::MiningTick {
            hand: Hand::Right,
            hardness: 0.0,
        })[0]
            .amplitude;
        let hard = pulses(Event::MiningTick {
            hand: Hand::Right,
            hardness: 5.0,
        })[0]
            .amplitude;
        assert!(hard > soft && hard <= 1.0);
    }
}
