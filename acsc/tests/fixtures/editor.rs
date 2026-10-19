use std::path::PathBuf;

fn main() {
    let record = PathBuf::from(std::env::var_os("ACSC_TEST_RECORD").unwrap());
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut bytes = Vec::new();
    for arg in &args {
        bytes.extend_from_slice(arg.to_str().unwrap().as_bytes());
        bytes.push(0);
    }
    std::fs::write(&record, bytes).unwrap();
    let config = PathBuf::from(args.last().unwrap());
    assert!(config.is_absolute());
    std::fs::copy(&config, record.with_extension("before")).unwrap();
    if let Some(content) = std::env::var_os("ACSC_TEST_EDITOR_CONTENT") {
        std::fs::write(&config, content.to_str().unwrap()).unwrap();
    }
    println!("editor stdout");
    eprintln!("editor stderr");
    std::process::exit(std::env::var("ACSC_TEST_EDITOR_STATUS").unwrap().parse().unwrap());
}
