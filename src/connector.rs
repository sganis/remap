use anyhow::{Ok, Result};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::mpsc::{Receiver, Sender},
};
use crate::{Rec, ClientEvent, ServerEvent};


pub struct Connector<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream: S,
    width: u16,
    height: u16,
}

impl<S> Connector<S>
where S: AsyncRead + AsyncWrite + Unpin
{
    pub fn new(stream: S, width: u16, height: u16) -> Self {
        Self { stream, width, height }
    }

    pub async fn run(&mut self, 
        connector_tx: Sender<ServerEvent>,
        mut canvas_rx: Receiver<ClientEvent>,
    ) -> Result<()> {        
        let message = ClientEvent::FramebufferUpdateRequest {
            incremental: false, x: 0, y: 0,
            width: self.width, height: self.height,
        };
        message.write(&mut self.stream).await?;

        loop {
            tokio::select! {
                server_msg = ServerEvent::read(&mut self.stream) => {
                    let message = server_msg?;
                    connector_tx.send(message).await?
                }
                canvas_event = canvas_rx.recv() => {
                    if let Some(canvas_event) = canvas_event {
                        canvas_event.write(&mut self.stream).await?;
                    }
                }
            }
        }
    }
}

