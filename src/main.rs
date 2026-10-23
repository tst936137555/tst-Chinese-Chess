#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() {
    if std::env::args().any(|a| a == "--self-test") {
        if let Err(e) = tst_xiangqi::self_test::run() {
            eprintln!("self-test failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    tst_xiangqi::launch();
}
