use std::fmt::{self, Debug, Display};

use chrono::{serde::ts_milliseconds, DateTime, Duration, Utc};
use serde::{de::Error, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{base64::Base64, serde_as};
use thiserror::Error;

pub mod rng;

#[derive(Debug)]
pub enum DecodeError {
    LzStrDecodeError,
    JsonDecodeError(serde_json::Error),
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Replay {
    #[serde(rename = "c")]
    pub metadata: Metadata,
    #[serde(rename = "d")]
    #[serde_as(as = "Base64")]
    pub data: Vec<u8>, // TODO
}

impl Replay {
    pub fn time(&self) -> Duration {
        self.metadata.game_end - self.metadata.game_start
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Metadata {
    #[serde(rename = "softDropId")]
    pub soft_drop_id: SoftDropSpeed,

    #[serde(rename = "gameStart")]
    #[serde(with = "ts_milliseconds")]
    pub game_start: DateTime<Utc>,
    #[serde(rename = "gameEnd")]
    #[serde(with = "ts_milliseconds")]
    pub game_end: DateTime<Utc>,

    pub seed: GameSeed,

    #[serde(rename = "bs")]
    pub block_skin: BlockSkin,

    #[serde(rename = "se")]
    pub sound_effects: SoundEffects,

    /// Delayed Auto Shift
    ///
    /// See [here](https://tetris.wiki/DAS).
    #[serde(default)]
    pub das: u16, // jstris allows [0, 4999]

    /// Auto Repeat Rate
    #[serde(default)]
    pub arr: u16, // jstris allows [0, 4999]

    #[serde(rename = "m")]
    pub game_mode: GameMode, // ???

    #[serde(rename = "v")]
    pub version: ExpectedJstrisReplayVersion<3, 3>, // TODO: version; we assume 3.3?
    // 40L mode
    pub r: Option<u16>, // ???

                        // todo: bbs? big blocks?
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ExpectedJstrisReplayVersion<const MAJOR: u8, const MINOR: u8>;

impl<const MAJ: u8, const MIN: u8> Debug for ExpectedJstrisReplayVersion<MAJ, MIN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f
            .debug_struct("JstrisReplayVersion")
            .field("major_ver", &MAJ)
            .field("minor_ver", &MIN)
            .finish()
    }
}


impl<const MAJ: u8, const MIN: u8> Display for ExpectedJstrisReplayVersion<MAJ, MIN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{MAJ}.{MIN}")
    }
}

impl<const MAJ: u8, const MIN: u8> ExpectedJstrisReplayVersion<MAJ, MIN> {
    pub const fn version(self) -> (u8, u8) {
        (MAJ, MIN)
    }
}

impl<const MAJ: u8, const MIN: u8> Serialize for ExpectedJstrisReplayVersion<MAJ, MIN> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let maj = MAJ as f32;
        let mut min = MIN as f32;
        while min >= 1. {
            min /= 10.;
        }

        serializer.serialize_f32(maj + min)
    }
}

