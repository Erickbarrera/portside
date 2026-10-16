use super::*;

fn shortcut(name: &str, exe: &str, id: &str) -> Shortcut {
    Shortcut {
        name: name.into(),
        exe: PathBuf::from(exe),
        start_dir: PathBuf::from("/apps"),
        launch_options: format!("--play {id}"),
    }
}

fn entries(steam: &Steam) -> Vec<Map> {
    match steam.read().expect("read").get("shortcuts") {
        Some(Value::Map(list)) => list
            .0
            .iter()
            .filter_map(|(_, v)| match v {
                Value::Map(m) => Some(m.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn adds_a_shortcut_to_a_fresh_account() {
    let dir = tempfile::tempdir().expect("tempdir");
    let steam = Steam::at(dir.path().to_path_buf());
    let id = steam
        .add(&shortcut("Example RPG", "/apps/Portside", "example-rpg"))
        .expect("add");
    assert_eq!(id, shortcut_id("\"/apps/Portside\"", "Example RPG"));
    assert!(id & 0x8000_0000 != 0);
    let list = entries(&steam);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].str("Exe"), Some("\"/apps/Portside\""));
    assert_eq!(list[0].str("StartDir"), Some("\"/apps\""));
    assert_eq!(list[0].str("LaunchOptions"), Some("--play example-rpg"));
    assert_eq!(list[0].int("appid"), Some(id));
    assert_eq!(
        steam.find_shortcut("--play example-rpg").expect("find"),
        Some(id)
    );
    assert_eq!(steam.find_shortcut("--play other").expect("find"), None);
}

#[test]
fn keeps_other_shortcuts_and_updates_its_own() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).expect("mkdir");
    // Someone's own shortcut, with a field Portside doesn't know.
    let theirs = Map(vec![(
        "shortcuts".into(),
        Value::Map(Map(vec![(
            "0".into(),
            Value::Map(Map(vec![
                ("appid".into(), Value::Int(0x8000_0001)),
                ("AppName".into(), Value::Str("Their Game".into())),
                ("Exe".into(), Value::Str("\"/opt/their\"".into())),
                ("SortAs".into(), Value::Str("zzz".into())),
            ])),
        )])),
    )]);
    let original = vdf::write(&theirs);
    std::fs::write(config.join("shortcuts.vdf"), &original).expect("write");

    let steam = Steam::at(dir.path().to_path_buf());
    let first = steam
        .add(&shortcut("Example RPG", "/apps/Portside", "example-rpg"))
        .expect("add");
    // Portside moved: the same shortcut is updated, keeping its id.
    let again = steam
        .add(&shortcut("Example RPG", "/new/Portside", "example-rpg"))
        .expect("add again");
    assert_eq!(first, again);

    let list = entries(&steam);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].str("SortAs"), Some("zzz"));
    assert_eq!(list[1].str("Exe"), Some("\"/new/Portside\""));
    // The file as it was before Portside touched it is kept once.
    let backup = std::fs::read(config.join("shortcuts.vdf.before-portside")).expect("backup");
    assert_eq!(backup, original);
}

#[test]
fn picks_the_account_used_last() {
    let dir = tempfile::tempdir().expect("tempdir");
    let userdata = dir.path().join("userdata");
    for id in ["0", "111", "222"] {
        std::fs::create_dir_all(userdata.join(id).join("config")).expect("mkdir");
    }
    std::fs::write(userdata.join("111/config/localconfig.vdf"), "x").expect("write");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(userdata.join("222/config/localconfig.vdf"), "x").expect("write");
    let steam = Steam::latest_user(dir.path()).expect("user");
    assert_eq!(steam.user_dir, userdata.join("222"));
}

#[test]
fn writes_artwork_in_steams_sizes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let picture = image::RgbImage::from_pixel(64, 36, image::Rgb([40, 120, 200]));
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(picture)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("encode");
    write_art(dir.path(), 0x8000_0042, &png).expect("art");
    let id = 0x8000_0042u32;
    for (name, w, h) in [
        (format!("{id}p.png"), 600, 900),
        (format!("{id}.png"), 920, 430),
        (format!("{id}_hero.png"), 1920, 620),
    ] {
        let img = image::open(dir.path().join(&name)).expect("open");
        assert_eq!((img.width(), img.height()), (w, h), "{name}");
    }
    assert!(write_art(dir.path(), 1, b"not an image").is_err());
}
