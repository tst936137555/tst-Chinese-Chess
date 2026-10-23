use std::{fs, path::PathBuf};
use tst_xiangqi::audio::{effect_samples, music_samples, stereo, wav};
fn main() {
    let dir = PathBuf::from("dist/audio-preview");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("闲枰-立体声背景乐.wav"), wav(&music_samples())).unwrap();
    let mut demo = Vec::new();
    for kind in 0..6 {
        for pan in if kind < 3 {
            vec![-0.7, 0., 0.7]
        } else {
            vec![0.]
        } {
            demo.extend(stereo(&effect_samples(kind), pan));
            demo.extend(vec![0.; 44_100 / 2]);
        }
    }
    fs::write(dir.join("木棋-落子吃子将军与结算.wav"), wav(&demo)).unwrap();
    println!("{}", dir.display());
}
