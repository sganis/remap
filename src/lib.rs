#![allow(unused)]

pub mod util;
pub mod canvas;

#[cfg(target_os = "linux")]
pub mod capture;

#[cfg(target_os = "linux")]
pub mod input;

use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

/* ============================== Core Types ============================== */

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    pub width: i32,
    pub height: i32,
}

/* ===== Modifier bitmask shared by client & server ===== */
pub const MOD_SHIFT: u16 = 0x0001;
pub const MOD_CTRL:  u16 = 0x0002;
pub const MOD_ALT:   u16 = 0x0004; // Option on macOS
pub const MOD_META:  u16 = 0x0008; // Super/Command/Windows

/* ========================= Async BE helpers ============================ */

async fn read_u8<R: AsyncRead + Unpin>(r: &mut R) -> Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).await?;
    Ok(b[0])
}
async fn read_u16_be<R: AsyncRead + Unpin>(r: &mut R) -> Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).await?;
    Ok(u16::from_be_bytes(b))
}
async fn read_u32_be<R: AsyncRead + Unpin>(r: &mut R) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).await?;
    Ok(u32::from_be_bytes(b))
}
async fn read_i32_be<R: AsyncRead + Unpin>(r: &mut R) -> Result<i32> {
    Ok(read_u32_be(r).await? as i32)
}
async fn write_u8<W: AsyncWrite + Unpin>(w: &mut W, v: u8) -> Result<()> {
    w.write_all(&[v]).await?;
    Ok(())
}
async fn write_u16_be<W: AsyncWrite + Unpin>(w: &mut W, v: u16) -> Result<()> {
    w.write_all(&v.to_be_bytes()).await?;
    Ok(())
}
async fn write_u32_be<W: AsyncWrite + Unpin>(w: &mut W, v: u32) -> Result<()> {
    w.write_all(&v.to_be_bytes()).await?;
    Ok(())
}
async fn write_i32_be<W: AsyncWrite + Unpin>(w: &mut W, v: i32) -> Result<()> {
    write_u32_be(w, v as u32).await
}

/* --- length-prefixed bytes & strings --- */
async fn read_vec_be<R: AsyncRead + Unpin>(r: &mut R) -> Result<Vec<u8>> {
    let len = read_u32_be(r).await? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}
async fn write_vec_be<W: AsyncWrite + Unpin>(w: &mut W, v: &[u8]) -> Result<()> {
    write_u32_be(w, v.len() as u32).await?;
    w.write_all(v).await?;
    Ok(())
}
async fn read_string_be<R: AsyncRead + Unpin>(r: &mut R) -> Result<String> {
    let bytes = read_vec_be(r).await?;
    Ok(String::from_utf8(bytes).unwrap_or_else(|e| {
        e.into_bytes().into_iter().map(|c| c as char).collect()
    }))
}
async fn write_string_be<W: AsyncWrite + Unpin>(w: &mut W, s: &str) -> Result<()> {
    write_vec_be(w, s.as_bytes()).await
}

/* ========================= Client → Server ============================== */

#[derive(Debug)]
pub enum ClientEvent {
    SetEncodings(Vec<Encoding>),
    FramebufferUpdateRequest {
        incremental: bool,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    KeyEvent {
        down: bool,
        key: u8,
        mods: u16,
    },
    PointerEvent {
        buttons: u8,
        x: u16,
        y: u16,
    },
    CutText(String),
    ClientResize {
        width: u16,
        height: u16,
    },
}

impl ClientEvent {
    pub async fn read<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
        let message_type = read_u8(reader).await?;
        match message_type {
            2 => {
                // SetEncodings
                let mut pad = [0u8; 1];
                reader.read_exact(&mut pad).await?;
                let count = read_u16_be(reader).await?;
                let mut encodings = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    encodings.push(Encoding::read(reader).await?);
                }
                Ok(ClientEvent::SetEncodings(encodings))
            }
            3 => {
                let incremental = read_u8(reader).await? != 0;
                let x = read_u16_be(reader).await?;
                let y = read_u16_be(reader).await?;
                let width = read_u16_be(reader).await?;
                let height = read_u16_be(reader).await?;
                Ok(ClientEvent::FramebufferUpdateRequest { incremental, x, y, width, height })
            }
            4 => {
                let down = read_u8(reader).await? != 0;
                let mods = read_u16_be(reader).await?;
                let key = read_u8(reader).await?;
                Ok(ClientEvent::KeyEvent { down, key, mods })
            }
            5 => {
                let buttons = read_u8(reader).await?;
                let x = read_u16_be(reader).await?;
                let y = read_u16_be(reader).await?;
                Ok(ClientEvent::PointerEvent { buttons, x, y })
            }
            6 => {
                let mut pad = [0u8; 3];
                reader.read_exact(&mut pad).await?;
                Ok(ClientEvent::CutText(read_string_be(reader).await?))
            }
            7 => {
                let width = read_u16_be(reader).await?;
                let height = read_u16_be(reader).await?;
                Ok(ClientEvent::ClientResize { width, height })
            }
            _ => anyhow::bail!("unsupported client to server message type: {}", message_type),
        }
    }

    pub async fn write<W>(&self, writer: &mut W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            ClientEvent::SetEncodings(encodings) => {
                write_u8(writer, 2).await?;
                writer.write_all(&[0u8; 1]).await?;
                write_u16_be(writer, encodings.len() as u16).await?;
                for e in encodings {
                    e.write(writer).await?;
                }
            }
            ClientEvent::FramebufferUpdateRequest { incremental, x, y, width, height } => {
                write_u8(writer, 3).await?;
                write_u8(writer, if *incremental { 1 } else { 0 }).await?;
                write_u16_be(writer, *x).await?;
                write_u16_be(writer, *y).await?;
                write_u16_be(writer, *width).await?;
                write_u16_be(writer, *height).await?;
            }
            ClientEvent::KeyEvent { down, key, mods } => {
                write_u8(writer, 4).await?;
                write_u8(writer, if *down { 1 } else { 0 }).await?;
                write_u16_be(writer, *mods).await?;
                write_u8(writer, *key).await?;
            }
            ClientEvent::PointerEvent { buttons, x, y } => {
                write_u8(writer, 5).await?;
                write_u8(writer, *buttons).await?;
                write_u16_be(writer, *x).await?;
                write_u16_be(writer, *y).await?;
            }
            ClientEvent::CutText(text) => {
                write_u8(writer, 6).await?;
                writer.write_all(&[0u8; 3]).await?;
                write_string_be(writer, text).await?;
            }
            ClientEvent::ClientResize { width, height } => {
                write_u8(writer, 7).await?;
                write_u16_be(writer, *width).await?;
                write_u16_be(writer, *height).await?;
            }
        }
        Ok(())
    }
}

