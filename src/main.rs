use std::path::PathBuf;

use pretty_env_logger;
use structopt::StructOpt;

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

#[derive(Debug, StructOpt)]
#[structopt(name = "Another World", about = "A virtual machine for running Another World")]
struct Opt {
    /// Set path of game assets
    #[structopt(parse(from_os_str), long, default_value = "data", name = "PATH")]
    asset_path: PathBuf,
    /// Run with Amiga assets
    #[structopt(long)]
    amiga: bool,
    /// Start with game part
    #[structopt(long, default_value = "2")]
    game_part: u8,
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    pretty_env_logger::init();
    let mut resource = resource::Resource::new(opt.asset_path, opt.amiga);
    resource.read_memlist()?;

    let sdl_context = sdl2::init().unwrap();

    let sys = sys::SDLSys::new(sdl_context);
    let video = video::Video::new();
    let vm = vm::VirtualMachine::new(resource, video, sys);
    let mut engine = engine::Engine::new(vm, opt.game_part);

    engine.run();
    Ok(())
}
