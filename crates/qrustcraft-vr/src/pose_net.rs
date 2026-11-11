//! Pose as first-class network state.
//!
//! Vivecraft sends VR poses on a side channel and then, per hit, *temporarily rewrites the server
//! player* to the hand pose (AimFixHandler) and patches about twenty gameplay mixins to read
//! "aim position/direction". Here the pose is an ordinary part of the player's input packet and
//! the server derives aim, eye height and reach from it directly, so there is nothing to
//! rewrite and every client, VR or not, can render everyone's hands.
//!
//! Wire format (little endian), 4 + 1 + 3 x 10 = 35 bytes at full pose, no padding:
//! `tick: u32`, `flags: u8`, then head, left hand, right hand each as
//! `position: 3 x i16` (1/4096 m, relative to the player's feet, +-8 m) and
//! `orientation: u32` (smallest-three quaternion, 10 bits per component + 2-bit index).

use crate::{Hand, Pose, Quat, Vec3};

/// Capability bits exchanged in the handshake. A bitset, not a version ordinal: a feature is
/// available when both sides set its bit, and adding one never needs a "legacy" path.
pub mod caps {
    pub const POSE: u32 = 1 << 0;
    pub const HAND_AIM: u32 = 1 << 1;
    pub const ROOMSCALE: u32 = 1 << 2;
    pub const CLIMB: u32 = 1 << 3;
    pub const TELEPORT: u32 = 1 << 4;
    pub const HAPTICS: u32 = 1 << 5;
    pub const FULL_BODY: u32 = 1 << 6;
}

/// Capabilities both sides agreed on.
pub fn negotiate(client: u32, server: u32) -> u32 {
    client & server
}

pub const FLAG_SEATED: u8 = 1;
pub const FLAG_LEFT_HANDED: u8 = 2;
pub const FLAG_CRAWLING: u8 = 4;
/// The left hand's pose is stale (the controller is not tracked): don't validate or render it.
pub const FLAG_LEFT_UNTRACKED: u8 = 8;
pub const FLAG_RIGHT_UNTRACKED: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerPose {
    pub tick: u32,
    pub flags: u8,
    /// Head, left hand, right hand, relative to the player's feet. A hand whose
    /// `FLAG_*_UNTRACKED` bit is set carries a stale pose; read hands through [`Self::hand`].
    pub head: Pose,
    pub hands: [Pose; 2],
}

impl PlayerPose {
    /// The flag bit saying hand `h` is not tracked.
    pub fn untracked_flag(h: Hand) -> u8 {
        match h {
            Hand::Left => FLAG_LEFT_UNTRACKED,
            Hand::Right => FLAG_RIGHT_UNTRACKED,
        }
    }

    /// Build from the hands the client has this tick; an untracked hand keeps `fallback` (its
    /// last pose, so other players see it hang where it was) and gets its untracked bit.
    pub fn with_hands(
        tick: u32,
        flags: u8,
        head: Pose,
        hands: [Option<Pose>; 2],
        fallback: [Pose; 2],
    ) -> Self {
        let mut flags = flags & !(FLAG_LEFT_UNTRACKED | FLAG_RIGHT_UNTRACKED);
        for (h, p) in [Hand::Left, Hand::Right].into_iter().zip(hands) {
            if p.is_none() {
                flags |= Self::untracked_flag(h);
            }
        }
        Self {
            tick,
            flags,
            head,
            hands: [
                hands[0].unwrap_or(fallback[0]),
                hands[1].unwrap_or(fallback[1]),
            ],
        }
    }

    /// Hand `h`'s pose, if it was tracked.
    pub fn hand(&self, h: Hand) -> Option<Pose> {
        let i = usize::from(h == Hand::Right);
        (self.flags & Self::untracked_flag(h) == 0).then_some(self.hands[i])
    }
}

pub const PACKET_LEN: usize = 4 + 1 + 3 * 10;
const POS_SCALE: f32 = 4096.0;

fn put_pose(out: &mut Vec<u8>, p: &Pose) {
    for c in p.position.to_array() {
        let q = (c * POS_SCALE)
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        out.extend_from_slice(&q.to_le_bytes());
    }
    out.extend_from_slice(&pack_quat(p.orientation).to_le_bytes());
}

