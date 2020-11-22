use serde::{Deserialize, Serialize};
use diskomap::cfg::{Cfg, Integrity};
use diskomap::format::{convert, MapOperation};
use std::fs;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    name: String,
    age: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct NewUser {
    name: String,
    last_visit_date_time: Option<u64>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This example demonstrate how convert history file of map,
    // for other configuration of storing or new types of key-values.

    let file = "db/before_convert_db.txt";
    let mut users = diskomap::BTreeMap::open_or_create(file, Cfg::default())?;
    users.insert(0, User { name: "Masha".to_string(), age: 23 })?;
    users.insert(3, User { name: "Sasha".to_string(), age: 58 })?;
    users.insert(5, User { name: "Pasha".to_string(), age: 33 })?;
    drop(users);
    println!("Source file content:");
    print!("{}", fs::read_to_string(file)?);

    // Convert map history file for new configuration of storing with Sha256 blockchain integrity.
    let converted_file = "db/converted_db.txt";
    let old_cfg = Cfg::default();
    let mut new_cfg = Cfg::default();
    new_cfg.integrity = Some(Integrity::Sha256Chain([0; 32]));

    convert::<i32, User, i32, User, _>(file, old_cfg, converted_file, new_cfg, |map_operation| {
        map_operation
    })?;

    println!("Converted file with Sha256Chain integrity:");
    print!("{}", fs::read_to_string(converted_file)?);

    // Convert map history file for new 'User' structure and crc32 integrity of storing.
    let mut old_cfg = Cfg::default();
    old_cfg.integrity = Some(Integrity::Sha256Chain([0; 32]));
    let mut new_cfg = Cfg::default();
    new_cfg.integrity = Some(Integrity::Crc32);

    convert::<i32, User, i32, NewUser, _>(converted_file, old_cfg, converted_file, new_cfg, |map_operation| {
        match map_operation {
            MapOperation::Insert(key, user) => {
                MapOperation::Insert(key, NewUser { name: user.name, last_visit_date_time: None })
            },
            MapOperation::Remove(key) => {
                MapOperation::Remove(key)
            },
        }
    })?;

    println!("Converted file content with NewUser struct value and crc32 integrity:");
    print!("{}", fs::read_to_string(converted_file)?);

    Ok(())
}