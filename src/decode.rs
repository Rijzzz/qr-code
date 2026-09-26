use image::DynamicImage;
use crate::matrix;

pub fn from_image(img: &DynamicImage) -> Option<String> {
    let (grid, version) = matrix::read_grid(img)?;
    let sz = grid.len();

    let (ec, mask) = matrix::read_format(&grid)?;

    let res = build_res(sz, version);
    let mut unmasked_bits: Vec<u8> = Vec::new();

    let mut going_up = true;
    let mut col = sz - 1;
    loop {
        let right = col;
        let left = if col > 0 && col - 1 == 6 { 5 } else if col > 0 { col - 1 } else { break };
        let rows: Vec<usize> = if going_up { (0..sz).rev().collect() } else { (0..sz).collect() };
        for r in rows {
            for &c in &[right, left] {
                if !res[r][c] {
                    let raw_bit = if grid[r][c] { 1u8 } else { 0u8 };
                    unmasked_bits.push(apply_mask_bit(raw_bit, mask, r, c));
                }
            }
        }
        going_up = !going_up;
        if col < 2 { break; }
        let next = if col == 7 { 5 } else { col - 2 };
        col = next;
        if col == 6 { col = 5; }
    }

    bits_to_message(&unmasked_bits, version, ec)
}

fn apply_mask_bit(bit: u8, mask: u8, r: usize, c: usize) -> u8 {
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
    if flip { bit ^ 1 } else { bit }
}

fn bits_to_message(bits: &[u8], version: usize, ec: u8) -> Option<String> {
    let codewords = deinterleave(bits, version, ec)?;

    let mut pos = 0usize;

    let read = |cw: &[u8], p: &mut usize, n: usize| -> u32 {
        let mut val = 0u32;
        for _ in 0..n {
            if *p < cw.len() * 8 {
                val = (val << 1) | ((cw[*p / 8] >> (7 - *p % 8)) & 1) as u32;
                *p += 1;
            }
        }
        val
    };

    let mode = read(&codewords, &mut pos, 4);

    match mode {
        0b0001 => {
            let cnt_w = if version <= 9 { 10 } else { 12 };
            let count = read(&codewords, &mut pos, cnt_w) as usize;
            let mut digits = String::new();
            let full = count / 3;
            let rem = count % 3;
            for _ in 0..full {
                let v = read(&codewords, &mut pos, 10);
                digits.push_str(&format!("{:03}", v));
            }
            if rem == 2 {
                let v = read(&codewords, &mut pos, 7);
                digits.push_str(&format!("{:02}", v));
            } else if rem == 1 {
                let v = read(&codewords, &mut pos, 4);
                digits.push_str(&format!("{}", v));
            }
            Some(digits)
        }
        0b0100 => {
            let cnt_w = if version <= 9 { 8 } else { 16 };
            let count = read(&codewords, &mut pos, cnt_w) as usize;
            let mut bytes = Vec::new();
            for _ in 0..count {
                bytes.push(read(&codewords, &mut pos, 8) as u8);
            }
            String::from_utf8(bytes).ok()
        }
        _ => None,
    }
}

fn deinterleave(bits: &[u8], version: usize, ec: u8) -> Option<Vec<u8>> {
    let (ec_per_blk, g1n, g1d, g2n, g2d) = crate::encode::EC_TABLE[version - 1][ec as usize];
    let ec_per_blk = ec_per_blk as usize;
    let g1n = g1n as usize;
    let g1d = g1d as usize;
    let g2n = g2n as usize;
    let g2d = g2d as usize;
    let total_blocks = g1n + g2n;
    let max_data = g1d.max(g2d);
    let total_data = g1n * g1d + g2n * g2d;
    let total_cw = total_data + total_blocks * ec_per_blk;

    if bits.len() < total_cw * 8 { return None; }

    let cw: Vec<u8> = bits.chunks(8)
        .take(total_cw)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b))
        .collect();

    let mut data_blocks: Vec<Vec<u8>> = (0..total_blocks).map(|i| {
        let len = if i < g1n { g1d } else { g2d };
        vec![0u8; len]
    }).collect();

    let mut src = 0usize;
    for col in 0..max_data {
        for blk in 0..total_blocks {
            let len = if blk < g1n { g1d } else { g2d };
            if col < len {
                data_blocks[blk][col] = cw[src];
                src += 1;
            }
        }
    }

    let mut result = Vec::new();
    for blk in data_blocks { result.extend_from_slice(&blk); }
    Some(result)
}

fn build_res(sz: usize, version: usize) -> Vec<Vec<bool>> {
    let coords = matrix::ALIGN[version - 1];
    let n = coords.len();
    let mut res = vec![vec![false; sz]; sz];

    for (fr, fc) in [(0usize, 0usize), (0, sz - 7), (sz - 7, 0)] {
        for r in 0..7 { for c in 0..7 { res[fr+r][fc+c] = true; } }
    }

    for i in 0..8usize {
        res[7][i] = true;
        res[i][7] = true;
        res[7][sz - 1 - i] = true;
        res[i][sz - 8] = true;
        res[sz - 8][i] = true;
        res[sz - 1 - i][7] = true;
    }

    for i in 8..sz-8 { res[6][i] = true; res[i][6] = true; }

    for i in 0..n {
        for j in 0..n {
            let r = coords[i] as usize;
            let c = coords[j] as usize;
            if res[r][c] { continue; }
            for dr in 0..5usize {
                for dc in 0..5usize { res[r-2+dr][c-2+dc] = true; }
            }
        }
    }

    for &(r, c) in &matrix::FMT1 { res[r][c] = true; }
    for i in 0..8 { res[sz-1-i][8] = true; res[8][sz-1-i] = true; }
    res[sz-8][8] = true;

    if version >= 7 {
        for i in 0..18 {
            res[i%6][sz-11+i/6] = true;
            res[sz-11+i/6][i%6] = true;
        }
    }

    res
}