fn get_pose(b: &[u8]) -> Pose {
    let c = |i: usize| f32::from(i16::from_le_bytes([b[i * 2], b[i * 2 + 1]])) / POS_SCALE;
    Pose::new(
        Vec3::new(c(0), c(1), c(2)),
        unpack_quat(u32::from_le_bytes([b[6], b[7], b[8], b[9]])),
    )
}

/// Smallest-three: drop the largest component (recoverable from unit length), store the other
/// three in 10 bits each in [-1/sqrt2, 1/sqrt2].
fn pack_quat(q: Quat) -> u32 {
    let q = q.normalize();
    let a = q.to_array();
    let (mut idx, mut max) = (0, 0.0f32);
    for (i, v) in a.iter().enumerate() {
        if v.abs() > max {
            max = v.abs();
            idx = i;
        }
    }
    let sign = if a[idx] < 0.0 { -1.0 } else { 1.0 };
    let mut bits = idx as u32;
    let mut shift = 2;
    for (i, v) in a.iter().enumerate() {
        if i == idx {
            continue;
        }
        let f = (v * sign * std::f32::consts::SQRT_2).clamp(-1.0, 1.0);
        bits |= (((f * 0.5 + 0.5) * 1023.0).round() as u32) << shift;
        shift += 10;
    }
    bits
}

fn unpack_quat(bits: u32) -> Quat {
    let idx = (bits & 3) as usize;
    let mut a = [0.0f32; 4];
    let mut shift = 2;
    let mut sum = 0.0;
    for (i, slot) in a.iter_mut().enumerate() {
        if i == idx {
            continue;
        }
        let f = (((bits >> shift) & 1023) as f32 / 1023.0 - 0.5) * 2.0 / std::f32::consts::SQRT_2;
        *slot = f;
        sum += f * f;
        shift += 10;
    }
    a[idx] = (1.0 - sum).max(0.0).sqrt();
    Quat::from_array(a).normalize()
}

impl PlayerPose {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PACKET_LEN);
        out.extend_from_slice(&self.tick.to_le_bytes());
        out.push(self.flags);
        put_pose(&mut out, &self.head);
        put_pose(&mut out, &self.hands[0]);
        put_pose(&mut out, &self.hands[1]);
        out
    }

    pub fn decode(b: &[u8]) -> Option<Self> {
        if b.len() != PACKET_LEN {
            return None;
        }
        let pose = |i: usize| get_pose(&b[5 + i * 10..15 + i * 10]);
        Some(Self {
            tick: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            flags: b[4],
            head: pose(0),
            hands: [pose(1), pose(2)],
        })
    }
}

/// Server rules for validating a client's pose and the hits it claims. One table instead of
/// thirty flags.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// Max distance of a hand from the head (arm plus shoulder), metres.
    pub max_hand_from_head: f32,
    /// Max distance the head may be from the player's entity (room-scale slack).
    pub max_head_from_entity: f32,
    /// Slack added to a reach check for pose quantisation and latency.
    pub reach_tolerance: f32,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            max_hand_from_head: 1.4,
            max_head_from_entity: 4.0,
            reach_tolerance: 0.25,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reject {
    HandTooFar,
    HeadTooFarFromBody,
    OutOfReach,
}

impl Rules {
    /// Is this pose physically plausible? Poses are relative to the feet. Untracked hands are
    /// not checked (their pose is stale, not claimed); they cannot be used to hit either, see
    /// [`PlayerPose::hand`].
    pub fn validate(&self, p: &PlayerPose) -> Result<(), Reject> {
        if p.head.position.length() > self.max_head_from_entity {
            return Err(Reject::HeadTooFarFromBody);
        }
        if [Hand::Left, Hand::Right]
            .into_iter()
            .filter_map(|h| p.hand(h))
            .any(|h| h.position.distance(p.head.position) > self.max_hand_from_head)
        {
            return Err(Reject::HandTooFar);
        }
        Ok(())
    }

