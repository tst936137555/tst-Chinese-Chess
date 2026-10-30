//! Serial isolated startup/idle measurements. Run after building the release application.
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
use tst_xiangqi::{
    game::{Game, Outcome},
    rules::Side,
    storage::Storage,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/ui-profile".into()),
    );
    let binary = std::env::current_exe()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "tst-xiangqi.exe"
        } else {
            "tst-xiangqi"
        });
    let mut game = Game::default();
    let mut seed = 936137555_u64;
    for _ in 0..300 {
        if game.outcome != Outcome::Playing {
            break;
        }
        let moves = game.board.legal_moves();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        game.play(moves[seed as usize % moves.len()])?;
    }
    game.resign(Side::Red);
    for (scene, count) in [
        ("home", 0),
        ("home-full", 150),
        ("game", 0),
        ("review", 150),
        ("4k", 0),
        ("animation", 0),
        ("animation-4k", 0),
    ] {
        let directory = root.join(scene);
        std::fs::create_dir_all(directory.join("data"))?;
        let store = Storage::new(directory.join("data/state.json"));
        store.update(|data| {
            data.active = None;
            data.drafts.clear();
            data.clear_archives();
            for index in 0..count {
                let mut fixture = game.clone();
                fixture.fork_identity();
                let mut record = fixture.snapshot(true, 0, false);
                record.favorite = index < 50;
                data.archives.push(record);
            }
            Ok(())
        })?;
        let mut command = Command::new(&binary);
        command.args([
            "--profile-ui",
            directory.to_str().unwrap(),
            "--profile-scene",
            if scene == "home-full" { "home" } else { scene },
            "--profile-seconds",
            "10",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(format!("{scene}: {status}").into());
                }
                break;
            }
            if started.elapsed() > Duration::from_secs(60) {
                child.kill()?;
                child.wait()?;
                return Err(format!("{scene}: timeout").into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        println!("{}", directory.join("result.json").display());
    }
    Ok(())
}
