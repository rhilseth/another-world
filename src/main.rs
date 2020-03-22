use pretty_env_logger;

mod bank;
mod buffer;
mod engine;
mod font;
mod mixer;
mod opcode;
mod parts;
mod player;
mod resource;
mod sfxplayer;
mod strings;
mod sys;
mod video;
mod vm;

fn main() -> std::io::Result<()> {
    pretty_env_logger::init();
    let mut resource = resource::Resource::new();
    resource.read_memlist()?;

    let sdl_context = sdl2::init().unwrap();

    let sys = sys::SDLSys::new(sdl_context);
    let video = video::Video::new();
    let vm = vm::VirtualMachine::new(resource, video, sys);
    let mut engine = engine::Engine::new(vm);

    engine.run();
    Ok(())
}
