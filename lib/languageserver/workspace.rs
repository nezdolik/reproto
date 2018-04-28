//! A dynamically compiled and updated environment.

use ast;
use core::errors::Result;
use core::{self, Context, Diagnostics, Encoding, Handle, Import, Loc, Position, Resolved,
           ResolvedByPrefix, Resolver, RpPackage, RpRequiredPackage, RpVersionedPackage, Source,
           Span};
use env;
use manifest;
use parser;
use std::collections::Bound;
use std::collections::{hash_map, BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use ty;
use url::Url;

/// Specifies a rename.
#[derive(Debug, Clone)]
pub enum Rename {
    Prefix { prefix: String },
}

/// The result of a find_rename call.
#[derive(Debug, Clone)]
pub enum RenameResult<'a> {
    /// All renames are in the same file as where the rename was requested.
    Local { ranges: &'a Vec<Range> },
    /// A package was renamed, and the range indicates the endl of the import that should be
    /// replaced.
    ImplicitPackage {
        ranges: &'a Vec<Range>,
        position: Position,
    },
}

/// Specifies a type completion.
#[derive(Debug, Clone)]
pub enum Completion {
    /// Completions for type from a different package.
    Absolute {
        prefix: Option<String>,
        path: Vec<String>,
        suffix: Option<String>,
    },
    /// Completions for a given package.
    Package { results: BTreeSet<String> },
    /// Any type, including primitive types.
    Any,
}

/// Specifies a jump
#[derive(Debug, Clone)]
pub enum Jump {
    /// Perform an absolute jump.
    Absolute {
        prefix: Option<String>,
        path: Vec<String>,
    },
    /// Jump to the specified package prefix.
    Package { prefix: String },
    /// Jump to where the prefix is declared.
    Prefix { prefix: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Range {
    /// Start position.
    pub start: Position,
    /// End position.
    pub end: Position,
}

impl Range {
    pub fn contains(&self, p: &Position) -> bool {
        self.start <= *p && *p <= self.end
    }
}

/// Information about a single prefix.
#[derive(Debug, Clone)]
pub struct Prefix {
    /// The span of the prefix.
    pub span: Span,
    /// The package the prefix referes to.
    pub package: RpVersionedPackage,
}

/// Information about a single symbol.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// The name of the symbol.
    pub name: Loc<String>,
    /// Markdown documentation comment.
    pub comment: Option<String>,
}

impl Symbol {
    /// Convert symbol into documentation.
    pub fn to_documentation(&self) -> Option<ty::Documentation> {
        let comment = match self.comment.as_ref() {
            Some(comment) => comment,
            None => return None,
        };

        let doc = ty::MarkupContent {
            kind: ty::MarkupKind::Markdown,
            value: comment.to_string(),
        };

        Some(ty::Documentation::MarkupContent(doc))
    }
}

#[derive(Debug, Clone)]
pub struct LoadedFile {
    /// Url of the loaded file.
    pub url: Url,
    /// Jumps available in the file.
    pub jumps: BTreeMap<Position, (Range, Jump)>,
    /// Corresponding locations that have available type completions.
    pub completions: BTreeMap<Position, (Range, Completion)>,
    /// Rename locations.
    pub renames: BTreeMap<Position, (Range, Rename)>,
    /// All the locations that a given prefix is present at.
    pub prefix_ranges: HashMap<String, Vec<Range>>,
    /// Implicit prefixes which _cannot_ be renamed.
    pub implicit_prefixes: HashMap<String, Position>,
    /// package prefixes.
    pub prefixes: HashMap<String, Prefix>,
    /// Symbols present in the file.
    /// The key is the path that the symbol is located in.
    pub symbols: HashMap<Vec<String>, Vec<Symbol>>,
    /// Exact symbol lookup.
    pub symbol: HashMap<Vec<String>, Span>,
    /// Diagnostics for this file.
    pub diag: Diagnostics,
}

impl LoadedFile {
    pub fn new(url: Url, source: Source) -> Self {
        Self {
            url: url.clone(),
            jumps: BTreeMap::new(),
            completions: BTreeMap::new(),
            renames: BTreeMap::new(),
            prefix_ranges: HashMap::new(),
            implicit_prefixes: HashMap::new(),
            prefixes: HashMap::new(),
            symbols: HashMap::new(),
            symbol: HashMap::new(),
            diag: Diagnostics::new(source.clone()),
        }
    }

    /// Reset all state in the loaded file.
    pub fn clear(&mut self) {
        self.symbols.clear();
        self.symbol.clear();
        self.prefixes.clear();
        self.jumps.clear();
        self.completions.clear();
        self.renames.clear();
        self.prefix_ranges.clear();
        self.diag.clear();
    }

    /// Insert the specified jump.
    pub fn insert_jump(&mut self, span: Span, jump: Jump) -> Result<()> {
        let (start, end) = self.diag.source.span_to_range(span, Encoding::Utf16)?;
        let range = Range { start, end };
        self.jumps.insert(start, (range, jump));
        Ok(())
    }

    /// Set an implicit prefix.
    ///
    /// These prefixes _can not_ be renamed since they are the last part of the package.
    pub fn implicit_prefix(&mut self, prefix: &str, span: Span) -> Result<()> {
        let (start, _) = self.diag.source.span_to_range(span, Encoding::Utf16)?;
        self.implicit_prefixes.insert(prefix.to_string(), start);
        Ok(())
    }

    /// Register a location that is only used to trigger a rename action, but should not be locally
    /// replaced itself.
    pub fn register_rename_prefix(&mut self, prefix: &str, span: Span) -> Result<()> {
        let (start, end) = self.diag.source.span_to_range(span, Encoding::Utf16)?;
        let range = Range { start, end };

        // replace the explicit rename.
        let rename = Rename::Prefix {
            prefix: prefix.to_string(),
        };

        self.renames.insert(start, (range, rename));
        Ok(())
    }

    /// Register the location of a prefix.
    ///
    /// This sets the span up as a location that can be renamed for the given prefix.
    pub fn register_prefix(&mut self, prefix: &str, span: Span) -> Result<()> {
        let (start, end) = self.diag.source.span_to_range(span, Encoding::Utf16)?;
        let range = Range { start, end };

        // replace the explicit rename.
        let rename = Rename::Prefix {
            prefix: prefix.to_string(),
        };

        self.renames.insert(start, (range, rename));

        self.prefix_ranges
            .entry(prefix.to_string())
            .or_insert_with(Vec::new)
            .push(range);

        Ok(())
    }
}

#[derive(Clone)]
pub struct Workspace {
    /// Path of the workspace.
    pub root_path: PathBuf,
    /// Path to manifest.
    pub manifest_path: PathBuf,
    /// Packages which have been loaded through project.
    pub packages: HashMap<RpVersionedPackage, Url>,
    /// The URL of files which have been loaded through project.
    pub loaded_files: HashSet<Url>,
    /// Files which have been loaded through project, including their files.
    pub files: HashMap<Url, LoadedFile>,
    /// Versioned packages that have been looked up.
    lookup: HashMap<RpRequiredPackage, RpVersionedPackage>,
    /// Files which are currently being edited.
    pub edited_files: HashMap<Url, LoadedFile>,
    /// Context where to populate compiler errors.
    ctx: Rc<Context>,
}

impl Workspace {
    /// Create a new workspace from the given path.
    pub fn new<P: AsRef<Path>>(root_path: P, ctx: Rc<Context>) -> Self {
        Self {
            root_path: root_path.as_ref().to_owned(),
            manifest_path: root_path.as_ref().join(env::MANIFEST_NAME),
            packages: HashMap::new(),
            loaded_files: HashSet::new(),
            files: HashMap::new(),
            lookup: HashMap::new(),
            edited_files: HashMap::new(),
            ctx,
        }
    }

    /// Access all files in the workspace.
    pub fn files(&self) -> Vec<(&Url, &LoadedFile)> {
        let mut files = Vec::new();
        files.extend(self.files.iter());
        files.extend(self.edited_files.iter());
        files
    }

    /// Access the loaded file with the given Url.
    pub fn file(&self, url: &Url) -> Option<&LoadedFile> {
        if let Some(file) = self.edited_files.get(url) {
            return Some(file);
        }

        if let Some(file) = self.files.get(url) {
            return Some(file);
        }

        None
    }

    /// Initialize the current project.
    pub fn initialize(&mut self, handle: &Handle) -> Result<()> {
        env::initialize(handle)?;
        Ok(())
    }

    /// Reload the workspace.
    pub fn reload(&mut self) -> Result<()> {
        self.packages.clear();
        self.files.clear();
        self.loaded_files.clear();
        self.lookup.clear();

        let mut manifest = manifest::Manifest::default();

        if !self.manifest_path.is_file() {
            error!(
                "no manifest in root of workspace: {}",
                self.manifest_path.display()
            );
            return Ok(());
        }

        manifest.path = Some(self.manifest_path.to_owned());
        manifest.from_yaml(File::open(&self.manifest_path)?, env::convert_lang)?;

        let mut resolver = env::resolver(&manifest)?;

        for package in &manifest.packages {
            self.process(resolver.as_mut(), package)?;
        }

        self.try_compile(manifest)?;
        Ok(())
    }

    /// Try to compile the current environment.
    fn try_compile(&mut self, manifest: manifest::Manifest) -> Result<()> {
        let ctx = self.ctx.clone();
        ctx.clear()?;

        let lang = manifest.lang_or_nolang();
        let package_prefix = manifest.package_prefix.clone();
        let mut env = lang.into_env(ctx.clone(), package_prefix, self);

        for package in &manifest.packages {
            if let Err(e) = env.import(package) {
                debug!("failed to import: {}: {}", package, e.display());
            }
        }

        if let Err(e) = lang.compile(ctx.clone(), env, manifest) {
            // ignore and just go off diagnostics?
            debug!("compile error: {}", e.display());
        }

        return Ok(());
    }

    fn process(
        &mut self,
        resolver: &mut Resolver,
        package: &RpRequiredPackage,
    ) -> Result<Option<RpVersionedPackage>> {
        // need method to report errors in this stage.
        let (url, source, versioned) = {
            let entry = match self.lookup.entry(package.clone()) {
                hash_map::Entry::Occupied(e) => return Ok(Some(e.get().clone())),
                hash_map::Entry::Vacant(e) => e,
            };

            let resolved = match resolver.resolve(package) {
                Ok(resolved) => resolved,
                Err(_) => return Ok(None),
            };

            let Resolved { version, source } = match resolved.into_iter().last() {
                Some(resolved) => resolved,
                None => return Ok(None),
            };

            let path = match source.path().map(|p| p.to_owned()) {
                Some(path) => path,
                None => return Ok(None),
            };

            let versioned = RpVersionedPackage::new(package.package.clone(), version);
            entry.insert(versioned.clone());

            // TODO: report error through diagnostics.
            let path = match path.canonicalize() {
                Ok(path) => path,
                Err(_) => return Ok(None),
            };

            let path = path.canonicalize()
                .map_err(|e| format!("cannot canonicalize path: {}: {}", path.display(), e))?;

            let url = Url::from_file_path(&path)
                .map_err(|_| format!("cannot build url from path: {}", path.display()))?;

            (url, source, versioned)
        };

        self.loaded_files.insert(url.clone());

        if let Some(mut loaded) = self.edited_files.remove(&url) {
            loaded.clear();
            self.inner_process(resolver, &mut loaded)?;
            self.edited_files.insert(url.clone(), loaded);
        } else {
            let mut loaded = LoadedFile::new(url.clone(), source);

            self.inner_process(resolver, &mut loaded)?;
            self.files.insert(url.clone(), loaded);
        };

        self.packages.insert(versioned.clone(), url);
        Ok(Some(versioned))
    }

    fn inner_process(&mut self, resolver: &mut Resolver, loaded: &mut LoadedFile) -> Result<()> {
        let content = {
            let mut content = String::new();
            let mut reader = loaded.diag.source.read()?;
            reader.read_to_string(&mut content)?;
            content
        };

        let file = match parser::parse(&mut loaded.diag, content.as_str()) {
            Ok(file) => file,
            Err(()) => {
                return Ok(());
            }
        };

        for u in &file.uses {
            let (u, span) = Loc::borrow_pair(u);

            let range = match u.range {
                Some(ref range) => match core::Range::parse(range.as_str()) {
                    Ok(range) => range,
                    Err(_) => continue,
                },
                None => core::Range::any(),
            };

            let package = {
                let (package, span) = Loc::borrow_pair(&u.package);

                let (start, end) = loaded.diag.source.span_to_range(span, Encoding::Utf16)?;
                let range = Range { start, end };

                let content = &content[span.start..span.end];
                let completion = self.package_completion(content, resolver)?;
                loaded.completions.insert(start, (range, completion));
                package
            };

            let parts = match *package {
                ast::Package::Package { ref parts } => parts,
                ast::Package::Error => {
                    continue;
                }
            };

            let endl = match u.endl {
                Some(endl) => endl,
                None => continue,
            };

            let prefix = if let Some(ref alias) = u.alias {
                // note: can be renamed!
                let (alias, span) = Loc::borrow_pair(alias);
                loaded.register_prefix(alias.as_ref(), span)?;
                Some(alias.as_ref())
            } else {
                // note: _cannot_ be renamed since they are implicit.
                match parts.last() {
                    Some(suffix) => {
                        let (suffix, span) = Loc::borrow_pair(suffix);

                        // implicit prefixes cannot be renamed directly.
                        loaded.implicit_prefix(suffix.as_ref(), endl)?;
                        loaded.register_rename_prefix(suffix.as_ref(), span)?;
                        Some(suffix.as_ref())
                    }
                    None => None,
                }
            };

            let package = RpPackage::new(parts.iter().map(|p| p.to_string()).collect());
            let package = RpRequiredPackage::new(package.clone(), range);
            let package = self.process(resolver, &package)?;

            if let Some(prefix) = prefix {
                let prefix = prefix.to_string();

                loaded.insert_jump(
                    span,
                    Jump::Package {
                        prefix: prefix.clone(),
                    },
                )?;

                if let Some(package) = package {
                    loaded.prefixes.insert(prefix, Prefix { span, package });
                };
            }
        }

        let mut queue = VecDeque::new();

        queue.extend(file.decls.iter().map(|d| (vec![], d)));

        while let Some((mut path, decl)) = queue.pop_front() {
            let comment = decl.comment();

            let comment = if !comment.is_empty() {
                Some(
                    comment
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            } else {
                None
            };

            loaded
                .symbols
                .entry(path.clone())
                .or_insert_with(Vec::default)
                .push(Symbol {
                    name: Loc::map(decl.name(), |n| n.to_string()),
                    comment,
                });

            path.push(decl.name().to_string());

            loaded.symbol.insert(path.clone(), Loc::span(&decl.name()));

            self.process_decl(&path, loaded, content.as_str(), decl)?;

            queue.extend(decl.decls().map(|decl| (path.clone(), decl)));
        }

        Ok(())
    }

    /// Process all locations assocaited with the declarations.
    ///
    /// * `completions`, locations which are applicable for type completions.
    fn process_decl<'input>(
        &mut self,
        current: &Vec<String>,
        loaded: &mut LoadedFile,
        content: &str,
        decl: &ast::Decl<'input>,
    ) -> Result<()> {
        use ast::Decl::*;

        match *decl {
            Type(ref ty) => for f in ty.fields() {
                self.process_ty(current, loaded, content, &f.ty)?;
            },
            Tuple(ref tuple) => for f in tuple.fields() {
                self.process_ty(current, loaded, content, &f.ty)?;
            },
            Interface(ref interface) => for f in interface.fields() {
                self.process_ty(current, loaded, content, &f.ty)?;
            },
            Enum(ref _en) => {}
            Service(ref service) => {
                for e in service.endpoints() {
                    for a in &e.arguments {
                        self.process_ty(current, loaded, content, a.channel.ty())?;
                    }

                    if let Some(response) = e.response.as_ref() {
                        self.process_ty(current, loaded, content, response.ty())?;
                    }
                }
            }
        }

        Ok(())
    }

    fn process_ty<'input>(
        &mut self,
        current: &Vec<String>,
        loaded: &mut LoadedFile,
        content: &str,
        ty: &Loc<ast::Type<'input>>,
    ) -> Result<()> {
        let (ty, span) = Loc::borrow_pair(ty);

        match *ty {
            ast::Type::Array { ref inner } => {
                self.process_ty(current, loaded, content, inner.as_ref())?;
            }
            ast::Type::Map { ref key, ref value } => {
                self.process_ty(current, loaded, content, key.as_ref())?;
                self.process_ty(current, loaded, content, value.as_ref())?;
            }
            ref ty => {
                match *ty {
                    ast::Type::Name { ref name } => {
                        let (name, _) = Loc::borrow_pair(name);

                        if let ast::Name::Absolute { ref prefix, .. } = *name {
                            if let Some(ref prefix) = *prefix {
                                let (prefix, span) = Loc::borrow_pair(prefix);
                                loaded.register_prefix(prefix, span)?;
                            }
                        }
                    }
                    _ => {}
                }

                // load jump-to definitions
                if let ast::Type::Name { ref name } = *ty {
                    self.jumps(name, current, loaded)?;
                }

                let (start, end) = loaded.diag.source.span_to_range(span, Encoding::Utf16)?;
                let range = Range { start, end };

                let content = &content[span.start..span.end];
                let completion = self.type_completion(current, content)?;

                loaded.completions.insert(start, (range, completion));
            }
        }

        Ok(())
    }

    /// Register all available jumps.
    fn jumps<'input>(
        &self,
        name: &Loc<ast::Name<'input>>,
        current: &Vec<String>,
        loaded: &mut LoadedFile,
    ) -> Result<()> {
        let (name, _) = Loc::borrow_pair(name);

        match *name {
            ast::Name::Relative { ref parts } => {
                let mut path = current.clone();

                for p in parts {
                    let (p, span) = Loc::borrow_pair(p);

                    path.push(p.to_string());

                    loaded.insert_jump(
                        span,
                        Jump::Absolute {
                            prefix: None,
                            path: path.clone(),
                        },
                    )?;
                }
            }
            ast::Name::Absolute {
                ref prefix,
                ref parts,
            } => {
                let mut path = Vec::new();

                if let Some(ref prefix) = *prefix {
                    let (prefix, span) = Loc::borrow_pair(prefix);

                    loaded.insert_jump(
                        span,
                        Jump::Prefix {
                            prefix: prefix.to_string(),
                        },
                    )?;
                }

                let prefix = prefix.as_ref().map(|p| p.to_string());

                for p in parts {
                    let (p, span) = Loc::borrow_pair(p);

                    path.push(p.to_string());

                    loaded.insert_jump(
                        span,
                        Jump::Absolute {
                            prefix: prefix.clone(),
                            path: path.clone(),
                        },
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Build a package completion.
    fn package_completion(&self, content: &str, resolver: &mut Resolver) -> Result<Completion> {
        debug!("package completion from {:?}", content);

        let mut parts = content.split(|c: char| c.is_whitespace());

        let content = match parts.next() {
            Some(content) => content,
            None => content,
        };

        let mut parts = content
            .split(".")
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let suffix = parts.pop();
        let package = RpPackage::new(parts);

        let resolved = resolver.resolve_by_prefix(&package)?;

        let mut results = BTreeSet::new();

        for r in resolved {
            if let Some(value) = r.package.parts().skip(package.len()).next() {
                if let Some(suffix) = suffix.as_ref() {
                    let suffix = suffix.to_lowercase();

                    if !value.to_lowercase().starts_with(&suffix) {
                        continue;
                    }
                }

                results.insert(value.to_string());
            }
        }

        Ok(Completion::Package { results })
    }

    /// Figure out the kind of completion to support.
    fn type_completion(&self, current: &Vec<String>, content: &str) -> Result<Completion> {
        if content.chars().all(|c| c.is_whitespace()) {
            return Ok(Completion::Any);
        }

        if content.starts_with("::") {
            let content = &content[2..];

            let mut path = current.clone();
            path.extend(content.split("::").map(|p| p.to_string()));

            let suffix = path.pop().and_then(|s| {
                if !s.chars().all(|c| c.is_whitespace()) {
                    Some(s.to_string())
                } else {
                    None
                }
            });

            return Ok(Completion::Absolute {
                prefix: None,
                path,
                suffix,
            });
        }

        let mut path = content
            .split("::")
            .map(|p| p.to_string())
            .collect::<Vec<_>>();

        if !path.is_empty() {
            let prefix = if let Some(first) = path.first() {
                if first.chars().all(|c| c.is_lowercase()) {
                    Some(first.to_string())
                } else {
                    None
                }
            } else {
                None
            };

            if prefix.is_some() {
                path.remove(0);
            }

            let suffix = path.pop().and_then(|s| {
                if !s.chars().all(|c| c.is_whitespace()) {
                    Some(s.to_string())
                } else {
                    None
                }
            });

            return Ok(Completion::Absolute {
                prefix,
                path,
                suffix,
            });
        }

        Ok(Completion::Any)
    }

    /// Find the type completion associated with the given position.
    pub fn find_completion(
        &self,
        url: &Url,
        position: ty::Position,
    ) -> Option<(&LoadedFile, &Completion)> {
        let file = match self.file(url) {
            Some(file) => file,
            None => return None,
        };

        let end = Position {
            line: position.line as usize,
            col: position.character as usize,
        };

        let mut range = file.completions
            .range((Bound::Unbounded, Bound::Included(&end)));

        let (range, value) = match range.next_back() {
            Some((_, &(ref range, ref value))) => (range, value),
            None => return None,
        };

        if !range.contains(&end) {
            return None;
        }

        Some((file, value))
    }

    /// Find the associated jump.
    pub fn find_jump(&self, url: &Url, position: ty::Position) -> Option<(&LoadedFile, &Jump)> {
        let file = match self.file(url) {
            Some(file) => file,
            None => return None,
        };

        let end = Position {
            line: position.line as usize,
            col: position.character as usize,
        };

        let mut range = file.jumps.range((Bound::Unbounded, Bound::Included(&end)));

        let (range, value) = match range.next_back() {
            Some((_, &(ref range, ref value))) => (range, value),
            None => return None,
        };

        if !range.contains(&end) {
            return None;
        }

        Some((file, value))
    }

    /// Find the specified rename.
    pub fn find_rename<'a>(
        &'a self,
        url: &Url,
        position: ty::Position,
    ) -> Option<RenameResult<'a>> {
        let file = match self.file(url) {
            Some(file) => file,
            None => return None,
        };

        let end = Position {
            line: position.line as usize,
            col: position.character as usize,
        };

        let mut range = file.renames
            .range((Bound::Unbounded, Bound::Included(&end)));

        let (range, value) = match range.next_back() {
            Some((_, &(ref range, ref value))) => (range, value),
            None => return None,
        };

        if !range.contains(&end) {
            return None;
        }

        match *value {
            Rename::Prefix { ref prefix } => {
                let ranges = match file.prefix_ranges.get(prefix) {
                    Some(ranges) => ranges,
                    None => return None,
                };

                // implicit prefixes cannot be renamed.
                if let Some(position) = file.implicit_prefixes.get(prefix) {
                    return Some(RenameResult::ImplicitPackage {
                        ranges,
                        position: *position,
                    });
                }

                return Some(RenameResult::Local { ranges });
            }
        }
    }

    /// Get URL to the manifest.
    pub fn manifest_url(&self) -> Result<Url> {
        let url = Url::from_file_path(&self.manifest_path)
            .map_err(|_| format!("cannot convert to url: {}", self.manifest_path.display()))?;

        Ok(url)
    }
}

impl Resolver for Workspace {
    /// Resolve a single package.
    fn resolve(&mut self, package: &RpRequiredPackage) -> Result<Vec<Resolved>> {
        let mut result = Vec::new();

        if let Some(looked_up) = self.lookup.get(package) {
            if let Some(url) = self.packages.get(looked_up) {
                if let Some(loaded) = self.file(url) {
                    result.push(Resolved {
                        version: looked_up.version.clone(),
                        source: loaded.diag.source.clone(),
                    });
                }
            }
        }

        Ok(result)
    }

    /// Not supported for workspace.
    fn resolve_by_prefix(&mut self, _: &RpPackage) -> Result<Vec<ResolvedByPrefix>> {
        Ok(vec![])
    }
}