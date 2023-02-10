use std::sync::Arc;
use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf},
    sync::mpsc::{Receiver, Sender}, 
    net::TcpStream,
};
use crate::{ClientEvent, ServerEvent};

pub struct Client<T>
where T: AsyncRead + AsyncWrite + Unpin
{
    stream: T,
    width: u16,
    height: u16,
}

impl<T> Client<T>
where T: AsyncRead + AsyncWrite + Unpin
{
    pub fn new(stream: T, width: u16, height: u16) -> Self {
        Self { stream, width, height }
    }

    pub async fn run(&mut self, 
        client_tx: Sender<ServerEvent>,
        mut canvas_rx: Receiver<ClientEvent>) -> Result<()> {              
        // first request full image (incremental=false)
        let message = ClientEvent::FramebufferUpdateRequest {
            incremental: false, x: 0, y: 0,
            width: self.width, height: self.height,
        };
        message.write(&mut self.stream).await?;
        
        // // reader
        // let mut reader = self.stream.try_clone().expect("could not clone the stream");

        // thread::spawn(move || {
        //     let mut stout = io::stdout();
        //     io::copy(&mut stream2, &mut stout).expect("error while reading from the stream");
        //     loop {
        //         let server_msg = ServerEvent::read(&mut reader).await.unwrap();
        //         println!("recieved from server: {:?}", server_msg);
        //         client_tx.send(server_msg).await.unwrap();      
        //     }


        // });
        
        // tokio::spawn(async move { 
        //     let reader = reader.as_ref();
        //     loop {
        //         let server_msg = ServerEvent::read(&mut reader).await.unwrap();
        //         println!("recieved from server: {:?}", server_msg);
        //         client_tx.send(server_msg).await.unwrap();      
        //     }


        // });

        loop {
            // // // send input events
            // while let Ok(client_msg) = canvas_rx.try_recv() {
            //     println!("recieved from canvas: {:?}", client_msg);
            //     client_msg.write(&mut self.stream).await?;                 
            // }

            // // get server reply
            // let server_msg = ServerEvent::read(&mut self.stream).await?;
            // println!("recieved from server: {:?}", server_msg);
            // client_tx.send(server_msg).await?
        
            tokio::select! {
                client_msg = canvas_rx.recv() => {
                    println!("recieved from canvas: {:?}", client_msg);
                    if let Some(client_msg) = client_msg {
                        client_msg.write(&mut self.stream).await?;
                    }
                }
                server_msg = ServerEvent::read(&mut self.stream) => {
                    //println!("recieved from server: {:?}", server_msg);
                    let message = server_msg?;
                    client_tx.send(message).await?
                }                
            }
        }
        
        Ok(())
    }
}

