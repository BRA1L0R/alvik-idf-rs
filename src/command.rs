use serde::{de::Visitor, ser::SerializeTuple, Deserialize, Serialize};

#[derive(Debug)]
pub struct UnknownCommand {
    command: u8,

    /// guarantees that a user cannot build and send
    /// an unknown command type
    _unbuildable: (),
}

impl UnknownCommand {
    fn new(command: u8) -> Self {
        Self {
            command,
            _unbuildable: (),
        }
    }
}

macro_rules! impl_message {
    ($($idx:expr => $def:ident { $($field:ident: $type:ty),+ }),+) => {
        // #[repr(u8)]
        #[derive(Debug)]
        pub enum Message {
            Unknown(UnknownCommand),
            $($def { $($field: $type),+ } ),+
        }

        pub struct MessageEnumVisitor;
        impl<'de> Visitor<'de> for MessageEnumVisitor {
            type Value = Message;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("expected a seq of items")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let command: u8 = seq.next_element()?.unwrap();

                match command {
                    $(
                        $idx => {
                            #[derive(Deserialize)]
                            struct Helper {
                                $(
                                    $field: $type,
                                )+
                            }

                            let Helper { $($field),+ } = seq.next_element()?.unwrap();
                            Ok(Message::$def { $($field),+ })
                        }
                    ),+
                    _ => Ok(Message::Unknown(UnknownCommand::new(command))),
                }
            }
        }

        impl<'de> Deserialize<'de> for Message {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_tuple(2, MessageEnumVisitor)
            }
        }

        impl Serialize for Message {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let idx = match self {
                    Self::Unknown(_) => unreachable!(),
                    $(
                        Self::$def { .. } => $idx ,
                    )+
                };

                let mut tuple = serializer.serialize_tuple(2)?;
                tuple.serialize_element(&idx)?;

                match self {
                    Self::Unknown(_) => unreachable!(),
                    $(
                        Self::$def { $($field),+ } => {
                            #[derive(Serialize)]
                            struct Helper<'a> {
                                $($field: &'a $type),+
                            }

                            let helper = Helper { $($field),+ };
                            tuple.serialize_element(&helper)?;
                        },
                    )+
                };

                tuple.end()
            }
        }
    };
}

impl_message! {
    // Body -> Brain
    b'j' => WheelSpeed { left: f32, right: f32 },
    b'l' => LineSensor { left_line: u16, center_line: u16, right_line: u16 },
    b'c' => ColorSensor { red: u16, green: u16, blue: u16 },
    b'i' => Imu { ax: f32, ay: f32, az: f32, gx: f32, gy: f32, gz: f32 },
    b'p' => Battery { value: f32 },
    b'd' => Distance { left_tof: u16, center_tof: u16, right_tof: u16 },
    b't' => Touch { value: u8 },
    b'n' => TiltShake { value: u8 },
    b'b' => Behaviour { value: u8 },
    b'f' => DistanceMatrix { matrix: [u8; 7] },
    b'q' => ImuPosition { roll: f32, pitch: f32, yaw: f32 },
    b'w' => WheelsPosition { left_wheel: f32, right_wheel: f32 },
    b'v' => Velocity { linear: f32, angular: f32 },
    b'x' => AckU8 { ack: u8 },
    b'z' => AckF32 { x: f32, y: f32, theta: f32 },
    0x7E => FirmwareVersion { value: [u8; 3] },

    // Brain -> Body
    b'L' => SetLed { value: u8 }
}

// pub struct StructVisitor<'de, T: Deserialize<'de>>(PhantomData<T>);

// impl<'de, T: Deserialize<'de>> Visitor<'de> for StructVisitor<'de, T> {
//     fn visit
// }

// impl Serialize for Message {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         let idx = match self {
//             Self::Test(_) => b'A',
//         };

//         let mut tuple = serializer.serialize_tuple(2)?;

//         tuple.serialize_element(&idx);
//         match self {
//             Self::Test(inner) => tuple.serialize_element(inner)?,
//         };

//         tuple.end()
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// #[repr(u8)]
// pub enum Message {
//     B(u16, u16) = b'B',
//     Tilda(u16) = b'~',
// }

// impl Serialize for Message {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//     }
// }
