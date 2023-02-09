use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use softbuffer;
use std::time::{Duration, Instant};


fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let context = unsafe { softbuffer::Context::new(&window) }.unwrap();
    let mut surface = unsafe { softbuffer::Surface::new(&context, &window) }.unwrap();

    let mut cur_keys = [false; 255];
    let mut prv_keys = cur_keys.clone();
    let mut cur_pos : (u16, u16) = (0,0);
    
    let mut frames = 0;
    let mut start = Instant::now();
    
    event_loop.run(move |event, _, control_flow| {
        frames += 1;
        if start.elapsed().as_secs() >= 1 {
            println!("{:.0}", frames as f64 / start.elapsed().as_millis() as f64 * 1000.0);
            start = std::time::Instant::now();
            frames = 0;
        }

        let next_frame_time = Instant::now() + Duration::from_millis(30_000);
        *control_flow = ControlFlow::WaitUntil(next_frame_time);

        match event {
            Event::NewEvents(_) => {
                // Leave now_keys alone, but copy over all changed keys
                prv_keys.copy_from_slice(&cur_keys);
            },
            Event::RedrawRequested(window_id) => {
                let (width, height) = {
                    let size = window.inner_size();
                    (size.width, size.height)
                };
                let buffer = (0..((width * height) as usize))
                    .map(|index| {
                        let y = index / (width as usize);
                        let x = index % (width as usize);
                        let red = x % 255;
                        let green = y % 255;
                        let blue = (x * y) % 255;
                        let color = blue | (green << 8) | (red << 16);
                        color as u32
                    })
                    .collect::<Vec<_>>();

                surface.set_buffer(&buffer, width as u16, height as u16);
            },
            Event::WindowEvent { 
                event: WindowEvent::CloseRequested, .. 
            } => {
                *control_flow = ControlFlow::Exit;
            },
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. }, .. 
            } => {
                println!("Mouse state: {:?} button: {:?}, pos: {:?}", 
                    state, button, cur_pos);
            },
            Event::WindowEvent {
                event: WindowEvent::MouseWheel { delta, phase, .. }, .. 
            } => {
                println!("Mouse wheel delta: {:?} phase: {:?}, pos: {:?}", 
                    delta, phase, cur_pos);
            },
           
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. 
                }, .. 
            } => {
                cur_pos = ( position.x as u16, position.y as u16 );
                //println!("Cursor moved position: {:?}", position);
            },
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput {
                    input:winit::event::KeyboardInput {
                        virtual_keycode:Some(keycode), state, .. 
                    }, .. 
                }, .. 
            } => {
                match state {
                    winit::event::ElementState::Pressed => {
                        cur_keys[keycode as usize] = true;
                        println!("key pressed : {:?}", keycode);
                    },
                    winit::event::ElementState::Released => {
                        cur_keys[keycode as usize] = false;
                        println!("key released: {:?}", keycode);
                    }
                }
            },            
            _ => {}
        }

    });
}