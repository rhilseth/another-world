use pretty_env_logger;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

mod bank;
mod buffer;
mod engine;
mod opcode;
mod parts;
mod resource;
mod strings;
mod video;
mod vm;

fn main() -> std::io::Result<()> {
    pretty_env_logger::init();
    let mut resource = resource::Resource::new();
    resource.read_memlist()?;

    let video = video::Video::new();
    let vm = vm::VirtualMachine::new(resource, video);
    let mut engine = engine::Engine::new(vm);

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("Another world", 640, 400)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().expect("Expected canvas");
    canvas.set_logical_size(320, 200).expect("Expected logical size");
    let mut event_pump = sdl_context.event_pump().unwrap();
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
            engine.run();
        }
    }
    Ok(())
}
