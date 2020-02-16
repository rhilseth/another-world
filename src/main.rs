use pretty_env_logger;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

mod bank;
mod buffer;
mod engine;
mod mixer;
mod opcode;
mod parts;
mod resource;
mod strings;
mod sys;
mod video;
mod vm;

fn main() -> std::io::Result<()> {
    pretty_env_logger::init();
    let mut resource = resource::Resource::new();
    resource.read_memlist()?;

    let sdl_context = sdl2::init().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let sys = sys::SDLSys::new(sdl_context);
    let video = video::Video::new();
    let vm = vm::VirtualMachine::new(resource, video, sys);
    let mut engine = engine::Engine::new(vm);

    'outer: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'outer,
                _ => {}
            }
        }
        engine.run();
    }
    Ok(())
}
