// Ferramenta de inspeção: `cargo run -p hellius-core --example probe -- <arquivo> [função]`.
use hellius_core::{open_native, parse_address, CodeKind, Engine};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("uso: probe <arquivo> [nome da função]");
    let want = args.next();
    let t = std::time::Instant::now();
    let (sum, eng) = open_native(&path, &mut |_, _, _| {}).expect("falha ao abrir");
    let fns = eng.list_functions();
    println!("{} {} {} bits; {} funções; {} strings; {} imports; parcial={} ({:?})",
        sum.format, sum.arch, sum.bits, fns.len(), eng.list_strings().len(), eng.list_imports().len(), eng.is_partial(), t.elapsed());
    let f = match &want {
        Some(n) => fns.iter().find(|f| &f.name == n).expect("função não encontrada"),
        None => fns.iter().find(|f| f.name == "entry").unwrap_or(&fns[0]),
    };
    let a = parse_address(&f.address).unwrap();
    println!("== {} @ {} ({} B, {} blocos, in={} out={})", f.name, f.address, f.size_bytes, f.block_count, f.xrefs_in.len(), f.xrefs_out.len());
    for kind in [CodeKind::Disasm, CodeKind::Pseudo] {
        for l in eng.lines(a, kind).unwrap().iter().take(std::env::var("PROBE_LINES").ok().and_then(|v| v.parse().ok()).unwrap_or(18)) {
            println!("{}  {:<20} {}{}", l.address, l.bytes.clone().unwrap_or_default(), l.text, l.comment.as_ref().map(|c| format!("  ; {c}")).unwrap_or_default());
        }
        println!("--");
    }
}
