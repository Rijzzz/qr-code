use image::{DynamicImage, GrayImage, Luma};

pub const ALIGN: [&[u8]; 10] = [
    &[],
    &[6, 18],
    &[6, 22],
    &[6, 26],
    &[6, 30],
    &[6, 34],
    &[6, 22, 38],
    &[6, 24, 42],
    &[6, 26, 46],
    &[6, 28, 50],
];

pub const FMT1: [(usize, usize); 15] = [
    (8,0),(8,1),(8,2),(8,3),(8,4),(8,5),(8,7),(8,8),
    (7,8),(5,8),(4,8),(3,8),(2,8),(1,8),(0,8),
];

struct Mat {
    #[allow(dead_code)]
    sz: usize,
    m: Vec<Vec<bool>>,
    res: Vec<Vec<bool>>,
}

impl Mat {
    fn new(sz: usize) -> Self {
        Mat { sz, m: vec![vec![false; sz]; sz], res: vec![vec![false; sz]; sz] }
    }

    fn put(&mut self, r: usize, c: usize, dark: bool) {
        self.m[r][c] = dark;
        self.res[r][c] = true;
    }

    fn clone_data(&self) -> Vec<Vec<bool>> {
        self.m.clone()
    }
}

pub fn build(bits: &[u8], version: usize, ec: u8) -> Vec<Vec<bool>> {
    let sz = 4 * version + 17;
    let mut mat = Mat::new(sz);

    place_finders(&mut mat, sz);
    place_timing(&mut mat, sz);
    place_alignment(&mut mat, version);
    reserve_format(&mut mat, sz);

    mat.put(4 * version + 9, 8, true);

    if version >= 7 {
        reserve_version(&mut mat, version, sz);
    }

    place_data(&mut mat, bits, sz);

    let mut best_mask = 0u8;
    let mut best_score = u32::MAX;

    for mask in 0..8u8 {
        let mut dm = mat.clone_data();
        apply_mask_to(&mut dm, &mat.res, mask, sz);
        let mut dm2 = dm.clone();
        write_format(&mut dm2, ec, mask, sz);
        let s = penalty(&dm2, sz);
        if s < best_score {
            best_score = s;
            best_mask = mask;
        }
    }

    let mut final_m = mat.clone_data();
    apply_mask_to(&mut final_m, &mat.res, best_mask, sz);
    write_format(&mut final_m, ec, best_mask, sz);
    if version >= 7 {
        write_version(&mut final_m, version, sz);
    }

    final_m
}

fn place_finders(mat: &mut Mat, sz: usize) {
    let draw_finder = |mat: &mut Mat, fr: usize, fc: usize| {
        for r in 0..7 {
            for c in 0..7 {
                let dark = r == 0 || r == 6 || c == 0 || c == 6
                    || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
                mat.put(fr + r, fc + c, dark);
            }
        }
    };

    draw_finder(mat, 0, 0);
    draw_finder(mat, 0, sz - 7);
    draw_finder(mat, sz - 7, 0);

    for i in 0..8usize {
        mat.put(7, i, false);
        mat.put(i, 7, false);
        mat.put(7, sz - 1 - i, false);
        mat.put(i, sz - 8, false);
        mat.put(sz - 8, i, false);
        mat.put(sz - 1 - i, 7, false);
    }
}

fn place_timing(mat: &mut Mat, sz: usize) {
    for i in 8..sz - 8 {
        mat.put(6, i, i % 2 == 0);
        mat.put(i, 6, i % 2 == 0);
    }
}

fn place_alignment(mat: &mut Mat, version: usize) {
    let coords = ALIGN[version - 1];
    let n = coords.len();
    for i in 0..n {
        for j in 0..n {
            let r = coords[i] as usize;
            let c = coords[j] as usize;
            if mat.res[r][c] { continue; }
            for dr in 0..5usize {
                for dc in 0..5usize {
                    let (ar, ac) = (r - 2 + dr, c - 2 + dc);
                    let dark = dr == 0 || dr == 4 || dc == 0 || dc == 4
                        || (dr == 2 && dc == 2);
                    mat.put(ar, ac, dark);
                }
            }
        }
    }
}

