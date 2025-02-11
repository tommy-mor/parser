use std::fs;
use test_case::test_case;

use fnv::FnvHashMap;
use std::collections::HashMap;
use tf_demo_parser::demo::packet::datatable::{ParseSendTable, SendTableName, ServerClass};
use tf_demo_parser::demo::parser::MessageHandler;
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropName};
use tf_demo_parser::{Demo, DemoParser, MessageType, ParserState};

#[derive(Default)]
pub struct SendPropAnalyser {
    tables: Vec<ParseSendTable>,
    prop_names: FnvHashMap<SendPropIdentifier, (SendTableName, SendPropName)>,
}

impl SendPropAnalyser {
    pub fn new() -> Self {
        SendPropAnalyser::default()
    }
}

impl MessageHandler for SendPropAnalyser {
    type Output = (
        Vec<ParseSendTable>,
        FnvHashMap<SendPropIdentifier, (SendTableName, SendPropName)>,
    );

    fn does_handle(_message_type: MessageType) -> bool {
        false
    }

    fn handle_data_tables(
        &mut self,
        tables: &[ParseSendTable],
        _server_classes: &[ServerClass],
        _parser_state: &ParserState,
    ) {
        for table in tables {
            for prop_def in &table.props {
                self.prop_names.insert(
                    prop_def.identifier(),
                    (table.name.clone(), prop_def.name.clone()),
                );
            }
        }

        self.tables = tables.to_vec()
    }

    fn into_output(self, _state: &ParserState) -> Self::Output {
        (self.tables, self.prop_names)
    }
}

#[test_case("test_data/gully.dem")]
fn flatten_test(input_file: &str) {
    let file = fs::read(input_file).expect("Unable to read file");
    let demo = Demo::new(&file);
    let (_, (send_tables, prop_names)) =
        DemoParser::new_with_analyser(demo.get_stream(), SendPropAnalyser::new())
            .parse()
            .expect("Failed to parse");
    let flat_props: HashMap<SendTableName, Vec<String>> = send_tables
        .iter()
        .map(|table| {
            (
                table.name.clone(),
                table
                    .flatten_props(&send_tables)
                    .unwrap()
                    .into_iter()
                    .map(|prop| {
                        let (table_name, prop_name) = &prop_names[&prop.identifier];
                        format!("{}.{}", table_name, prop_name)
                    })
                    .collect(),
            )
        })
        .collect();

    insta::with_settings!({sort_maps =>true}, {
        insta::assert_json_snapshot!(flat_props);
    });
}
