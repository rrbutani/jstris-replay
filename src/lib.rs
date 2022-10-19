use std::{
    fmt::{self, Debug, Display},
    hash::Hash,
};

use chrono::{serde::ts_milliseconds, DateTime, Duration, Utc};
use derive_more::{Deref, DerefMut};
use serde::{de::Error, Deserialize, Serialize, ser::SerializeStruct};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{base64::Base64, serde_as, ser::SerializeAsWrap};
use thiserror::Error;

pub mod rng;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("error encountered when decoding the URI encoded LZ string")]
    LzStrDecodeError,
    #[error("error encountered when decoding the embedded JSON data: {}", .0)]
    JsonDecodeError(serde_json::Error),
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct JstrisReplay {
    #[serde(rename = "c")]
    pub metadata: Metadata,
    #[serde(rename = "d")]
    #[serde_as(as = "Base64")]
    pub data: EventList, // TODO
}

// Manual serialize impl because `EventList` needs explicit encoding –– can't
// provide an `AsRef<[u8]>` impl (see notes below about the Right Way to do
// this).
impl Serialize for JstrisReplay {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut s = serializer.serialize_struct("Replay", 2)?;

        s.serialize_field("c", &self.metadata)?;
        let data = self.data.encode();
        s.serialize_field("d", &SerializeAsWrap::<_, Base64>::new(&data))?;

        s.end()
    }
}

impl JstrisReplay {
    pub fn time(&self) -> Duration {
        self.metadata.game_end - self.metadata.game_start
    }
}

// TODO: do this the Right Way: switch to having the in memory repr just be a
// raw vec of `u8`s and do the translation to/from on "field" access
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct EventList {
    inner: Vec<Event>,
}

impl EventList {
    // TODO: method on `JstrisReplay` that turns these durations into real times
    // by adding the durations to the game start time
    pub fn iter(&self) -> impl Iterator<Item = (Input, Duration)> + '_ {
        let mut base = Duration::milliseconds(0);
        let mut prev = 0;

        self.inner.iter().map(move |&Event { timestamp, input }| {
            if timestamp.millis() < prev {
                // eprint!("{prev} -> {timestamp}; jumping base from {base} to:");
                base = base + Duration::milliseconds(0x1000);
                // eprintln!(" {base}");
            }
            prev = timestamp.millis();

            (input, base + Duration::milliseconds(timestamp.millis() as _))
        })
    }
}

impl EventList {
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.inner.capacity() * 2);

        v.extend(
            self.inner
                .iter()
                .flat_map(|&e| Into::<u16>::into(e).to_be_bytes()),
        );

        if self.inner.len() % 2 == 1 {
            v.extend([ 0, 0 ]);
            // TODO: do we really need to pad here?
        }

        v
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
pub enum EventListParseError {
    #[error("events are four bytes each; got {num_bytes} bytes which is not a multiple of 4")]
    NotAligned { num_bytes: usize },
    #[error("error decoding event: {}", .0)]
    EventDecodeError(#[from] EventDecodeError),
}

impl TryFrom<Vec<u8>> for EventList {
    type Error = EventListParseError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        /*
        // debug_assert_eq!(std::mem::size_of::<Event>(), 4);

        // if bytes.len() % 4 != 0 {
        //     return Err(EventListParseError::NotAligned { num_bytes: bytes.len() })
        // }

        // // We want to reuse the `bytes` Vec instead of making a copy.
        // let elems = bytes.len() / 4;

        // // We're going to do an unsafe cast to produce a `Vec<Event>`; to do
        // // this we have to make sure the `byte` vec's underlying allocation is
        // // a multiple of 4 bytes.
        // bytes.shrink_to_fit();
        // if bytes.capacity() != elems * 4 { panic!("couldn't shrink the bytes vector!"); }

        // let (ptr, len, cap) = bytes.into_raw_parts();
        // TODO: store a raw pointer instead and reconstruct the vector on drop
        // the above is not sound because the alignment of `Event` isn't the
        // same as `u8` so we can't just dealloc as normal.
        */

