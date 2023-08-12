use std::io::{BufWriter, Write};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

/// Create a new sauro binding
#[derive(Parser)]
pub struct NewCommand {
    /// Use verbose output
    #[arg(long, short)]
    verbose: bool,
    /// Initialize a new repository for the given version control system
    #[arg(long)]
    vcs: Option<cargo::ops::VersionControl>,
    /// Set the package name, default is the directory name
    #[arg(long)]
    name: Option<String>,
    /// Project root directory
    path: Utf8PathBuf,
}

impl NewCommand {
    pub fn run(&self) -> Result<()> {
        self.run_cargo_new()?;
        self.fix_project()?;

        Ok(())
    }

    fn run_cargo_new(&self) -> Result<()> {
        let config = {
            let config = cargo::util::config::Config::default()?;
            let verbosity = if self.verbose {
                cargo::core::shell::Verbosity::Verbose
            } else {
                cargo::core::shell::Verbosity::Normal
            };
            config.shell().set_verbosity(verbosity);
            config
        };

        let options = {
            let vcs = self.vcs;
            let path = self.path.clone().into_std_path_buf();
            let name = self.name.clone();
            cargo::ops::NewOptions::new(vcs, false, true, path, name, None, None)?
        };

        cargo::ops::new(&options, &config)?;

        Ok(())
    }

    fn fix_project(&self) -> Result<()> {
        self.fix_git_ignore()?;
        self.fix_manifest()?;
        self.fix_lib_source()?;

        Ok(())
    }

    fn fix_git_ignore(&self) -> Result<()> {
        let gitignore = self.path.join(".gitignore");
        if gitignore.exists() {
            let ofile = std::fs::OpenOptions::new().append(true).open(gitignore)?;
            let mut ofile = BufWriter::new(ofile);
            writeln!(&mut ofile, "/bindings")?;
        }

        Ok(())
    }

    fn fix_manifest(&self) -> Result<()> {
        let manifest = self.path.join("Cargo.toml");
        let mut local_manifest =
            cargo::util::toml_mut::manifest::LocalManifest::try_new(manifest.as_std_path())?;
        let data = &mut local_manifest.manifest.data;

        let lib_table = {
            let crate_type = toml_edit::Value::Array(toml_edit::Array::from_iter(["cdylib"]));
            toml_edit::Item::Table(toml_edit::Table::from_iter([("crate-type", crate_type)]))
        };
        data.insert("lib", lib_table);

        let dependencies = data.get_mut("dependencies").unwrap();
        let dependencies = dependencies.as_table_mut().unwrap();

        dependencies.insert("sauro", toml_edit::value("*"));

        local_manifest.write()?;

        Ok(())
    }

    fn fix_lib_source(&self) -> Result<()> {
        let src_lib_rs = self.path.join("src/lib.rs");
        std::fs::write(src_lib_rs, SRC_LIB_RS)?;

        Ok(())
    }
}

const SRC_LIB_RS: &str = r#"#[sauro::bindgen]
mod deno {
    fn add(left: usize, right: usize) -> usize {
        left + right
    }
}"#;
