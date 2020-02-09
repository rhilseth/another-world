use log::{debug, warn};
use std::cmp;

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

#[derive(Debug)]
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
    pub fn read_vertices(buffer: &mut Buffer, zoom: u16) -> Polygon {
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

    fn num_points(&self) -> usize {
        self.points.len()
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

fn calc_step(p1: &Point, p2: &Point) -> (i32, u16) {
    let dy = p2.y as i32 - p1.y as i32;
    let mul = if dy == 0 {
        0x4000
    } else {
        0x4000 / dy
    };
    let step = (p2.x as i32 - p1.x as i32) * mul * 4;
    (step, dy as u16)
}

pub struct Video {
    pages: [Page; 4],
    pub palette_requested: Option<Palette>,
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
                let dst_slice = &mut self.pages[q].data[dst_i..dst_i_end];
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
        &mut self,
        buffer: &mut Buffer,
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
        &mut self,
        buffer: &mut Buffer,
        zoom: u16,
        point: Point
    ) {
        let mut pt = point;
        pt.x -= buffer.fetch_byte() as i16 * zoom as i16 / 64;
        pt.y -= buffer.fetch_byte() as i16 * zoom as i16 / 64;

        let children = buffer.fetch_byte() as usize;
        for _ in 0..children {
            let mut offset = buffer.fetch_word() as usize;

            let x = buffer.fetch_byte() as i16 * zoom as i16 / 64;
            let y = buffer.fetch_byte() as i16 * zoom as i16 / 64;
            let po = Point { x: pt.x + x, y: pt.y + y };

            let mut color = 0xff;
            let _bp = offset;
            offset &= 0x7fff;

            if _bp & 0x8000 > 0 {
                color = buffer.fetch_byte() & 0x7f;
                buffer.offset += 1;
            }

            let bak_offset = buffer.offset;
            buffer.offset = offset * 2;

            self.read_and_draw_polygon(buffer, color, zoom, po);

            buffer.offset = bak_offset;
        }
    }

    fn fill_polygon(
        &mut self,
        polygon: Polygon,
        color: u8,
        zoom: u16,
        point: Point,
    ) {
        if polygon.bbw == 0 && polygon.bbh == 1 && polygon.num_points() == 4 {
            self.draw_point(color, point);
            return;
        }

        let mut x1 = point.x - polygon.bbw as i16 / 2;
        let mut x2 = point.x + polygon.bbw as i16 / 2;
        let y1 = point.y - polygon.bbh as i16 / 2;
        let y2 = point.y + polygon.bbh as i16 / 2;

        if x1 > 319 || x2 < 0 || y1 > 199 || y2 < 0 {
            return;
        }

        let mut hliney = y1;
        let mut i = 0;
        let mut j = polygon.num_points() - 1;

        x2 = polygon.points[i].x + x1;
        x1 = polygon.points[j].x + x1;

        i = i + 1;
        j = j - 1;

        let mut cpt1 = (x1 as u32) << 16;
        let mut cpt2 = (x2 as u32) << 16;

        let mut num_points = polygon.num_points();
        loop {
            num_points -= 2;
            if num_points == 0 {
                break;
            }
            let (step1, _) = calc_step(&polygon.points[j + 1], &polygon.points[j]);
            let (step2, h) = calc_step(&polygon.points[i - 1], &polygon.points[i]);

            i += 1;
            j -= 1;

            cpt1 = (cpt1 & 0xffff0000) | 0x7fff;
            cpt2 = (cpt2 & 0xffff0000) | 0x8000;

            if h == 0 {
                cpt1 = (cpt1 as i64 + step1 as i64) as u32;
                cpt2 = (cpt2 as i64 + step2 as i64) as u32;
            } else {
                for _ in 0..h {
                    if hliney >= 0 {
                        x1 = (cpt1 >> 16) as i16;
                        x2 = (cpt2 >> 16) as i16;
                        if x1 <= 319 && x2 >= 0 {
                            if x1 < 0 {
                                x1 = 0;
                            }
                            if x2 > 319 {
                                x2 = 319;
                            }
                            match color {
                                0..=0x0f => self.draw_line_n(x1, x2, color, hliney),
                                0x11..=0xff => self.draw_line_p(x1, x2, color, hliney),
                                0x10 => self.draw_line_blend(x1, x2, color, hliney),
                            }
                        }
                    }
                    cpt1 = (cpt1 as i64 + step1 as i64) as u32;
                    cpt2 = (cpt2 as i64 + step2 as i64) as u32;
                    hliney += 1;
                    if hliney > 199 {
                        return;
                    }
                }
            }
        }
    }

    fn draw_line_n(&mut self, x1: i16, x2: i16, color: u8, hliney: i16) {
        debug!("draw_line_n({}, {}, {})", x1, x2, color);
        let xmax = cmp::max(x1, x2);
        let xmin = cmp::min(x1, x2);
        let mut offset = (hliney * 160 + xmin / 2) as usize;

        let mut w = (xmax / 2 - xmin / 2 + 1) as u16;
        let mut cmaske: u8 = 0;
        let mut cmasks: u8 = 0;

        if xmin & 1 > 0 {
            w -= 1;
            cmasks = 0xf0;
        }

        if xmax & 1 == 0 {
            w -= 1;
            cmaske = 0x0f;
        }

        let colb = ((color & 0xf) << 4) | (color & 0xf);
        if cmasks != 0 {
            self.pages[self.cur_page_ptr1].data[offset] = (self.pages[self.cur_page_ptr1].data[offset] & cmasks) | (colb & 0x0f);
            offset += 1;
        }
        while w > 0 {
            self.pages[self.cur_page_ptr1].data[offset] = colb;
            offset += 1;
            w -= 1;
        }
        if cmaske != 0 {
            self.pages[self.cur_page_ptr1].data[offset] = (self.pages[self.cur_page_ptr1].data[offset] & cmaske) | (colb & 0xf0);
            offset += 1;
        }
    }

    fn draw_line_p(&mut self, x1: i16, x2: i16, color: u8, hliney: i16) {
        debug!("draw_line_p({}, {}, {})", x1, x2, color);
        let xmax = cmp::max(x1, x2);
        let xmin = cmp::min(x1, x2);
        let mut offset = (hliney * 160 + xmin / 2) as usize;

        let mut w = (xmax / 2 - xmin / 2 + 1) as u16;
        let mut cmaske: u8 = 0;
        let mut cmasks: u8 = 0;

        if xmin & 1 > 0 {
            w -= 1;
            cmasks = 0xf0;
        }

        if xmax & 1 == 0 {
            w -= 1;
            cmaske = 0x0f;
        }

        if cmasks != 0 {
            let p = self.pages[self.cur_page_ptr1].data[offset];
            let q = self.pages[0].data[offset];
            self.pages[self.cur_page_ptr1].data[offset] = (p & cmasks) | (q & 0x0f);
            offset += 1;
        }
        while w > 0 {
            self.pages[self.cur_page_ptr1].data[offset] = self.pages[0].data[offset];
            offset += 1;
            w -= 1;
        }
        if cmaske != 0 {
            let p = self.pages[self.cur_page_ptr1].data[offset];
            let q = self.pages[0].data[offset];
            self.pages[self.cur_page_ptr1].data[offset] = (p & cmaske) | (q & 0xf0);
            offset += 1;
        }
    }

    fn draw_line_blend(&mut self, x1: i16, x2: i16, color: u8, hliney: i16) {
        debug!("draw_line_blend({}, {}, {})", x1, x2, color);
        let xmax = cmp::max(x1, x2);
        let xmin = cmp::min(x1, x2);
        let mut offset = (hliney * 160 + xmin / 2) as usize;

        let mut w = (xmax / 2 - xmin / 2 + 1) as u16;
        let mut cmaske: u8 = 0;
        let mut cmasks: u8 = 0;

        if xmin & 1 > 0 {
            w -= 1;
            cmasks = 0xf7;
        }

        if xmax & 1 == 0 {
            w -= 1;
            cmaske = 0x7f;
        }

        if cmasks != 0 {
            let p = self.pages[self.cur_page_ptr1].data[offset];
            self.pages[self.cur_page_ptr1].data[offset] = (p & cmasks) | 0x08;
            offset += 1;
        }
        while w > 0 {
            let p = self.pages[self.cur_page_ptr1].data[offset];
            self.pages[self.cur_page_ptr1].data[offset] = (p & 0x77) | 0x88;
            offset += 1;
            w -= 1;
        }
        if cmaske != 0 {
            let p = self.pages[self.cur_page_ptr1].data[offset];
            self.pages[self.cur_page_ptr1].data[offset] = (p & cmaske) | 0x80;
            offset += 1;
        }
    }

    fn draw_point(&mut self, color: u8, point: Point) {
        debug!("draw_point({}, {:?})", color, point);
        if point.x >= 0 && point.x <= 319 && point.y >= 0 && point.y <= 199 {
            let offset = (point.y * 160 + point.x / 2) as usize;

            let (mut cmasko, mut cmaskn) = if point.x & 1 > 0 {
                (0xf0, 0x0f)
            } else {
                (0x0f, 0xf0)
            };

            let mut colb = (color << 4) | color;
            if color == 0x10 {
                cmaskn &= 0x88;
                cmasko = !cmaskn;
                colb = 0x88;
            } else if color == 0x11 {
                colb = self.pages[0].data[offset];
            }
            let b = self.pages[self.cur_page_ptr1].data[offset];
            self.pages[self.cur_page_ptr1].data[offset] = (b & cmasko) | (colb & cmaskn);
        }
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
