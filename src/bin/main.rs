use std::env;
use std::fs;

use main_error::MainError;
use serde::{Deserialize, Serialize};
use tf_demo_parser::demo::data::ServerTick;
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::Analyser;
use tf_demo_parser::demo::parser::analyser::MatchState;
use tf_demo_parser::demo::parser::player_summary_analyzer::PlayerSummaryAnalyzer;
pub use tf_demo_parser::{Demo, DemoParser, Parse, ParseError, ParserState, Stream};

#[cfg(feature = "jemallocator")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonDemo {
    header: Header,
    #[serde(flatten)]
    state: MatchState,
}

fn main() -> Result<(), MainError> {
    #[cfg(feature = "better_panic")]
    better_panic::install();

    #[cfg(feature = "trace")]
    tracing_subscriber::fmt::init();

    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("1 argument required");
        return Ok(());
    }
    let path = args[1].clone();
    let file = fs::read(path)?;
    let demo = Demo::new(&file);

    let parser = DemoParser::new_with_analyser(demo.get_stream(), Analyser::new());
    let (header, state) = parser.parse()?;

    println!("{:?}", header);

    // for (user_id, user_data) in state.users {
    //     let player_name = user_data.name;
    //     let player_team = user_data.team;
    //     if let Some(&max_tick) = user_data.health.last() {
    //         let mut k: u32 = 0;

    //         println!("Player:{}:{}", player_team, player_name);

    //         for (tick, health) in user_data.health {
    //             while (k < tick) {
    //                 k = k + 1;
    //                 println!("{}: {}", k, health);
    //             }
    //         }
    //     }
    // }

    for player in state.players {
        let mut tick: ServerTick = 0.into();
        let mut pitchi: usize = 0;
        let mut viewi: usize = 0;

        if let Some(max_pitch_tick) = player.pitch_angle.last() {
            if let Some(max_view_tick) = player.view_angle.last() {
                let last: u32 = std::cmp::min(max_pitch_tick.0, max_view_tick.0)
                    .try_into()
                    .unwrap();
                while tick < last {
                    let mut pitchtick = player.pitch_angle[pitchi].0;
                    let mut viewtick = player.view_angle[viewi].0;
                    println!(
                        "{}, {}, {}",
                        tick, player.pitch_angle[pitchi].1, player.view_angle[viewi].1
                    );

                    while pitchtick < tick {
                        pitchtick = player.pitch_angle[pitchi].0;
                        pitchi = pitchi + 1;
                    }
                    while viewtick < tick {
                        viewtick = player.view_angle[viewi].0;
                        viewi = viewi + 1;
                    }

                    tick = tick + 1;
                }
            }
        }
    }

    Ok(())
}