fn reserve_format(mat: &mut Mat, sz: usize) {
    for &(r, c) in &FMT1 { mat.put(r, c, false); }
    for i in 0..8usize {
        mat.put(sz - 1 - i, 8, false);
        mat.put(8, sz - 1 - i, false);
    }
}

fn reserve_version(mat: &mut Mat, version: usize, sz: usize) {
    let vi = version_info(version);
    for i in 0..18 {
        let bit = (vi >> i) & 1 != 0;
        mat.put(i % 6, sz - 11 + i / 6, bit);
        mat.put(sz - 11 + i / 6, i % 6, bit);
    }
}

fn place_data(mat: &mut Mat, bits: &[u8], sz: usize) {
    let mut bit_i = 0;
    let mut going_up = true;
    let mut col = sz - 1;

    loop {
        let right = col;
        let left = if col > 0 && col - 1 == 6 { 5 } else if col > 0 { col - 1 } else { break };

        let rows: Vec<usize> = if going_up {
            (0..sz).rev().collect()
        } else {
            (0..sz).collect()
        };

        for r in rows {
            for &c in &[right, left] {
                if !mat.res[r][c] {
                    mat.m[r][c] = bit_i < bits.len() && bits[bit_i] != 0;
                    bit_i += 1;
                }
            }
        }

        going_up = !going_up;

        if col < 2 { break; }
        let next_right = if col == 7 { 5 } else { col - 2 };
        if next_right == 0 && left == 0 { break; }
        col = next_right;
        if col == 6 { col = 5; }
    }
}

fn apply_mask_to(m: &mut Vec<Vec<bool>>, res: &[Vec<bool>], mask: u8, sz: usize) {
    for r in 0..sz {
        for c in 0..sz {
            if res[r][c] { continue; }
            let flip = match mask {
                0 => (r + c) % 2 == 0,
                1 => r % 2 == 0,
                2 => c % 3 == 0,
                3 => (r + c) % 3 == 0,
                4 => (r / 2 + c / 3) % 2 == 0,
                5 => (r * c) % 2 + (r * c) % 3 == 0,
                6 => ((r * c) % 2 + (r * c) % 3) % 2 == 0,
                7 => ((r + c) % 2 + (r * c) % 3) % 2 == 0,
                _ => false,
            };
            if flip { m[r][c] = !m[r][c]; }
        }
    }
}

fn format_bits(ec: u8, mask: u8) -> u16 {
    let ec_ind = [0b01u16, 0b00, 0b11, 0b10][ec as usize];
    let data = (ec_ind << 3) | mask as u16;
    let mut rem = data << 10;
    for i in (0..5).rev() {
        if rem & (1 << (i + 10)) != 0 { rem ^= 0x537 << i; }
    }
    ((data << 10) | (rem & 0x3FF)) ^ 0x5412
}

fn write_format(m: &mut Vec<Vec<bool>>, ec: u8, mask: u8, sz: usize) {
    let fmt = format_bits(ec, mask);
    for (i, &(r, c)) in FMT1.iter().enumerate() {
        m[r][c] = (fmt >> (14 - i)) & 1 != 0;
    }
    for i in 0..8usize {
        m[sz - 1 - i][8] = (fmt >> i) & 1 != 0;
    }
    for i in 0..7usize {
        m[8][sz - 1 - i] = (fmt >> (14 - i)) & 1 != 0;
    }
    m[8][sz - 8] = (fmt >> 7) & 1 != 0;
    m[sz - 8][8] = true;
}

fn write_version(m: &mut Vec<Vec<bool>>, version: usize, sz: usize) {
    let vi = version_info(version);
    for i in 0..18 {
        let bit = (vi >> i) & 1 != 0;
        m[i % 6][sz - 11 + i / 6] = bit;
        m[sz - 11 + i / 6][i % 6] = bit;
    }
}

fn version_info(version: usize) -> u32 {
    let data = version as u32;
    let mut rem = data << 12;
    for i in (0..6).rev() {
        if rem & (1 << (i + 12)) != 0 { rem ^= 0x1F25 << i; }
    }
    (data << 12) | (rem & 0xFFF)
}

