#![allow(unused)]
pub mod util;
pub mod canvas;
#[cfg(unix)]
pub mod capture;

#[cfg(unix)]
pub mod input;

use std::io::{Read, Write};
use anyhow::Result;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    pub width: i32, 
    pub height: i32
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
        x:  u16,
        y:  u16,
        width:       u16,
        height:      u16,
    },
    KeyEvent {
        down:        bool,
        key:         u8,
    },
    PointerEvent {
        buttons: u8,
        x:  u16,
        y:  u16
    },
    CutText(String),
    // extensions
}

impl Message for ClientEvent {
    fn read_from<R: Read>(reader: &mut R) -> Result<ClientEvent> {
        let message_type =
            match reader.read_u8() {
                Err(_)  => return anyhow::bail!("Disconnected"),
                result => result?
            };
        match message_type {
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
                    x:  reader.read_u16::<BigEndian>()?,
                    y:  reader.read_u16::<BigEndian>()?,
                    width:       reader.read_u16::<BigEndian>()?,
                    height:      reader.read_u16::<BigEndian>()?
                })
            },
            4 => {
                let down = reader.read_u8()? != 0;
                reader.read_exact(&mut [0u8; 2])?;
                let key = reader.read_u8()?;
                Ok(ClientEvent::KeyEvent { down, key })
            },
            5 => {
                Ok(ClientEvent::PointerEvent {
                    buttons: reader.read_u8()?,
                    x:  reader.read_u16::<BigEndian>()?,
                    y:  reader.read_u16::<BigEndian>()?
                })
            },
            6 => {
                reader.read_exact(&mut [0u8; 3])?;
                Ok(ClientEvent::CutText(String::read_from(reader)?))
            },
            _ => anyhow::bail!("client to server message type")
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
                incremental, x, y, width, height 
            } => {
                writer.write_u8(3)?;
                writer.write_u8(if *incremental { 1 } else { 0 })?;
                writer.write_u16::<BigEndian>(*x)?;
                writer.write_u16::<BigEndian>(*y)?;
                writer.write_u16::<BigEndian>(*width)?;
                writer.write_u16::<BigEndian>(*height)?;
            },
            ClientEvent::KeyEvent { down, key } => {
                writer.write_u8(4)?;
                writer.write_u8(if *down { 1 } else { 0 })?;
                writer.write_all(&[0u8; 2])?;
                writer.write_u8(*key)?;
            },
            ClientEvent::PointerEvent { buttons, x, y } => {
                writer.write_u8(5)?;
                writer.write_u8(*buttons)?;
                writer.write_u16::<BigEndian>(*x)?;
                writer.write_u16::<BigEndian>(*y)?;
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
                Err(_) => return anyhow::bail!("Disconnected"),
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
            _ => anyhow::bail!("server to client message type")
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
    pub src_x: u16,
    pub src_y: u16,
}

impl Message for CopyRect {
    fn read_from<R: Read>(reader: &mut R) -> Result<CopyRect> {
        Ok(CopyRect {
            src_x: reader.read_u16::<BigEndian>()?,
            src_y: reader.read_u16::<BigEndian>()?
        })
    }

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.src_x)?;
        writer.write_u16::<BigEndian>(self.src_y)?;
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

