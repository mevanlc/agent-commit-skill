use std::io::Write;

fn main() {
    let path = std::path::PathBuf::from(std::env::var_os("ACSC_TEST_RECORD").unwrap());
    let mut file = std::fs::File::create(&path).unwrap();
    for arg in std::env::args_os().skip(1) {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            file.write_all(arg.as_bytes()).unwrap();
        }
        #[cfg(not(unix))]
        file.write_all(arg.to_str().unwrap().as_bytes()).unwrap();
        file.write_all(&[0]).unwrap();
    }
    std::fs::write(path.with_extension("cwd"), std::env::current_dir().unwrap().to_str().unwrap()).unwrap();
    std::fs::write(path.with_extension("env"), std::env::var("ACSC_TEST_ENV").unwrap()).unwrap();
    println!("wrapper stdout");
    eprintln!("wrapper stderr");
    std::process::exit(std::env::var("ACSC_TEST_STATUS").unwrap().parse().unwrap());
}
