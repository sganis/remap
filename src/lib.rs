#![allow(unused)]
pub mod util;
pub mod canvas;
pub mod client;
pub mod input;

#[cfg(unix)]
pub mod capture;

use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};


#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    pub width: i32, 
    pub height: i32
}

#[derive(Debug)]
pub enum ClientEvent {
    FramebufferUpdateRequest {
        incremental: bool,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    KeyEvent {
        down: bool,
        key: u32,
    },
    PointerEvent {
        button_mask: u8,
        x:  u16,
        y:  u16
    },
}

impl ClientEvent {
    pub async fn read<S>(reader: &mut S) -> Result<Self>
    where S: AsyncRead + Unpin 
    {
        let message = reader.read_u8().await?;
        match message {
            3 => {
                Ok(ClientEvent::FramebufferUpdateRequest {
                    incremental: reader.read_u8().await? != 0,
                    x: reader.read_u16().await?,
                    y: reader.read_u16().await?,
                    width: reader.read_u16().await?,
                    height: reader.read_u16().await?
                })
            },
            4 => {
                let down = reader.read_u8().await? != 0;
                reader.read_exact(&mut [0u8; 2]).await?;
                let key = reader.read_u32().await?;
                Ok(ClientEvent::KeyEvent { down, key })
            },
            5 => {
                Ok(ClientEvent::PointerEvent {
                    button_mask: reader.read_u8().await?,
                    x:  reader.read_u16().await?,
                    y:  reader.read_u16().await?
                })
            },
            _ => anyhow::bail!("Unexpected message")
        }
    }
    pub async fn write<S>(&self, writer: &mut S) -> Result<()>
    where S: AsyncWrite + Unpin 
    {
        match self {
            ClientEvent::FramebufferUpdateRequest { 
                incremental, x, y, width, height } => {
                writer.write_u8(3).await?;
                writer.write_u8(if *incremental { 1 } else { 0 }).await?;
                writer.write_u16(*x).await?;
                writer.write_u16(*y).await?;
                writer.write_u16(*width).await?;
                writer.write_u16(*height).await?;
            },
            ClientEvent::KeyEvent { down, key } => {
                writer.write_u8(4).await?;
                writer.write_u8(if *down { 1 } else { 0 }).await?;
                writer.write_all(&[0u8; 2]).await?;
                writer.write_u32(*key).await?;
            },
            ClientEvent::PointerEvent { button_mask, x, y } => {
                writer.write_u8(5).await?;
                writer.write_u8(*button_mask).await?;
                writer.write_u16(*x).await?;
                writer.write_u16(*y).await?;
            },
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
}

impl ServerEvent {
    pub async fn read<S>(reader: &mut S) -> Result<Self>
    where S: AsyncRead + Unpin
    {
        let message_type = reader.read_u8().await?;
        println!("message type: {}", message_type);

        match message_type {
            0 => {
                reader.read_exact(&mut [0u8; 1]).await?;
                let count = reader.read_u16().await?; 
                println!("count: {}", count);
        
                let mut rectangles = Vec::<Rec>::new();
                for _ in 0..count {
                    let r = Rec::read(reader).await?;
                    rectangles.push(r);
                }
                Ok(ServerEvent::FramebufferUpdate {
                    count,
                    rectangles,
                })
            },            
            _ => anyhow::bail!("cannot read server to client message type")
        }
    }

    pub async fn write<S>(&self, writer: &mut S) -> Result<()>
    where S: AsyncWrite + Unpin 
    {
        match self {
            ServerEvent::FramebufferUpdate { count, rectangles } => {
                writer.write_u8(0).await?;
                writer.write_all(&[0u8; 1]).await?;
                writer.write_u16(*count).await?;
                for r in rectangles.iter() {
                    r.write(writer).await?;
                }
            },            
            _ => anyhow::bail!("cannot write server to client message type")
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

impl Rec {
    async fn read<S>(reader: &mut S) -> Result<Self>
    where S: AsyncRead + Unpin 
    {
        let x = reader.read_u16().await?;
        let y = reader.read_u16().await?;
        let width = reader.read_u16().await?;
        let height = reader.read_u16().await?;
        let length = reader.read_u32().await?;
        let mut bytes = vec![0; length as usize];
        reader.read_exact(&mut bytes).await?;
        Ok(Rec { x,y,width,height,bytes })        
    }

    pub async fn write<S>(&self, writer: &mut S) -> Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        writer.write_u16(self.x).await?;
        writer.write_u16(self.y).await?;
        writer.write_u16(self.width).await?;
        writer.write_u16(self.height).await?;
        let length = self.bytes.len() as u32;
        writer.write_u32(length).await?;
        writer.write_all(&self.bytes).await?;
        Ok(())
    }
}


