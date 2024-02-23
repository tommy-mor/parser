use crate::demo::data::{DemoTick, ServerTick};
use crate::demo::gameevent_gen::{
    GameEvent, PlayerDeathEvent, PlayerSpawnEvent, TeamPlayRoundWinEvent,
};
use crate::demo::message::packetentities::{EntityId, PacketEntity};
use crate::demo::message::usermessage::{ChatMessageKind, SayText2Message, UserMessage};
use crate::demo::message::{self, Message, MessageType};
use crate::demo::packet::datatable::ServerClassName;
use crate::demo::packet::stringtable::StringTableEntry;
use crate::demo::parser::handler::{BorrowMessageHandler, MessageHandler};
use crate::demo::sendprop::SendPropIdentifier;
use crate::demo::vector::Vector;
use crate::{ParserState, ReadResult, Stream};
use bitbuffer::{BitWrite, BitWriteStream, Endianness};
use num_enum::TryFromPrimitive;
use parse_display::{Display, FromStr};
use serde::de::Error;
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::ops::{Index, IndexMut};
use std::vec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub kind: ChatMessageKind,
    pub from: String,
    pub text: String,
    pub tick: DemoTick,
}

impl ChatMessage {
    pub fn from_message(message: &SayText2Message, tick: DemoTick) -> Self {
        ChatMessage {
            kind: message.kind,
            from: message
                .from
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            text: message.plain_text(),
            tick,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Copy,
    PartialEq,
    Eq,
    Hash,
    TryFromPrimitive,
    Default,
    Display,
)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Team {
    #[default]
    Other = 0,
    Spectator = 1,
    Red = 2,
    Blue = 3,
}

impl Team {
    pub fn new<U>(number: U) -> Self
    where
        u8: TryFrom<U>,
    {
        Team::try_from(u8::try_from(number).unwrap_or_default()).unwrap_or_default()
    }

    pub fn is_player(&self) -> bool {
        *self == Team::Red || *self == Team::Blue
    }
}

#[derive(
    Debug, Clone, Serialize, Copy, PartialEq, Eq, Hash, TryFromPrimitive, Display, FromStr, Default,
)]
#[display(style = "lowercase")]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Class {
    #[default]
    Other = 0,
    Scout = 1,
    Sniper = 2,
    Soldier = 3,
    Demoman = 4,
    Medic = 5,
    Heavy = 6,
    Pyro = 7,
    Spy = 8,
    Engineer = 9,
}

impl<'de> Deserialize<'de> for Class {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        #[serde(untagged)]
        enum IntOrStr<'a> {
            Int(u8),
            Str(&'a str),
        }

        let raw = IntOrStr::deserialize(deserializer)?;
        match raw {
            IntOrStr::Int(class) => Class::try_from_primitive(class).map_err(D::Error::custom),
            IntOrStr::Str(class) if class.len() == 1 => {
                Class::try_from_primitive(class.parse().map_err(D::Error::custom)?)
                    .map_err(D::Error::custom)
            }
            IntOrStr::Str(class) => class.parse().map_err(D::Error::custom),
        }
    }
}