        // TODO: do we really need a multiple of 4 bytes (i.e. pairs of events)?

        if bytes.len() % 4 != 0 {
            return Err(EventListParseError::NotAligned {
                num_bytes: bytes.len(),
            });
        }

        let inner = bytes
            // .array_chunks() // not stable yet
            .chunks(2)
            .map(|arr| arr.try_into().unwrap())
            .map(u16::from_be_bytes)
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;

        // Ok(EventList { inner, _encoded: RefCell::new(None) })
        Ok(EventList { inner })
    }
}

// impl DerefMut for EventList {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         // Drop the cached encoded form if there's potential the actual data
//         // will be modified:
//         self._encoded.get_mut().take();
//         &mut self.inner
//     }
// }

// impl AsRef<[u8]> for EventList {
//     fn as_ref(&self) -> &[u8] {
//         if let Some(inner) = *self._encoded.borrow() {
//             inner
//         } else {
//             *self._encoded.borrow_mut() = Some(self.encode());
//             self._encoded.borrow()
//         }
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Event {
    timestamp: TwelveBitMillisecondTimestamp,
    input: Input,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
pub enum EventDecodeError {}

impl TryFrom<u16> for Event {
    type Error = EventDecodeError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let delay = value >> 4;
        let input = (value & 0x0F) as u8;

        // TODO: validation

        Ok(Event {
            timestamp: delay.try_into().unwrap(),
            input: Input::from_raw(input),
        })
    }
}

