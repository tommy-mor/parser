use std::env;
use std::fs;

use main_error::MainError;
use serde::{Deserialize, Serialize};
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

        for (user_id, user_data) in state.users {
            let player_name = user_data.name;
            if let  Some(&max_tick)= user_data.health.last() {
            let mut k : u32 = 0;

            println!("Player: {}", player_name);

            for i in 0..max_tick.0.into() {
            }

            for (tick, health) in user_data.health {
                while(k < tick) {
                    k = k + 1;
                    println!("{}: {}", k, health);
                }
            }
            }

        }

    Ok(())
}
