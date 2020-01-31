use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;

use byteorder::{BigEndian, ReadBytesExt};
use log::warn;

use crate::bank::Bank;
use crate::parts;

const MEM_BLOCK_SIZE: usize = 600 * 1024;

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

#[derive(Debug)]
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
    memory: [u8; MEM_BLOCK_SIZE],
    current_part_id: u16,
    script_bak_ptr: usize,
    script_cur_ptr: usize,
    vid_bak_ptr: usize,
    vid_cur_ptr: usize,
    pub seg_palettes: usize,
    pub seg_bytecode: usize,
    pub seg_cinematic: usize,
    pub seg_video2: usize,
}

impl Resource {
    pub fn new() -> Resource {
        Resource {
            mem_list: Vec::new(),
            memory: [0; MEM_BLOCK_SIZE],
            current_part_id: 0,
            script_bak_ptr: 0,
            script_cur_ptr: 0,
            vid_bak_ptr: MEM_BLOCK_SIZE - 0x800 * 16,
            vid_cur_ptr: MEM_BLOCK_SIZE - 0x800 * 16,
            seg_palettes: 0,
            seg_bytecode: 0,
            seg_cinematic: 0,
            seg_video2: 0,
        }
    }

    pub fn read_memlist(&mut self) -> std::io::Result<()> {
        let mut file = File::open("data/Memlist.bin")?;
        self.read_entries(&mut file);
        Ok(())
    }

    pub fn setup_part(&mut self, part_id: u16) {
        if part_id == self.current_part_id {
            return;
        }

        if part_id < parts::GAME_PART_FIRST || part_id > parts::GAME_PART_LAST {
            panic!("Unknown part: {:x}", part_id);
        }

        let index = (part_id - parts::GAME_PART_FIRST) as usize;

        let palette_index = parts::PARTS[index].palette;
        let code_index = parts::PARTS[index].code;
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
        self.seg_cinematic = self.mem_list[video_cinematic_index].buf_ptr;

        if let Some(video2_index) = video2_index {
            self.seg_video2 = self.mem_list[video2_index].buf_ptr;
        }

        self.current_part_id = part_id;

        self.script_bak_ptr = self.script_cur_ptr;
    }

    fn read_bank(mem_entry: &MemEntry) -> std::io::Result<Bank> {
        let file_name = format!("data/Bank{:02}", mem_entry.bank_id);
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
        let mut to_load: Vec<&mut MemEntry> = self.mem_list.iter_mut()
            .filter(|e| e.state == MemEntryState::LoadMe)
            .collect();

        // Sort by rank_num in descending order
        to_load.sort_by(|a, b| b.rank_num.cmp(&a.rank_num));

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

            if entry.bank_id == 0 {
                warn!("Resource: entry.bank_id == 0");
                entry.state = MemEntryState::NotNeeded;
                continue;
            }

            let bank = Resource::read_bank(&entry).expect("Could not read bank");

            let load_destination_end = load_destination + entry.size;
            let dst = &mut self.memory[load_destination..load_destination_end];
            dst.copy_from_slice(&bank.data());
            if let EntryType::PolyAnim = entry.entry_type {
                // video->copyPage(_vidCurPtr);
                unimplemented!("video->copyPage");
                entry.state = MemEntryState::NotNeeded;
            } else {
                entry.buf_ptr = load_destination;
                entry.state = MemEntryState::Loaded;
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
