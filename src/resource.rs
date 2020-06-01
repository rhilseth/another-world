use std::fs::File;
use std::io::prelude::*;
use std::io::{Error, ErrorKind, SeekFrom};
use std::path::{Path, PathBuf};

use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use log::{debug, info, warn};

use crate::bank::Bank;
use crate::buffer::Buffer;
use crate::mixer::MixerChunk;
use crate::parts;
use crate::sfxplayer::{SfxInstrument, SfxModule};

const MEM_BLOCK_SIZE: usize = 600 * 1024;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AssetPlatform {
    PC,
    Amiga,
    AtariST,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum MemEntryState {
    NotNeeded = 0,
    Loaded,
    LoadMe,
    EndOfMemList = 0xff,
}

impl MemEntryState {
    fn from_u8(val: u8) -> Self {
        match val {
            0 => MemEntryState::NotNeeded,
            1 => MemEntryState::Loaded,
            2 => MemEntryState::LoadMe,
            0xff => MemEntryState::EndOfMemList,
            _ => panic!("Unknown MemEntryState: {}", val),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum EntryType {
    Sound,
    Music,
    PolyAnim,
    Palette,
    Bytecode,
    PolyCinematic,
    Unknown(u8),
}

impl EntryType {
    fn from_u8(val: u8) -> Self {
        match val {
            0 => EntryType::Sound,
            1 => EntryType::Music,
            2 => EntryType::PolyAnim,
            3 => EntryType::Palette,
            4 => EntryType::Bytecode,
            5 => EntryType::PolyCinematic,
            n => EntryType::Unknown(n),
        }
    }
}

#[derive(Debug)]
pub struct MemEntry {
    state: MemEntryState,
    entry_type: EntryType,
    buf_ptr: usize,
    unk4: u16,
    rank_num: u8,
    bank_id: u8,
    bank_offset: u32,
    unkc: u16,
    packed_size: usize,
    unk10: u16,
    size: usize,
}

pub struct Resource {
    mem_list: Vec<MemEntry>,
    pub memory: Vec<u8>,
    pub current_part_id: u16,
    script_bak_ptr: usize,
    script_cur_ptr: usize,
    vid_bak_ptr: usize,
    vid_cur_ptr: usize,
    pub seg_palettes: usize,
    pub seg_bytecode: usize,
    pub seg_cinematic: usize,
    pub seg_video2: usize,
    pub copy_vid_ptr: bool,
    asset_path: PathBuf,
    pub asset_platform: AssetPlatform,
}

impl Resource {
    pub fn new(asset_path: PathBuf, asset_platform: AssetPlatform) -> Resource {
        Resource {
            mem_list: Vec::new(),
            memory: vec![0; MEM_BLOCK_SIZE],
            current_part_id: 0,
            script_bak_ptr: 0,
            script_cur_ptr: 0,
            vid_bak_ptr: MEM_BLOCK_SIZE - 0x800 * 16,
            vid_cur_ptr: MEM_BLOCK_SIZE - 0x800 * 16,
            seg_palettes: 0,
            seg_bytecode: 0,
            seg_cinematic: 0,
            seg_video2: 0,
            copy_vid_ptr: false,
            asset_path,
            asset_platform,
        }
    }

    pub fn detect_platform(asset_path: PathBuf) -> Resource {
        let asset_platform = if asset_path.join("another").exists() {
            info!("Detected Amiga binary");
            AssetPlatform::Amiga
        } else if asset_path.join("START.PRG").exists() {
            info!("Detected Atari ST binary");
            AssetPlatform::AtariST
        } else {
            info!("Assuming PC / Memlist.bin version");
            AssetPlatform::PC
        };
        Resource::new(asset_path, asset_platform)
    }

    fn find_memlist_offset<R: Read>(reader: &mut R) -> std::io::Result<u64> {
        let mut count = 0;
        for (offset, b) in reader.bytes().enumerate() {
            if b? == 0xff {
                count += 1;
            } else {
                count = 0;
            }
            if count == 20 {
                return Ok(offset as u64 - 2939);
            }
        }
        Err(Error::new(
            ErrorKind::UnexpectedEof,
            "Did not find memlist before eof",
        ))
    }

    fn read_memlist_from_executable(&mut self, executable_name: &str) -> std::io::Result<()> {
        let path = self.asset_path.join(executable_name);
        let mut file = File::open(&path)?;
        let offset = Resource::find_memlist_offset(&mut file)?;
        file.seek(SeekFrom::Start(offset))?;
        self.read_entries(&mut file);
        Ok(())
    }

    pub fn read_memlist(&mut self) -> std::io::Result<()> {
        match self.asset_platform {
            AssetPlatform::PC => {
                let path = self.asset_path.join("Memlist.bin");
                let mut file = File::open(path)?;
                self.read_entries(&mut file);
            }
            AssetPlatform::Amiga => {
                self.read_memlist_from_executable("another")?;
            }
            AssetPlatform::AtariST => {
                self.read_memlist_from_executable("START.PRG")?;
            }
        }
        Ok(())
    }

    pub fn setup_part(&mut self, part_id: u16) {
        debug!("setup_part: {}", part_id);
        if part_id == self.current_part_id {
            return;
        }

        if part_id < parts::GAME_PART_FIRST || part_id > parts::GAME_PART_LAST {
            panic!("Unknown part: {:x}", part_id);
        }

        let index = (part_id - parts::GAME_PART_FIRST) as usize;
        debug!("Part id index: {}", index);

        let palette_index = parts::PARTS[index].palette;
        let code_index = parts::PARTS[index].code;
        debug!("Code index: {}", code_index);
        let video_cinematic_index = parts::PARTS[index].video1;
        let video2_index = parts::PARTS[index].video2;

        self.invalidate_all();

        self.mem_list[palette_index].state = MemEntryState::LoadMe;
        self.mem_list[code_index].state = MemEntryState::LoadMe;
        self.mem_list[video_cinematic_index].state = MemEntryState::LoadMe;

        if let Some(video2_index) = video2_index {
            self.mem_list[video2_index].state = MemEntryState::LoadMe;
        }

        self.load_marked_as_needed();

        self.seg_palettes = self.mem_list[palette_index].buf_ptr;
        self.seg_bytecode = self.mem_list[code_index].buf_ptr;
        debug!(
            "seg_bytecode: 0x{:04x} value: {:x}",
            self.seg_bytecode, self.memory[self.seg_bytecode]
        );
        self.seg_cinematic = self.mem_list[video_cinematic_index].buf_ptr;

        if let Some(video2_index) = video2_index {
            self.seg_video2 = self.mem_list[video2_index].buf_ptr;
        }

        self.current_part_id = part_id;

        self.script_bak_ptr = self.script_cur_ptr;
    }

    pub fn read_byte(&mut self, index: usize) -> u8 {
        self.memory[index]
    }

    pub fn read_word(&mut self, index: usize) -> u16 {
        BigEndian::read_u16(&self.memory[index..])
    }

    pub fn invalidate_resource(&mut self) {
        for entry in self.mem_list.iter_mut() {
            match entry.entry_type {
                EntryType::PolyAnim | EntryType::Unknown(_) => {
                    entry.state = MemEntryState::NotNeeded;
                }
                _ => {}
            }
        }
        self.script_cur_ptr = self.script_bak_ptr;
    }

    pub fn load_memory_entry(&mut self, resource_id: u16) {
        let resource_id = resource_id as usize;
        let entry = &mut self.mem_list[resource_id];
        if entry.state == MemEntryState::NotNeeded {
            entry.state = MemEntryState::LoadMe;
            self.load_marked_as_needed();
        }
    }

    pub fn video_page_data(&self) -> Vec<u8> {
        debug!("video_page_data()");
        let mut buf = Vec::new();

        let mut off = self.vid_cur_ptr;
        let mut next_add = 1;
        for _h in 0..200 {
            for _w in 0..40 {
                let mut p = match self.asset_platform {
                    AssetPlatform::AtariST => [
                        self.memory[off + 6],
                        self.memory[off + 4],
                        self.memory[off + 2],
                        self.memory[off],
                    ],
                    AssetPlatform::Amiga | AssetPlatform::PC => [
                        self.memory[off + 8000 * 3],
                        self.memory[off + 8000 * 2],
                        self.memory[off + 8000],
                        self.memory[off],
                    ],
                };
                for _j in 0..8 {
                    let mut acc = 0;
                    for i in 0..4 {
                        acc <<= 1;
                        acc |= if (p[i] & 0x80) > 0 { 1 } else { 0 };
                        p[i] <<= 1;
                    }
                    buf.push(acc);
                }
                off += next_add;
                if next_add == 1 {
                    next_add = match self.asset_platform {
                        AssetPlatform::AtariST => 7,
                        _ => 1,
                    };
                } else {
                    next_add = 1;
                }
            }
        }
        buf
    }

    pub fn get_entry_mixer_chunk(&self, resource_id: u16) -> Option<MixerChunk> {
        let resource_id = resource_id as usize;
        let entry = &self.mem_list[resource_id];

        if entry.state != MemEntryState::Loaded {
            return None;
        }
        debug!("sound buf_ptr {}", entry.buf_ptr);
        let header = &self.memory[entry.buf_ptr..entry.buf_ptr + 8];
        let data = &self.memory[entry.buf_ptr + 8..];
        let len = (BigEndian::read_u16(header) * 2) as usize;
        let loop_len = (BigEndian::read_u16(&header[2..]) * 2) as usize;

        // When looping, buffer length is larger than len
        let data_len = len + loop_len;
        Some(MixerChunk::new(&data[0..data_len], len, loop_len))
    }

    pub fn load_sfx_module(&self, resource_id: u16, delay: &mut u16, pos: u8) -> Option<SfxModule> {
        debug!("load_sfx_module(0x{:x}, {}, {}", resource_id, delay, pos);
        let resource_id = resource_id as usize;
        let entry = &self.mem_list[resource_id];

        if entry.state != MemEntryState::Loaded || entry.entry_type != EntryType::Music {
            return None;
        }
        let data = &self.memory[entry.buf_ptr..];
        let cur_order = pos;
        let num_order = BigEndian::read_u16(&data[0x3e..]) as u8;
        debug!(
            "load_sfx_module() cur_order = 0x{:x}, num_order = 0x{:x}",
            cur_order, num_order
        );
        let mut order_table = [0; 0x80];
        for (i, order) in order_table.iter_mut().enumerate() {
            *order = data[0x40 + i];
        }
        if *delay == 0 {
            *delay = BigEndian::read_u16(&data)
        }
        let data = &data[0xc0..entry.size];
        let mut samples = Vec::new();
        for i in 0..15 {
            let buf = &self.memory[entry.buf_ptr + 2 + i * 4..];
            samples.push(self.prepare_instrument(&buf));
        }

        let module = SfxModule::new(data.into(), cur_order, num_order, order_table, samples);
        Some(module)
    }

    fn prepare_instrument(&self, buf: &[u8]) -> Option<SfxInstrument> {
        let mut buffer = Buffer::new(&buf);
        let resource_id = buffer.fetch_word();
        if resource_id == 0 {
            return None;
        }
        let volume = buffer.fetch_word();
        let entry = &self.mem_list[resource_id as usize];
        if entry.state != MemEntryState::Loaded || entry.entry_type != EntryType::Sound {
            panic!("Error loading instrument 0x{:x}", resource_id);
        }
        let mut data = self.memory[entry.buf_ptr..entry.buf_ptr + entry.size].to_vec();
        if data.len() == 0 {
            return None;
        }
        for item in data.iter_mut().take(12).skip(8) {
            *item = 0;
        }
        Some(SfxInstrument::new(data, volume))
    }

    fn read_bank(
        asset_path: &Path,
        mem_entry: &MemEntry,
        asset_platform: &AssetPlatform,
    ) -> std::io::Result<Bank> {
        let file_name = match asset_platform {
            AssetPlatform::PC => asset_path.join(format!("Bank{:02x}", mem_entry.bank_id)),
            AssetPlatform::Amiga => asset_path.join(format!("bank{:02X}", mem_entry.bank_id)),
            AssetPlatform::AtariST => asset_path.join(format!("BANK{:02X}", mem_entry.bank_id)),
        };
        debug!("Reading bank: {}", file_name.to_string_lossy());
        let mut file = File::open(file_name)?;
        file.seek(SeekFrom::Start(mem_entry.bank_offset as u64))?;

        let mut data = vec![0; mem_entry.packed_size as usize];
        file.read_exact(&mut data)?;
        let bank = if mem_entry.packed_size == mem_entry.size {
            Bank::Uncompressed(data)
        } else {
            Bank::Compressed(data)
        };
        Ok(bank)
    }

    fn invalidate_all(&mut self) {
        for entry in self.mem_list.iter_mut() {
            entry.state = MemEntryState::NotNeeded;
        }

        self.script_cur_ptr = 0;
    }

    fn load_marked_as_needed(&mut self) {
        let mut to_load: Vec<&mut MemEntry> = self
            .mem_list
            .iter_mut()
            .filter(|e| e.state == MemEntryState::LoadMe)
            .collect();

        for entry in to_load {
            let load_destination = match entry.entry_type {
                EntryType::PolyAnim => self.vid_cur_ptr,
                _ => {
                    if entry.size > self.vid_bak_ptr - self.script_cur_ptr {
                        warn!("Resource: Not enough memory to load resource");
                        entry.state = MemEntryState::NotNeeded;
                        continue;
                    }
                    self.script_cur_ptr
                }
            };
            debug!("load(): {:?} 0x{:x}", entry.entry_type, load_destination);

            if entry.bank_id == 0 {
                warn!("Resource: entry.bank_id == 0");
                entry.state = MemEntryState::NotNeeded;
                continue;
            }

            let bank = Resource::read_bank(&self.asset_path, &entry, &self.asset_platform)
                .expect("Could not read bank");
            debug!("read_bank() rank_num: {} packed_size: 0x{:x} size: 0x{:x} type={:?} pos={:x} bank_id={:x}", entry.rank_num, entry.packed_size, entry.size, entry.entry_type, entry.bank_offset, entry.bank_id);

            let load_destination_end = load_destination + entry.size;
            let dst = &mut self.memory[load_destination..load_destination_end];
            let data = bank.data();
            assert!(data.len() == entry.size);
            dst.copy_from_slice(&data);
            if let EntryType::PolyAnim = entry.entry_type {
                self.copy_vid_ptr = true;
                entry.state = MemEntryState::NotNeeded;
            } else {
                entry.buf_ptr = load_destination;
                entry.state = MemEntryState::Loaded;
                self.script_cur_ptr += entry.size;
            }
        }
    }

    fn read_entries<R: Read>(&mut self, reader: &mut R) {
        loop {
            let entry = MemEntry {
                state: MemEntryState::from_u8(reader.read_u8().unwrap()),
                entry_type: EntryType::from_u8(reader.read_u8().unwrap()),
                buf_ptr: reader.read_u16::<BigEndian>().unwrap() as usize,
                unk4: reader.read_u16::<BigEndian>().unwrap(),
                rank_num: reader.read_u8().unwrap(),
                bank_id: reader.read_u8().unwrap(),
                bank_offset: reader.read_u32::<BigEndian>().unwrap(),
                unkc: reader.read_u16::<BigEndian>().unwrap(),
                packed_size: reader.read_u16::<BigEndian>().unwrap() as usize,
                unk10: reader.read_u16::<BigEndian>().unwrap(),
                size: reader.read_u16::<BigEndian>().unwrap() as usize,
            };
            if let MemEntryState::EndOfMemList = entry.state {
                break;
            }
            self.mem_list.push(entry);
        }
        //for res in self.mem_list.iter() {
        //    println!("{:?}", res);
        //}
        //println!("Len: {}", self.mem_list.len());
    }
}