/* ========================= Server → Client ============================== */

#[derive(Debug)]
pub enum ServerEvent {
    FramebufferUpdate { count: u16, rectangles: Vec<Rec> },
    SetColorMapEntries { first: u16, colors: Vec<(u16, u16, u16)> },
    Bell,
    CutText(String),
}

impl ServerEvent {
    pub async fn read<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
        let message_type = read_u8(reader).await?;
        match message_type {
            0 => {
                // FramebufferUpdate — 1-BYTE PAD
                let mut pad = [0u8; 1];
                reader.read_exact(&mut pad).await?;
                let count = read_u16_be(reader).await?;
                let mut rectangles = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    rectangles.push(Rec::read(reader).await?);
                }
                Ok(ServerEvent::FramebufferUpdate { count, rectangles })
            }
            1 => {
                // SetColorMapEntries — 1-BYTE PAD
                let mut pad = [0u8; 1];
                reader.read_exact(&mut pad).await?;
                let first = read_u16_be(reader).await?;
                let n = read_u16_be(reader).await?;
                let mut colors = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    let r = read_u16_be(reader).await?;
                    let g = read_u16_be(reader).await?;
                    let b = read_u16_be(reader).await?;
                    colors.push((r, g, b));
                }
                Ok(ServerEvent::SetColorMapEntries { first, colors })
            }
            2 => Ok(ServerEvent::Bell),
            3 => {
                // ServerCutText — 3-BYTE PAD
                let mut pad3 = [0u8; 3];
                reader.read_exact(&mut pad3).await?;
                let text = read_string_be(reader).await?;
                Ok(ServerEvent::CutText(text))
            }
            other => anyhow::bail!("unsupported server to client message type {}", other),
        }
    }

    pub async fn write<W>(&self, writer: &mut W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            ServerEvent::FramebufferUpdate { count, rectangles } => {
                write_u8(writer, 0).await?;
                writer.write_all(&[0u8; 1]).await?; // pad
                write_u16_be(writer, *count).await?;
                for r in rectangles {
                    r.write(writer).await?;
                }
            }
            ServerEvent::SetColorMapEntries { first, colors } => {
                write_u8(writer, 1).await?;
                writer.write_all(&[0u8; 1]).await?; // pad
                write_u16_be(writer, *first).await?;
                write_u16_be(writer, colors.len() as u16).await?;
                for (r, g, b) in colors {
                    write_u16_be(writer, *r).await?;
                    write_u16_be(writer, *g).await?;
                    write_u16_be(writer, *b).await?;
                }
            }
            ServerEvent::Bell => {
                write_u8(writer, 2).await?;
            }
            ServerEvent::CutText(text) => {
                write_u8(writer, 3).await?;
                writer.write_all(&[0u8; 3]).await?; // pad
                write_string_be(writer, text).await?;
            }
        }
        Ok(())
    }
}

/* ========================= Raw pixel rectangles ========================= */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub bytes: Vec<u8>,
}

impl Rec {
    pub async fn read<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
        let x = read_u16_be(reader).await?;
        let y = read_u16_be(reader).await?;
        let width = read_u16_be(reader).await?;
        let height = read_u16_be(reader).await?;
        let bytes = read_vec_be(reader).await?;
        Ok(Rec { x, y, width, height, bytes })
    }

    pub async fn write<W>(&self, writer: &mut W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        write_u16_be(writer, self.x).await?;
        write_u16_be(writer, self.y).await?;
        write_u16_be(writer, self.width).await?;
        write_u16_be(writer, self.height).await?;
        write_vec_be(writer, &self.bytes).await?;
        Ok(())
    }
}

/* ============================== Encodings ============================== */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Unknown(i32),
    Raw,
    CopyRect,
    Rre,
    Hextile,
    Zrle,
    Cursor,
    DesktopSize,
}

impl Encoding {
    pub async fn read<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
        let encoding = read_i32_be(reader).await?;
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

    pub async fn write<W>(&self, writer: &mut W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
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
        write_i32_be(writer, n).await
    }
}