fn penalty(m: &[Vec<bool>], sz: usize) -> u32 {
    let mut score = 0u32;

    for r in 0..sz {
        let mut run = 1u32;
        for c in 1..sz {
            if m[r][c] == m[r][c - 1] { run += 1; } else { if run >= 5 { score += run - 2; } run = 1; }
        }
        if run >= 5 { score += run - 2; }
    }
    for c in 0..sz {
        let mut run = 1u32;
        for r in 1..sz {
            if m[r][c] == m[r - 1][c] { run += 1; } else { if run >= 5 { score += run - 2; } run = 1; }
        }
        if run >= 5 { score += run - 2; }
    }

    for r in 0..sz - 1 {
        for c in 0..sz - 1 {
            let d = m[r][c];
            if d == m[r][c + 1] && d == m[r + 1][c] && d == m[r + 1][c + 1] { score += 3; }
        }
    }

    let pat1 = [true,false,true,true,true,false,true,false,false,false,false];
    let pat2 = [false,false,false,false,true,false,true,true,true,false,true];
    for r in 0..sz {
        for c in 0..sz.saturating_sub(10) {
            let row_slice: Vec<bool> = (c..c+11).map(|x| m[r][x]).collect();
            if row_slice == pat1 || row_slice == pat2 { score += 40; }
            let col_slice: Vec<bool> = (c..c+11).map(|x| m[x][r]).collect();
            if col_slice == pat1 || col_slice == pat2 { score += 40; }
        }
    }

    let dark: u32 = m.iter().flat_map(|r| r.iter()).filter(|&&b| b).count() as u32;
    let total = (sz * sz) as u32;
    let pct = dark * 100 / total;
    let diff = ((pct as i32 - 50).abs() / 5) as u32;
    score += diff * 10;

    score
}

pub fn to_png(mat: &[Vec<bool>], scale: u32) -> DynamicImage {
    let quiet = 4u32;
    let sz = mat.len() as u32;
    let full = (sz + quiet * 2) * scale;
    let mut img = GrayImage::from_pixel(full, full, Luma([255u8]));
    for (r, row) in mat.iter().enumerate() {
        for (c, &dark) in row.iter().enumerate() {
            let px = if dark { 0u8 } else { 255u8 };
            for dr in 0..scale {
                for dc in 0..scale {
                    let pr = (quiet + r as u32) * scale + dr;
                    let pc = (quiet + c as u32) * scale + dc;
                    img.put_pixel(pc, pr, Luma([px]));
                }
            }
        }
    }
    DynamicImage::ImageLuma8(img)
}

pub fn print_block(mat: &[Vec<bool>]) {
    let quiet = "    ";
    let top_bot: String = " ".repeat(mat.len() * 2 + 8);
    println!("{}", top_bot);
    println!("{}", top_bot);
    for row in mat.iter() {
        print!("{}", quiet);
        for &dark in row {
            print!("{}", if dark { "  " } else { "\x1b[7m  \x1b[0m" });
        }
        println!("{}", quiet);
    }
    println!("{}", top_bot);
    println!("{}", top_bot);
}

pub fn read_grid(img: &DynamicImage) -> Option<(Vec<Vec<bool>>, usize)> {
    let w = img.width() as usize;
    let h = img.height() as usize;
    if w != h { return None; }

    let gray = img.to_luma8();

    for v in 1..=10 {
        let sz = 4 * v + 17;
        let full = sz + 8;
        if w % full != 0 { continue; }
        let scale = w / full;
        let quiet = 4;

        let mut grid = vec![vec![false; sz]; sz];
        for r in 0..sz {
            for c in 0..sz {
                let pr = (quiet + r) * scale + scale / 2;
                let pc = (quiet + c) * scale + scale / 2;
                let pix = gray.get_pixel(pc as u32, pr as u32)[0];
                grid[r][c] = pix < 128;
            }
        }

        if grid[0][0] && grid[0][6] && grid[6][0]
            && !grid[0][7] && !grid[7][0]
        {
            return Some((grid, v));
        }
    }
    None
}

pub fn read_format(grid: &[Vec<bool>]) -> Option<(u8, u8)> {
    let mut fmt = 0u16;
    for (i, &(r, c)) in FMT1.iter().enumerate() {
        if grid[r][c] { fmt |= 1 << (14 - i); }
    }
    fmt ^= 0x5412;

    let ec_ind = (fmt >> 13) as u8 & 0b11;
    let mask = (fmt >> 10) as u8 & 0b111;
    let ec = [1u8, 0, 3, 2][ec_ind as usize];

    Some((ec, mask))
}

