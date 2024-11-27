mod it_should {

    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn read_from_stdin_and_write_to_stdout() {
        let mut cmd = Command::cargo_bin("bencode2json").unwrap();
        cmd.write_stdin("4:spam")
            .assert()
            .success()
            .stdout(r#""<string>spam</string>""#);
    }

    #[test]
    fn read_from_a_file_and_write_to_a_file() {
        let temp_dir = tempdir().unwrap();

        let output_file = temp_dir.path().join("output.json");

        let mut cmd = Command::cargo_bin("bencode2json").unwrap();

        cmd.arg("-i")
            .arg("tests/fixtures/sample.bencode")
            .arg("-o")
            .arg(output_file.to_str().unwrap())
            .assert()
            .success();

        let output_content = fs::read_to_string(output_file).expect("Failed to read output file");

        assert_eq!(output_content.trim(), r#"["<string>spam</string>"]"#);
    }

    #[test]
    fn create_the_output_file_if_it_does_not_exist() {
        let temp_dir = tempdir().unwrap();

        let output_file = temp_dir.path().join("new_file.json");

        let mut cmd = Command::cargo_bin("bencode2json").unwrap();

        cmd.arg("-i")
            .arg("tests/fixtures/sample.bencode")
            .arg("-o")
            .arg(output_file.to_str().unwrap())
            .assert()
            .success();

        let output_content = fs::read_to_string(output_file).expect("Failed to read output file");

        assert_eq!(output_content.trim(), r#"["<string>spam</string>"]"#);
    }

    #[test]
    fn fail_when_the_bencoded_input_is_invalid() {
        let mut cmd = Command::cargo_bin("bencode2json").unwrap();
        cmd.write_stdin("a")
            .assert()
            .failure()
            .stderr(predicate::str::contains("Error: Unrecognized first"));
    }

    #[test]
    fn fail_reading_from_non_existing_file() {
        let temp_dir = tempdir().unwrap();

        let output_file = temp_dir.path().join("output.json");

        let mut cmd = Command::cargo_bin("bencode2json").unwrap();

        cmd.arg("-i")
            .arg("non_existing_file.bencode")
            .arg("-o")
            .arg(output_file.to_str().unwrap())
            .assert()
            .failure();
    }

    #[test]
    fn fail_creating_the_output_file_if_the_dir_does_not_exist() {
        let temp_dir = tempdir().unwrap();

        let output_file = temp_dir.path().join("non_existing_dir/new_file.json");

        let mut cmd = Command::cargo_bin("bencode2json").unwrap();

        cmd.arg("-i")
            .arg("tests/fixtures/sample.bencode")
            .arg("-o")
            .arg(output_file.to_str().unwrap())
            .assert()
            .failure();
    }
}
