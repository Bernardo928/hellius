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
    // PROBE_ALL=1: decompila todas as funções (pânico, tempo, gotos, __asm).
    if std::env::var("PROBE_ALL").is_ok() {
        let (mut gotos, mut asm, mut fallback, mut lines, mut panics) = (0, 0, 0, 0, 0);
        let mut slow: Vec<(u128, String)> = vec![];
        let mut big: Vec<(usize, String, u64)> = vec![];
        for f in &fns {
            let a = parse_address(&f.address).unwrap();
            let t = std::time::Instant::now();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| eng.lines(a, CodeKind::Pseudo).map(|l| l.to_vec())));
            let ms = t.elapsed().as_millis();
            slow.push((ms, f.name.clone()));
            if let Ok(Ok(ls)) = &r {
                big.push((ls.len(), f.name.clone(), f.size_bytes));
            }
            match r {
                Ok(Ok(ls)) => {
                    lines += ls.len();
                    if ls.first().is_some_and(|l| l.text.contains("simplificado")) {
                        fallback += 1;
                    }
                    gotos += ls.iter().filter(|l| l.text.trim_start().starts_with("goto ")).count();
                    asm += ls.iter().filter(|l| l.text.contains("__asm(")).count();
                }
                _ => {
                    panics += 1;
                    println!("PÂNICO em {} {}", f.name, f.address);
                }
            }
        }
        slow.sort();
        slow.reverse();
        println!("funções={} linhas={} gotos={} asm={} fallback={} pânicos={} total={:?}", fns.len(), lines, gotos, asm, fallback, panics, t.elapsed());
        println!("mais lentas: {:?}", &slow[..slow.len().min(5)]);
        big.sort();
        big.reverse();
        println!("maiores (linhas, nome, bytes): {:?}", &big[..big.len().min(6)]);
        return;
    }
    let f = match &want {
        Some(n) => fns.iter().find(|f| &f.name == n).expect("função não encontrada"),
        None => fns.iter().find(|f| f.name == "entry").unwrap_or(&fns[0]),
    };
    let a = parse_address(&f.address).unwrap();
    println!("== {} @ {} ({} B, {} blocos, in={} out={})", f.name, f.address, f.size_bytes, f.block_count, f.xrefs_in.len(), f.xrefs_out.len());
    // PROBE_PSEUDO=1: só o pseudocódigo, sem endereços (para comparar com o fonte).
    if std::env::var("PROBE_PSEUDO").is_ok() {
        for l in eng.lines(a, CodeKind::Pseudo).unwrap() {
            println!("{}", l.text);
        }
        return;
    }
    for kind in [CodeKind::Disasm, CodeKind::Pseudo] {
        for l in eng.lines(a, kind).unwrap().iter().take(std::env::var("PROBE_LINES").ok().and_then(|v| v.parse().ok()).unwrap_or(18)) {
            println!("{}  {:<20} {}{}", l.address, l.bytes.clone().unwrap_or_default(), l.text, l.comment.as_ref().map(|c| format!("  ; {c}")).unwrap_or_default());
        }
        println!("--");
    }
}
