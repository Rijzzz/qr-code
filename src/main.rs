mod encode;
mod matrix;
mod decode;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: qr <text> [--ec L|M|Q|H] [--out file.png] [--scale N] [--terminal]");
        eprintln!("       qr --decode <file.png>");
        eprintln!("       qr --test");
        std::process::exit(1);
    }

    if args[1] == "--decode" {
        if args.len() < 3 {
            eprintln!("usage: qr --decode <file.png>");
            std::process::exit(1);
        }
        let img = image::open(&args[2]).unwrap_or_else(|_| {
            eprintln!("cant open {}", args[2]);
            std::process::exit(1);
        });
        match decode::from_image(&img) {
            Some(s) => println!("{}", s),
            None => eprintln!("failed to decode"),
        }
        return;
    }

    if args[1] == "--test" {
        run_test();
        return;
    }

    let text = &args[1];
    let mut ec: u8 = 1;
    let mut out: Option<String> = None;
    let mut scale: u32 = 10;
    let mut terminal = false;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--ec" => {
                i += 1;
                ec = match args[i].to_uppercase().as_str() {
                    "L" => 0,
                    "M" => 1,
                    "Q" => 2,
                    "H" => 3,
                    _ => { eprintln!("ec must be L M Q or H"); std::process::exit(1); }
                };
            }
            "--out" => { i += 1; out = Some(args[i].clone()); }
            "--scale" => { i += 1; scale = args[i].parse().unwrap_or(10); }
            "--terminal" => { terminal = true; }
            _ => {}
        }
        i += 1;
    }

    let version = encode::choose_version(text.len(), ec).unwrap_or_else(|| {
        eprintln!("text too long (max version 10 supported)");
        std::process::exit(1);
    });

    let bits = encode::encode_data(text.as_bytes(), version, ec);
    let mat = matrix::build(&bits, version, ec);

    if terminal || out.is_none() {
        matrix::print_block(&mat);
    }

    if let Some(path) = out {
        let img = matrix::to_png(&mat, scale);
        img.save(&path).unwrap_or_else(|_| {
            eprintln!("failed to save {}", path);
        });
        eprintln!("saved {} (version {} ec {})", path, version, ["L","M","Q","H"][ec as usize]);
    }
}

fn run_test() {
    let cases = [
        ("HELLO WORLD", 1u8),
        ("https://github.com/Rijzzz", 1u8),
        ("12345678901234567890", 0u8),
    ];

    for (text, ec) in cases {
        let v = encode::choose_version(text.len(), ec).expect("no version");
        let bits = encode::encode_data(text.as_bytes(), v, ec);
        let mat = matrix::build(&bits, v, ec);
        let img = matrix::to_png(&mat, 8);
        let path = format!("test_qr_{}.png", text.chars().take(10).collect::<String>().replace(['/', ':'], "_"));
        img.save(&path).unwrap();

        match decode::from_image(&img) {
            Some(s) if s == text => eprintln!("[PASS] \"{}\" (v{} ec{})", text, v, ["L","M","Q","H"][ec as usize]),
            Some(s) => eprintln!("[FAIL] got \"{}\" expected \"{}\"", s, text),
            None => eprintln!("[FAIL] decode returned None for \"{}\"", text),
        }
    }
}
