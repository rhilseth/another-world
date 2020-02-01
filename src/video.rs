use crate::buffer::Buffer;

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
    pub fn read_vertices(data: &[u8], zoom: u16, index: usize) -> (Polygon, usize) {
        let mut i = index;
        let mut read_byte = || {
            let result = data[i];
            i += 1;
            result
        };
        let bbw = read_byte() as u16 * zoom / 64;
        let bbh = read_byte() as u16 * zoom / 64;
        let num_points = read_byte() as usize;
        assert!((num_points & 1) == 0 && num_points < MAX_POINTS);

        let zoom = zoom as i16;
        let mut points = Vec::new();
        for j in 0..num_points {
            let x = read_byte() as i16 * zoom / 64;
            let y = read_byte() as i16 * zoom / 64;
            points.push(Point { x, y });
        }
        (
            Polygon {
                bbw,
                bbh,
                points,
            },
            i
        )
    }
}

pub struct Video {
}

impl Video {
    pub fn read_and_draw_polygon(
        &self,
        mut buffer: Buffer,
        color: u8,
        zoom: u16,
        point: Point
    ) {

        self.read_and_draw_polygon_hierarchy(buffer, zoom, point);
    }

    fn read_and_draw_polygon_hierarchy(
        &self,
        mut buffer: Buffer,
        zoom: u16,
        point: Point
    ) {

    }
}
