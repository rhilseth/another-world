use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::{thread, time};

use pretty_env_logger;
use structopt::StructOpt;

use anotherworld::input;
use anotherworld::mixer;
use anotherworld::resource;
use anotherworld::sys;
use anotherworld::video;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Another World resource inspector",
    about = "A tool to inspect Another World resources"
)]
struct Opt {
    /// Set path of game assets
    #[structopt(parse(from_os_str), long, default_value = "data", name = "PATH")]
    asset_path: PathBuf,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    List { },
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    pretty_env_logger::init();
    let memlist_reader = resource::MemlistReader::detect_platform(opt.asset_path);
    let mut res = memlist_reader.read_memlist()?;

    let sdl_context = sdl2::init().unwrap();

    let (width, height, _zoom) = if false {
        (640, 400, 2)
    } else {
        (320, 200, 1)
    };

    let palette = video::Palette {
        entries: [
            video::Color { r: 0, g: 0, b: 0, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
            video::Color { r: 255, g: 255, b: 255, a: 255 },
        ],
    };

    let mut video = video::Video::new(width, height);
    video.palette_requested = Some(palette);
    video.change_page_ptr1(0);

    let event_pump = sdl_context.event_pump().unwrap();
    let mut user_input = input::UserInput::new(event_pump);

    let mut sys = sys::SDLSys::new(sdl_context, width, height);

    let mixer = Arc::new(RwLock::new(mixer::Mixer::new()));
    sys.start_audio(mixer.clone());

    match opt.cmd {
        Command::List { } => {
            for i in 0..res.mem_list.len() {
                println!("i : {}", i);
                if res.mem_list[i].entry_type == resource::EntryType::Sound {
                    let resource_id = i as u16;
                    video.fill_video_page(0, 0);
                    video.draw_string(1, 1, 10, &format!("Resource: {:03} - {:#?}", i, res.mem_list[i].entry_type), 1);
                    video.update_display(&mut sys, 0);

                    res.load_memory_entry(resource_id);
                    if let Some(chunk) = res.get_entry_mixer_chunk(resource_id) {
                        let mut write_guard = mixer.write().expect("Expected non-poisoned RwLock");
                        let vol = 255;
                        write_guard.play_channel(0, chunk, 10000, vol);
                    }
                    if user_input.process_events().quit == true {
                        return Ok(());
                    }
                    res.invalidate_resource();
                    thread::sleep(time::Duration::from_millis(1000));
                }
            }
        },
    }
    Ok(())
}
