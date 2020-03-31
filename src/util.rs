pub fn resize(buffer: &[u8], factor: u16) -> Vec<u8> {
    let factor = factor as usize;
    let width = 320 * factor;
    let height = 200 * factor;
    let mut result = vec![0; width * height];
    for j in 0..height {
        for i in 0..width {
            result[j * width + i] = buffer[j / factor * 320 + i / factor];
        }
    }
    result
}
