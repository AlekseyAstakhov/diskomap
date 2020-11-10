use diskomap::Cfg;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_name = "db/db.txt";

    let mut cfg = Cfg::default();
    cfg.on_write_error = Some(Box::new(|err| {
        // This closure will be called on the background thread if there is an error writing to the file.
        dbg!(err);
        std::process::exit(3);
    }));
    let mut map = diskomap::BTreeMap::open_or_create(file_name, cfg)?;
    map.insert(8, "Dasha".to_string())?;

    Ok(())
}
