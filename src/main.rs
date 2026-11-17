#[cfg(not(target_os = "android"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = std::env::current_dir()?.join(".miora");
    std::fs::create_dir_all(&app_dir)?;

    miora::run(app_dir.join("miora.sqlite3"))
}
