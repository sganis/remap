pub mod util;
pub mod canvas;

#[cfg(target_os = "linux")]
pub mod capture;

#[cfg(target_os = "linux")]
pub mod input;

use anyhow::Result;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    pub width: i32,
    pub height: i32,
}

pub trait Message {
    fn read_from<R: Read>(reader: &mut R) -> Result<Self>
    where
        Self: Sized;
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}

#[derive(Debug)]
pub enum ClientEvent {
    SetEncodings(Vec<Encoding>),
    FramebufferUpdateRequest { incremental: bool, x: u16, y: u16, width: u16, height: u16 },
    KeyEvent { down: bool, key: u8 },
    PointerEvent { buttons: u8, x: u16, y: u16 },
    CutText(String),
}

impl Message for ClientEvent {
    fn read_from<R: Read>(reader: &mut R) -> Result<ClientEvent> {
        let message_type = match reader.read_u8() { Err(_) => anyhow::bail!("Disconnected"), Ok(mt) => mt };
        match message_type {
            2 => { reader.read_exact(&mut [0u8; 1])?;
                   let count = reader.read_u16::<BigEndian>()?;
                   let mut encodings = Vec::with_capacity(count as usize);
                   for _ in 0..count { encodings.push(Encoding::read_from(reader)?); }
                   Ok(ClientEvent::SetEncodings(encodings)) }
            3 => Ok(ClientEvent::FramebufferUpdateRequest {
                incremental: reader.read_u8()? != 0,
                x: reader.read_u16::<BigEndian>()?,
                y: reader.read_u16::<BigEndian>()?,
                width: reader.read_u16::<BigEndian>()?,
                height: reader.read_u16::<BigEndian>()?,
            }),
            4 => { let down = reader.read_u8()? != 0;
                   reader.read_exact(&mut [0u8; 2])?;
                   let key = reader.read_u8()?;
                   Ok(ClientEvent::KeyEvent { down, key }) }
            5 => Ok(ClientEvent::PointerEvent {
                buttons: reader.read_u8()?,
                x: reader.read_u16::<BigEndian>()?,
                y: reader.read_u16::<BigEndian>()?,
            }),
            6 => { reader.read_exact(&mut [0u8; 3])?;
                   Ok(ClientEvent::CutText(String::read_from(reader)?)) }
            _ => anyhow::bail!("client to server message type"),
        }
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            ClientEvent::SetEncodings(encodings) => {
                writer.write_u8(2)?; writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(encodings.len() as u16)?;
                for e in encodings { e.write_to(writer)?; }
            }
            ClientEvent::FramebufferUpdateRequest { incremental, x, y, width, height } => {
                writer.write_u8(3)?; writer.write_u8(if *incremental { 1 } else { 0 })?;
                writer.write_u16::<BigEndian>(*x)?; writer.write_u16::<BigEndian>(*y)?;
                writer.write_u16::<BigEndian>(*width)?; writer.write_u16::<BigEndian>(*height)?;
            }
            ClientEvent::KeyEvent { down, key } => {
                writer.write_u8(4)?; writer.write_u8(if *down { 1 } else { 0 })?;
                writer.write_all(&[0u8; 2])?; writer.write_u8(*key)?;
            }
            ClientEvent::PointerEvent { buttons, x, y } => {
                writer.write_u8(5)?; writer.write_u8(*buttons)?;
                writer.write_u16::<BigEndian>(*x)?; writer.write_u16::<BigEndian>(*y)?;
            }
            ClientEvent::CutText(text) => {
                writer.write_u8(6)?; writer.write_all(&[0u8; 3])?;
                text.write_to(writer)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ServerEvent {
    FramebufferUpdate { count: u16, rectangles: Vec<Rec> },
    Bell,
    CutText(String),
}

impl Message for ServerEvent {
    fn read_from<R: Read>(reader: &mut R) -> Result<ServerEvent> {
        let message_type = match reader.read_u8() { Err(_) => anyhow::bail!("Disconnected"), Ok(mt) => mt };
        match message_type {
            0 => {
                reader.read_exact(&mut [0u8; 1])?;
                let count = reader.read_u16::<BigEndian>()?;
                let mut rectangles = Vec::with_capacity(count as usize);
                for _ in 0..count { rectangles.push(Rec::read_from(reader)?); }
                Ok(ServerEvent::FramebufferUpdate { count, rectangles })
            }
            2 => Ok(ServerEvent::Bell),
            3 => { reader.read_exact(&mut [0u8; 3])?;
                   Ok(ServerEvent::CutText(String::read_from(reader)?)) }
            _ => anyhow::bail!("server to client message type"),
        }
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            ServerEvent::FramebufferUpdate { count, rectangles } => {
                writer.write_u8(0)?; writer.write_all(&[0u8; 1])?;
                writer.write_u16::<BigEndian>(*count)?;
                for r in rectangles { r.write_to(writer)?; }
            }
            ServerEvent::Bell => { writer.write_u8(2)?; }
            ServerEvent::CutText(text) => {
                writer.write_u8(3)?; writer.write_all(&[0u8; 3])?;
                text.write_to(writer)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub bytes: Vec<u8>,
}

impl Message for Rec {
    fn read_from<R: Read>(reader: &mut R) -> Result<Rec> {
        Ok(Rec {
            x: reader.read_u16::<BigEndian>()?,
            y: reader.read_u16::<BigEndian>()?,
            width: reader.read_u16::<BigEndian>()?,
            height: reader.read_u16::<BigEndian>()?,
            bytes: Vec::<u8>::read_from(reader)?,
        })
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.x)?;
        writer.write_u16::<BigEndian>(self.y)?;
        writer.write_u16::<BigEndian>(self.width)?;
        writer.write_u16::<BigEndian>(self.height)?;
        self.bytes.write_to(writer)?;
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
        writer.write_u32::<BigEndian>(self.len() as u32)?;
        writer.write_all(self)?;
        Ok(())
    }
}

impl Message for String {
    fn read_from<R: Read>(reader: &mut R) -> Result<String> {
        let length = reader.read_u32::<BigEndian>()?;
        let mut bytes = vec![0; length as usize];
        reader.read_exact(&mut bytes)?;
        Ok(bytes.into_iter().map(|c| c as char).collect())
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let bytes: Vec<u8> = self.chars().map(|c| c as u8).collect();
        writer.write_u32::<BigEndian>(bytes.len() as u32)?;
        writer.write_all(&bytes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Unknown(i32),
    Raw, CopyRect, Rre, Hextile, Zrle, Cursor, DesktopSize,
}

impl Message for Encoding {
    fn read_from<R: Read>(reader: &mut R) -> Result<Encoding> {
        let encoding = reader.read_i32::<BigEndian>()?;
        Ok(match encoding {
            0 => Encoding::Raw,
            1 => Encoding::CopyRect,
            2 => Encoding::Rre,
            5 => Encoding::Hextile,
            16 => Encoding::Zrle,
            -239 => Encoding::Cursor,
            -223 => Encoding::DesktopSize,
            n => Encoding::Unknown(n),
        })
    }
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let n = match self {
            Encoding::Raw => 0,
            Encoding::CopyRect => 1,
            Encoding::Rre => 2,
            Encoding::Hextile => 5,
            Encoding::Zrle => 16,
            Encoding::Cursor => -239,
            Encoding::DesktopSize => -223,
            Encoding::Unknown(n) => *n,
        };
        writer.write_i32::<BigEndian>(n)?;
        Ok(())
    }
}
