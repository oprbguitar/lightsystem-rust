use pos_devices::{AppConfig, InterfaceConfig};

#[test]
fn creates_instance_config_with_terminal_defaults_and_separate_instance_id() {
    let root = tempfile::tempdir().unwrap();
    let first = AppConfig::load_or_create(root.path().join("a/config.toml"), "CAJA01").unwrap();
    let second = AppConfig::load_or_create(root.path().join("b/config.toml"), "CAJA02").unwrap();
    assert_eq!(first.instance_id, "CAJA01");
    assert_eq!(second.instance_id, "CAJA02");
    assert_eq!(first.interface.columns, 80);
    assert_eq!(first.interface.rows, 25);
    assert_eq!(first.interface.background_color, "black");
    assert_eq!(InterfaceConfig::default().text_color, "green");
}
