pub mod util;
use std::io::{ErrorKind as IoErrorKind, Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use bitflags::bitflags;

#[cfg(unix)]
use enigo::{Enigo, MouseButton as EnigoMouseButton, 
    MouseControllable, Key, KeyboardControllable};


bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct Modifier: u32 {
        const SHIFT = 1;
        const LOCK = 2;
        const CONTROL = 4;
        const MOD1 = 8;
        const MOD2 = 16;
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum EventAction {
    FramebufferUpdateRequest {
        incremental: bool,
        x:  u16,
        y:  u16,
        width:       u16,
        height:      u16,
    },
    KeyPress {
        key: String, 
    },
    Click { 
        x: i32, 
        y: i32,
        button: u32,
    },
    MouseMove { 
        x: i32, 
        y: i32,
    },
    Resize { 
        width: i32, 
        height: i32,
    },
    Scroll {
        value: i32, // negative up, positive down
    },
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Event {
    pub action : EventAction,
    pub modifiers: u32,
}
impl Event {
    pub fn as_bytes(&mut self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
    pub fn from_bytes(buffer: &[u8]) -> Self {
        bincode::deserialize(buffer).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    pub width: i32, 
    pub height: i32
}

#[cfg(unix)]
pub struct Input {
    enigo: Option<Enigo>,
    server_geometry: Geometry,
    client_geometry: Geometry,
}

#[cfg(unix)]
#[allow(dead_code)]
impl Input {
    pub fn new() -> Self {
        Self { 
            enigo: Some(Enigo::new()),
            server_geometry : Geometry::default(),
            client_geometry : Geometry::default(),
        }
    }
    pub fn set_server_geometry(&mut self, geometry: Geometry) {
        self.server_geometry = geometry;
        println!("enigo: set window size");
        self.enigo.as_mut().unwrap().set_window_size(geometry.width, geometry.height);
    }
    pub fn set_client_geometry(&mut self, geometry: Geometry) {
        self.client_geometry = geometry;
    }
    pub fn focus(&mut self) {
        self.enigo.as_mut().unwrap().window_focus();
    }
    pub fn set_window(&mut self, window: i32) {
        self.enigo.as_mut().unwrap().set_window(window);
    }
    pub fn get_window_pid(&mut self) -> i32 {
        self.enigo.as_mut().unwrap().window_pid()
    }
    pub fn search_window_by_pid(&mut self, pid: i32) -> i32 {
        self.enigo.as_mut().unwrap().search_window_by_pid(pid)
    }
    pub fn mouse_click(&mut self, x: i32, y:i32, button: u32, modifiers: u32) {
        let button = match button {
            1 => EnigoMouseButton::Left,
            3 => EnigoMouseButton::Right,
            2 => EnigoMouseButton::Middle,
            _ => todo!()
        };
        self.enigo.as_mut().unwrap().mouse_move_to(x, y);
        self.enigo.as_mut().unwrap().mouse_click(button);
    }
    pub fn mouse_move(&mut self, x: i32, y:i32, modifiers: u32) {
        self.enigo.as_mut().unwrap().mouse_move_to(x, y);
    }
    pub fn key_press(&mut self, key: &str, modifiers: u32) {
        println!(" key to match: {:?}", key);
        let k = match key {
            "Return" => Key::Return,
            "BackSpace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Page_Up" => Key::PageUp,
            "Page_Down" => Key::PageDown,
            "Up" => Key::UpArrow,
            "Down" => Key::DownArrow,
            "Left" => Key::LeftArrow,
            "Right" => Key::RightArrow,
            "End" => Key::End,
            "Home" => Key::Home,
            "Tab" => Key::Tab,
            "Escape" => Key::Escape,
            c => Key::Layout(c.chars().next().unwrap()),
        };
        println!(" key detected: {:?}", k);
        self.enigo.as_mut().unwrap().key_click(k);
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    pub bits_per_pixel: u8,
    pub depth:          u8,
    pub big_endian:     bool,
    pub true_colour:    bool,
    pub red_max:        u16,
    pub green_max:      u16,
    pub blue_max:       u16,
    pub red_shift:      u8,
    pub green_shift:    u8,
    pub blue_shift:     u8,
}
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Unexpected(&'static str),
    Server(String),
    AuthenticationUnavailable,
    AuthenticationFailure(String),
    Disconnected
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::Io(ref inner) => inner.fmt(f),
            Error::Unexpected(ref descr) =>
                write!(f, "unexpected {}", descr),
            Error::Server(ref descr) =>
                write!(f, "server error: {}", descr),
            Error::AuthenticationFailure(ref descr) =>
                write!(f, "authentication failure: {}", descr),
            _ => f.write_str(&self.to_string())
        }
    }
}

impl std::error::Error for Error {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self {
            Error::Io(ref inner) => Some(inner),
            _ => None
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error { Error::Io(error) }
}

pub type Result<T> = std::result::Result<T, Error>;

impl Message for PixelFormat {
    fn read_from<R: Read>(reader: &mut R) -> Result<PixelFormat> {
        let pixel_format = PixelFormat {
            bits_per_pixel: reader.read_u8()?,
            depth:          reader.read_u8()?,
            big_endian:     reader.read_u8()? != 0,
            true_colour:    reader.read_u8()? != 0,
            red_max:        reader.read_u16::<BigEndian>()?,
            green_max:      reader.read_u16::<BigEndian>()?,
            blue_max:       reader.read_u16::<BigEndian>()?,
            red_shift:      reader.read_u8()?,
            green_shift:    reader.read_u8()?,
            blue_shift:     reader.read_u8()?,
        };
        reader.read_exact(&mut [0u8; 3])?;
        Ok(pixel_format)
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8(self.bits_per_pixel)?;
        writer.write_u8(self.depth)?;
        writer.write_u8(if self.big_endian { 1 } else { 0 })?;
        writer.write_u8(if self.true_colour { 1 } else { 0 })?;
        writer.write_u16::<BigEndian>(self.red_max)?;
        writer.write_u16::<BigEndian>(self.green_max)?;
        writer.write_u16::<BigEndian>(self.blue_max)?;
        writer.write_u8(self.red_shift)?;
        writer.write_u8(self.green_shift)?;
        writer.write_u8(self.blue_shift)?;
        writer.write_all(&[0u8; 3])?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CopyRect {
    pub src_x_position: u16,
    pub src_y_position: u16,
}

impl Message for CopyRect {
    fn read_from<R: Read>(reader: &mut R) -> Result<CopyRect> {
        Ok(CopyRect {
            src_x_position: reader.read_u16::<BigEndian>()?,
            src_y_position: reader.read_u16::<BigEndian>()?
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.src_x_position)?;
        writer.write_u16::<BigEndian>(self.src_y_position)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Unknown(i32),
    // core spec
    Raw,
    CopyRect,
    Rre,
    Hextile,
    Zrle,
    Cursor,
    DesktopSize,
    // extensions
}

impl Message for Encoding {
    fn read_from<R: Read>(reader: &mut R) -> Result<Encoding> {
        let encoding = reader.read_i32::<BigEndian>()?;
        match encoding {
            0    => Ok(Encoding::Raw),
            1    => Ok(Encoding::CopyRect),
            2    => Ok(Encoding::Rre),
            5    => Ok(Encoding::Hextile),
            16   => Ok(Encoding::Zrle),
            -239 => Ok(Encoding::Cursor),
            -223 => Ok(Encoding::DesktopSize),
            n    => Ok(Encoding::Unknown(n))
        }
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let encoding = match self {
            Encoding::Raw => 0,
            Encoding::CopyRect => 1,
            Encoding::Rre => 2,
            Encoding::Hextile => 5,
            Encoding::Zrle => 16,
            Encoding::Cursor => -239,
            Encoding::DesktopSize => -223,
            Encoding::Unknown(n) => *n
        };
        writer.write_i32::<BigEndian>(encoding)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum C2S {
    // core spec
    SetPixelFormat(PixelFormat),
    SetEncodings(Vec<Encoding>),
    FramebufferUpdateRequest {
        incremental: bool,
        x_position:  u16,
        y_position:  u16,
        width:       u16,
        height:      u16,
    },
    KeyEvent {
        down:        bool,
        key:         u32,
    },
    PointerEvent {
        button_mask: u8,
        x_position:  u16,
        y_position:  u16
    },
    CutText(String),
    // extensions
}

pub trait Message {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> where Self: Sized;
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}

impl Message for Vec<u8> {
    fn read_from<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
        let length = reader.read_u32::<BigEndian>()?;
        let mut buffer = vec![0; length as usize];
        reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let length = self.len() as u32; // TODO: check?
        writer.write_u32::<BigEndian>(length)?;
        writer.write_all(&self)?;
        Ok(())
    }
}

/* All strings in VNC are either ASCII or Latin-1, both of which
   are embedded in Unicode. */
impl Message for String {
    fn read_from<R: Read>(reader: &mut R) -> Result<String> {
        let length = reader.read_u32::<BigEndian>()?;
        let mut string = vec![0; length as usize];
        reader.read_exact(&mut string)?;
        Ok(string.iter().map(|c| *c as char).collect())
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let length = self.len() as u32; // TODO: check?
        writer.write_u32::<BigEndian>(length)?;
        writer.write_all(&self.chars().map(|c| c as u8).collect::<Vec<u8>>())?;
        Ok(())
    }
}



impl Message for C2S {
    fn read_from<R: Read>(reader: &mut R) -> Result<C2S> {
        let message_type =
            match reader.read_u8() {
                Err(ref e) if e.kind() == IoErrorKind::UnexpectedEof =>
                    return Err(Error::Disconnected),
                result => result?
            };
        match message_type {
            0 => {
                reader.read_exact(&mut [0u8; 3])?;
                Ok(C2S::SetPixelFormat(PixelFormat::read_from(reader)?))
            },
            2 => {
                reader.read_exact(&mut [0u8; 1])?;
                let count = reader.read_u16::<BigEndian>()?;
                let mut encodings = Vec::new();
                for _ in 0..count {
                    encodings.push(Encoding::read_from(reader)?);
                }
                Ok(C2S::SetEncodings(encodings))
            },
            3 => {
                Ok(C2S::FramebufferUpdateRequest {
                    incremental: reader.read_u8()? != 0,
                    x_position:  reader.read_u16::<BigEndian>()?,
                    y_position:  reader.read_u16::<BigEndian>()?,
                    width:       reader.read_u16::<BigEndian>()?,
                    height:      reader.read_u16::<BigEndian>()?
                })
            },
            4 => {
                let down = reader.read_u8()? != 0;
                reader.read_exact(&mut [0u8; 2])?;
                let key = reader.read_u32::<BigEndian>()?;
                Ok(C2S::KeyEvent { down, key })
            },
            5 => {
                Ok(C2S::PointerEvent {
                    button_mask: reader.read_u8()?,
                    x_position:  reader.read_u16::<BigEndian>()?,
                    y_position:  reader.read_u16::<BigEndian>()?
                })
            },
            6 => {
                reader.read_exact(&mut [0u8; 3])?;
                Ok(C2S::CutText(String::read_from(reader)?))
            },
            _ => Err(Error::Unexpected("client to server message type"))
        }
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            C2S::SetPixelFormat(ref pixel_format) => {
                writer.write_u8(0)?;
                writer.write_all(&[0u8; 3])?;
                PixelFormat::write_to(pixel_format, writer)?;
            },
            C2S::SetEncodings(ref encodings) => {
                writer.write_u8(2)?;
                writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(encodings.len() as u16)?; // TODO: check?
                for encoding in encodings {
                    Encoding::write_to(encoding, writer)?;
                }
            },
            C2S::FramebufferUpdateRequest { 
                incremental, x_position, y_position, width, height 
            } => {
                writer.write_u8(3)?;
                writer.write_u8(if *incremental { 1 } else { 0 })?;
                writer.write_u16::<BigEndian>(*x_position)?;
                writer.write_u16::<BigEndian>(*y_position)?;
                writer.write_u16::<BigEndian>(*width)?;
                writer.write_u16::<BigEndian>(*height)?;
            },
            C2S::KeyEvent { down, key } => {
                writer.write_u8(4)?;
                writer.write_u8(if *down { 1 } else { 0 })?;
                writer.write_all(&[0u8; 2])?;
                writer.write_u32::<BigEndian>(*key)?;
            },
            C2S::PointerEvent { button_mask, x_position, y_position } => {
                writer.write_u8(5)?;
                writer.write_u8(*button_mask)?;
                writer.write_u16::<BigEndian>(*x_position)?;
                writer.write_u16::<BigEndian>(*y_position)?;
            },
            C2S::CutText(ref text) => {
                String::write_to(text, writer)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Rectangle {
    pub x_position: u16,
    pub y_position: u16,
    pub width:      u16,
    pub height:     u16,
    pub encoding:   Encoding,
}

impl Message for Rectangle {
    fn read_from<R: Read>(reader: &mut R) -> Result<Rectangle> {
        Ok(Rectangle {
            x_position: reader.read_u16::<BigEndian>()?,
            y_position: reader.read_u16::<BigEndian>()?,
            width:      reader.read_u16::<BigEndian>()?,
            height:     reader.read_u16::<BigEndian>()?,
            encoding:   Encoding::read_from(reader)?
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.x_position)?;
        writer.write_u16::<BigEndian>(self.y_position)?;
        writer.write_u16::<BigEndian>(self.width)?;
        writer.write_u16::<BigEndian>(self.height)?;
        Encoding::write_to(&self.encoding, writer)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Colour {
    pub red:   u16,
    pub green: u16,
    pub blue:  u16
}

impl Message for Colour {
    fn read_from<R: Read>(reader: &mut R) -> Result<Colour> {
        Ok(Colour {
            red:   reader.read_u16::<BigEndian>()?,
            green: reader.read_u16::<BigEndian>()?,
            blue:  reader.read_u16::<BigEndian>()?
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.red)?;
        writer.write_u16::<BigEndian>(self.green)?;
        writer.write_u16::<BigEndian>(self.blue)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum S2C {
    // core spec
    FramebufferUpdate {
        count:        u16,
        /* Vec<Rectangle> has to be read out manually */
    },
    SetColourMapEntries {
        first_colour: u16,
        colours:      Vec<Colour>
    },
    Bell,
    CutText(String),
    // extensions
}

impl Message for S2C {
    fn read_from<R: Read>(reader: &mut R) -> Result<S2C> {
        let message_type =
            match reader.read_u8() {
                Err(ref e) if e.kind() == IoErrorKind::UnexpectedEof =>
                    return Err(Error::Disconnected),
                result => result?
            };
        match message_type {
            0 => {
                reader.read_exact(&mut [0u8; 1])?;
                Ok(S2C::FramebufferUpdate {
                    count: reader.read_u16::<BigEndian>()?
                })
            },
            1 => {
                reader.read_exact(&mut [0u8; 1])?;
                let first_colour = reader.read_u16::<BigEndian>()?;
                let count = reader.read_u16::<BigEndian>()?;
                let mut colours = Vec::new();
                for _ in 0..count {
                    colours.push(Colour::read_from(reader)?);
                }
                Ok(S2C::SetColourMapEntries { first_colour, colours })
            },
            2 => {
                Ok(S2C::Bell)
            },
            3 => {
                reader.read_exact(&mut [0u8; 3])?;
                Ok(S2C::CutText(String::read_from(reader)?))
            },
            _ => Err(Error::Unexpected("server to client message type"))
        }
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            S2C::FramebufferUpdate { count } => {
                writer.write_u8(0)?;
                writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(*count)?;
            },
            S2C::SetColourMapEntries { first_colour, ref colours } => {
                writer.write_u8(1)?;
                writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(*first_colour)?;
                for colour in colours {
                    Colour::write_to(colour, writer)?;
                }
            },
            S2C::Bell => {
                writer.write_u8(2)?;
            },
            S2C::CutText(ref text) => {
                writer.write_u8(3)?;
                writer.write_all(&[0u8; 3])?;
                String::write_to(text, writer)?;
            }
        }
        Ok(())
    }
}