impl<'de, const MAJ: u8, const MIN: u8> Deserialize<'de> for ExpectedJstrisReplayVersion<MAJ, MIN> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ver = f32::deserialize(deserializer)?;

        // bleh
        let ver = format!("{ver}");
        let (maj, min) = ver.split_once('.').ok_or_else(|| D::Error::custom("invalid version number"))?;

        let maj: u8 = maj.parse().map_err(D::Error::custom)?;
        let min: u8 = min.parse().map_err(D::Error::custom)?;

        if maj != MAJ {
            return Err(<D::Error as serde::de::Error>::custom(format!(
                "expected major version {MAJ}, got major version {maj} in `{ver}`"
            )));
        }
        if min != MIN {
            return Err(<D::Error as serde::de::Error>::custom(format!(
                "expected minor version {MIN}, got minor version {min} in `{ver}`"
            )));
        }

        Ok(Self)
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize_repr, Deserialize_repr,
)]
#[repr(u16)]
pub enum BlockSkin {
    SolidColor = 0,
    // Invisible = 1,  // not exposed via replay
    // Monochrome = 2, // not exposed via replay
    /// https://s.jezevec10.com/res/b1.png
    Bevel = 1,
    /// https://s.jezevec10.com/res/b2.png
    BevelFlat = 2,
    /// https://s.jezevec10.com/res/b3.png
    ThinBorder = 3,
    /// https://s.jezevec10.com/res/b4.png
    Gradient = 4,
    /// https://s.jezevec10.com/res/b8.png
    Bubble = 8,
    /// https://s.jezevec10.com/res/b9.png
    Pointy = 9,
    /// https://s.jezevec10.com/res/b10.png
    Rounded = 10,
    /// https://s.jezevec10.com/res/b11.png
    PictureFrame = 11,
    /// https://s.jezevec10.com/res/b12.png
    BevelRounded = 12,
    /// https://s.jezevec10.com/res/b13.png
    Cats = 13,
}
// TODO: missing numbers in the above?
// TODO: inline images above!

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize_repr,
    Deserialize_repr,
    Default,
)]
#[repr(u8)]
pub enum SoundEffects {
    Nullpomino = 0,
    Yotipo = 1,
    Rainforest = 2,
    TetraX = 3,
    #[default]
    None = 4,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum SoftDropSpeed {
    Slow = 0,
    Medium = 1,
    Fast = 2,
    Ultra = 3,
    Instant = 4,
}

impl SoftDropSpeed {
    // https://harddrop.com/forums/index.php?showtopic=7087&st=135&p=92057&#entry92057
    pub const fn steps(self) -> u8 {
        use SoftDropSpeed::*;

        match self {
            Slow | Medium => 0,
            Fast => 1,
            Ultra => 2,
            Instant => 20,
        }
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum GameMode {
    // TODO: non-sprint modes?
    // See: https://harddrop.com/forums/index.php?showtopic=7087&st=135&p=92057&#entry92057
    _40Line = 1,
    _20Line = 2,
    _100Line = 3,
    _1000Line = 4,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameSeed {
    bytes: [u8; 6],
    len: u8,
}

#[derive(Debug, Error)]
pub enum GameSeedParseError {
    #[error("expected 6 or fewer (but not zero) bytes in the string")]
    WrongLength,
    #[error("{} is not alphanum", *c as char)]
    InvalidChar { c: u8 },
}

impl TryFrom<&str> for GameSeed {
    type Error = GameSeedParseError;

    fn try_from(str: &str) -> Result<Self, Self::Error> {
        if str.len() > 6 || str.is_empty() {
            return Err(GameSeedParseError::WrongLength);
        }

        let mut out = [0; 6];

        for (c, &b) in out.iter_mut().zip(str.as_bytes().iter()) {
            match b {
                b'a'..=b'z' | b'0'..=b'9' => *c = b,
                _ => return Err(GameSeedParseError::InvalidChar { c: b }),
            }
        }

        Ok(GameSeed {
            bytes: out,
            len: str.len() as u8,
        })
    }
}

impl AsRef<[u8]> for GameSeed {
    fn as_ref(&self) -> &[u8] {
        &self.bytes[0..(self.len as usize)]
    }
}

impl AsRef<str> for GameSeed {
    fn as_ref(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.bytes.as_ref()) }
    }
}

impl Debug for GameSeed {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{self}")
    }
}

impl Display for GameSeed {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "\"")?;
        for &c in AsRef::<[u8]>::as_ref(&self.bytes) {
            write!(fmt, "{}", c as char)?
        }
        write!(fmt, "\"")
    }
}

impl<'de> Deserialize<'de> for GameSeed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .as_str()
            .try_into()
            .map_err(D::Error::custom)
    }
}

impl Serialize for GameSeed {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(unsafe { std::str::from_utf8_unchecked(self.as_ref()) })
    }
}

pub fn decode_uri_string(replay_uri_string: impl AsRef<[u8]>) -> Result<Replay, DecodeError> {
    let bytes = replay_uri_string.as_ref();
    let compressed = bytes.iter().copied().map(u32::from).collect::<Vec<_>>();

    let str = lz_str::decompress_uri(&compressed).ok_or(DecodeError::LzStrDecodeError)?;
    decode_json(str)
}

pub fn decode_json(json: impl AsRef<str>) -> Result<Replay, DecodeError> {
    serde_json::from_str::<Replay>(json.as_ref()).map_err(DecodeError::JsonDecodeError)
}

pub fn encode_uri_string(replay: &Replay) -> Result<String, serde_json::Error> {
    let json = serde_json::to_string(replay)?;
    let vec = lz_str::compress_uri(&json);
    Ok(vec.iter().map(|c| char::try_from(*c).unwrap()).collect())
}

// TODO: roundtrip tests
