use log::{debug, warn};

use crate::buffer::Buffer;
use crate::strings::STRINGS_TABLE_ENG;

const MAX_POINTS: usize = 50;

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

pub struct Video {
}

impl Video {
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
}
