pub const EC_TABLE: [[(u8, u8, u8, u8, u8); 4]; 10] = [
    [(7,1,19,0,0),(10,1,16,0,0),(13,1,13,0,0),(17,1,9,0,0)],
    [(10,1,34,0,0),(16,1,28,0,0),(22,1,22,0,0),(28,1,16,0,0)],
    [(15,1,55,0,0),(26,1,44,0,0),(18,2,17,0,0),(22,2,13,0,0)],
    [(20,1,80,0,0),(18,2,32,0,0),(26,2,24,0,0),(16,4,9,0,0)],
    [(26,1,108,0,0),(24,2,43,0,0),(18,2,15,2,16),(22,2,11,2,12)],
    [(18,2,68,0,0),(16,4,27,0,0),(24,4,19,0,0),(28,4,15,0,0)],
    [(20,2,78,0,0),(18,4,31,0,0),(18,2,14,4,15),(26,4,13,1,14)],
    [(24,2,97,0,0),(22,2,38,2,39),(22,4,18,2,19),(26,4,14,2,15)],
    [(30,2,116,0,0),(22,3,36,2,37),(20,4,16,4,17),(24,4,12,4,13)],
    [(18,2,68,2,69),(26,4,43,1,44),(24,6,19,2,20),(28,6,15,2,16)],
];

pub const REMAINDER: [u8; 10] = [0, 7, 7, 7, 7, 7, 0, 0, 0, 0];

pub fn choose_version(n: usize, ec: u8) -> Option<usize> {
    for v in 1..=10 {
        let (_, g1n, g1d, g2n, g2d) = EC_TABLE[v - 1][ec as usize];
        let cap = g1n as usize * g1d as usize + g2n as usize * g2d as usize;
        if cap >= n + 3 {
            return Some(v);
        }
    }
    None
}

pub fn encode_data(data: &[u8], version: usize, ec: u8) -> Vec<u8> {
    let codewords = data_codewords(data, version, ec);
    let (gf_exp, gf_log) = build_gf();

    let (ec_per_blk, g1n, g1d, g2n, g2d) = EC_TABLE[version - 1][ec as usize];
    let ec_per_blk = ec_per_blk as usize;
    let mut data_blocks: Vec<Vec<u8>> = Vec::new();
    let mut ec_blocks: Vec<Vec<u8>> = Vec::new();
    let mut off = 0;

    for _ in 0..g1n as usize {
        let blk = codewords[off..off + g1d as usize].to_vec();
        off += g1d as usize;
        ec_blocks.push(rs_ec(&blk, ec_per_blk, &gf_exp, &gf_log));
        data_blocks.push(blk);
    }
    for _ in 0..g2n as usize {
        let blk = codewords[off..off + g2d as usize].to_vec();
        off += g2d as usize;
        ec_blocks.push(rs_ec(&blk, ec_per_blk, &gf_exp, &gf_log));
        data_blocks.push(blk);
    }

    let max_d = data_blocks.iter().map(|b| b.len()).max().unwrap_or(0);
    let max_e = ec_blocks.iter().map(|b| b.len()).max().unwrap_or(0);

    let mut interleaved: Vec<u8> = Vec::new();
    for i in 0..max_d {
        for b in &data_blocks { if i < b.len() { interleaved.push(b[i]); } }
    }
    for i in 0..max_e {
        for b in &ec_blocks { if i < b.len() { interleaved.push(b[i]); } }
    }

    let mut bits: Vec<u8> = Vec::new();
    for byte in interleaved {
        for shift in (0..8).rev() { bits.push((byte >> shift) & 1); }
    }
    for _ in 0..REMAINDER[version - 1] { bits.push(0); }

    bits
}

fn data_codewords(data: &[u8], version: usize, ec: u8) -> Vec<u8> {
    let (_, g1n, g1d, g2n, g2d) = EC_TABLE[version - 1][ec as usize];
    let cap = g1n as usize * g1d as usize + g2n as usize * g2d as usize;

    let all_numeric = data.iter().all(|&b| b.is_ascii_digit());
    let mut bits: Vec<u8> = Vec::new();

    if all_numeric {
        push_bits(&mut bits, 0b0001, 4);
        let cnt_w = if version <= 9 { 10 } else { 12 };
        push_bits(&mut bits, data.len() as u32, cnt_w);
        for chunk in data.chunks(3) {
            let val: u32 = chunk.iter().fold(0, |a, &b| a * 10 + (b - b'0') as u32);
            let w = match chunk.len() { 3 => 10, 2 => 7, _ => 4 };
            push_bits(&mut bits, val, w);
        }
    } else {
        push_bits(&mut bits, 0b0100, 4);
        let cnt_w = if version <= 9 { 8 } else { 16 };
        push_bits(&mut bits, data.len() as u32, cnt_w);
        for &b in data { push_bits(&mut bits, b as u32, 8); }
    }

    let cap_bits = cap * 8;
    let term = (cap_bits - bits.len()).min(4);
    for _ in 0..term { bits.push(0); }
    while bits.len() % 8 != 0 { bits.push(0); }

    let mut cw: Vec<u8> = bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b))
        .collect();

    let pads = [0xECu8, 0x11u8];
    let mut pi = 0;
    while cw.len() < cap { cw.push(pads[pi]); pi ^= 1; }

    cw
}

fn push_bits(v: &mut Vec<u8>, val: u32, n: usize) {
    for shift in (0..n).rev() { v.push(((val >> shift) & 1) as u8); }
}

fn build_gf() -> ([u8; 512], [u8; 256]) {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut x = 1u32;
    for i in 0..255 {
        exp[i] = x as u8;
        exp[i + 255] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x & 0x100 != 0 { x ^= 0x11D; }
    }
    (exp, log)
}

fn gf_mul(a: u8, b: u8, exp: &[u8; 512], log: &[u8; 256]) -> u8 {
    if a == 0 || b == 0 { return 0; }
    exp[(log[a as usize] as usize + log[b as usize] as usize) % 255]
}

fn poly_mul(a: &[u8], b: &[u8], exp: &[u8; 512], log: &[u8; 256]) -> Vec<u8> {
    let mut res = vec![0u8; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            res[i + j] ^= gf_mul(ai, bj, exp, log);
        }
    }
    res
}

fn gen_poly(n: usize, exp: &[u8; 512], log: &[u8; 256]) -> Vec<u8> {
    let mut p = vec![1u8];
    for i in 0..n {
        p = poly_mul(&p, &[1, exp[i]], exp, log);
    }
    p
}

fn rs_ec(data: &[u8], ec_count: usize, exp: &[u8; 512], log: &[u8; 256]) -> Vec<u8> {
    let gen = gen_poly(ec_count, exp, log);
    let mut buf = vec![0u8; data.len() + ec_count];
    buf[..data.len()].copy_from_slice(data);
    for i in 0..data.len() {
        let c = buf[i];
        if c != 0 {
            for (j, &g) in gen.iter().enumerate() {
                buf[i + j] ^= gf_mul(g, c, exp, log);
            }
        }
    }
    buf[data.len()..].to_vec()
}
