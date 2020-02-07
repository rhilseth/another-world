use log::{debug, warn};

use crate::buffer::Buffer;
use crate::strings::STRINGS_TABLE_ENG;
use crate::sys::SDLSys;

const MAX_POINTS: usize = 50;
const VID_PAGE_SIZE: usize = 320 * 200 / 2;
const NUM_COLORS: usize = 16;

#[derive(Copy, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct Palette {
    pub entries: [Color; NUM_COLORS],
}

impl Palette {
    pub fn from_bytes(buffer: &[u8]) -> Palette {
        let mut entries = [Color { r: 0, g: 0, b: 0, a: 0 }; NUM_COLORS];
        for i in 0..NUM_COLORS {
            let c1 = buffer[i * 2];
            let c2 = buffer[i * 2 + 1];
            let r = (((c1 & 0x0f) << 2) | ((c1 & 0x0f) >> 2)) << 2;
            let g = (((c2 & 0xf0) >> 2) | ((c2 & 0xf0) >> 6)) << 2;
            let b = (((c2 & 0x0f) >> 2) | ((c2 & 0x0f) << 2)) << 2;
            let a = 0xff;
            entries[i] = Color {
                r,
                g,
                b,
                a,
            };
        }
        Palette { entries }
    }
}

pub struct Point {
    pub x: i16,
    pub y: i16,
}

struct Polygon {
    bbw: u16,
    bbh: u16,
    points: Vec<Point>,
}

impl Polygon {
    pub fn read_vertices(mut buffer: Buffer, zoom: u16) -> Polygon {
        let bbw = buffer.fetch_byte() as u16 * zoom / 64;
        let bbh = buffer.fetch_byte() as u16 * zoom / 64;
        let num_points = buffer.fetch_byte() as usize;
        assert!((num_points & 1) == 0 && num_points < MAX_POINTS);

        let zoom = zoom as i16;
        let mut points = Vec::new();
        for j in 0..num_points {
            let x = buffer.fetch_byte() as i16 * zoom / 64;
            let y = buffer.fetch_byte() as i16 * zoom / 64;
            points.push(Point { x, y });
        }
        Polygon { bbw, bbh, points }
    }
}

#[derive(Copy, Clone)]
pub struct Page {
    pub data: [u8; VID_PAGE_SIZE],
}

impl Page {
    pub fn new() -> Page {
        Page {
            data: [0; VID_PAGE_SIZE],
        }
    }
}

pub struct Video {
    pages: [Page; 4],
    palette_requested: Option<Palette>,
    cur_page_ptr1: usize,
    cur_page_ptr2: usize,
    cur_page_ptr3: usize,
}

impl Video {
    pub fn new() -> Video {
        Video {
            pages: [Page::new(); 4],
            palette_requested: None,
            cur_page_ptr1: 2,
            cur_page_ptr2: 2,
            cur_page_ptr3: 1,
        }
    }

    pub fn update_display(&mut self, sys: &mut SDLSys, page_id: u8) {
        if page_id != 0xfe {
            if page_id == 0xff {
                let tmp = self.cur_page_ptr3;
                self.cur_page_ptr3 = self.cur_page_ptr2;
                self.cur_page_ptr2 = tmp;
            } else {
                self.cur_page_ptr2 = self.get_page_id(page_id);
            }
        }

        if let Some(palette) = self.palette_requested.take() {
            sys.set_palette(&palette);
        }
        sys.update_display(&self.pages[self.cur_page_ptr2]);
    }

    pub fn change_page_ptr1(&mut self, page_id: u8) {
        self.cur_page_ptr1 = self.get_page_id(page_id);
    }

    pub fn fill_video_page(&self, page_id: u8, color: u8) {
        let mut page = self.get_page(page_id);

        let c = (color << 4) | color;
        for b in page.data.iter_mut() {
            *b = c;
        }
    }

    pub fn copy_page(&mut self, src_page_id: u8, dst_page_id: u8, vscroll: i16) {
        let vscroll = vscroll as isize;
        let mut src_page_id = src_page_id;
        if src_page_id == dst_page_id {
            return;
        }

        if src_page_id >= 0xfe || ((src_page_id & 0xbf) & 0x80) == 0 {
            if src_page_id < 0xfe {
                src_page_id = src_page_id & 0xbf;
            }
            let src_page = self.get_page(src_page_id);
            let q = self.get_page_id(dst_page_id);
            self.pages[q] = src_page;
        } else {
            src_page_id = src_page_id & 0xbf;
            let src_page = self.get_page(src_page_id & 3);
            let q = self.get_page_id(dst_page_id);
            let mut src_i = 0;
            let mut dst_i = 0;
            if vscroll >= -199 && vscroll <= 199 {
                let mut h: isize = 200;
                if vscroll < 0 {
                    h = h + vscroll;
                    src_i += (- vscroll * 160) as isize;
                } else {
                    h = h - vscroll;
                    dst_i += (vscroll * 160) as isize;
                }
                assert!(src_i > 0);
                assert!(dst_i > 0);
                let dst_i_end = (dst_i + h * 160) as usize;
                let dst_i = dst_i as usize;
                let mut dst_slice = &mut self.pages[q].data[dst_i..dst_i_end];
                let src_i_end = (src_i + h * 160) as usize;
                let src_i = src_i as usize;
                dst_slice.copy_from_slice(&src_page.data[src_i..src_i_end]);
            }
        }
    }

    pub fn draw_string(&self, color: u16, x: u16, y: u16, string_id: u16) {
        debug!("DrawString(0x{:04x}, {}, {}, {})", string_id, x, y, color);
        if let Some(entry) = STRINGS_TABLE_ENG.get(&string_id) {
            debug!("DrawString(): {}", entry);
        } else {
            warn!("String with id 0x{:03x} not found", string_id);
        }
    }

    pub fn read_and_draw_polygon(
        &self,
        mut buffer: Buffer,
        color: u8,
        zoom: u16,
        point: Point
    ) {
        let mut color = color;
        let mut i = buffer.fetch_byte();

        if i >= 0xc0 {
            if color & 0x80 > 0 {
                color = i & 0x3f;
            }

            let polygon = Polygon::read_vertices(buffer, zoom);
            self.fill_polygon(polygon, color, zoom, point);
        } else {
            i &= 0x3f;
            if i == 2 {
                self.read_and_draw_polygon_hierarchy(buffer, zoom, point);
            } else {
                warn!("read_and_draw_polygon: i != 2 ({})", i);
            }
        }
    }

    fn read_and_draw_polygon_hierarchy(
        &self,
        mut buffer: Buffer,
        zoom: u16,
        point: Point
    ) {
        unimplemented!("read_and_draw_polygon_hierarchy");
    }

    fn fill_polygon(
        &self,
        polygon: Polygon,
        color: u8,
        zoom: u16,
        point: Point,
    ) {
        unimplemented!("fill_polygon");
    }

    fn get_page_id(&self, page_id: u8) -> usize {
        let page_id = page_id as usize;
        match page_id {
            0..=3 => page_id,
            0xff => self.cur_page_ptr3,
            0xfe => self.cur_page_ptr2,
            _ => {
                warn!("get_page() id != [0, 1, 2, 3, 0xfe, 0xff]");
                0
            }
        }
    }

    fn get_page(&self, page_id: u8) -> Page {
        self.pages[self.get_page_id(page_id)]
    }
}
