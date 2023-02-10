#![allow(unused)]
pub mod util;
pub mod canvas;

#[cfg(unix)]
pub mod capture;

use std::io::{ErrorKind as IoErrorKind, Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[cfg(unix)]
use enigo::{Enigo, MouseButton as EnigoMouseButton, MouseControllable, Key, KeyboardControllable};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
    pub fn key_down(&mut self, key: &str) {
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
        self.enigo.as_mut().unwrap().key_down(k);
    }
    pub fn key_up(&mut self, key: &str) {
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
        self.enigo.as_mut().unwrap().key_up(k);
    }
}

pub trait Message {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self> where Self: Sized;
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}

#[derive(Debug)]
pub enum ClientEvent {
    // core spec
    //SetPixelFormat(PixelFormat),
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

impl Message for ClientEvent {
    fn read_from<R: Read>(reader: &mut R) -> Result<ClientEvent> {
        let message_type =
            match reader.read_u8() {
                Err(ref e) if e.kind() == IoErrorKind::UnexpectedEof =>
                    return Err(Error::Disconnected),
                result => result?
            };
        match message_type {
            // 0 => {
            //     reader.read_exact(&mut [0u8; 3])?;
            //     Ok(ClientEvent::SetPixelFormat(PixelFormat::read_from(reader)?))
            // },
            2 => {
                reader.read_exact(&mut [0u8; 1])?;
                let count = reader.read_u16::<BigEndian>()?;
                let mut encodings = Vec::new();
                for _ in 0..count {
                    encodings.push(Encoding::read_from(reader)?);
                }
                Ok(ClientEvent::SetEncodings(encodings))
            },
            3 => {
                Ok(ClientEvent::FramebufferUpdateRequest {
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
                Ok(ClientEvent::KeyEvent { down, key })
            },
            5 => {
                Ok(ClientEvent::PointerEvent {
                    button_mask: reader.read_u8()?,
                    x_position:  reader.read_u16::<BigEndian>()?,
                    y_position:  reader.read_u16::<BigEndian>()?
                })
            },
            6 => {
                reader.read_exact(&mut [0u8; 3])?;
                Ok(ClientEvent::CutText(String::read_from(reader)?))
            },
            _ => Err(Error::Unexpected("client to server message type".to_string()))
        }
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            // ClientEvent::SetPixelFormat(ref pixel_format) => {
            //     writer.write_u8(0)?;
            //     writer.write_all(&[0u8; 3])?;
            //     PixelFormat::write_to(pixel_format, writer)?;
            // },
            ClientEvent::SetEncodings(ref encodings) => {
                writer.write_u8(2)?;
                writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(encodings.len() as u16)?; // TODO: check?
                for encoding in encodings {
                    Encoding::write_to(encoding, writer)?;
                }
            },
            ClientEvent::FramebufferUpdateRequest { 
                incremental, x_position, y_position, width, height 
            } => {
                writer.write_u8(3)?;
                writer.write_u8(if *incremental { 1 } else { 0 })?;
                writer.write_u16::<BigEndian>(*x_position)?;
                writer.write_u16::<BigEndian>(*y_position)?;
                writer.write_u16::<BigEndian>(*width)?;
                writer.write_u16::<BigEndian>(*height)?;
            },
            ClientEvent::KeyEvent { down, key } => {
                writer.write_u8(4)?;
                writer.write_u8(if *down { 1 } else { 0 })?;
                writer.write_all(&[0u8; 2])?;
                writer.write_u32::<BigEndian>(*key)?;
            },
            ClientEvent::PointerEvent { button_mask, x_position, y_position } => {
                writer.write_u8(5)?;
                writer.write_u8(*button_mask)?;
                writer.write_u16::<BigEndian>(*x_position)?;
                writer.write_u16::<BigEndian>(*y_position)?;
            },
            ClientEvent::CutText(ref text) => {
                String::write_to(text, writer)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ServerEvent {
    FramebufferUpdate {
        count:  u16,
        rectangles:  Vec<Rec>,
    },
    Bell,
    CutText(String),
}

impl Message for ServerEvent {
    fn read_from<R: Read>(reader: &mut R) -> Result<ServerEvent> {
        let message_type =
            match reader.read_u8() {
                Err(ref e) if e.kind() == IoErrorKind::UnexpectedEof =>
                    return Err(Error::Disconnected),
                result => result?
            };
        match message_type {
            0 => {
                reader.read_exact(&mut [0u8; 1])?;
                let count = reader.read_u16::<BigEndian>()?; 
                let mut rectangles = Vec::<Rec>::new();
                for _ in 0..count {
                    let r = Rec::read_from(reader)?;
                    rectangles.push(r);
                }
                Ok(ServerEvent::FramebufferUpdate {
                    count,
                    rectangles,
                })
            },            
            2 => {
                Ok(ServerEvent::Bell)
            },
            3 => {
                reader.read_exact(&mut [0u8; 3])?;
                Ok(ServerEvent::CutText(String::read_from(reader)?))
            },
            _ => Err(Error::Unexpected("server to client message type".to_string()))
        }
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            ServerEvent::FramebufferUpdate { count, rectangles } => {
                writer.write_u8(0)?;
                writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(*count)?;
                for r in rectangles.iter() {
                    Rec::write_to(&r, writer);
                }
            },            
            ServerEvent::Bell => {
                writer.write_u8(2)?;
            },
            ServerEvent::CutText(ref text) => {
                writer.write_u8(3)?;
                writer.write_all(&[0u8; 3])?;
                String::write_to(text, writer)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    pub bytes: Vec<u8>,
}

impl Message for Rec {
    fn read_from<R: Read>(reader: &mut R) -> Result<Rec> {
        Ok(Rec {
            x:          reader.read_u16::<BigEndian>()?,
            y:          reader.read_u16::<BigEndian>()?,
            width:      reader.read_u16::<BigEndian>()?,
            height:     reader.read_u16::<BigEndian>()?,
            bytes:      Vec::<u8>::read_from(reader)?
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.x)?;
        writer.write_u16::<BigEndian>(self.y)?;
        writer.write_u16::<BigEndian>(self.width)?;
        writer.write_u16::<BigEndian>(self.height)?;
        Vec::<u8>::write_to(&self.bytes, writer)?;
        Ok(())
    }
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
pub enum Error {
    Io(std::io::Error),
    Unexpected(String),
    Server(String),
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
            _ => f.write_str(&self.to_string())
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error { Error::Io(e) }
}
impl From<std::sync::mpsc::RecvError> for Error {
    fn from(e: std::sync::mpsc::RecvError) -> Error { 
        Error::Unexpected(format!("Channel recv error: {:?}",e)) 
    }
}
pub type Result<T> = std::result::Result<T, Error>;