    /// May a hand at `hand` hit `target` with a tool of `reach` metres? The server uses the hand
    /// position the client sent, so this replaces "rewrite the player to the hand for this packet".
    pub fn check_hit(&self, hand: Vec3, target: Vec3, reach: f32) -> Result<(), Reject> {
        if hand.distance(target) <= reach + self.reach_tolerance {
            Ok(())
        } else {
            Err(Reject::OutOfReach)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PlayerPose {
        PlayerPose {
            tick: 123_456,
            flags: FLAG_LEFT_HANDED | FLAG_CRAWLING,
            head: Pose::new(
                Vec3::new(0.12, 1.62, -0.3),
                Quat::from_euler(glam::EulerRot::YXZ, 0.7, -0.3, 0.1),
            ),
            hands: [
                Pose::new(Vec3::new(-0.3, 1.1, -0.4), Quat::from_rotation_x(-0.9)),
                Pose::new(Vec3::new(0.35, 1.2, -0.5), Quat::from_rotation_z(2.5)),
            ],
        }
    }

    #[test]
    fn round_trips_within_quantisation() {
        let p = sample();
        let bytes = p.encode();
        assert_eq!(bytes.len(), PACKET_LEN);
        let d = PlayerPose::decode(&bytes).unwrap();
        assert_eq!((d.tick, d.flags), (p.tick, p.flags));
        for (a, b) in [
            (p.head, d.head),
            (p.hands[0], d.hands[0]),
            (p.hands[1], d.hands[1]),
        ] {
            assert!(a.position.distance(b.position) < 0.001, "{a:?} {b:?}");
            assert!(
                a.orientation.angle_between(b.orientation) < 0.01,
                "{a:?} {b:?}"
            );
        }
    }

    #[test]
    fn rotations_survive_in_every_quadrant() {
        for k in 0..64 {
            let q = Quat::from_euler(
                glam::EulerRot::YXZ,
                k as f32 * 0.37,
                k as f32 * 0.21 - 3.0,
                k as f32 * 0.11,
            );
            let r = unpack_quat(pack_quat(q));
            assert!(q.angle_between(r) < 0.01, "k={k}");
        }
    }

    #[test]
    fn untracked_hands_are_flagged_and_survive_the_wire() {
        let s = sample();
        let p = PlayerPose::with_hands(
            9,
            FLAG_SEATED | FLAG_LEFT_UNTRACKED,
            s.head,
            [None, Some(s.hands[1])],
            s.hands,
        );
        assert_eq!(p.flags, FLAG_SEATED | FLAG_LEFT_UNTRACKED);
        let d = PlayerPose::decode(&p.encode()).unwrap();
        assert!(d.hand(Hand::Left).is_none() && d.hand(Hand::Right).is_some());
        let both = PlayerPose::with_hands(
            9,
            p.flags,
            s.head,
            [Some(s.hands[0]), Some(s.hands[1])],
            s.hands,
        );
        assert_eq!(both.flags, FLAG_SEATED);
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert!(PlayerPose::decode(&[0; PACKET_LEN - 1]).is_none());
        assert!(PlayerPose::decode(&[0; PACKET_LEN + 1]).is_none());
    }

    #[test]
    fn capabilities_are_an_intersection() {
        use caps::*;
        let n = negotiate(POSE | HAND_AIM | CLIMB, POSE | HAND_AIM | TELEPORT);
        assert_eq!(n, POSE | HAND_AIM);
    }

    #[test]
    fn implausible_poses_are_rejected() {
        let rules = Rules::default();
        assert_eq!(rules.validate(&sample()), Ok(()));
        let mut far_hand = sample();
        far_hand.hands[1].position = Vec3::new(3.0, 1.2, 0.0);
        assert_eq!(rules.validate(&far_hand), Err(Reject::HandTooFar));
        let mut far_head = sample();
        far_head.head.position = Vec3::new(9.0, 1.6, 0.0);
        assert_eq!(rules.validate(&far_head), Err(Reject::HeadTooFarFromBody));
        // The same far hand, marked untracked, is a stale pose, not a claim.
        let mut stale = far_hand;
        stale.flags |= FLAG_RIGHT_UNTRACKED;
        assert_eq!(rules.validate(&stale), Ok(()));
        assert!(stale.hand(Hand::Right).is_none() && stale.hand(Hand::Left).is_some());
    }

    #[test]
    fn hit_reach_uses_the_hand_with_tolerance() {
        let r = Rules::default();
        let hand = Vec3::new(0.0, 1.2, 0.0);
        assert!(r.check_hit(hand, Vec3::new(0.0, 1.2, 2.6), 2.5).is_ok());
        assert_eq!(
            r.check_hit(hand, Vec3::new(0.0, 1.2, 4.0), 2.5),
            Err(Reject::OutOfReach)
        );
    }
}
