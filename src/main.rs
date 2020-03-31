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
mod util;
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
    /// Disable protection bypass
    #[structopt(long)]
    no_bypass: bool,
    /// Enable hires graphics
    #[structopt(long)]
    hires: bool,
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    pretty_env_logger::init();
    let mut resource = resource::Resource::new(opt.asset_path, opt.amiga);
    resource.read_memlist()?;

    let sdl_context = sdl2::init().unwrap();

    let (width, height, zoom) = if opt.hires {
        (640, 400, 2)
    } else {
        (320, 200, 1)
    };

    let sys = sys::SDLSys::new(sdl_context, width, height);
    let video = video::Video::new(width, height);
    let mut vm = vm::VirtualMachine::new(resource, video, sys, zoom);
    if !opt.no_bypass {
        vm.set_variable(0xbc, 0x10);
        vm.set_variable(0xc6, 0x80);
        vm.set_variable(0xdc, 33);
        let value = if opt.amiga {
            6000
        } else {
            4000
        };
        vm.set_variable(0xf2, value);
    }

    let mut engine = engine::Engine::new(vm, opt.game_part);

    engine.run();
    Ok(())
}
