use super::*;
mod archives;
mod home;
mod play;
mod result;
mod review;
pub(super) fn date_label(ms: u64) -> String {
    let s = ms / 1000 + 8 * 3600;
    let z = (s / 86400) as i64 + 719468;
    let era = z / 146097;
    let d = z - era * 146097;
    let y = (d - d / 1460 + d / 36524 - d / 146096) / 365;
    let mut year = y + era * 400;
    let day = d - (365 * y + y / 4 - y / 100);
    let m = (5 * day + 2) / 153;
    let dd = day - (153 * m + 2) / 5 + 1;
    let mm = m + if m < 10 { 3 } else { -9 };
    if mm <= 2 {
        year += 1;
    }
    format!(
        "{year:04}-{mm:02}-{dd:02} {:02}:{:02}",
        s / 3600 % 24,
        s / 60 % 60
    )
}
