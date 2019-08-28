#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fs::{self, File};

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::rc::Rc;
use tf_demo_parser::demo::message::packetentities::{EntityId, PacketEntity, PVS};
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::datatable::{
    ParseSendTable, SendTableName, ServerClass, ServerClassName,
};
use tf_demo_parser::demo::packet::stringtable::StringTableEntry;
use tf_demo_parser::demo::parser::MessageHandler;
use tf_demo_parser::demo::sendprop::{SendPropDefinition, SendPropName, SendPropValue};
use tf_demo_parser::{Demo, DemoParser, MatchState, MessageType, MessageTypeAnalyser, ParserState};

/// Compatible serialization with the js parser entity dumps
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum PVSCompat {
    Preserve = 0,
    Leave = 2,
    Enter = 1,
    Delete = 6,
}

impl From<PVS> for PVSCompat {
    fn from(pvs: PVS) -> Self {
        match pvs {
            PVS::Preserve => PVSCompat::Preserve,
            PVS::Leave => PVSCompat::Leave,
            PVS::Enter => PVSCompat::Enter,
            PVS::Delete => PVSCompat::Delete,
        }
    }
}

#[derive(PartialEq, Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EntityDump {
    tick: u32,
    server_class: ServerClassName,
    id: EntityId,
    props: HashMap<SendPropName, SendPropValue>,
    pvs: PVSCompat,
}

impl EntityDump {
    pub fn from_entity(entity: PacketEntity, tick: u32) -> Self {
        let id = entity.entity_index;
        EntityDump {
            tick,
            server_class: entity.server_class.name.clone(),
            id: entity.entity_index,
            props: entity
                .props
                .into_iter()
                .map(|prop| (prop.definition.name.clone(), prop.value))
                .collect(),
            pvs: entity.pvs.into(),
        }
    }
}

struct EntityDumper {
    entities: Vec<EntityDump>,
}

impl EntityDumper {
    pub fn new() -> Self {
        EntityDumper {
            entities: Vec::with_capacity(128),
        }
    }
}

impl MessageHandler for EntityDumper {
    type Output = Vec<EntityDump>;

    fn does_handle(message_type: MessageType) -> bool {
        match message_type {
            MessageType::PacketEntities => true,
            _ => false,
        }
    }

    fn handle_message(&mut self, message: Message, tick: u32) {
        match message {
            Message::PacketEntities(entity_message) => self.entities.extend(
                entity_message
                    .entities
                    .into_iter()
                    .map(|entity| EntityDump::from_entity(entity, tick)),
            ),
            _ => {}
        }
    }

    fn handle_string_entry(&mut self, table: &String, _index: usize, entry: &StringTableEntry) {}

    fn get_output(self, _state: ParserState) -> Self::Output {
        self.entities
    }
}

fn entity_test(input_file: &str, snapshot_file: &str) {
    let file = fs::read(input_file).expect("Unable to read file");
    let demo = Demo::new(file);
    let (_, entities) =
        DemoParser::parse_with_analyser(demo.get_stream(), EntityDumper::new()).unwrap();

    let json_file = File::open(snapshot_file).expect("Unable to read file");
    let mut reader = BufReader::new(json_file);
    let mut buffer = String::new();

    let mut expected = Vec::with_capacity(128);

    while reader.read_line(&mut buffer).expect("failed to read line") > 0 {
        let entity: EntityDump =
            serde_json::from_str(buffer.trim_end()).expect("failed to parse json");
        expected.push(entity);
        buffer.clear();
    }

    assert_eq!(expected.len(), entities.len());

    let entity_ids: Vec<_> = entities.iter().map(|entity| entity.id).collect();
    let expected_ids: Vec<_> = expected.iter().map(|entity| entity.id).collect();

    assert_eq!(expected_ids, entity_ids);

    for (i, (expected_entity, entity)) in expected.into_iter().zip(entities.into_iter()).enumerate()
    {
        assert_eq!(
            expected_entity.tick, entity.tick,
            "Failed comparing entity {}",
            i
        );
        assert_eq!(
            expected_entity.id, entity.id,
            "Failed comparing entity {}",
            i
        );
        assert_eq!(
            expected_entity.server_class, entity.server_class,
            "Failed comparing entity {}",
            i
        );
        assert_eq!(
            expected_entity.pvs, entity.pvs,
            "Failed comparing entity {}",
            i
        );
        let mut prop_names: Vec<_> = entity.props.keys().collect();
        let mut expected_prop_names: Vec<_> = expected_entity.props.keys().collect();
        prop_names.sort();
        expected_prop_names.sort();

        assert_eq!(
            expected_prop_names, prop_names,
            "Failed comparing entity {}",
            i
        );

        for prop_name in expected_prop_names {
            assert_eq!(
                expected_entity.props.get(prop_name),
                entity.props.get(prop_name),
                "Failed comparing entity {} prop {}",
                i,
                prop_name
            );
        }

        assert_eq!(expected_entity, entity, "Failed comparing entity {}", i);
    }
}

#[test]
fn entity_test_short() {
    better_panic::install();

    entity_test("data/small.dem", "data/small_entities.json");
}