#[test]
fn test_class_deserialize() {
    assert_eq!(Class::Scout, serde_json::from_str(r#""scout""#).unwrap());
    assert_eq!(Class::Scout, serde_json::from_str(r#""1""#).unwrap());
    assert_eq!(Class::Scout, serde_json::from_str("1").unwrap());
}

impl Class {
    pub fn new<U>(number: U) -> Self
    where
        u8: TryFrom<U>,
    {
        Class::try_from(u8::try_from(number).unwrap_or_default()).unwrap_or_default()
    }
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize, Clone)]
#[serde(from = "HashMap<Class, u8>")]
pub struct ClassList([u8; 10]);

impl ClassList {
    /// Get an iterator for all classes played and the number of spawn on the class
    pub fn iter(&self) -> impl Iterator<Item = (Class, u8)> + '_ {
        self.0
            .iter()
            .copied()
            .enumerate()
            .map(|(class, count)| (Class::new(class), count))
            .filter(|(_, count)| *count > 0)
    }

    /// Get an iterator for all classes played and the number of spawn on the class, sorted by the number of spawns
    pub fn sorted(&self) -> impl Iterator<Item = (Class, u8)> {
        let mut classes = self.iter().collect::<Vec<(Class, u8)>>();
        classes.sort_by(|a, b| a.1.cmp(&b.1).reverse());
        classes.into_iter()
    }
}

#[test]
fn test_classlist_sorted() {
    let list = ClassList([0, 1, 5, 0, 0, 3, 0, 0, 0, 0]);
    assert_eq!(
        list.sorted().collect::<Vec<_>>(),
        &[(Class::Sniper, 5), (Class::Medic, 3), (Class::Scout, 1)]
    )
}

impl Index<Class> for ClassList {
    type Output = u8;

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn index(&self, class: Class) -> &Self::Output {
        &self.0[class as u8 as usize]
    }
}

impl IndexMut<Class> for ClassList {
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn index_mut(&mut self, class: Class) -> &mut Self::Output {
        &mut self.0[class as u8 as usize]
    }
}

impl Serialize for ClassList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let count = self.0.iter().filter(|c| **c > 0).count();
        let mut classes = serializer.serialize_map(Some(count))?;
        for (class, count) in self.0.iter().copied().enumerate() {
            if count > 0 {
                classes.serialize_entry(&class, &count)?;
            }
        }

        classes.end()
    }
}

impl From<HashMap<Class, u8>> for ClassList {
    fn from(map: HashMap<Class, u8>) -> Self {
        let mut classes = ClassList::default();

        for (class, count) in map.into_iter() {
            classes[class] = count;
        }

        classes
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Default,
    Display,
)]
pub struct UserId(u16);

impl<E: Endianness> BitWrite<E> for UserId {
    fn write(&self, stream: &mut BitWriteStream<E>) -> ReadResult<()> {
        (self.0 as u32).write(stream)
    }
}

impl From<u32> for UserId {
    fn from(int: u32) -> Self {
        UserId(int as u16)
    }
}

impl From<u16> for UserId {
    fn from(int: u16) -> Self {
        UserId(int)
    }
}

impl From<UserId> for u16 {
    fn from(id: UserId) -> Self {
        id.0
    }
}

impl From<UserId> for u32 {
    fn from(id: UserId) -> Self {
        id.0 as u32
    }
}

