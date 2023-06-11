use std::{
    collections::HashMap,
    env::args,
    error::Error,
    fs::File,
    io::{BufRead, BufReader},
};

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use jstris_replay_re::{
    decode_uri_string, encode_uri_string, BlockSkin, ExpectedJstrisReplayVersion, GameMode,
    JstrisReplay, Metadata, SoftDropSpeed, SoundEffects, decode_json,
};
use soup::{NodeExt, QueryBuilderExt};

struct JstrisLeaderboardIter {
    remaining: Vec<u32>, // replay ids, reverse order! (worst ... best)
    next_page: String,   // worst time seen so far..
}

impl JstrisLeaderboardIter {
    fn new() -> Self {
        Self {
            remaining: Vec::with_capacity(200),
            next_page: "0.0".to_string(),
        }
    }

    async fn next(&mut self) -> reqwest::Result<Option<String>> {
        let next = if let Some(next) = self.remaining.pop() {
            next
        } else {
            // grab the next page!
            let page = reqwest::get(format!(
                "https://jstris.jezevec10.com/sprint?lines=40L&page={}",
                self.next_page
            ))
            .await?
            .text()
            .await?;

            let soup = soup::Soup::new(&page);
            let m = soup
                .tag("a")
                .attr("target", "_blank")
                .find_all()
                .map(|x| {
                    let link = x.get("href").unwrap();
                    (x, link)
                })
                .filter(|(_, link)| link.contains("replay"))
                .map(|(elem, link)| {
                    let siblings = elem
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .tag("td")
                        .find_all()
                        .collect::<Vec<_>>();
                    let time = siblings[2].tag("strong").find().unwrap().text();

                    let replay_id = link
                        .strip_prefix("https://jstris.jezevec10.com/replay/")
                        .unwrap()
                        .to_string();

                    (time, replay_id)
                })
                .collect::<Vec<_>>();

            let last = m.last().unwrap();
            self.next_page = last.0.clone();


            let iter = m.into_iter()
                .rev()
                .map(|(_, replay_id)| replay_id.parse::<u32>().unwrap());

            self.remaining.extend(iter);

            println!("got next page of leaderboard: {} entries", self.remaining.len());
            self.remaining.pop().unwrap()
        };

        Ok(Some(format!("replay:{next}")))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + 'static>> {
    decode_json(r#"{"c":{"v":3.3,"softDropId":4,"gameStart":1684543650931,"gameEnd":1684543666545,"seed":"c07yl8j","m":1,"bs":0,"se":0,"das":83,"r":0},"d":"AeAD5wcyDacP0BQ3FRIWWhZSGVUZUhwXHZEi4yRXJFMmeiZzKRAsRyy6LdEuJjMTOFc61T4nQBFFU0nHS+RQZ1CxVgNYMFlXWvpcRlzRYhNkF2WgaHVq8mz3bZputHKAdId3wnv3e\/J+NoK3hZGK0433jfOU15aanRGdhaIXqHeqKq31rvCyJ7UgulK+J74iv7q\/ssenyeXNp88m0IHVs9fX2Yrc8d1l4PfjdORw6bfr1fAn8jH3c\/in+KP6ivqD\/iAAlwDWAyEIYwp3CnMLwBECEtcU5BfhGucc9CEwI6coRSqXLoAz0jXhO8c9mkAkQyBIYk73XEZc8GJCZLFpx221bjFxV3OReNN8B3wDgaeCuoVWigeMEJFik1GVJ5cwmgWccp5Hn7ahQaaDqoeuMLJHtCW397sRwFPDp8W0x8HKp8zx0jPWt9qF2oHeF98g44XkYufH58LqdO3R8Ify2vPk+AD8RwBVAecEOgZAC5IPdxHxEpUWlx53IOAmMiyHLso25zlkPVBAJ0NBQ1ZIg0vnTfpPQFSCWRdaQVrlX4dgoWXjaCdoI2paalNvYHFHc1p0snZ1dpJ593qmfdGDE4U3hqSJgIzHjvGUQ5cnlyOcl57Kn\/Cj9aVSqfep8q4RrrWyF7QatkW2QbvHvaq\/kMTSyMXLl9Gn1iTcB+E15EflsOnF6wLup+\/28ZH2w\/dX91P5evlzAGcAYwJKAkMIRwhDCUoJQwpWEbcTkBjSHTce8R9VI6ck0CoyLWcvATRTNec14ziqOKM\/xz\/DQRpBE0HmSadMFFPXX8Ff5WNXapdx4HYXeGZ60IEXhJCJ4o+3lPGVFZjXndCgB6Pgp3WpIq0nsqGzBbXXu5e\/BsSHx9rH4MkkzRLO0c\/n\/\/A="}"#).unwrap();

    // for arg in args().skip(1) {
    let mut replays = JstrisLeaderboardIter::new();
    while let Some(arg) = replays.next().await? {
        let res = if let Some(replay_id) = arg.strip_prefix("replay:") {
            println!("fetching replay: {replay_id}...");
            reqwest::get(format!(
                "https://jstris.jezevec10.com/replay/data?id={replay_id}&type=0"
                // "https://jstris.jezevec10.com/replay/data?id=70293904&type=0"
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

        if res.metadata.arr != 0 {
            println!("non-zero arr! ({arg})");
        } else {
            continue;
        }

        println!("{arg}: [{}] {res:#X?}", res.time());

        let mut prev = Duration::milliseconds(0);
        let fps = 30;
        let mut total_err = Duration::milliseconds(0);

        let mut frame_freq = HashMap::<_, usize>::new();
        let mut input_freq = HashMap::<_, usize>::new();

        for (inp, ts) in res.data.iter() {
            let diff = ts - prev;
            // let frames = (diff / (1000 / fps)).num_milliseconds();
            let frames = (diff * fps / 1000).num_milliseconds();
            let err = diff - Duration::milliseconds(((frames as i32) * 1000 / fps) as _);

            let frames = if err.num_milliseconds() > (1000 / fps / 2) as _ {
                frames + 1
            } else {
                frames
            };

            let err = diff - Duration::milliseconds(((frames as i32) * 1000 / fps) as _);

            total_err = total_err + err;
            prev = ts;
            println!("  @{ts} [+{diff:7}, {frames:02}f e:{err}]: {inp:?}");

            *frame_freq.entry(frames).or_default() += 1;
            *input_freq.entry(inp).or_default() += 1;
        }
        println!("accumulated drift when mapping to frames: {total_err}");
        println!(
            "observed elapsed time: {} vs recorded: {} (err: {})",
            prev,
            res.time(),
            res.time() - prev
        );

        let mut frame_freq: Vec<_> = frame_freq.into_iter().collect();
        frame_freq.sort_by_key(|(_v, f)| *f);
        println!("\nframe delays by frequency:");
        for (v, f) in frame_freq.iter().rev() {
            println!("  - {v:2} frames: {f:3}");
        }

        let mut input_freq: Vec<_> = input_freq.into_iter().collect();
        input_freq.sort_by_key(|(_i, f)| *f);
        println!("\ninputs by frequency:");
        for (i, f) in input_freq.iter().rev() {
            println!("  - {i: >15?}: {f:3}");
        }

        let bits = {
            let bits_for_frame = frame_freq.len().next_power_of_two().trailing_zeros();
            let bits_for_input = input_freq.len().next_power_of_two().trailing_zeros();
            let len = res.data.len();

            println!("\nnaïve: {bits_for_frame} bits for frame, {bits_for_input} bits for input, {len} events");
            (bits_for_frame + bits_for_input) * (len as u32)
        };
        println!(
            "  - {bits} bits, {} bytes",
            bits / 8 + if bits % 8 == 0 { 0 } else { 1 }
        );

        // let mut rng = jstris_replay_re::rng::JstrisBag::new(res.metadata.seed);

        // for piece in rng.iter().take(50) {
        //     println!("{piece:?}")
        // }
    }

    return Ok(());

    let replay = JstrisReplay {
        metadata: Metadata {
            soft_drop_id: SoftDropSpeed::Instant,
            game_start: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            game_end: DateTime::from_utc(NaiveDateTime::from_timestamp(60, 0), Utc),
            seed: "8bf82p".try_into().unwrap(),
            block_skin: BlockSkin::Bevel,
            sound_effects: SoundEffects::default(),
            das: 100,
            arr: 0,
            version: ExpectedJstrisReplayVersion::new(),
            game_mode: GameMode::_40Line,
            r: Some(1),
            bbs: None,
        },
        data: vec![
            // needs to be a multiple of 4
            // 20, 241, 38, 103,
            // 0b0001_0100, 0b1111_0001, 0b0010_0110, 0b0110_0111,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0001_1111,
            /*
                0..3

                4..7

                8..11

                12..15 next ev?
                  - left/right?
                    + 0000: left two?
                    + 0001: center
                    + 0010: left
                    + 0011: right
                    + 0100: rot left
                    + 0101: rot right
                    + 0110: 180?
                    + 0111: double drop?
                16..27: timestamp
                  - 12 bits, units of ms
                  - special case for 0xFF_F? continuation, no event in tick?
                28..31: curr ev?
                  - list:
                    + 0000: left 2?
                    + 0001: nothing?
                    + 0010: das left
                    + 0011: das right
                    + 0100: rot left
                    + 0101: rot right
                    + 0110: flip
                    + 0111: left 1, place
                    + 1000: left 1?
                    + 1001: left 1?
                    + 1010: swap
                    + 1011: left 1?
                    + 1100: new line from below
                    + 1101: left 1?
                    + 1110: nothing?
                    + 1111: left 1?

                    from jstris source code:
                        MOVE_LEFT: 0,
                        MOVE_RIGHT: 1,
                        DAS_LEFT: 2,
                        DAS_RIGHT: 3,
                        ROTATE_LEFT: 4,
                        ROTATE_RIGHT: 5,
                        ROTATE_180: 6,
                        HARD_DROP: 7,
                        SOFT_DROP_BEGIN_END: 8,
                        GRAVITY_STEP: 9,
                        HOLD_BLOCK: 10,
                        GARBAGE_ADD: 11,
                        SGARBAGE_ADD: 12,
                        REDBAR_SET: 13,
                        ARR_MOVE: 14,
                        AUX: 15

                    AUX:
                        AFK: 0,
                        BLOCK_SET: 1,
                        MOVE_TO: 2,
                        RANDOMIZER: 3,
                        MATRIX_MOD: 4,
                        WIDE_GARBAGE_ADD: 5


            */
            // 39, 46, 150, 48,
            // 164, 64, 39, 178,
            // 40, 247, 23, 150,
            // 89, 145, 218, 196,

            // 95, 201, 225, 212,
            // 36, 5, 36, 88,
            // 102, 179, 179, 121,
            // 217, 232, 74, 137,
            // 123, 34, 28, 34,
            // 76, 66, 13, 135,
            // 125, 207, 30, 20,
            // 78, 110, 239, 63,
            // 119, 180, 223, 184,
            // 123, 229, 99, 241,
            // 40, 5, 192, 24,
            // 220, 27, 132, 37,
            // 71, 44, 49, 50,
            // 36, 59, 192, 67,
            // 116, 69, 190, 42,

            // 93, 46, 216, 47, 26, 55, 139, 185, 208, 188, 146, 73, 203, 204, 168, 207,

            // 242, 81, 63, 112, 248, 49, 241, 243, 0, 115, 233, 53, 95, 189, 188, 30, 77, 95, 42,
            // 159, 229, 32, 116, 0, 232, 130, 14, 130, 46, 4, 188, 229, 74, 38, 56, 135, 150, 8, 195,
            // 196, 212, 5, 120, 117, 120, 226, 232, 10, 236, 35, 51, 117, 157, 186, 212, 46, 213, 61,
            // 113, 33, 115, 244, 147, 74, 152, 17, 155, 196, 161, 201, 177, 119, 181, 225, 185, 4,
            // 222, 96, 228, 87, 231, 145, 244, 167, 244, 174, 252, 144, 1, 122, 5, 112, 138, 82, 15,
            // 147, 146, 120, 20, 234, 23, 216, 28, 10, 43, 148, 172, 125, 47, 248, 52, 128, 181, 186,
            // 62, 24, 65, 211, 196, 24, 197, 234, 72, 191, 101, 156, 40, 3, 148, 35, 202, 75, 122,
            // 83, 229, 84, 141, 109, 38, 99, 142, 206, 191, 6, 143, 119, 39, 153, 7, 248, 208, 26,
            // 128, 141, 202, 79, 202, 85, 58, 129, 130, 186, 34, 200, 11, 42, 139, 123, 245, 211,
            // 157, 214, 250, 248, 98, 253, 73, 33, 193, 59, 111, 69, 161, 79, 9, 82, 67, 107, 129,
            // 108, 169, 127, 65, 129, 169, 146, 28, 205, 151, 206, 62, 104, 192, 236, 202, 109, 95,
            // 118, 243, 189, 50, 254, 90, 31, 32, 130, 4, 2, 180, 224,
        ]
        .try_into()
        .unwrap(),
    };
    let replay_str = encode_uri_string(&replay)?;
    println!("{replay_str}");

    Ok(())
}
