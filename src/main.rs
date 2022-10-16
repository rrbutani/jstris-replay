use std::{
    env::args,
    error::Error,
    fs::File,
    io::{BufRead, BufReader},
};

use chrono::{DateTime, NaiveDateTime, Utc};
use jstris_replay_re::{
    decode_uri_string, encode_uri_string, BlockSkin, Metadata, Replay, SoftDropSpeed,
    SoundEffects,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + 'static>> {
    for arg in args().skip(1) {
        let res = if let Some(replay_id) = arg.strip_prefix("replay:") {
            reqwest::get(format!(
                "https://jstris.jezevec10.com/replay/data?id={replay_id}&type=0"
            ))
            .await?
            .json()
            .await?
        } else {
            let f = File::open(&arg)?;
            let mut f = BufReader::new(f);
            let mut s = String::new();
            f.read_line(&mut s)?;

            decode_uri_string(s.as_bytes()).unwrap()
        };

        println!("{arg}: [{}] {res:#?}", res.time());

        let mut rng = jstris_replay_re::rng::JstrisBag::new(res.metadata.seed);

        for piece in rng.iter().take(50) {
            println!("{piece:?}")
        }
    }
    // let replay = Replay {
    //     metadata: Metadata {
    //         soft_drop_id: SoftDropSpeed::Instant,
    //         game_start: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
    //         game_end: DateTime::from_utc(NaiveDateTime::from_timestamp(60, 0), Utc),
    //         seed: "8bf82p".try_into().unwrap(),
    //         block_skin: BlockSkin::Bevel,
    //         sound_effects: SoundEffects::default(),
    //         das: 100,
    //         arr: 0,
    //         v: 3.3,
    //         m: 1,
    //         r: 1,
    //     },
    //     data: vec![ // needs to be a multiple of 4
    //         // 20, 241, 38, 103,
    //         // 0b0001_0100, 0b1111_0001, 0b0010_0110, 0b0110_0111,
    //         0b0000_0000, 0b0000_0000, 0b0000_0000, 0b0001_1111,
    //         /*
    //             0..3

    //             4..7

    //             8..11

    //             12..15 next ev?
    //               - left/right?
    //                 + 0000: left two?
    //                 + 0001: center
    //                 + 0010: left
    //                 + 0011: right
    //                 + 0100: rot left
    //                 + 0101: rot right
    //                 + 0110: 180?
    //                 + 0111: double drop?
    //             16..27: timestamp
    //               - 12 bits, units of ms
    //               - special case for 0xFF_F? continuation, no event in tick?
    //             28..31: curr ev?
    //               - list:
    //                 + 0000: left 2?
    //                 + 0001: nothing?
    //                 + 0010: das left
    //                 + 0011: das right
    //                 + 0100: rot left
    //                 + 0101: rot right
    //                 + 0110: flip
    //                 + 0111: left 1, place
    //                 + 1000: left 1?
    //                 + 1001: left 1?
    //                 + 1010: swap
    //                 + 1011: left 1?
    //                 + 1100: new line from below
    //                 + 1101: left 1?
    //                 + 1110: nothing?
    //                 + 1111: left 1?

    //                 from jstris source code:
    //                     MOVE_LEFT: 0,
    //                     MOVE_RIGHT: 1,
    //                     DAS_LEFT: 2,
    //                     DAS_RIGHT: 3,
    //                     ROTATE_LEFT: 4,
    //                     ROTATE_RIGHT: 5,
    //                     ROTATE_180: 6,
    //                     HARD_DROP: 7,
    //                     SOFT_DROP_BEGIN_END: 8,
    //                     GRAVITY_STEP: 9,
    //                     HOLD_BLOCK: 10,
    //                     GARBAGE_ADD: 11,
    //                     SGARBAGE_ADD: 12,
    //                     REDBAR_SET: 13,
    //                     ARR_MOVE: 14,
    //                     AUX: 15

    //         */


    //         // 39, 46, 150, 48,
    //         // 164, 64, 39, 178,
    //         // 40, 247, 23, 150,
    //         // 89, 145, 218, 196,

    //         // 95, 201, 225, 212,
    //         // 36, 5, 36, 88,
    //         // 102, 179, 179, 121,
    //         // 217, 232, 74, 137,
    //         // 123, 34, 28, 34,
    //         // 76, 66, 13, 135,
    //         // 125, 207, 30, 20,
    //         // 78, 110, 239, 63,
    //         // 119, 180, 223, 184,
    //         // 123, 229, 99, 241,
    //         // 40, 5, 192, 24,
    //         // 220, 27, 132, 37,
    //         // 71, 44, 49, 50,
    //         // 36, 59, 192, 67,
    //         // 116, 69, 190, 42,

    //         // 93, 46, 216, 47, 26, 55, 139, 185, 208, 188, 146, 73, 203, 204, 168, 207,

    //         // 242, 81, 63, 112, 248, 49, 241, 243, 0, 115, 233, 53, 95, 189, 188, 30, 77, 95, 42,
    //         // 159, 229, 32, 116, 0, 232, 130, 14, 130, 46, 4, 188, 229, 74, 38, 56, 135, 150, 8, 195,
    //         // 196, 212, 5, 120, 117, 120, 226, 232, 10, 236, 35, 51, 117, 157, 186, 212, 46, 213, 61,
    //         // 113, 33, 115, 244, 147, 74, 152, 17, 155, 196, 161, 201, 177, 119, 181, 225, 185, 4,
    //         // 222, 96, 228, 87, 231, 145, 244, 167, 244, 174, 252, 144, 1, 122, 5, 112, 138, 82, 15,
    //         // 147, 146, 120, 20, 234, 23, 216, 28, 10, 43, 148, 172, 125, 47, 248, 52, 128, 181, 186,
    //         // 62, 24, 65, 211, 196, 24, 197, 234, 72, 191, 101, 156, 40, 3, 148, 35, 202, 75, 122,
    //         // 83, 229, 84, 141, 109, 38, 99, 142, 206, 191, 6, 143, 119, 39, 153, 7, 248, 208, 26,
    //         // 128, 141, 202, 79, 202, 85, 58, 129, 130, 186, 34, 200, 11, 42, 139, 123, 245, 211,
    //         // 157, 214, 250, 248, 98, 253, 73, 33, 193, 59, 111, 69, 161, 79, 9, 82, 67, 107, 129,
    //         // 108, 169, 127, 65, 129, 169, 146, 28, 205, 151, 206, 62, 104, 192, 236, 202, 109, 95,
    //         // 118, 243, 189, 50, 254, 90, 31, 32, 130, 4, 2, 180, 224,
    //     ],
    // };
    // let replay_str = encode_uri_string(&replay)?;
    // println!("{replay_str}");

    Ok(())
}