impl PartialEq<u16> for UserId {
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spawn {
    pub user: UserId,
    pub class: Class,
    pub team: Team,
    pub tick: DemoTick,
}

impl Spawn {
    pub fn from_event(event: &PlayerSpawnEvent, tick: DemoTick) -> Self {
        Spawn {
            user: UserId::from(event.user_id),
            class: Class::new(event.class),
            team: Team::new(event.team),
            tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub classes: ClassList,
    pub name: String,
    pub user_id: UserId,
    pub steam_id: String,
    #[serde(skip)]
    pub entity_id: EntityId,
    pub team: Team,
    pub health: Vec<(DemoTick, u16)>,
}

impl From<crate::demo::data::UserInfo> for UserInfo {
    fn from(info: crate::demo::data::UserInfo) -> Self {
        UserInfo {
            classes: ClassList::default(),
            name: info.player_info.name,
            user_id: info.player_info.user_id,
            steam_id: info.player_info.steam_id,
            entity_id: info.entity_id,
            health: vec![],
            team: Team::default(),
        }
    }
}

impl PartialEq for UserInfo {
    fn eq(&self, other: &UserInfo) -> bool {
        self.classes == other.classes
            && self.name == other.name
            && self.user_id == other.user_id
            && self.steam_id == other.steam_id
            && self.team == other.team
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Death {
    pub weapon: String,
    pub victim: UserId,
    pub assister: Option<UserId>,
    pub killer: UserId,
    pub tick: DemoTick,
}

impl Death {
    pub fn from_event(event: &PlayerDeathEvent, tick: DemoTick) -> Self {
        let assister = if event.assister < (16 * 1024) {
            Some(UserId::from(event.assister))
        } else {
            None
        };
        Death {
            assister,
            tick,
            killer: UserId::from(event.attacker),
            weapon: event.weapon.to_string(),
            victim: UserId::from(event.user_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Round {
    pub winner: Team,
    pub length: f32,
    pub end_tick: DemoTick,
}

impl Round {
    pub fn from_event(event: &TeamPlayRoundWinEvent, tick: DemoTick) -> Self {
        Round {
            winner: Team::new(event.team),
            length: event.round_time,
            end_tick: tick,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct World {
    pub boundary_min: Vector,
    pub boundary_max: Vector,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pause {
    from: DemoTick,
    to: DemoTick,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Analyser {
    state: MatchState,
    pause_start: Option<DemoTick>,
    user_id_map: HashMap<EntityId, UserId>,

    class_names: Vec<ServerClassName>,
}

use std::str::FromStr;

impl Analyser {
    pub fn handle_entity(
        &mut self,
        entity: &PacketEntity,
        parser_state: &ParserState,
        tick: ServerTick,
    ) {
        let class_name = self
            .class_names
            .get(usize::from(entity.server_class))
            .map(|c| c.as_str())
            .unwrap_or("");

        match class_name {
            "CTFPlayerResource" => {
                for prop in entity.props(parser_state) {
                    if let Some((table_name, prop_name)) = prop.identifier.names() {
                        if let Ok(player_id) = u32::from_str(prop_name.as_str()) {
                            let entity_id = EntityId::from(player_id);
                            if let Some(player) = self
                                .state
                                .players
                                .iter_mut()
                                .find(|p| p.entity == entity_id)
                            {
                                match table_name.as_str() {
                                    "m_iPlayerClass" => {
                                        player.class = Class::new(
                                            i64::try_from(&prop.value).unwrap_or_default(),
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            "CTFPlayer" => {
                const LOCAL_EYE_ANGLES: SendPropIdentifier =
                    SendPropIdentifier::new("DT_TFLocalPlayerExclusive", "m_angEyeAngles[1]");
                const NON_LOCAL_EYE_ANGLES: SendPropIdentifier =
                    SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_angEyeAngles[1]");
                const LOCAL_PITCH_ANGLES: SendPropIdentifier =
                    SendPropIdentifier::new("DT_TFLocalPlayerExclusive", "m_angEyeAngles[0]");
                const NON_LOCAL_PITCH_ANGLES: SendPropIdentifier =
                    SendPropIdentifier::new("DT_TFNonLocalPlayerExclusive", "m_angEyeAngles[0]");

                let player = self.state.get_or_create_player(entity.entity_index);

                for prop in &entity.props {
                    match prop.identifier {
                        LOCAL_EYE_ANGLES | NON_LOCAL_EYE_ANGLES => {
                            player
                                .view_angle
                                .push((tick, f32::try_from(&prop.value).unwrap_or_default()));
                        }
                        LOCAL_PITCH_ANGLES | NON_LOCAL_PITCH_ANGLES => {
                            player
                                .pitch_angle
                                .push((tick, f32::try_from(&prop.value).unwrap_or_default()));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

impl MessageHandler for Analyser {
    type Output = MatchState;

    fn does_handle(message_type: MessageType) -> bool {
        matches!(
            message_type,
            MessageType::GameEvent
                | MessageType::UserMessage
                | MessageType::ServerInfo
                | MessageType::NetTick
                | MessageType::SetPause
                | MessageType::PacketEntities
        )
    }

    fn handle_data_tables(
        &mut self,
        _tables: &[crate::demo::packet::datatable::ParseSendTable],
        server_classes: &[crate::demo::packet::datatable::ServerClass],
        _parser_state: &ParserState,
    ) {
        self.class_names = server_classes
            .iter()
            .map(|class| class.name.clone())
            .collect();
    }

    fn handle_message(&mut self, message: &Message, tick: DemoTick, _parser_state: &ParserState) {
        match message {
            Message::NetTick(msg) => {
                if self.state.start_tick == 0 {
                    self.state.start_tick = msg.tick;
                }
            }
            Message::ServerInfo(message) => {
                self.state.interval_per_tick = message.interval_per_tick
            }
            Message::GameEvent(message) => self.handle_event(&message.event, tick),
            Message::UserMessage(message) => self.handle_user_message(message, tick),
            Message::SetPause(message) => {
                if message.pause {
                    self.pause_start = Some(tick);
                } else {
                    let start = self.pause_start.unwrap_or_default();
                    self.state.pauses.push(Pause {
                        from: start,
                        to: tick,
                    })
                }
            }
            Message::PacketEntities(message) => {
                if let Some(tick) = message.delta {
                    for entity in &message.entities {
                        self.handle_entity(entity, _parser_state, tick);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_string_entry(
        &mut self,
        table: &str,
        index: usize,
        entry: &StringTableEntry,
        _parser_state: &ParserState,
    ) {
        if table == "userinfo" {
            let _ = self.parse_user_info(
                index,
                entry.text.as_ref().map(|s| s.as_ref()),
                entry.extra_data.as_ref().map(|data| data.data.clone()),
            );
        }
    }

    fn into_output(self, _state: &ParserState) -> Self::Output {
        self.state
    }
}

impl BorrowMessageHandler for Analyser {
    fn borrow_output(&self, _state: &ParserState) -> &Self::Output {
        &self.state
    }
}

impl Analyser {
    pub fn new() -> Self {
        Self::default()
    }

    fn handle_user_message(&mut self, message: &UserMessage, tick: DemoTick) {
        if let UserMessage::SayText2(text_message) = message {
            if text_message.kind == ChatMessageKind::NameChange {
                if let Some(from) = text_message.from.clone() {
                    self.change_name(from.into(), text_message.plain_text());
                }
            } else {
                self.state
                    .chat
                    .push(ChatMessage::from_message(text_message, tick));
            }
        }
    }

    fn change_name(&mut self, from: String, to: String) {
        if let Some(user) = self.state.users.values_mut().find(|user| user.name == from) {
            user.name = to;
        }
    }

    fn handle_event(&mut self, event: &GameEvent, tick: DemoTick) {
        const WIN_REASON_TIME_LIMIT: u8 = 6;

        match event {
            GameEvent::PlayerDeath(event) => self.state.deaths.push(Death::from_event(event, tick)),
            GameEvent::PlayerSpawn(event) => {
                let spawn = Spawn::from_event(event, tick);
                if let Some(user_state) = self.state.users.get_mut(&spawn.user) {
                    user_state.classes[spawn.class] += 1;
                    user_state.team = spawn.team;
                }
            }
            GameEvent::TeamPlayRoundWin(event) => {
                if event.win_reason != WIN_REASON_TIME_LIMIT {
                    self.state.rounds.push(Round::from_event(event, tick))
                }
            }
            GameEvent::PlayerHurt(event) => {
                if let Some(user_state) = self.state.users.get_mut(&event.user_id.into()) {
                    user_state.health.push((tick, event.health));
                }
            }
            _ => {}
        }
    }

    fn parse_user_info(
        &mut self,
        index: usize,
        text: Option<&str>,
        data: Option<Stream>,
    ) -> ReadResult<()> {
        if let Some(user_info) =
            crate::demo::data::UserInfo::parse_from_string_table(index as u16, text, data)?
        {
            self.state
                .users
                .entry(user_info.player_info.user_id)
                .and_modify(|info| {
                    info.entity_id = user_info.entity_id;
                })
                .or_insert_with(|| user_info.into());
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Player {
    entity: EntityId,
    pub class: Class,
    pub view_angle: Vec<(ServerTick, f32)>,
    pub pitch_angle: Vec<(ServerTick, f32)>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MatchState {
    pub chat: Vec<ChatMessage>,
    pub users: BTreeMap<UserId, UserInfo>,
    pub deaths: Vec<Death>,
    pub rounds: Vec<Round>,
    pub start_tick: ServerTick,
    pub interval_per_tick: f32,
    pub pauses: Vec<Pause>,
    pub players: Vec<Player>,
}

impl MatchState {
    pub fn get_or_create_player(&mut self, entity_id: EntityId) -> &mut Player {
        let index = self
            .players
            .iter()
            .enumerate()
            .find(|(_index, player)| player.entity == entity_id)
            .map(|(index, _)| index);
        match index {
            Some(index) => &mut self.players[index],
            None => {
                self.players.push(Player {
                    entity: entity_id,
                    ..Default::default()
                });
                self.players.last_mut().unwrap()
            }
        }
    }
}
