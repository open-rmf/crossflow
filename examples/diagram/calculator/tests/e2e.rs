use assert_cmd::{Command, cargo};

#[test]
fn multiply_by_3() {
    Command::new(cargo::cargo_bin!("calculator"))
        .args(["run", "diagrams/multiply_by_3.json", "4"])
        .assert()
        .stdout("response: 12.0\n");
}
