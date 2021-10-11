use std::path::PathBuf;

use pretty_env_logger;
use structopt::StructOpt;

use anotherworld::engine;
use anotherworld::input;
use anotherworld::resource;
use anotherworld::resource::AssetPlatform;
use anotherworld::sys;
use anotherworld::video;
use anotherworld::vm;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Another World",
    about = "A virtual machine for running Another World"
)]
struct Opt {
    /// Set path of game assets
    #[structopt(parse(from_os_str), long, default_value = "data", name = "PATH")]
    asset_path: PathBuf,
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
    let memlist_reader = resource::MemlistReader::detect_platform(opt.asset_path);
    let resource = memlist_reader.read_memlist()?;
    let asset_platform = resource.asset_platform;

    let sdl_context = sdl2::init().unwrap();

    let (width, height, zoom) = if opt.hires {
        (640, 400, 2)
    } else {
        (320, 200, 1)
    };

    let event_pump = sdl_context.event_pump().unwrap();
    let user_input = input::UserInput::new(event_pump);
    let sys = sys::SDLSys::new(sdl_context, width, height);
    let video = video::Video::new(width, height);
    let mut vm = vm::VirtualMachine::new(resource, video, sys, user_input,zoom);
    if !opt.no_bypass {
        vm.set_variable(0xbc, 0x10);
        vm.set_variable(0xc6, 0x80);
        vm.set_variable(0xdc, 33);
        let value = match asset_platform {
            AssetPlatform::Amiga | AssetPlatform::AtariST => 6000,
            AssetPlatform::PC => 4000,
        };
        vm.set_variable(0xf2, value);
    }

    let mut engine = engine::Engine::new(vm, opt.game_part);

    engine.run();
    Ok(())
}