impl From<Event> for u16 {
    fn from(ev: Event) -> u16 {
        ev.timestamp.millis() << 4 | (ev.input as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Input {
    MoveLeft = 0,
    MoveRight = 1,
    DasLeft = 2,
    DasRight = 3,
    RotateLeft = 4,
    RotateRight = 5,
    Rotate180 = 6,
    HardDrop = 7,
    SoftDropBeginEnd = 8,
    GravityStep = 9,
    HoldBlock = 10,
    GarbageAdd = 11,
    SGarbageAdd = 12,
    RedBarSet = 13,
    ArrMove = 14,
    Aux = 15,
}

// impl Display for Input {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         use Input::*;
//         match self {
//             MoveLeft => "",
//             MoveRight => todo!(),
//             DasLeft => todo!(),
//             DasRight => todo!(),
//             RotateLeft => todo!(),
//             RotateRight => todo!(),
//             Rotate180 => todo!(),
//             HardDrop => todo!(),
//             SoftDropBeginEnd => todo!(),
//             GravityStep => todo!(),
//             HoldBlock => todo!(),
//             GarbageAdd => todo!(),
//             SGarbageAdd => todo!(),
//             RedBarSet => todo!(),
//             ArrMove => todo!(),
//             Aux => todo!(),
//         }
//     }
// }

impl Input {
    #[inline]
    pub fn from_raw(raw: u8) -> Self {
        assert!(raw & 0xF0 == 0);
        // debug_assert!(disc) variant_count == 16

        unsafe { core::mem::transmute(raw) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum AuxInput {
    Afk = 0,
    BlockSet = 1,
    MoveTo = 2,
    Randomizer = 3,
    MatrixMod = 4,
    WideGarbageMod = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TwelveBitMillisecondTimestamp(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum TwelveBitMillisecondTimestampConversionError<Source: Display = u16> {
    TooBig { duration: Source },
    Invalid { duration: Source },
}

impl TryFrom<Duration> for TwelveBitMillisecondTimestamp {
    type Error = TwelveBitMillisecondTimestampConversionError<Duration>;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        use TwelveBitMillisecondTimestampConversionError as E;

        match duration.num_milliseconds() {
            val if val.is_positive() => Ok(Self(val as u16)),
            _ => Err(E::Invalid { duration }),
        }
    }
}

impl From<TwelveBitMillisecondTimestamp> for Duration {
    fn from(timestamp: TwelveBitMillisecondTimestamp) -> Duration {
        Duration::milliseconds(timestamp.0 as _)
    }
}

impl TryFrom<u16> for TwelveBitMillisecondTimestamp {
    type Error = TwelveBitMillisecondTimestampConversionError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        use TwelveBitMillisecondTimestampConversionError as E;

        match value {
            0..=0x0F_FF => Ok(Self(value)),
            _ => Err(E::TooBig { duration: value }),
        }
    }
}

impl TwelveBitMillisecondTimestamp {
    pub const fn millis(self) -> u16 {
        self.0
    }
}

impl Display for TwelveBitMillisecondTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Into::<Duration>::into(*self))
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
    pub version: ExpectedJstrisReplayVersion<3, 0>, // we're compatible with 3.0 and up (tested through 3.3)

    pub r: Option<u16>, // ???

    // todo: bbs? big blocks?
    pub bbs: Option<u8>, // todo: this should actually be a bool on our end but
                         // not on the wire?
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpectedJstrisReplayVersion<const MAJOR: u8 = 3, const MINOR: u8 = 3> {
    actual_minor: u8,
}

impl<const MAJ: u8, const MIN: u8> Debug for ExpectedJstrisReplayVersion<MAJ, MIN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JstrisReplayVersion")
            .field("major_ver", &MAJ)
            .field("minor_ver", &self.actual_minor)
            .finish()
    }
}

impl<const MAJ: u8, const MIN: u8> Display for ExpectedJstrisReplayVersion<MAJ, MIN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{MAJ}.{}", self.actual_minor)
    }
}

impl<const MIN: u8> Default for ExpectedJstrisReplayVersion<3, MIN> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const MIN: u8> ExpectedJstrisReplayVersion<3, MIN> {
    pub const fn new() -> Self {
        Self { actual_minor: 3 }
    }
}

impl<const MAJ: u8, const MIN: u8> ExpectedJstrisReplayVersion<MAJ, MIN> {
    pub const fn version(self) -> (u8, u8) {
        (MAJ, self.actual_minor)
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
        let (maj, min) = if let Some(pair) = ver.split_once('.') {
            pair
        } else {
            (&*ver, "0")
        };

        let maj: u8 = maj.parse().map_err(D::Error::custom)?;
        let min: u8 = min.parse().map_err(D::Error::custom)?;

        if maj != MAJ {
            return Err(<D::Error as serde::de::Error>::custom(format!(
                "expected major version {MAJ}, got major version {maj} in version number `{ver}`"
            )));
        }
        if !(min >= MIN) {
            return Err(<D::Error as serde::de::Error>::custom(format!(
                "expected minor version {MIN}, got minor version {min} in version number `{ver}`"
            )));
        }

        Ok(Self { actual_minor: min })
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

pub fn decode_uri_string(replay_uri_string: impl AsRef<[u8]>) -> Result<JstrisReplay, DecodeError> {
    let bytes = replay_uri_string.as_ref();
    let compressed = bytes.iter().copied().map(u32::from).collect::<Vec<_>>();

    let str = lz_str::decompress_uri(&compressed).ok_or(DecodeError::LzStrDecodeError)?;
    decode_json(str)
}

pub fn decode_json(json: impl AsRef<str>) -> Result<JstrisReplay, DecodeError> {
    serde_json::from_str::<JstrisReplay>(json.as_ref()).map_err(DecodeError::JsonDecodeError)
}

pub fn encode_uri_string(replay: &JstrisReplay) -> Result<String, serde_json::Error> {
    let json = serde_json::to_string(replay)?;
    let vec = lz_str::compress_uri(&json);
    Ok(vec.iter().map(|c| char::try_from(*c).unwrap()).collect())
}

// TODO: roundtrip tests
