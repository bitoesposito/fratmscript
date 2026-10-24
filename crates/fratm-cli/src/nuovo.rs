//! `fratm nuovo`: creates a new FratmScript project.
//!
//! The default template is a Vite app that imports `.fratm` files through the
//! `fratmscript` plugin; `--semplice` writes just a `main.fratm` to run with
//! `fratm lancia`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Files of the Vite template: (path, contents). `{{name}}` and `{{version}}`
/// are replaced when the project is written.
const VITE_FILES: &[(&str, &str)] = &[
    (
        "package.json",
        r##"{
  "name": "{{name}}",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "devDependencies": {
    "fratmscript": "^{{version}}",
    "vite": "^5.4.0"
  }
}
"##,
    ),
    (
        "vite.config.js",
        r##"import fratmscript from 'fratmscript/vite';

export default {
  plugins: [fratmscript()],
};
"##,
    ),
    (
        "index.html",
        r##"<!doctype html>
<html lang="it">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{name}}</title>
  </head>
  <body>
    <h1>{{name}} 🤌</h1>
    <p id="conteggio"></p>
    <button id="sforna">Sforna na pizza</button>
    <script type="module" src="/src/main.fratm"></script>
  </body>
</html>
"##,
    ),
    (
        "src/main.fratm",
        r##"// 'O punto 'e partenza: index.html carica stu file.
chiamm { descrivi } da "./pizze.fratm"

chist è conteggio = document.querySelector("#conteggio")
chist è sforna = document.querySelector("#sforna")
tien pizze = 0

facc aggiorna() {
    conteggio.textContent = descrivi(pizze)
}

sforna.addEventListener("click", () => {
    pizze++
    aggiorna()
})

aggiorna()
"##,
    ),
    (
        "src/pizze.fratm",
        r##"// Nu modulo: 'e funzioni cu "mann for" se ponno chiammà da n'ato file.
mann for facc descrivi(pizze) {
    si (pizze == 0) {
        piglie "Ancora nisciuna pizza. Accendi 'o furno!"
    }
    piglie `Pizze sfurnate: ${pizze}`
}
"##,
    ),
    (".gitignore", "node_modules\ndist\n"),
    (
        "README.md",
        r##"# {{name}}

A [FratmScript](https://github.com/kkratterf/fratmscript) project made with `fratm nuovo`.

```bash
npm install
npm run dev     # development server
npm run build   # production build in dist/
```

The code is in `src/*.fratm`; the `fratmscript` Vite plugin compiles it to JavaScript.
"##,
    ),
];

const SIMPLE_FILES: &[(&str, &str)] = &[(
    "main.fratm",
    r##"// Lancialo cu: fratm lancia main.fratm
chist è nomme = "{{name}}"

facc saluta(chi) {
    piglie `Uè, ${chi}!`
}

stamm a dì(saluta(nomme))
"##,
)];

/// Which template `fratm nuovo` writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Vite,
    Simple,
}

/// Writes a new project in `dir` and returns the files it created.
/// Fails if `dir` exists and is not empty.
pub fn create(dir: &Path, template: Template, version: &str) -> io::Result<Vec<PathBuf>> {
    if dir.exists() && fs::read_dir(dir)?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("'a cartella {} esiste già e nun è vacante", dir.display()),
        ));
    }
    let name = package_name(dir);
    let files = match template {
        Template::Vite => VITE_FILES,
        Template::Simple => SIMPLE_FILES,
    };
    let mut written = Vec::new();
    for (path, contents) in files {
        let target = dir.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = contents
            .replace("{{name}}", &name)
            .replace("{{version}}", version);
        fs::write(&target, contents)?;
        written.push(target);
    }
    Ok(written)
}

/// A valid npm package name from the folder name: lower case, with anything
/// other than letters, digits, `-`, `_` and `.` turned into `-`.
fn package_name(dir: &Path) -> String {
    let base = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut name: String = base
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    name = name
        .trim_matches(|c| matches!(c, '-' | '.' | '_'))
        .to_string();
    if name.is_empty() {
        "progetto-fratm".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fratm_core::{compile, CompileOptions};

    fn temp_dir(name: &str) -> PathBuf {
        // The project folder keeps `name`, which becomes the package name
        let parent = std::env::temp_dir().join(format!("fratm-nuovo-{}", std::process::id()));
        let dir = parent.join(name);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn package_names_are_valid_for_npm() {
        assert_eq!(
            package_name(Path::new("/tmp/Pizzeria Da Gennaro")),
            "pizzeria-da-gennaro"
        );
        assert_eq!(package_name(Path::new("caffè")), "caff");
        assert_eq!(package_name(Path::new("ok_name.js")), "ok_name.js");
        assert_eq!(package_name(Path::new("---")), "progetto-fratm");
    }

    #[test]
    fn vite_template_compiles_and_fills_placeholders() {
        let dir = temp_dir("Vite App");
        let files = create(&dir, Template::Vite, "1.2.3").unwrap();
        assert_eq!(files.len(), VITE_FILES.len());

        let package = fs::read_to_string(dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&package).unwrap();
        assert_eq!(json["name"], "vite-app");
        assert_eq!(json["devDependencies"]["fratmscript"], "^1.2.3");
        assert!(!package.contains("{{"));

        let main = compile(
            &fs::read_to_string(dir.join("src/main.fratm")).unwrap(),
            CompileOptions::default(),
        )
        .unwrap();
        assert!(
            main.code
                .starts_with("import { descrivi } from \"./pizze.fratm\";"),
            "{}",
            main.code
        );
        assert!(main.code.contains("pizze++;"), "{}", main.code);
        let pizze = compile(
            &fs::read_to_string(dir.join("src/pizze.fratm")).unwrap(),
            CompileOptions::default(),
        )
        .unwrap();
        assert!(
            pizze.code.contains("export function descrivi(pizze)"),
            "{}",
            pizze.code
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn simple_template_is_one_file() {
        let dir = temp_dir("semplice");
        create(&dir, Template::Simple, "1.2.3").unwrap();
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let main = compile(
            &fs::read_to_string(dir.join("main.fratm")).unwrap(),
            CompileOptions::default(),
        )
        .unwrap();
        assert!(
            main.code.contains("const nomme = \"semplice\";"),
            "{}",
            main.code
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_a_folder_that_is_not_empty() {
        let dir = temp_dir("piena");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.fratm"), "stamm a dì(1)\n").unwrap();
        let error = create(&dir, Template::Simple, "1.2.3").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(dir.join("main.fratm")).unwrap(),
            "stamm a dì(1)\n"
        );
        fs::remove_dir_all(&dir).unwrap();

        // An empty folder is fine
        fs::create_dir_all(&dir).unwrap();
        create(&dir, Template::Simple, "1.2.3").unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }
}
