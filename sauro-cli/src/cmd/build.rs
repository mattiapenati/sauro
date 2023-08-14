use std::{collections::HashMap, fs::File, io::BufReader};

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use sauro_core::syntax;

use crate::expand;

/// Compile the project and create the binding source code
#[derive(Parser)]
pub struct BuildCommand {
    /// Use verbose output
    #[arg(long, short)]
    verbose: bool,
    /// Build artifacts in release mode, with optimizations
    #[arg(long, short)]
    release: bool,
    /// Output directory, relative to project root or absolute
    #[arg(long, default_value_t = default_output_path())]
    output: Utf8PathBuf,
    /// Project root directory
    #[arg(default_value_t = current_dir())]
    path: Utf8PathBuf,
}

impl BuildCommand {
    pub fn run(&self) -> Result<()> {
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
        let project = Project::new(&self.path, &config)?;
        let packages = project.build(BuildOptions {
            release: self.release,
        })?;
        for pkg in packages {
            pkg.expand(&self.output)?;
        }

        Ok(())
    }
}

fn default_output_path() -> Utf8PathBuf {
    Utf8PathBuf::new().join("bindings")
}

fn current_dir() -> Utf8PathBuf {
    std::env::current_dir()
        .expect("current working directory is not valid")
        .try_into()
        .unwrap()
}

struct Project<'cfg> {
    config: &'cfg cargo::util::config::Config,
    packages: Vec<cargo::core::Package>,
}

impl<'cfg> Project<'cfg> {
    fn new(
        path: impl AsRef<Utf8Path>,
        config: &'cfg cargo::util::config::Config,
    ) -> anyhow::Result<Self> {
        let path = path.as_ref().to_owned();
        let manifest_file = path.join("Cargo.toml");
        let workspace = cargo::core::Workspace::new(manifest_file.as_std_path(), config)?;

        let packages = workspace
            .members()
            .filter(has_sauro_as_deps)
            .cloned()
            .collect::<Vec<_>>();

        Ok(Self { config, packages })
    }
}

fn has_sauro_as_deps(pkg: &&cargo::core::Package) -> bool {
    pkg.dependencies()
        .iter()
        .any(|pkg| pkg.package_name() == "sauro")
}

struct BuildOptions {
    release: bool,
}

impl<'cfg> Project<'cfg> {
    fn build(&self, build_options: BuildOptions) -> anyhow::Result<Vec<Package>> {
        let mut packages = vec![];
        for pkg in &self.packages {
            let name = pkg.name().as_str().to_owned();
            let ws = cargo::core::Workspace::new(pkg.manifest_path(), self.config)?;

            let mut options = cargo::ops::CompileOptions::new(
                self.config,
                cargo::core::compiler::CompileMode::Build,
            )?;
            if build_options.release {
                options.build_config.requested_profile = "release".into();
            }
            let compilation = cargo::ops::compile(&ws, &options)?;
            let dylib = compilation
                .cdylibs
                .into_iter()
                .map(|l| l.path)
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing library for package {}", name))?;
            let dylib = Utf8PathBuf::try_from(dylib)?;

            let src = cargo::sources::PathSource::new(
                pkg.root(),
                pkg.package_id().source_id(),
                self.config,
            );
            let sources = src
                .list_files(pkg)?
                .into_iter()
                .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
                .map(Utf8PathBuf::try_from)
                .collect::<Result<_, _>>()?;

            packages.push(Package {
                name,
                sources,
                dylib,
            });
        }
        Ok(packages)
    }
}

#[derive(Debug)]
struct Package {
    name: String,
    sources: Vec<Utf8PathBuf>,
    dylib: Utf8PathBuf,
}

impl Package {
    fn expand(&self, output: &Utf8Path) -> anyhow::Result<()> {
        let root = output.join(&self.name);
        let dylib_filename = self.dylib.file_name().unwrap();
        let dylib_name = Self::dylib_name(dylib_filename)?;
        let common_prefix = path_common_prefix(&self.sources)
            .ok_or_else(|| anyhow!("missing common prefix for sources of {} package", self.name))?;

        let mut files = HashMap::new();
        for filename_rs in &self.sources {
            let dylib_prefix = filename_rs
                .strip_prefix(&common_prefix)
                .unwrap()
                .parent()
                .unwrap()
                .components()
                .fold("./".to_owned(), |p, _| format!("{}../", p));

            let filename_ts = Self::typescript_filename(filename_rs, &common_prefix);
            if let Some(source_ts) = Self::expand_source(filename_rs, &dylib_name, &dylib_prefix)? {
                files.insert(filename_ts, source_ts);
            }
        }

        std::fs::create_dir_all(&root)?;
        std::fs::copy(&self.dylib, root.join(dylib_filename))?;
        for (filename, content) in files {
            let filename = root.join(filename);
            if let Some(parent) = filename.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(filename, content)?;
        }

        Ok(())
    }

    fn dylib_name(filename: &str) -> anyhow::Result<String> {
        let (name, _) = filename.rsplit_once('.').unwrap();

        #[cfg(not(windows))]
        let name = name.strip_prefix("lib").unwrap_or(name);

        Ok(name.to_owned())
    }

    fn typescript_filename(source_rs: &Utf8Path, common_prefix: &Utf8Path) -> Utf8PathBuf {
        let filename = source_rs
            .strip_prefix(common_prefix)
            .unwrap()
            .with_extension("ts");
        if filename.file_stem().is_some_and(|s| s == "lib") {
            filename.with_file_name("mod.ts")
        } else {
            filename
        }
    }

    fn expand_source(
        source: &Utf8Path,
        dylib_name: &str,
        dylib_prefix: &str,
    ) -> anyhow::Result<Option<String>> {
        let ifile = File::open(source)?;
        let content = std::io::read_to_string(BufReader::new(ifile))?;
        let ast = syn::parse_file(&content)?;

        fn has_sauro_bindgen_attr(item: &syn::ItemMod) -> bool {
            for attr in &item.attrs {
                if let syn::Meta::Path(path) = &attr.meta {
                    let segments = &path.segments;
                    if segments.len() == 2
                        && segments[0].ident == "sauro"
                        && segments[1].ident == "bindgen"
                    {
                        return true;
                    }
                }
            }
            false
        }

        let mut mods = ast
            .items
            .into_iter()
            .flat_map(|item| match item {
                syn::Item::Mod(m) if has_sauro_bindgen_attr(&m) => Some(m),
                _ => None,
            })
            .collect::<Vec<_>>();

        if mods.len() > 1 {
            anyhow::bail!(
                "more then one bindgen per file is not supported (file: {})",
                source
            );
        }

        mods.pop()
            .map(|item_mod| {
                syntax::parse_module(item_mod)
                    .map_err(anyhow::Error::from)
                    .and_then(|module| expand::expand_module(&module, dylib_name, dylib_prefix))
            })
            .transpose()
    }
}

fn path_common_prefix(p: &[Utf8PathBuf]) -> Option<Utf8PathBuf> {
    fn common_prefix_impl(p: &Utf8Path, q: &Utf8Path) -> Option<Utf8PathBuf> {
        let p = p.components();
        let q = q.components();

        let mut components = p.zip(q).map_while(|(p, q)| (p == q).then_some(p));
        let first = Utf8Path::to_owned(components.next()?.as_ref());
        components
            .fold(first, |prefix, component| prefix.join(component))
            .into()
    }

    let mut iter = p.iter();
    let mut common_prefix = iter.next()?.parent().unwrap().to_owned();
    for next in iter {
        common_prefix = common_prefix_impl(&common_prefix, next)?;
    }
    Some(common_prefix)
}
