//! Lists the files of a database that are only in iCloud, i.e. that opening
//! it would have to wait for macOS to download.
//!
//! Usage: cloud <file.cbh|file.2cbh>
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: cloud <database>")?;
    let started = std::time::Instant::now();
    let files = cbh_reader::companions::not_downloaded(path.as_ref())?;
    let bytes: u64 = files.iter().map(|f| f.bytes).sum();
    for f in &files {
        println!("{} {}", f.bytes, f.path.display());
    }
    println!(
        "{} files, {bytes} bytes not downloaded ({:.1} ms)",
        files.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
