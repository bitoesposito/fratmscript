//! `fratm lancia` end to end (needs Node.js, like the golden tests).

use std::process::Command;

fn lancia(name: &str, source: &str) -> (String, String, Option<i32>) {
    let dir = std::env::temp_dir().join(format!("fratm-lancia-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{}.fratm", name));
    std::fs::write(&file, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fratm"))
        .arg("lancia")
        .arg(&file)
        .env("NO_COLOR", "1")
        .output()
        .expect("fratm runs");
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn runtime_error_is_in_neapolitan_at_the_fratm_line() {
    let source = "stamm a dì(\"uno\")\nfacc f(o) {\n    piglie o.nomme.lungh\n}\nf({})\n";
    let (stdout, stderr, code) = lancia("typeerror", source);
    assert_eq!(stdout, "uno\n");
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("Errore a runtime: Riga 3, colonna 20: Staje cercanno 'e leggere 'lungh'"),
        "{}",
        stderr
    );
    assert!(
        stderr.contains("3 │     piglie o.nomme.lungh"),
        "{}",
        stderr
    );
    assert!(!stderr.contains("node:internal"), "{}", stderr);
}

#[test]
fn async_errors_and_thrown_errors_are_reported() {
    let source = "mo vir facc main() {\n    aspett nu bell Promise(r => setTimeout(r, 5))\n    iett nu bell Error(\"Pizza bruciata\")\n}\nmain()\n";
    let (_, stderr, code) = lancia("async", source);
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("Riga 3, colonna 5: Error: Pizza bruciata"),
        "{}",
        stderr
    );
}

#[test]
fn successful_programs_exit_zero() {
    let (stdout, stderr, code) = lancia("ok", "stamm a dì(1 + 1)\n");
    assert_eq!((stdout.as_str(), code), ("2\n", Some(0)), "{}", stderr);
}
