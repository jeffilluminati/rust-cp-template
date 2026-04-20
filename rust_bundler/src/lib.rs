#![allow(dead_code)]

use std::io::Read;
use std::mem;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, Group, Punct, Spacing, TokenStream, TokenTree};
// use syn::ToTokens;
use syn::__private::ToTokens;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;

use log::{debug, info, error};
use std::collections::{HashMap, HashSet};

mod cargo_loader;

pub fn bundle_specific_binary<P: AsRef<Path>>(
    package_path: P,
    binary_selected: Option<String>,
    bundler_config: HashMap<BundlerConfig, String>,
) -> String {
    let (bin, lib) = cargo_loader::select_bin_and_lib(package_path, binary_selected);
    let base_path = Path::new(&lib.src_path)
        .parent()
        .expect("lib.src_path has no parent");
    let crate_name = &lib.name.replace("-", "_");

    info!("Expanding binary {:?}", bin.src_path);
    let syntax_tree =
        read_file(Path::new(&bin.src_path)).expect("failed to read binary target source");
    let original_binary_source = ensure_trailing_newline(syntax_tree.clone());
    let binary_file = syn::parse_file(&syntax_tree).expect("failed to parse binary target source");
    let binary_ref_source = binary_file.clone().into_token_stream().to_string();
    let needs_crate_alias = file_references_selected_crate(&binary_file, crate_name);
    let original_binary_items_len = binary_file.items.len();
    let mut file = binary_file;
    ensure_selected_extern_crate(&mut file, crate_name);
    let mut expander = Expander::new(base_path, "", crate_name);
    expander.set_pub_mod_allow_list(&file);
    expander.visit_file_mut(&mut file);
    let support_items_len = file.items.len().saturating_sub(original_binary_items_len);
    let support_code = if support_items_len > 0 {
        let mut support_file = syn::File {
            shebang: None,
            attrs: Vec::new(),
            items: file.items[..support_items_len].to_vec(),
        };
        strip_redundant_support(&mut support_file);
        prune_unused_support_items(&mut support_file, crate_name, &binary_ref_source);
        if support_contains_only_tools(&support_file) {
            prune_tools_support_macros(&mut support_file, &binary_ref_source);
            prune_tools_macro_arms(&mut support_file, &binary_ref_source);
            prune_unused_support_items(&mut support_file, crate_name, &binary_ref_source);
            prune_tools_support_macros(&mut support_file, &binary_ref_source);
        }
        Some(render_support_code(support_file, crate_name, needs_crate_alias))
    } else {
        None
    };

    let bundled = match support_code {
        Some(code) => {
            let mut bundled = original_binary_source;
            bundled.push_str(&code);
            bundled.push('\n');
            bundled
        }
        None => original_binary_source,
    };

    rewrite_for_edition(
        bundled,
        bundler_config
            .get(&BundlerConfig::RustEdition)
            .map(String::as_str),
    )
}

/// Creates a single-source-file version of a Cargo package.
#[deprecated]
pub fn bundle<P: AsRef<Path>>(package_path: P) -> String {
    bundle_specific_binary(package_path, None, HashMap::new())
}

struct Expander<'a> {
    base_path: &'a Path,
    parent_name: &'a str,
    crate_name: &'a str,
    current_module_path: Vec<String>,
    entry_refs: HashSet<String>,
    child_entry_refs: HashMap<String, HashSet<String>>,
    binary_ref_source: Option<String>,
}

impl<'a> Expander<'a> {
    fn new(base_path: &'a Path, parent_name: &'a str, crate_name: &'a str) -> Expander<'a> {
        Expander {
            base_path,
            parent_name,
            crate_name,
            current_module_path: if parent_name.is_empty() {
                Vec::new()
            } else {
                vec![parent_name.to_string()]
            },
            entry_refs: HashSet::new(),
            child_entry_refs: HashMap::new(),
            binary_ref_source: None,
        }
    }

    fn child(
        &self,
        base_path: &'a Path,
        parent_name: &'a str,
        entry_refs: HashSet<String>,
    ) -> Expander<'a> {
        let mut current_module_path = self.current_module_path.clone();
        current_module_path.push(parent_name.to_string());
        Expander {
            base_path,
            parent_name,
            crate_name: self.crate_name,
            current_module_path,
            entry_refs,
            child_entry_refs: HashMap::new(),
            binary_ref_source: self.binary_ref_source.clone(),
        }
    }

    fn expand_items(&mut self, items: &mut Vec<syn::Item>) {
        debug!("expand_items, count={}", items.len());
        self.expand_extern_crate(items);
        self.expand_use_path(items);
    }

    fn expand_extern_crate(&mut self, items: &mut Vec<syn::Item>) {
        let mut new_items = vec![];
        for item in items.drain(..) {
            if is_selected_extern_crate(&item, self.crate_name) {
                info!(
                    "expanding crate(lib.rs) {} in {}",
                    self.crate_name,
                    self.base_path.to_str().unwrap()
                );
                let lib_rs_code =
                    read_file(&self.base_path.join("lib.rs")).expect("failed to read lib.rs");
                debug!("Loaded lib.rs: {}", lib_rs_code.len());
                let lib = syn::parse_file(&lib_rs_code);
                let lib = match lib {
                    Ok(x) => x,
                    Err(e) => {
                        error!("syn lib failed {:?}", e);
                        std::process::exit(1);
                    }
                };
                debug!("parsed lib: {}", debug_str_items(&lib.items));
                let mut lib_items = lib.items;
                let plan = compute_required_direct_modules(
                    self.base_path,
                    "",
                    &self.current_module_path,
                    &lib_items,
                    &self.entry_refs,
                );
                self.child_entry_refs = plan.child_entry_refs.clone();
                prune_items_with_plan(&self.current_module_path, &mut lib_items, &plan.required_children);
                new_items.extend(lib_items);
            } else {
                new_items.push(item);
            }
        }
        *items = new_items;
    }

    fn expand_use_path(&self, items: &mut Vec<syn::Item>) {
        let mut new_items = vec![];
        for mut item in items.drain(..) {
            if let syn::Item::Use(ref mut item_use) = item {
                strip_crate_from_use_tree(&mut item_use.tree, self.crate_name);
            }
            new_items.push(item);
        }
        *items = new_items;
    }

    fn expand_mods(&self, item: &mut syn::ItemMod) {
        if item.content.is_some() {
            return;
        }
        let name = item.ident.to_string();
        let next_module_path = extend_module_path(&self.current_module_path, &name);
        let new_style_path = self.base_path.join(self.parent_name);
        let other_base_path = self.base_path.join(&name);
        let (base_path, code) = vec![
            (self.base_path, format!("{}.rs", name)),
            (&new_style_path, format!("{}.rs", name)),
            (&other_base_path, String::from("mod.rs")),
        ].into_iter()
            .flat_map(|(base_path, file_name)| {
                read_file(&base_path.join(file_name)).map(|code| (base_path, code))
            })
            .next()
            .expect("mod not found");
        info!("expanding mod {} in {}", name, base_path.to_str().unwrap());
        let mut file = syn::parse_file(&code).expect("failed to parse file");
        if self.parent_name == "tools" && name == "main" {
            strip_tools_main_stub(&mut file);
        }
        let mut current_entry_refs = self.child_entry_refs.get(&name).cloned().unwrap_or_default();
        if let Some(source) = &self.binary_ref_source {
            current_entry_refs.extend(collect_named_crate_ref_names_from_tokens(
                source,
                self.crate_name,
                &next_module_path,
            ));
        }
        let mut child_expander = self.child(base_path, name.as_str(), current_entry_refs);
        if self.current_module_path.is_empty() || module_path_is_tools(&next_module_path) {
            let plan = compute_required_direct_modules(
                base_path,
                &name,
                &next_module_path,
                &file.items,
                &child_expander.entry_refs,
            );
            prune_items_with_plan(
                &next_module_path,
                &mut file.items,
                &plan.required_children,
            );
            child_expander.child_entry_refs = plan.child_entry_refs;
        }
        child_expander.visit_file_mut(&mut file);
        item.content = Some((Default::default(), file.items));
    }

    fn expand_crate_path(&mut self, path: &mut syn::Path) {
        if path_starts_with(path, self.crate_name) {
            let new_segments = mem::replace(&mut path.segments, Punctuated::new())
                .into_pairs()
                .skip(1)
                .collect();
            path.segments = new_segments;
        }
    }

    fn set_pub_mod_allow_list(&mut self, file: &syn::File) {
        debug!("set_pub_mod_allow_list");
        let source = file.clone().into_token_stream().to_string();
        self.entry_refs
            .extend(collect_named_crate_ref_names_from_tokens(
                &source,
                self.crate_name,
                &self.current_module_path,
            ));
        self.binary_ref_source = Some(source);
        debug!("binary root refs: {:?}", &self.entry_refs);
    }
}

fn strip_crate_from_use_tree(tree: &mut syn::UseTree, crate_name: &str) {
    match tree {
        syn::UseTree::Path(path) if path.ident == crate_name => {
            *tree = syn::parse_str::<syn::UseTree>(&format!(
                "crate::{}",
                path.tree.to_token_stream()
            ))
            .expect("failed to rewrite crate-qualified use tree");
            strip_crate_from_use_tree(tree, crate_name);
        }
        syn::UseTree::Path(path) => {
            strip_crate_from_use_tree(&mut path.tree, crate_name);
        }
        syn::UseTree::Group(group) => {
            for item in &mut group.items {
                strip_crate_from_use_tree(item, crate_name);
            }
        }
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => {}
    }
}

fn extract_root_mods_from_use_tree(
    tree: &syn::UseTree,
    crate_name: &str,
    seen_crate_name: bool,
) -> Vec<String> {
    match tree {
        syn::UseTree::Path(path) if seen_crate_name => vec![path.ident.to_string()],
        syn::UseTree::Path(path) if path.ident == crate_name => {
            extract_root_mods_from_use_tree(&path.tree, crate_name, true)
        }
        syn::UseTree::Path(_) => Vec::new(),
        syn::UseTree::Group(group) => {
            let mut result = Vec::new();
            for item in &group.items {
                result.extend(extract_root_mods_from_use_tree(
                    item,
                    crate_name,
                    seen_crate_name,
                ));
            }
            result
        }
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => Vec::new(),
    }
}

struct UsedRootCollector<'a> {
    crate_name: &'a str,
    used_roots: HashSet<String>,
}

impl<'a> UsedRootCollector<'a> {
    fn new(crate_name: &'a str) -> Self {
        Self {
            crate_name,
            used_roots: HashSet::new(),
        }
    }

    fn finish(self) -> HashSet<String> {
        self.used_roots
    }
}

impl<'a> VisitMut for UsedRootCollector<'a> {
    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        let first = path.segments.first().map(|segment| segment.ident.to_string());
        let second = path.segments.iter().nth(1).map(|segment| segment.ident.to_string());
        if let (Some(first), Some(second)) = (first, second) {
            if first == self.crate_name {
                self.used_roots.insert(second);
            }
        }
        syn::visit_mut::visit_path_mut(self, path);
    }
}

impl<'a> VisitMut for Expander<'a> {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        debug!(
            "Custom visit_file_mut, item: {}",
            debug_str_items(&file.items)
        );
        for it in &mut file.attrs {
            self.visit_attribute_mut(it)
        }
        // debug!("{:?}", file);
        self.expand_items(&mut file.items);
        for it in &mut file.items {
            self.visit_item_mut(it)
        }
    }

    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        for it in &mut item.attrs {
            self.visit_attribute_mut(it)
        }
        self.visit_visibility_mut(&mut item.vis);
        self.visit_ident_mut(&mut item.ident);
        self.expand_mods(item);
        if let Some(ref mut it) = item.content {
            for it in &mut (it).1 {
                self.visit_item_mut(it);
            }
        }
    }

    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        self.expand_crate_path(path);
        for mut el in Punctuated::pairs_mut(&mut path.segments) {
            let it = el.value_mut();
            self.visit_path_segment_mut(it)
        }
    }
}

fn is_selected_extern_crate(item: &syn::Item, crate_name: &str) -> bool {
    if let syn::Item::ExternCrate(ref item) = *item {
        if item.ident == crate_name {
            return true;
        }
    }
    false
}

fn ensure_selected_extern_crate(file: &mut syn::File, crate_name: &str) {
    if file
        .items
        .iter()
        .any(|item| is_selected_extern_crate(item, crate_name))
    {
        return;
    }
    if !file_references_selected_crate(file, crate_name) {
        return;
    }

    let extern_crate_item =
        syn::parse_str::<syn::Item>(&format!("extern crate {};", crate_name))
            .expect("failed to synthesize extern crate item");
    file.items.insert(0, extern_crate_item);
}

fn file_references_selected_crate(file: &syn::File, crate_name: &str) -> bool {
    let mut collector = UsedRootCollector::new(crate_name);
    let mut file_clone = file.clone();
    collector.visit_file_mut(&mut file_clone);
    let has_path_reference = !collector.finish().is_empty();
    let has_use_reference = file.items.iter().any(|item| {
        if let syn::Item::Use(item_use) = item {
            !extract_root_mods_from_use_tree(&item_use.tree, crate_name, false).is_empty()
        } else {
            false
        }
    });
    has_path_reference || has_use_reference
}

fn path_starts_with(path: &syn::Path, segment: &str) -> bool {
    matches!(
        (path.segments.first(), path.segments.iter().nth(1)),
        (Some(el), Some(_)) if el.ident == segment
    )
}

fn strip_tools_main_stub(file: &mut syn::File) {
    let mut stripped_solve = false;
    let mut stripped_main_macro = false;
    file.items.retain(|item| {
        if !stripped_solve && is_tools_main_solve_stub(item) {
            stripped_solve = true;
            return false;
        }
        if stripped_solve && !stripped_main_macro && is_tools_main_macro_invocation(item) {
            stripped_main_macro = true;
            return false;
        }
        true
    });
}

fn is_tools_main_solve_stub(item: &syn::Item) -> bool {
    matches!(item, syn::Item::Fn(item_fn) if item_fn.sig.ident == "solve" && item_fn.sig.inputs.is_empty())
}

fn is_tools_main_macro_invocation(item: &syn::Item) -> bool {
    let syn::Item::Macro(item_macro) = item else {
        return false;
    };
    let mut segments = item_macro.mac.path.segments.iter();
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(first), Some(second), None) if first.ident == "crate" && second.ident == "main"
    )
}

fn ensure_trailing_newline(mut source: String) -> String {
    if !source.ends_with('\n') {
        source.push('\n');
    }
    source
}

fn rewrite_for_edition(source: String, edition: Option<&str>) -> String {
    match edition {
        Some("2024") => rewrite_edition_2024(source),
        _ => source,
    }
}

fn rewrite_edition_2024(source: String) -> String {
    const NEEDLE: &str = "extern \"C\"";
    let mut rewritten = String::with_capacity(source.len());
    let mut rest = source.as_str();
    while let Some(idx) = rest.find(NEEDLE) {
        let (prefix, tail) = rest.split_at(idx);
        rewritten.push_str(prefix);
        if prefix.trim_end().ends_with("unsafe") {
            rewritten.push_str(NEEDLE);
        } else {
            rewritten.push_str("unsafe ");
            rewritten.push_str(NEEDLE);
        }
        rest = &tail[NEEDLE.len()..];
    }
    rewritten.push_str(rest);
    rewritten
}

fn strip_redundant_support(file: &mut syn::File) {
    let mut stripper = SupportStripper;
    stripper.visit_file_mut(file);
}

fn prune_unused_support_items(file: &mut syn::File, crate_name: &str, binary_source: &str) {
    let entry_refs = collect_named_crate_ref_names_from_tokens(binary_source, crate_name, &[]);
    let mut previous = None;
    loop {
        let snapshot = file.items.iter().map(ToTokens::to_token_stream).collect::<TokenStream>().to_string();
        if previous.as_ref() == Some(&snapshot) {
            break;
        }
        previous = Some(snapshot);
        prune_unused_items_in_scope(
            &mut file.items,
            &[],
            &entry_refs,
            crate_name,
            binary_source,
            "",
        );
    }
}

fn prune_unused_support_macros(file: &mut syn::File, binary_source: &str) {
    let macro_names = collect_named_macro_names_from_items(&file.items);
    if macro_names.is_empty() {
        return;
    }
    let mut reachable = collect_macro_invocation_names(binary_source, &macro_names);
    reachable.extend(collect_macro_invocations_from_non_definition_items(
        &file.items,
        &macro_names,
    ));
    let mut pending = reachable.iter().cloned().collect::<Vec<_>>();

    while let Some(name) = pending.pop() {
        let mut macro_tokens = Vec::new();
        collect_named_macro_tokens(&file.items, &name, &mut macro_tokens);
        for tokens in macro_tokens {
            for dep in collect_macro_invocation_names(&tokens, &macro_names) {
                if reachable.insert(dep.clone()) {
                    pending.push(dep);
                }
            }
        }
    }

    retain_reachable_named_macros(&mut file.items, &reachable);
}

fn prune_unused_macro_arms(file: &mut syn::File, binary_source: &str) {
    let macro_defs = collect_named_macro_arms_from_items(&file.items)
        .into_iter()
        .filter(|(name, _)| should_prune_macro_arms(name))
        .collect::<HashMap<_, _>>();
    if macro_defs.is_empty() {
        return;
    }

    let macro_names = macro_defs.keys().cloned().collect::<HashSet<_>>();
    let mut reachable_arms = HashMap::<String, HashSet<usize>>::new();
    let mut seen_invocations = HashSet::<(String, String)>::new();
    let mut pending = collect_macro_invocations_from_source(binary_source, &macro_names);
    pending.extend(collect_macro_invocations_from_non_definition_items_with_args(
        &file.items,
        &macro_names,
    ));

    while let Some(invocation) = pending.pop() {
        let key = (invocation.name.clone(), invocation.args.to_string());
        if !seen_invocations.insert(key) {
            continue;
        }
        let Some(arms) = macro_defs.get(&invocation.name) else {
            continue;
        };
        for (idx, arm) in arms.iter().enumerate() {
            if !macro_arm_matches_invocation(arm, &invocation.args) {
                continue;
            }
            if reachable_arms
                .entry(invocation.name.clone())
                .or_default()
                .insert(idx)
            {
                pending.extend(collect_macro_invocations_from_stream(
                    &arm.body.stream(),
                    &macro_names,
                ));
            }
        }
    }

    retain_reachable_macro_arms(&mut file.items, &reachable_arms);
}

fn prune_tools_support_macros(file: &mut syn::File, binary_source: &str) {
    with_named_module_file_mut(file, "tools", |tools_file| {
        prune_unused_support_macros(tools_file, binary_source);
    });
}

fn prune_tools_macro_arms(file: &mut syn::File, binary_source: &str) {
    with_named_module_file_mut(file, "tools", |tools_file| {
        prune_unused_macro_arms(tools_file, binary_source);
    });
}

fn support_contains_only_tools(file: &syn::File) -> bool {
    let mut saw_tools = false;
    for item in &file.items {
        let syn::Item::Mod(item_mod) = item else {
            return false;
        };
        if item_mod.ident != "tools" {
            return false;
        }
        saw_tools = true;
    }
    saw_tools
}

fn with_named_module_file_mut(
    file: &mut syn::File,
    module_name: &str,
    mut f: impl FnMut(&mut syn::File),
) {
    for item in &mut file.items {
        let syn::Item::Mod(item_mod) = item else {
            continue;
        };
        if item_mod.ident != module_name {
            continue;
        }
        let Some((_, items)) = &mut item_mod.content else {
            continue;
        };
        let mut nested = syn::File {
            shebang: None,
            attrs: Vec::new(),
            items: mem::take(items),
        };
        f(&mut nested);
        *items = nested.items;
        break;
    }
}

fn should_prune_macro_arms(name: &str) -> bool {
    matches!(name, "prepare" | "main" | "scan" | "scan_value")
}

fn collect_macro_invocations_from_non_definition_items(
    items: &[syn::Item],
    macro_names: &HashSet<String>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in items {
        match item {
            syn::Item::Macro(item_macro) if item_macro.ident.is_some() => {}
            syn::Item::Mod(item_mod) => {
                if let Some((_, child_items)) = &item_mod.content {
                    names.extend(collect_macro_invocations_from_non_definition_items(
                        child_items,
                        macro_names,
                    ));
                }
            }
            _ => {
                names.extend(collect_macro_invocation_names(
                    &item.to_token_stream().to_string(),
                    macro_names,
                ));
            }
        }
    }
    names
}

#[derive(Clone)]
struct MacroInvocation {
    name: String,
    args: TokenStream,
}

#[derive(Clone)]
struct MacroArm {
    matcher: Group,
    body: Group,
    matcher_prefix: Vec<String>,
    matcher_tokens: Vec<String>,
    has_metavar: bool,
}

impl MacroArm {
    fn new(matcher: Group, body: Group) -> Self {
        let matcher_tokens = token_signature_strings(&matcher.stream());
        let has_metavar = token_stream_contains_metavar(&matcher.stream());
        let matcher_prefix = matcher_literal_prefix(&matcher.stream());
        Self {
            matcher,
            body,
            matcher_prefix,
            matcher_tokens,
            has_metavar,
        }
    }
}

fn collect_macro_invocations_from_source(
    source: &str,
    macro_names: &HashSet<String>,
) -> Vec<MacroInvocation> {
    source
        .parse::<TokenStream>()
        .map(|tokens| collect_macro_invocations_from_stream(&tokens, macro_names))
        .unwrap_or_default()
}

fn collect_macro_invocations_from_non_definition_items_with_args(
    items: &[syn::Item],
    macro_names: &HashSet<String>,
) -> Vec<MacroInvocation> {
    let mut invocations = Vec::new();
    for item in items {
        match item {
            syn::Item::Macro(item_macro) if item_macro.ident.is_some() => {}
            syn::Item::Mod(item_mod) => {
                if let Some((_, child_items)) = &item_mod.content {
                    invocations.extend(collect_macro_invocations_from_non_definition_items_with_args(
                        child_items,
                        macro_names,
                    ));
                }
            }
            _ => invocations.extend(collect_macro_invocations_from_stream(
                &item.to_token_stream(),
                macro_names,
            )),
        }
    }
    invocations
}

fn collect_macro_invocations_from_stream(
    tokens: &TokenStream,
    macro_names: &HashSet<String>,
) -> Vec<MacroInvocation> {
    let trees = tokens.clone().into_iter().collect::<Vec<_>>();
    let mut invocations = Vec::new();
    let mut idx = 0usize;
    while idx < trees.len() {
        if let Some((invocation, consumed)) = parse_macro_invocation_at(&trees, idx, macro_names) {
            invocations.push(invocation);
            idx += consumed;
            continue;
        }
        if let TokenTree::Group(group) = &trees[idx] {
            invocations.extend(collect_macro_invocations_from_stream(
                &group.stream(),
                macro_names,
            ));
        }
        idx += 1;
    }
    invocations
}

fn parse_macro_invocation_at(
    trees: &[TokenTree],
    start: usize,
    macro_names: &HashSet<String>,
) -> Option<(MacroInvocation, usize)> {
    let mut idx = start;
    let mut last_ident = match trees.get(idx)? {
        TokenTree::Punct(punct) if punct.as_char() == '$' => {
            idx += 1;
            match trees.get(idx)? {
                TokenTree::Ident(ident) => ident.to_string(),
                _ => return None,
            }
        }
        TokenTree::Ident(ident) => ident.to_string(),
        _ => return None,
    };
    idx += 1;

    loop {
        if matches!(
            (trees.get(idx), trees.get(idx + 1)),
            (Some(TokenTree::Punct(first)), Some(TokenTree::Punct(second)))
                if first.as_char() == ':' && second.as_char() == ':'
        ) {
            idx += 2;
            last_ident = match trees.get(idx)? {
                TokenTree::Punct(punct) if punct.as_char() == '$' => {
                    idx += 1;
                    match trees.get(idx)? {
                        TokenTree::Ident(ident) => ident.to_string(),
                        _ => return None,
                    }
                }
                TokenTree::Ident(ident) => ident.to_string(),
                _ => return None,
            };
            idx += 1;
        } else {
            break;
        }
    }

    if !matches!(trees.get(idx), Some(TokenTree::Punct(punct)) if punct.as_char() == '!') {
        return None;
    }
    if !macro_names.contains(&last_ident) {
        return None;
    }
    let TokenTree::Group(group) = trees.get(idx + 1)?.clone() else {
        return None;
    };
    Some((
        MacroInvocation {
            name: last_ident,
            args: group.stream(),
        },
        idx + 2 - start,
    ))
}

fn collect_named_macro_arms_from_items(
    items: &[syn::Item],
) -> HashMap<String, Vec<MacroArm>> {
    let mut defs = HashMap::new();
    collect_named_macro_arms_into(items, &mut defs);
    defs
}

fn collect_named_macro_arms_into(
    items: &[syn::Item],
    defs: &mut HashMap<String, Vec<MacroArm>>,
) {
    for item in items {
        match item {
            syn::Item::Macro(item_macro) => {
                if let Some(ident) = &item_macro.ident {
                    defs.insert(ident.to_string(), parse_macro_arms(&item_macro.mac.tokens));
                }
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, child_items)) = &item_mod.content {
                    collect_named_macro_arms_into(child_items, defs);
                }
            }
            _ => {}
        }
    }
}

fn parse_macro_arms(tokens: &TokenStream) -> Vec<MacroArm> {
    let trees = tokens.clone().into_iter().collect::<Vec<_>>();
    let mut arms = Vec::new();
    let mut idx = 0usize;
    while idx < trees.len() {
        while matches!(trees.get(idx), Some(TokenTree::Punct(punct)) if punct.as_char() == ';') {
            idx += 1;
        }
        let Some(TokenTree::Group(matcher)) = trees.get(idx).cloned() else {
            break;
        };
        idx += 1;
        if !matches!(trees.get(idx), Some(TokenTree::Punct(punct)) if punct.as_char() == '=') {
            break;
        }
        idx += 1;
        if !matches!(trees.get(idx), Some(TokenTree::Punct(punct)) if punct.as_char() == '>') {
            break;
        }
        idx += 1;
        let Some(TokenTree::Group(body)) = trees.get(idx).cloned() else {
            break;
        };
        idx += 1;
        while matches!(trees.get(idx), Some(TokenTree::Punct(punct)) if punct.as_char() == ';') {
            idx += 1;
        }
        arms.push(MacroArm::new(matcher, body));
    }
    arms
}

fn token_stream_contains_metavar(tokens: &TokenStream) -> bool {
    tokens.clone().into_iter().any(|token| match token {
        TokenTree::Punct(punct) if punct.as_char() == '$' => true,
        TokenTree::Group(group) => token_stream_contains_metavar(&group.stream()),
        _ => false,
    })
}

fn matcher_literal_prefix(tokens: &TokenStream) -> Vec<String> {
    let mut prefix = Vec::new();
    for token in tokens.clone() {
        match &token {
            TokenTree::Punct(punct) if punct.as_char() == '$' => break,
            TokenTree::Group(group) if token_stream_contains_metavar(&group.stream()) => break,
            _ => prefix.push(token_signature_string(&token)),
        }
    }
    prefix
}

fn token_signature_strings(tokens: &TokenStream) -> Vec<String> {
    tokens
        .clone()
        .into_iter()
        .map(|token| token_signature_string(&token))
        .collect()
}

fn token_signature_string(token: &TokenTree) -> String {
    match token {
        TokenTree::Group(group) => match group.delimiter() {
            Delimiter::Brace => format!("{{{}}}", group.stream()),
            Delimiter::Bracket => format!("[{}]", group.stream()),
            Delimiter::Parenthesis => format!("({})", group.stream()),
            Delimiter::None => group.stream().to_string(),
        },
        _ => token.to_string(),
    }
}

fn macro_arm_matches_invocation(arm: &MacroArm, args: &TokenStream) -> bool {
    let invocation_tokens = token_signature_strings(args);
    if !arm.has_metavar {
        return invocation_tokens == arm.matcher_tokens;
    }
    if !arm.matcher_prefix.is_empty() {
        return invocation_tokens.starts_with(&arm.matcher_prefix);
    }
    true
}

fn retain_reachable_macro_arms(
    items: &mut Vec<syn::Item>,
    reachable_arms: &HashMap<String, HashSet<usize>>,
) {
    for item in items {
        match item {
            syn::Item::Macro(item_macro) => {
                let Some(ident) = &item_macro.ident else {
                    continue;
                };
                let Some(keep) = reachable_arms.get(&ident.to_string()) else {
                    continue;
                };
                let arms = parse_macro_arms(&item_macro.mac.tokens);
                if keep.is_empty() || keep.len() == arms.len() {
                    continue;
                }
                item_macro.mac.tokens = rebuild_macro_arms(&arms, keep);
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, child_items)) = &mut item_mod.content {
                    retain_reachable_macro_arms(child_items, reachable_arms);
                }
            }
            _ => {}
        }
    }
}

fn rebuild_macro_arms(arms: &[MacroArm], keep: &HashSet<usize>) -> TokenStream {
    let mut tokens = TokenStream::new();
    for (idx, arm) in arms.iter().enumerate() {
        if !keep.contains(&idx) {
            continue;
        }
        tokens.extend(std::iter::once(TokenTree::Group(arm.matcher.clone())));
        tokens.extend(std::iter::once(TokenTree::Punct(Punct::new('=', Spacing::Joint))));
        tokens.extend(std::iter::once(TokenTree::Punct(Punct::new('>', Spacing::Alone))));
        tokens.extend(std::iter::once(TokenTree::Group(arm.body.clone())));
        tokens.extend(std::iter::once(TokenTree::Punct(Punct::new(';', Spacing::Alone))));
    }
    tokens
}

#[derive(Default)]
struct ItemInfo {
    defined_names: HashSet<String>,
    deps: HashSet<String>,
    child_entry_refs: HashMap<String, HashSet<String>>,
    use_binding_to_child: HashMap<String, String>,
    always_keep: bool,
}

fn prune_unused_items_in_scope(
    items: &mut Vec<syn::Item>,
    current_module_path: &[String],
    external_entry_refs: &HashSet<String>,
    crate_name: &str,
    binary_source: &str,
    ancestor_scope_tokens: &str,
) {
    if current_module_path.len() > 1 && !module_path_is_tools(current_module_path) {
        return;
    }
    if items.is_empty() {
        return;
    }

    let child_infos = build_child_infos(items, current_module_path);
    let child_module_names: HashSet<String> = child_infos.keys().cloned().collect();
    let child_module_indices: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if let syn::Item::Mod(item_mod) = item {
                Some((item_mod.ident.to_string(), idx))
            } else {
                None
            }
        })
        .collect();
    let macro_to_child = build_macro_to_child(&child_infos);
    let reexport_to_child = build_reexport_to_child(items, current_module_path, &child_infos);
    let macro_item_names = items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Macro(item_macro) => item_macro.ident.as_ref().map(ToString::to_string),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut item_infos = items
        .iter()
        .map(|item| ItemInfo {
            defined_names: collect_item_defined_names(item, current_module_path, &child_infos),
            ..ItemInfo::default()
        })
        .collect::<Vec<_>>();
    let candidate_names = item_infos
        .iter()
        .flat_map(|info| info.defined_names.iter().cloned())
        .collect::<HashSet<_>>();

    for (idx, item) in items.iter().enumerate() {
        item_infos[idx].deps = collect_item_deps(
            item,
            current_module_path,
            &candidate_names,
            &macro_item_names,
        );
        let defined_names = item_infos[idx].defined_names.clone();
        item_infos[idx].deps.retain(|name| !defined_names.contains(name));
        item_infos[idx].child_entry_refs = collect_child_entry_refs_from_item(
            item,
            current_module_path,
            &child_module_names,
        );
        item_infos[idx].use_binding_to_child = collect_use_binding_to_child(
            item,
            current_module_path,
            &child_infos,
        );
        item_infos[idx].always_keep = should_always_keep_item(item, crate_name, current_module_path);
    }

    let mut name_to_indices = HashMap::<String, Vec<usize>>::new();
    for (idx, info) in item_infos.iter().enumerate() {
        for name in &info.defined_names {
            name_to_indices.entry(name.clone()).or_default().push(idx);
        }
    }

    let mut keep = vec![false; items.len()];
    let mut pending = external_entry_refs.iter().cloned().collect::<Vec<_>>();
    let mut reachable_names = HashSet::<String>::new();
    let mut child_entry_refs = HashMap::<String, HashSet<String>>::new();

    pending.extend(collect_named_crate_ref_names_from_tokens(
        binary_source,
        crate_name,
        current_module_path,
    ));
    if module_path_is_tools(current_module_path) {
        pending.extend(collect_named_crate_ref_names_from_tokens(
            ancestor_scope_tokens,
            crate_name,
            current_module_path,
        ));
        pending.extend(
            collect_candidate_name_mentions(
                ancestor_scope_tokens,
                &candidate_names,
                &macro_item_names,
            )
            .into_iter(),
        );
    }

    let mark_item = |idx: usize,
                     keep: &mut [bool],
                     pending: &mut Vec<String>,
                     child_entry_refs: &mut HashMap<String, HashSet<String>>| {
        if keep[idx] {
            return;
        }
        keep[idx] = true;
        pending.extend(item_infos[idx].deps.iter().cloned());
        if !matches!(items[idx], syn::Item::Use(_)) {
            for (child, refs) in &item_infos[idx].child_entry_refs {
                child_entry_refs
                    .entry(child.clone())
                    .or_default()
                    .extend(refs.iter().cloned());
            }
        }
    };

    for (idx, info) in item_infos.iter().enumerate() {
        if info.always_keep {
            mark_item(idx, &mut keep, &mut pending, &mut child_entry_refs);
        }
    }

    while let Some(name) = pending.pop() {
        if !reachable_names.insert(name.clone()) {
            continue;
        }

        if let Some(indices) = name_to_indices.get(&name) {
            for idx in indices {
                mark_item(*idx, &mut keep, &mut pending, &mut child_entry_refs);
                if let Some(child) = item_infos[*idx].use_binding_to_child.get(&name) {
                    child_entry_refs
                        .entry(child.clone())
                        .or_default()
                        .insert(name.clone());
                    if let Some(child_idx) = child_module_indices.get(child) {
                        mark_item(*child_idx, &mut keep, &mut pending, &mut child_entry_refs);
                    }
                }
            }
        }

        if let Some(child) =
            resolve_ref_to_child(&name, &child_module_names, &reexport_to_child, &macro_to_child)
        {
            child_entry_refs
                .entry(child.clone())
                .or_default()
                .insert(name.clone());
            if let Some(child_idx) = child_module_indices.get(&child) {
                mark_item(*child_idx, &mut keep, &mut pending, &mut child_entry_refs);
            }
        }
    }

    // Collect tokens from kept items at this scope, per-item, to pass as sibling
    // context to children. This ensures that when pruning impl methods inside a child
    // module, references from sibling modules are visible (e.g., random_generator
    // calling rng.rand64() keeps rand64 alive inside xorshift). We exclude the child
    // module's own tokens to avoid false self-references.
    let kept_item_tokens: Vec<Option<String>> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            if keep[idx] {
                Some(item.to_token_stream().to_string())
            } else {
                None
            }
        })
        .collect();

    for (idx, item) in items.iter_mut().enumerate() {
        let syn::Item::Mod(item_mod) = item else {
            continue;
        };
        if !keep[idx] {
            continue;
        }
        let Some((_, child_items)) = item_mod.content.as_mut() else {
            continue;
        };
        let child_name = item_mod.ident.to_string();
        let mut child_refs = child_entry_refs.remove(&child_name).unwrap_or_default();
        child_refs.extend(collect_named_crate_ref_names_from_tokens(
            binary_source,
            crate_name,
            &extend_module_path(current_module_path, &child_name),
        ));
        if current_module_path.is_empty() && child_name != "tools" {
            continue;
        }
        // Build ancestor context: parent's ancestor tokens + sibling items (excluding this child).
        let sibling_tokens = kept_item_tokens
            .iter()
            .enumerate()
            .filter(|(i, t)| *i != idx && t.is_some())
            .map(|(_, t)| t.as_deref().unwrap())
            .collect::<Vec<_>>()
            .join(" ");
        let child_ancestor_tokens = format!("{} {}", ancestor_scope_tokens, &sibling_tokens);
        prune_unused_items_in_scope(
            child_items,
            &extend_module_path(current_module_path, &child_name),
            &child_refs,
            crate_name,
            binary_source,
            &child_ancestor_tokens,
        );
    }

    prune_unused_impl_methods_in_scope(items, &keep, current_module_path, binary_source, ancestor_scope_tokens);

    let mut trimmed = Vec::with_capacity(items.len());
    for (idx, mut item) in mem::take(items).into_iter().enumerate() {
        if !keep[idx] {
            continue;
        }
        if let syn::Item::Use(item_use) = &mut item {
            if !item_infos[idx].use_binding_to_child.is_empty() {
                let allowed_names = item_infos[idx]
                    .defined_names
                    .intersection(&reachable_names)
                    .cloned()
                    .collect::<HashSet<_>>();
                if !trim_use_item(item_use, &allowed_names) && !item_infos[idx].always_keep {
                    continue;
                }
            }
        }
        if let syn::Item::Impl(item_impl) = &item {
            if item_impl.trait_.is_none() && item_impl.items.is_empty() {
                continue;
            }
        }
        trimmed.push(item);
    }
    *items = trimmed;
}

fn render_support_code(file: syn::File, crate_name: &str, needs_crate_alias: bool) -> String {
    let mut code = String::new();
    if needs_crate_alias {
        code.push_str(&format!("extern crate self as {};", crate_name));
    }
    code.push_str(&compact_token_stream(strip_redundant_token_attrs(
        file.into_token_stream(),
    )));
    code
}

enum CompactSurfaceToken {
    Ident(String),
    Literal(String),
    Punct(char, Spacing),
    Open(char),
    Close(char),
}

fn compact_token_stream(tokens: TokenStream) -> String {
    let mut surface = Vec::new();
    push_compact_surface_tokens(tokens, &mut surface);
    let mut compact = String::new();
    let mut prev = None;
    for token in &surface {
        if let Some(prev_token) = prev {
            if compact_tokens_need_space(prev_token, token) {
                compact.push(' ');
            }
        }
        match token {
            CompactSurfaceToken::Ident(ident) | CompactSurfaceToken::Literal(ident) => {
                compact.push_str(ident);
            }
            CompactSurfaceToken::Punct(ch, _) | CompactSurfaceToken::Open(ch) | CompactSurfaceToken::Close(ch) => {
                compact.push(*ch);
            }
        }
        prev = Some(token);
    }
    compact
}

fn push_compact_surface_tokens(tokens: TokenStream, surface: &mut Vec<CompactSurfaceToken>) {
    for token in tokens {
        match token {
            TokenTree::Ident(ident) => surface.push(CompactSurfaceToken::Ident(ident.to_string())),
            TokenTree::Literal(lit) => surface.push(CompactSurfaceToken::Literal(lit.to_string())),
            TokenTree::Punct(punct) => {
                surface.push(CompactSurfaceToken::Punct(punct.as_char(), punct.spacing()));
            }
            TokenTree::Group(group) => match group.delimiter() {
                Delimiter::Parenthesis => {
                    surface.push(CompactSurfaceToken::Open('('));
                    push_compact_surface_tokens(group.stream(), surface);
                    surface.push(CompactSurfaceToken::Close(')'));
                }
                Delimiter::Bracket => {
                    surface.push(CompactSurfaceToken::Open('['));
                    push_compact_surface_tokens(group.stream(), surface);
                    surface.push(CompactSurfaceToken::Close(']'));
                }
                Delimiter::Brace => {
                    surface.push(CompactSurfaceToken::Open('{'));
                    push_compact_surface_tokens(group.stream(), surface);
                    surface.push(CompactSurfaceToken::Close('}'));
                }
                Delimiter::None => push_compact_surface_tokens(group.stream(), surface),
            },
        }
    }
}

fn compact_tokens_need_space(prev: &CompactSurfaceToken, curr: &CompactSurfaceToken) -> bool {
    if matches!(prev, CompactSurfaceToken::Punct(_, Spacing::Joint)) {
        return false;
    }
    match (prev, curr) {
        (CompactSurfaceToken::Punct('#', _), CompactSurfaceToken::Open('[')) => return false,
        (CompactSurfaceToken::Punct('!', _), CompactSurfaceToken::Open(_)) => return false,
        (CompactSurfaceToken::Punct('\'', _), CompactSurfaceToken::Ident(_)) => return false,
        (CompactSurfaceToken::Punct('$', _), CompactSurfaceToken::Ident(_)) => return false,
        (CompactSurfaceToken::Punct('/', _), CompactSurfaceToken::Punct('/' | '*', _)) => {
            return true;
        }
        (CompactSurfaceToken::Punct('*', _), CompactSurfaceToken::Punct('/', _)) => {
            return true;
        }
        (CompactSurfaceToken::Literal(_), CompactSurfaceToken::Punct('.', _)) => return true,
        _ => {}
    }
    if compact_token_is_wordlike(prev) && compact_token_is_wordlike(curr) {
        return true;
    }
    if matches!(prev, CompactSurfaceToken::Close(_))
        && (compact_token_is_wordlike(curr)
            || matches!(curr, CompactSurfaceToken::Punct('#', _)))
    {
        return true;
    }
    matches!(prev, CompactSurfaceToken::Punct(_, _))
        && matches!(curr, CompactSurfaceToken::Punct(_, _))
}

fn compact_token_is_wordlike(token: &CompactSurfaceToken) -> bool {
    matches!(
        token,
        CompactSurfaceToken::Ident(_) | CompactSurfaceToken::Literal(_)
    )
}

fn strip_redundant_token_attrs(tokens: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let mut stripped = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if is_hash_punct(&tokens[i]) {
            if i + 1 < tokens.len() && is_redundant_attr_group(&tokens[i + 1]) {
                i += 2;
                continue;
            }
            if i + 2 < tokens.len()
                && is_bang_punct(&tokens[i + 1])
                && is_redundant_attr_group(&tokens[i + 2])
            {
                i += 3;
                continue;
            }
        }

        stripped.push(match tokens[i].clone() {
            TokenTree::Group(group) => TokenTree::Group(strip_group_token_attrs(group)),
            other => other,
        });
        i += 1;
    }
    stripped.into_iter().collect()
}

fn strip_group_token_attrs(group: Group) -> Group {
    let mut stripped = Group::new(
        group.delimiter(),
        strip_redundant_token_attrs(group.stream()),
    );
    stripped.set_span(group.span());
    stripped
}

fn is_hash_punct(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == '#')
}

fn is_bang_punct(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == '!')
}

fn is_redundant_attr_group(token: &TokenTree) -> bool {
    let TokenTree::Group(group) = token else {
        return false;
    };
    if group.delimiter() != Delimiter::Bracket {
        return false;
    }
    attr_group_name(group)
        .map(|name| name == "doc" || name == "allow")
        .unwrap_or(false)
}

fn attr_group_name(group: &Group) -> Option<String> {
    let mut tokens = group.stream().into_iter();
    match tokens.next() {
        Some(TokenTree::Ident(ident)) => Some(ident.to_string()),
        _ => None,
    }
}

struct SupportStripper;

impl VisitMut for SupportStripper {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        strip_attrs(&mut file.attrs);
        retain_items(&mut file.items);
        for item in &mut file.items {
            self.visit_item_mut(item);
        }
    }

    fn visit_item_mut(&mut self, item: &mut syn::Item) {
        strip_item_attrs(item);
        syn::visit_mut::visit_item_mut(self, item);
    }

    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        strip_attrs(&mut item.attrs);
        if let Some((_, items)) = &mut item.content {
            retain_items(items);
            for child in items {
                self.visit_item_mut(child);
            }
        }
    }

    fn visit_item_impl_mut(&mut self, item: &mut syn::ItemImpl) {
        strip_attrs(&mut item.attrs);
        retain_impl_items(&mut item.items);
        for child in &mut item.items {
            self.visit_impl_item_mut(child);
        }
    }

    fn visit_impl_item_mut(&mut self, item: &mut syn::ImplItem) {
        strip_impl_item_attrs(item);
        syn::visit_mut::visit_impl_item_mut(self, item);
    }

    fn visit_item_trait_mut(&mut self, item: &mut syn::ItemTrait) {
        strip_attrs(&mut item.attrs);
        retain_trait_items(&mut item.items);
        for child in &mut item.items {
            self.visit_trait_item_mut(child);
        }
    }

    fn visit_trait_item_mut(&mut self, item: &mut syn::TraitItem) {
        strip_trait_item_attrs(item);
        syn::visit_mut::visit_trait_item_mut(self, item);
    }
}

fn retain_items(items: &mut Vec<syn::Item>) {
    let retained = items
        .drain(..)
        .filter(|item| !should_strip_attrs(item_attrs(item)))
        .collect();
    *items = retained;
}

fn retain_impl_items(items: &mut Vec<syn::ImplItem>) {
    let retained = items
        .drain(..)
        .filter(|item| !should_strip_attrs(impl_item_attrs(item)))
        .collect();
    *items = retained;
}

fn retain_trait_items(items: &mut Vec<syn::TraitItem>) {
    let retained = items
        .drain(..)
        .filter(|item| !should_strip_attrs(trait_item_attrs(item)))
        .collect();
    *items = retained;
}

fn strip_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| !is_redundant_attr(attr));
}

fn should_strip_attrs(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(is_test_only_attr)
}

fn is_redundant_attr(attr: &syn::Attribute) -> bool {
    attr.path.is_ident("doc") || attr.path.is_ident("allow")
}

fn is_test_only_attr(attr: &syn::Attribute) -> bool {
    attr.path.is_ident("test")
        || (attr.path.is_ident("cfg") && attr.tokens.to_string().contains("test"))
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        syn::Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(item) => &item.attrs,
        syn::ImplItem::Method(item) => &item.attrs,
        syn::ImplItem::Type(item) => &item.attrs,
        syn::ImplItem::Macro(item) => &item.attrs,
        syn::ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn trait_item_attrs(item: &syn::TraitItem) -> &[syn::Attribute] {
    match item {
        syn::TraitItem::Const(item) => &item.attrs,
        syn::TraitItem::Method(item) => &item.attrs,
        syn::TraitItem::Type(item) => &item.attrs,
        syn::TraitItem::Macro(item) => &item.attrs,
        syn::TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn strip_item_attrs(item: &mut syn::Item) {
    match item {
        syn::Item::Const(item) => strip_attrs(&mut item.attrs),
        syn::Item::Enum(item) => strip_attrs(&mut item.attrs),
        syn::Item::ExternCrate(item) => strip_attrs(&mut item.attrs),
        syn::Item::Fn(item) => strip_attrs(&mut item.attrs),
        syn::Item::ForeignMod(item) => strip_attrs(&mut item.attrs),
        syn::Item::Impl(item) => strip_attrs(&mut item.attrs),
        syn::Item::Macro(item) => strip_attrs(&mut item.attrs),
        syn::Item::Mod(item) => strip_attrs(&mut item.attrs),
        syn::Item::Static(item) => strip_attrs(&mut item.attrs),
        syn::Item::Struct(item) => strip_attrs(&mut item.attrs),
        syn::Item::Trait(item) => strip_attrs(&mut item.attrs),
        syn::Item::TraitAlias(item) => strip_attrs(&mut item.attrs),
        syn::Item::Type(item) => strip_attrs(&mut item.attrs),
        syn::Item::Union(item) => strip_attrs(&mut item.attrs),
        syn::Item::Use(item) => strip_attrs(&mut item.attrs),
        syn::Item::Verbatim(_) => {}
        _ => {}
    }
}

fn strip_impl_item_attrs(item: &mut syn::ImplItem) {
    match item {
        syn::ImplItem::Const(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Method(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Type(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Macro(item) => strip_attrs(&mut item.attrs),
        syn::ImplItem::Verbatim(_) => {}
        _ => {}
    }
}

fn strip_trait_item_attrs(item: &mut syn::TraitItem) {
    match item {
        syn::TraitItem::Const(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Method(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Type(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Macro(item) => strip_attrs(&mut item.attrs),
        syn::TraitItem::Verbatim(_) => {}
        _ => {}
    }
}

#[derive(Default)]
struct ModuleInfo {
    deps: HashSet<String>,
    exported_macros: HashSet<String>,
    public_names: HashSet<String>,
    child_entry_refs: HashMap<String, HashSet<String>>,
}

#[derive(Default)]
struct ModulePrunePlan {
    required_children: HashSet<String>,
    child_entry_refs: HashMap<String, HashSet<String>>,
}

fn extend_module_path(current_module_path: &[String], child_name: &str) -> Vec<String> {
    let mut next = current_module_path.to_vec();
    next.push(child_name.to_string());
    next
}

fn compute_required_direct_modules(
    base_path: &Path,
    parent_name: &str,
    current_module_path: &[String],
    items: &[syn::Item],
    entry_refs: &HashSet<String>,
) -> ModulePrunePlan {
    let child_module_names: HashSet<String> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(item_mod) = item {
                Some(item_mod.ident.to_string())
            } else {
                None
            }
        })
        .collect();
    if child_module_names.is_empty() {
        return ModulePrunePlan::default();
    }

    let mut child_infos = HashMap::<String, ModuleInfo>::new();
    for item in items {
        let syn::Item::Mod(item_mod) = item else {
            continue;
        };
        let name = item_mod.ident.to_string();
        child_infos.insert(
            name.clone(),
            collect_direct_module_info(
                base_path,
                parent_name,
                item_mod,
                current_module_path,
                &child_module_names,
            ),
        );
    }

    let macro_to_child = build_macro_to_child(&child_infos);
    let reexport_to_child = build_reexport_to_child(items, current_module_path, &child_infos);
    let mut initial_refs = entry_refs.clone();
    for item in items {
        if matches!(item, syn::Item::Mod(_) | syn::Item::Use(_)) {
            continue;
        }
        initial_refs.extend(collect_ref_names_from_tokens(
            &item.to_token_stream().to_string(),
            current_module_path,
        ));
    }

    let mut required_children = HashSet::new();
    let mut child_entry_refs = HashMap::<String, HashSet<String>>::new();
    let mut stack = Vec::<String>::new();
    for name in initial_refs {
        if let Some(child) =
            resolve_ref_to_child(&name, &child_module_names, &reexport_to_child, &macro_to_child)
        {
            child_entry_refs
                .entry(child.clone())
                .or_default()
                .insert(name);
            stack.push(child);
        }
    }

    while let Some(child) = stack.pop() {
        if !required_children.insert(child.clone()) {
            continue;
        }
        let Some(info) = child_infos.get(&child) else {
            continue;
        };
        for (next_child, refs) in &info.child_entry_refs {
            if next_child == &child {
                continue;
            }
            child_entry_refs
                .entry(next_child.clone())
                .or_default()
                .extend(refs.iter().cloned());
            if !required_children.contains(next_child) {
                stack.push(next_child.clone());
            }
        }
        for dep in &info.deps {
            if let Some(next_child) = resolve_ref_to_child(
                dep,
                &child_module_names,
                &reexport_to_child,
                &macro_to_child,
            ) {
                // Skip self-referential deps: when a module's internal $crate::
                // references resolve back to itself, they are not external
                // requirements and should not become entry refs for that module.
                if next_child == child {
                    continue;
                }
                if !info.child_entry_refs.contains_key(&next_child) {
                    child_entry_refs
                        .entry(next_child.clone())
                        .or_default()
                        .insert(dep.clone());
                }
                if !required_children.contains(&next_child) {
                    stack.push(next_child);
                }
            }
        }
    }

    ModulePrunePlan {
        required_children,
        child_entry_refs,
    }
}

fn prune_items_with_plan(
    current_module_path: &[String],
    items: &mut Vec<syn::Item>,
    required_children: &HashSet<String>,
) {
    let child_module_names: HashSet<String> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(item_mod) = item {
                Some(item_mod.ident.to_string())
            } else {
                None
            }
        })
        .collect();
    items.retain(|item| match item {
        syn::Item::Mod(item_mod) => required_children.contains(&item_mod.ident.to_string()),
        syn::Item::Use(_) => should_keep_use_item(
            item,
            current_module_path,
            &child_module_names,
            required_children,
        ),
        _ => true,
    });
}

fn should_keep_use_item(
    item: &syn::Item,
    current_module_path: &[String],
    child_module_names: &HashSet<String>,
    required_children: &HashSet<String>,
) -> bool {
    let referenced_children = collect_child_module_refs_from_item(
        item,
        current_module_path,
        child_module_names,
    );
    referenced_children.is_empty()
        || referenced_children
            .iter()
            .all(|name| required_children.contains(name))
}

fn resolve_ref_to_child(
    name: &str,
    child_module_names: &HashSet<String>,
    reexport_to_child: &HashMap<String, Option<String>>,
    macro_to_child: &HashMap<String, Option<String>>,
) -> Option<String> {
    if child_module_names.contains(name) {
        return Some(name.to_string());
    }
    if let Some(Some(child)) = reexport_to_child.get(name) {
        return Some(child.clone());
    }
    if let Some(Some(child)) = macro_to_child.get(name) {
        return Some(child.clone());
    }
    None
}

fn build_macro_to_child(
    child_infos: &HashMap<String, ModuleInfo>,
) -> HashMap<String, Option<String>> {
    let mut macro_to_child = HashMap::<String, Option<String>>::new();
    for (child, info) in child_infos {
        for macro_name in &info.exported_macros {
            insert_unique_mapping(&mut macro_to_child, macro_name.clone(), child.clone());
        }
    }
    macro_to_child
}

fn build_reexport_to_child(
    items: &[syn::Item],
    current_module_path: &[String],
    child_infos: &HashMap<String, ModuleInfo>,
) -> HashMap<String, Option<String>> {
    let mut reexport_to_child = HashMap::<String, Option<String>>::new();
    for item in items {
        let syn::Item::Use(item_use) = item else {
            continue;
        };
        for (name, child) in collect_reexported_names_from_use_item(
            item_use,
            current_module_path,
            child_infos,
        ) {
            insert_unique_mapping(&mut reexport_to_child, name, child);
        }
    }
    reexport_to_child
}

fn insert_unique_mapping(
    map: &mut HashMap<String, Option<String>>,
    name: String,
    child: String,
) {
    match map.get_mut(&name) {
        Some(slot) => *slot = None,
        None => {
            map.insert(name, Some(child));
        }
    }
}

fn collect_reexported_names_from_use_item(
    item_use: &syn::ItemUse,
    current_module_path: &[String],
    child_infos: &HashMap<String, ModuleInfo>,
) -> Vec<(String, String)> {
    let tokens = item_use.to_token_stream().to_string();
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let Some((child_name, start)) =
        parse_reexport_child_path(&parts, current_module_path, child_infos)
    else {
        return Vec::new();
    };
    let Some(info) = child_infos.get(child_name) else {
        return Vec::new();
    };
    collect_use_binding_names(&parts[start..], info)
        .into_iter()
        .map(|name| (name, child_name.to_string()))
        .collect()
}

fn parse_reexport_child_path<'a>(
    parts: &'a [&'a str],
    current_module_path: &[String],
    child_infos: &'a HashMap<String, ModuleInfo>,
) -> Option<(&'a str, usize)> {
    if parts.len() >= 4 && parts[0] == "pub" && parts[1] == "use" {
        return parse_reexport_child_path(&parts[2..], current_module_path, child_infos)
            .map(|(child, idx)| (child, idx + 2));
    }
    if parts.len() >= 2 && parts[0] == "use" {
        return parse_reexport_child_path(&parts[1..], current_module_path, child_infos)
            .map(|(child, idx)| (child, idx + 1));
    }
    if parts.len() >= 4 && parts[0] == "self" && parts[1] == "::" {
        let child = parts[2];
        if child_infos.contains_key(child) && parts[3] == "::" {
            return Some((child, 4));
        }
    }
    if parts.len() < 2 || parts[0] != "crate" || parts[1] != "::" {
        return None;
    }
    let mut idx = 2;
    for segment in current_module_path {
        if parts.get(idx).copied()? != segment.as_str() || parts.get(idx + 1) != Some(&"::") {
            return None;
        }
        idx += 2;
    }
    let child = *parts.get(idx)?;
    if child_infos.contains_key(child) && parts.get(idx + 1) == Some(&"::") {
        return Some((child, idx + 2));
    }
    None
}

fn collect_use_binding_names(parts: &[&str], info: &ModuleInfo) -> Vec<String> {
    if parts.is_empty() {
        return Vec::new();
    }
    match parts[0] {
        "*" => info.public_names.iter().cloned().collect(),
        "{" => {
            let mut names = Vec::new();
            let mut depth = 0usize;
            let mut i = 0usize;
            while i < parts.len() {
                match parts[i] {
                    "{" => depth += 1,
                    "}" => {
                        if depth == 1 {
                            break;
                        }
                        depth = depth.saturating_sub(1);
                    }
                    token if depth == 1 && normalized_ident_token(token).is_some() => {
                        if parts.get(i + 1) == Some(&"as") {
                            if let Some(alias) = parts.get(i + 2) {
                                if let Some(alias) = normalized_ident_token(alias) {
                                    names.push(alias);
                                }
                            }
                        } else if parts.get(i + 1) != Some(&"::") {
                            names.push(normalized_ident_token(token).unwrap());
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            names
        }
        token if normalized_ident_token(token).is_some() => {
            if parts.get(1) == Some(&"as") {
                parts
                    .get(2)
                    .and_then(|alias| normalized_ident_token(alias))
                    .map(|alias| vec![alias])
                    .unwrap_or_default()
            } else {
                vec![normalized_ident_token(token).unwrap()]
            }
        }
        _ => Vec::new(),
    }
}

fn collect_direct_module_info(
    base_path: &Path,
    parent_name: &str,
    item_mod: &syn::ItemMod,
    current_module_path: &[String],
    sibling_module_names: &HashSet<String>,
) -> ModuleInfo {
    let (module_base_path, items) = load_module_items(base_path, parent_name, item_mod);
    let mut info = ModuleInfo::default();
    info.public_names.extend(collect_public_item_names(&items));
    collect_module_info_from_items(
        &module_base_path,
        &item_mod.ident.to_string(),
        &items,
        current_module_path,
        sibling_module_names,
        &mut info,
    );
    info
}

fn collect_public_item_names(items: &[syn::Item]) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in items {
        let maybe_name = match item {
            syn::Item::Const(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Enum(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Fn(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.sig.ident.to_string())
            }
            syn::Item::Mod(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Static(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Struct(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Trait(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::TraitAlias(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Type(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Union(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                Some(item.ident.to_string())
            }
            syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => None,
            _ => None,
        };
        if let Some(name) = maybe_name {
            names.insert(name);
        }
        if let syn::Item::Use(item_use) = item {
            if matches!(item_use.vis, syn::Visibility::Public(_)) {
                collect_use_tree_binding_names(&item_use.tree, &mut names);
            }
        }
    }
    names
}

fn build_child_infos(
    items: &[syn::Item],
    current_module_path: &[String],
) -> HashMap<String, ModuleInfo> {
    let child_module_names: HashSet<String> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(item_mod) = item {
                Some(item_mod.ident.to_string())
            } else {
                None
            }
        })
        .collect();
    let mut child_infos = HashMap::<String, ModuleInfo>::new();
    for item in items {
        let syn::Item::Mod(item_mod) = item else {
            continue;
        };
        child_infos.insert(
            item_mod.ident.to_string(),
            collect_inline_module_info(item_mod, current_module_path, &child_module_names),
        );
    }
    child_infos
}

fn collect_inline_module_info(
    item_mod: &syn::ItemMod,
    current_module_path: &[String],
    sibling_module_names: &HashSet<String>,
) -> ModuleInfo {
    let Some((_, items)) = &item_mod.content else {
        return ModuleInfo::default();
    };
    let mut info = ModuleInfo::default();
    info.public_names.extend(collect_public_item_names(items));
    collect_module_info_from_items(
        Path::new("."),
        &item_mod.ident.to_string(),
        items,
        current_module_path,
        sibling_module_names,
        &mut info,
    );
    info
}

fn collect_item_defined_names(
    item: &syn::Item,
    current_module_path: &[String],
    child_infos: &HashMap<String, ModuleInfo>,
) -> HashSet<String> {
    match item {
        syn::Item::Const(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Enum(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::ExternCrate(item) => {
            let ident = item
                .rename
                .as_ref()
                .map(|rename| rename.1.to_string())
                .unwrap_or_else(|| item.ident.to_string());
            HashSet::from([ident])
        }
        syn::Item::Fn(item) => HashSet::from([item.sig.ident.to_string()]),
        syn::Item::ForeignMod(item) => collect_foreign_item_names(&item.items),
        syn::Item::Impl(item) => collect_impl_item_anchor_names(item),
        syn::Item::Macro(item) => item
            .ident
            .as_ref()
            .map(|ident| HashSet::from([ident.to_string()]))
            .unwrap_or_default(),
        syn::Item::Mod(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Static(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Struct(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Trait(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::TraitAlias(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Type(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Union(item) => HashSet::from([item.ident.to_string()]),
        syn::Item::Use(item_use) => collect_use_item_defined_names(item_use, current_module_path, child_infos),
        syn::Item::Verbatim(_) => HashSet::new(),
        _ => HashSet::new(),
    }
}

fn collect_foreign_item_names(items: &[syn::ForeignItem]) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in items {
        match item {
            syn::ForeignItem::Fn(item) => {
                names.insert(item.sig.ident.to_string());
            }
            syn::ForeignItem::Static(item) => {
                names.insert(item.ident.to_string());
            }
            syn::ForeignItem::Type(item) => {
                names.insert(item.ident.to_string());
            }
            _ => {}
        }
    }
    names
}

fn collect_impl_item_anchor_names(item: &syn::ItemImpl) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(name) = collect_type_path_ident(&item.self_ty) {
        names.insert(name);
    }
    names
}

fn collect_type_path_ident(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Group(item) => collect_type_path_ident(&item.elem),
        syn::Type::Paren(item) => collect_type_path_ident(&item.elem),
        syn::Type::Path(item) => item.path.segments.last().map(|segment| segment.ident.to_string()),
        syn::Type::Reference(item) => collect_type_path_ident(&item.elem),
        _ => None,
    }
}

fn collect_use_item_defined_names(
    item_use: &syn::ItemUse,
    current_module_path: &[String],
    child_infos: &HashMap<String, ModuleInfo>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_use_tree_binding_names(&item_use.tree, &mut names);
    for (name, _) in collect_reexported_names_from_use_item(item_use, current_module_path, child_infos) {
        names.insert(name);
    }
    names
}

fn collect_use_tree_binding_names(tree: &syn::UseTree, names: &mut HashSet<String>) {
    match tree {
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree_binding_names(item, names);
            }
        }
        syn::UseTree::Name(item) => {
            names.insert(item.ident.to_string());
        }
        syn::UseTree::Path(item) => collect_use_tree_binding_names(&item.tree, names),
        syn::UseTree::Rename(item) => {
            names.insert(item.rename.to_string());
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn collect_item_deps(
    item: &syn::Item,
    current_module_path: &[String],
    candidate_names: &HashSet<String>,
    macro_item_names: &HashSet<String>,
) -> HashSet<String> {
    let tokens = item.to_token_stream().to_string();
    let mut deps = collect_ref_names_from_tokens(&tokens, current_module_path);
    if !matches!(item, syn::Item::Mod(_)) {
        deps.extend(collect_candidate_name_mentions(
            &tokens,
            candidate_names,
            macro_item_names,
        ));
    }
    deps
}

fn collect_candidate_name_mentions(
    tokens: &str,
    candidate_names: &HashSet<String>,
    macro_names: &HashSet<String>,
) -> HashSet<String> {
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let mut mentions = HashSet::new();
    for (idx, token) in parts.iter().enumerate() {
        let Some(token) = normalized_ident_token(token) else {
            continue;
        };
        if !candidate_names.contains(&token) {
            continue;
        }
        if macro_names.contains(&token) && parts.get(idx + 1) != Some(&"!") {
            continue;
        }
        mentions.insert(token);
    }
    mentions
}

fn collect_child_entry_refs_from_item(
    item: &syn::Item,
    current_module_path: &[String],
    child_module_names: &HashSet<String>,
) -> HashMap<String, HashSet<String>> {
    collect_child_entry_refs_from_tokens(
        &item.to_token_stream().to_string(),
        current_module_path,
        child_module_names,
    )
}

fn collect_child_module_refs_from_item(
    item: &syn::Item,
    current_module_path: &[String],
    child_module_names: &HashSet<String>,
) -> HashSet<String> {
    collect_child_module_refs_from_tokens(
        &item.to_token_stream().to_string(),
        current_module_path,
        child_module_names,
    )
}

fn collect_child_module_refs_from_tokens(
    tokens: &str,
    current_module_path: &[String],
    child_module_names: &HashSet<String>,
) -> HashSet<String> {
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let mut refs = HashSet::<String>::new();
    let mut i = 0usize;
    while i < parts.len() {
        if parts[i] == "crate" && parts.get(i + 1) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 2, current_module_path) {
                extend_child_module_refs(&mut refs, &parts, start, child_module_names);
            }
        }
        if parts[i] == "$" && parts.get(i + 1) == Some(&"crate") && parts.get(i + 2) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 3, current_module_path) {
                extend_child_module_refs(&mut refs, &parts, start, child_module_names);
            }
        }
        if parts[i] == "self" || parts[i] == "super" {
            let mut start = i;
            while matches!(parts.get(start), Some(&"self") | Some(&"super"))
                && parts.get(start + 1) == Some(&"::")
            {
                start += 2;
            }
            extend_child_module_refs(&mut refs, &parts, start, child_module_names);
        }
        if child_module_names.contains(parts[i]) && parts.get(i + 1) == Some(&"::") {
            refs.insert(parts[i].to_string());
        }
        i += 1;
    }
    refs
}

fn extend_child_module_refs(
    refs: &mut HashSet<String>,
    parts: &[&str],
    start: usize,
    child_module_names: &HashSet<String>,
) {
    let Some(child) = parts.get(start).copied() else {
        return;
    };
    if child_module_names.contains(child) && parts.get(start + 1) == Some(&"::") {
        refs.insert(child.to_string());
    }
}

fn collect_child_entry_refs_from_tokens(
    tokens: &str,
    current_module_path: &[String],
    child_module_names: &HashSet<String>,
) -> HashMap<String, HashSet<String>> {
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let mut refs = HashMap::<String, HashSet<String>>::new();
    let mut i = 0usize;
    while i < parts.len() {
        if parts[i] == "crate" && parts.get(i + 1) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 2, current_module_path) {
                extend_child_entry_refs(&mut refs, &parts, start, child_module_names);
            }
        }
        if parts[i] == "$" && parts.get(i + 1) == Some(&"crate") && parts.get(i + 2) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 3, current_module_path) {
                extend_child_entry_refs(&mut refs, &parts, start, child_module_names);
            }
        }
        if parts[i] == "self" || parts[i] == "super" {
            let mut start = i;
            while matches!(parts.get(start), Some(&"self") | Some(&"super"))
                && parts.get(start + 1) == Some(&"::")
            {
                start += 2;
            }
            extend_child_entry_refs(&mut refs, &parts, start, child_module_names);
        }
        if child_module_names.contains(parts[i]) && parts.get(i + 1) == Some(&"::") {
            extend_child_entry_refs(&mut refs, &parts, i, child_module_names);
        }
        i += 1;
    }
    refs
}

fn extend_child_entry_refs(
    refs: &mut HashMap<String, HashSet<String>>,
    parts: &[&str],
    start: usize,
    child_module_names: &HashSet<String>,
) {
    let Some(child) = parts.get(start).copied() else {
        return;
    };
    if !child_module_names.contains(child) || parts.get(start + 1) != Some(&"::") {
        return;
    }
    let names = collect_names_after_path(parts, start + 2);
    if names.is_empty() {
        return;
    }
    refs.entry(child.to_string()).or_default().extend(names);
}

fn collect_use_binding_to_child(
    item: &syn::Item,
    current_module_path: &[String],
    child_infos: &HashMap<String, ModuleInfo>,
) -> HashMap<String, String> {
    let syn::Item::Use(item_use) = item else {
        return HashMap::new();
    };
    collect_reexported_names_from_use_item(item_use, current_module_path, child_infos)
        .into_iter()
        .collect()
}

fn should_always_keep_item(item: &syn::Item, crate_name: &str, current_module_path: &[String]) -> bool {
    match item {
        syn::Item::ExternCrate(item) => {
            if !current_module_path.is_empty() {
                return false;
            }
            item.ident == "self"
                && item
                    .rename
                    .as_ref()
                    .map(|rename| rename.1 == crate_name)
                    .unwrap_or(false)
        }
        syn::Item::Use(item_use) => use_tree_contains_underscore_binding(&item_use.tree),
        syn::Item::Macro(item_macro) => item_macro.ident.is_none(),
        _ => false,
    }
}

fn use_tree_contains_underscore_binding(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Group(group) => group.items.iter().any(use_tree_contains_underscore_binding),
        syn::UseTree::Name(_) => false,
        syn::UseTree::Path(item) => use_tree_contains_underscore_binding(&item.tree),
        syn::UseTree::Rename(item) => item.rename == "_",
        syn::UseTree::Glob(_) => false,
    }
}

fn trim_use_item(item_use: &mut syn::ItemUse, allowed_names: &HashSet<String>) -> bool {
    trim_use_tree(&mut item_use.tree, allowed_names)
}

fn trim_use_tree(tree: &mut syn::UseTree, allowed_names: &HashSet<String>) -> bool {
    match tree {
        syn::UseTree::Group(group) => {
            let mut trimmed = Punctuated::new();
            for mut item in mem::take(&mut group.items)
                .into_pairs()
                .map(|pair| pair.into_value())
            {
                if trim_use_tree(&mut item, allowed_names) {
                    trimmed.push(item);
                }
            }
            group.items = trimmed;
            !group.items.is_empty()
        }
        syn::UseTree::Name(item) => allowed_names.contains(&item.ident.to_string()),
        syn::UseTree::Path(item) => trim_use_tree(&mut item.tree, allowed_names),
        syn::UseTree::Rename(item) => {
            item.rename == "_" || allowed_names.contains(&item.rename.to_string())
        }
        syn::UseTree::Glob(_) => true,
    }
}

fn collect_named_macro_names_from_items(items: &[syn::Item]) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in items {
        match item {
            syn::Item::Macro(item_macro) => {
                if let Some(ident) = &item_macro.ident {
                    names.insert(ident.to_string());
                }
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, child_items)) = &item_mod.content {
                    names.extend(collect_named_macro_names_from_items(child_items));
                }
            }
            _ => {}
        }
    }
    names
}

fn collect_named_macro_tokens(items: &[syn::Item], target: &str, tokens: &mut Vec<String>) {
    for item in items {
        match item {
            syn::Item::Macro(item_macro) => {
                if item_macro
                    .ident
                    .as_ref()
                    .map(|ident| ident == target)
                    .unwrap_or(false)
                {
                    tokens.push(item_macro.to_token_stream().to_string());
                }
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, child_items)) = &item_mod.content {
                    collect_named_macro_tokens(child_items, target, tokens);
                }
            }
            _ => {}
        }
    }
}

fn collect_macro_invocation_names(tokens: &str, macro_names: &HashSet<String>) -> HashSet<String> {
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let mut names = HashSet::new();
    for idx in 0..parts.len().saturating_sub(1) {
        if macro_names.contains(parts[idx]) && parts.get(idx + 1) == Some(&"!") {
            names.insert(parts[idx].to_string());
        }
    }
    names
}

fn retain_reachable_named_macros(items: &mut Vec<syn::Item>, reachable: &HashSet<String>) {
    items.retain(|item| match item {
        syn::Item::Macro(item_macro) => item_macro
            .ident
            .as_ref()
            .map(|ident| reachable.contains(&ident.to_string()))
            .unwrap_or(true),
        _ => true,
    });
    for item in items {
        if let syn::Item::Mod(item_mod) = item {
            if let Some((_, child_items)) = &mut item_mod.content {
                retain_reachable_named_macros(child_items, reachable);
            }
        }
    }
}

fn prune_unused_impl_methods_in_scope(
    items: &mut [syn::Item],
    keep: &[bool],
    current_module_path: &[String],
    binary_source: &str,
    ancestor_scope_tokens: &str,
) {
    let scope_tokens = items
        .iter()
        .enumerate()
        .filter(|(idx, item)| {
            // Keep non-impl items and trait impls (which aren't pruned but may
            // reference inherent methods, e.g. Default::default calling new_with_seed).
            // Only exclude inherent impls since those are the targets of pruning.
            keep[*idx]
                && !matches!(item, syn::Item::Impl(item_impl) if item_impl.trait_.is_none())
        })
        .map(|(_, item)| item.to_token_stream().to_string())
        .collect::<Vec<_>>();

    let mut impl_groups = HashMap::<String, Vec<usize>>::new();
    for (idx, item) in items.iter().enumerate() {
        let syn::Item::Impl(item_impl) = item else {
            continue;
        };
        if !keep[idx] || item_impl.trait_.is_some() {
            continue;
        }
        let Some(self_ty) = collect_type_path_ident(&item_impl.self_ty) else {
            continue;
        };
        impl_groups.entry(self_ty).or_default().push(idx);
    }

    for indices in impl_groups.values() {
        let candidate_names = indices
            .iter()
            .flat_map(|idx| {
                let syn::Item::Impl(item_impl) = &items[*idx] else {
                    return HashSet::new().into_iter();
                };
                collect_impl_item_defined_names(&item_impl.items).into_iter()
            })
            .collect::<HashSet<_>>();
        if candidate_names.is_empty() {
            continue;
        }

        let mut pending = collect_candidate_name_mentions(
            binary_source,
            &candidate_names,
            &HashSet::new(),
        )
        .into_iter()
        .collect::<Vec<_>>();
        for tokens in &scope_tokens {
            pending.extend(
                collect_candidate_name_mentions(tokens, &candidate_names, &HashSet::new())
                    .into_iter(),
            );
        }
        // Also scan ancestor/sibling scope tokens so that cross-module method
        // references (e.g. random_generator calling rng.rand64()) are visible.
        if module_path_is_tools(current_module_path) {
            pending.extend(
                collect_candidate_name_mentions(ancestor_scope_tokens, &candidate_names, &HashSet::new())
                    .into_iter(),
            );
        }

        let mut reachable = HashSet::<String>::new();
        while let Some(name) = pending.pop() {
            if !reachable.insert(name.clone()) {
                continue;
            }
            for idx in indices {
                let syn::Item::Impl(item_impl) = &items[*idx] else {
                    continue;
                };
                for impl_item in &item_impl.items {
                    let defined_names = collect_impl_item_defined_names(std::slice::from_ref(impl_item));
                    if !defined_names.contains(&name) {
                        continue;
                    }
                    let tokens = impl_item.to_token_stream().to_string();
                    let deps = collect_candidate_name_mentions(
                        &tokens,
                        &candidate_names,
                        &HashSet::new(),
                    )
                    .into_iter()
                    .filter(|dep| !defined_names.contains(dep))
                    .collect::<Vec<_>>();
                    pending.extend(deps);
                }
            }
        }

        for idx in indices {
            let syn::Item::Impl(item_impl) = &mut items[*idx] else {
                continue;
            };
            item_impl.items.retain(|impl_item| {
                let defined_names =
                    collect_impl_item_defined_names(std::slice::from_ref(impl_item));
                defined_names.is_empty()
                    || defined_names
                        .iter()
                        .any(|name| reachable.contains(name))
            });
        }
    }
}

fn collect_impl_item_defined_names(items: &[syn::ImplItem]) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in items {
        match item {
            syn::ImplItem::Const(item) => {
                names.insert(item.ident.to_string());
            }
            syn::ImplItem::Method(item) => {
                names.insert(item.sig.ident.to_string());
            }
            syn::ImplItem::Type(item) => {
                names.insert(item.ident.to_string());
            }
            _ => {}
        }
    }
    names
}

fn load_module_items(
    base_path: &Path,
    parent_name: &str,
    item_mod: &syn::ItemMod,
) -> (PathBuf, Vec<syn::Item>) {
    if let Some((_, items)) = &item_mod.content {
        return (base_path.to_path_buf(), items.clone());
    }

    let name = item_mod.ident.to_string();
    let (module_base_path, code) = read_module_code(base_path, parent_name, &name);
    let file = syn::parse_file(&code).expect("failed to parse module file for dependency analysis");
    (module_base_path, file.items)
}

fn collect_module_info_from_items(
    base_path: &Path,
    parent_name: &str,
    items: &[syn::Item],
    current_module_path: &[String],
    sibling_module_names: &HashSet<String>,
    info: &mut ModuleInfo,
) {
    for item in items {
        let tokens = item.to_token_stream().to_string();
        info.deps
            .extend(collect_ref_names_from_tokens(&tokens, current_module_path));
        for (child, refs) in collect_child_entry_refs_from_tokens(
            &tokens,
            current_module_path,
            sibling_module_names,
        ) {
            info.child_entry_refs
                .entry(child)
                .or_default()
                .extend(refs);
        }
        if let syn::Item::Macro(item_macro) = item {
            if is_macro_export(item_macro) {
                if let Some(ident) = &item_macro.ident {
                    info.exported_macros.insert(ident.to_string());
                }
            }
        }
        if let syn::Item::Mod(item_mod) = item {
            let (child_base_path, child_items) = load_module_items(base_path, parent_name, item_mod);
            collect_module_info_from_items(
                &child_base_path,
                &item_mod.ident.to_string(),
                &child_items,
                current_module_path,
                sibling_module_names,
                info,
            );
        }
    }
}

fn read_module_code(base_path: &Path, parent_name: &str, name: &str) -> (PathBuf, String) {
    let new_style_path = base_path.join(parent_name);
    let other_base_path = base_path.join(name);
    vec![
        (base_path.to_path_buf(), format!("{}.rs", name)),
        (new_style_path, format!("{}.rs", name)),
        (other_base_path, String::from("mod.rs")),
    ]
    .into_iter()
    .find_map(|(candidate_base, file_name)| {
        read_file(&candidate_base.join(file_name)).map(|code| (candidate_base, code))
    })
    .expect("module source not found during dependency analysis")
}

fn collect_ref_names_from_tokens(tokens: &str, current_module_path: &[String]) -> HashSet<String> {
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let mut refs = HashSet::new();
    let mut i = 0;
    while i < parts.len() {
        if parts[i] == "crate" && parts.get(i + 1) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 2, current_module_path) {
                refs.extend(collect_names_after_path(&parts, start));
            } else {
                refs.extend(collect_names_after_path(&parts, i + 2));
            }
        }
        if parts[i] == "$" && parts.get(i + 1) == Some(&"crate") && parts.get(i + 2) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 3, current_module_path) {
                refs.extend(collect_names_after_path(&parts, start));
            } else {
                refs.extend(collect_names_after_path(&parts, i + 3));
            }
        }
        if parts[i] == "self" || parts[i] == "super" {
            let mut start = i;
            while matches!(parts.get(start), Some(&"self") | Some(&"super"))
                && parts.get(start + 1) == Some(&"::")
            {
                start += 2;
            }
            refs.extend(collect_names_after_path(&parts, start));
        }
        i += 1;
    }
    refs
}

fn collect_named_crate_ref_names_from_tokens(
    tokens: &str,
    crate_name: &str,
    current_module_path: &[String],
) -> HashSet<String> {
    let parts = tokens.split_whitespace().collect::<Vec<_>>();
    let mut refs = HashSet::new();
    let mut i = 0;
    while i < parts.len() {
        if parts[i] == crate_name && parts.get(i + 1) == Some(&"::") {
            if let Some(start) = consume_prefixed_module_path(&parts, i + 2, current_module_path) {
                refs.extend(collect_names_after_path(&parts, start));
            } else {
                refs.extend(collect_names_after_path(&parts, i + 2));
            }
        }
        i += 1;
    }
    refs
}

fn consume_prefixed_module_path(
    parts: &[&str],
    mut idx: usize,
    current_module_path: &[String],
) -> Option<usize> {
    for segment in current_module_path {
        if parts.get(idx).copied()? != segment.as_str() || parts.get(idx + 1) != Some(&"::") {
            return None;
        }
        idx += 2;
    }
    Some(idx)
}

fn collect_names_after_path(parts: &[&str], start: usize) -> HashSet<String> {
    let mut names = HashSet::new();
    let Some(token) = parts.get(start).copied() else {
        return names;
    };
    if token == "{" {
        let mut depth = 0usize;
        let mut i = start;
        while i < parts.len() {
            match parts[i] {
                "{" => depth += 1,
                "}" => {
                    if depth == 1 {
                        break;
                    }
                    depth = depth.saturating_sub(1);
                }
                current if depth == 1 && normalized_ident_token(current).is_some() => {
                    if parts.get(i + 1) == Some(&"as") {
                        if let Some(alias) = parts.get(i + 2) {
                            if let Some(alias) = normalized_ident_token(alias) {
                                names.insert(alias);
                            }
                        }
                    } else if parts.get(i + 1) != Some(&"::") {
                        names.insert(normalized_ident_token(current).unwrap());
                    }
                }
                _ => {}
            }
            i += 1;
        }
        return names;
    }
    if let Some(token) = normalized_ident_token(token) {
        names.insert(token);
    }
    names
}

fn is_ident_token(token: &str) -> bool {
    token
        .chars()
        .next()
        .map(|ch| ch == '_' || ch.is_ascii_alphabetic())
        .unwrap_or(false)
}

fn normalized_ident_token(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| {
        matches!(ch, ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}')
    });
    if is_ident_token(token) {
        Some(token.to_string())
    } else {
        None
    }
}

fn module_path_is_tools(current_module_path: &[String]) -> bool {
    matches!(current_module_path.first(), Some(name) if name == "tools")
}

fn is_macro_export(item_macro: &syn::ItemMacro) -> bool {
    item_macro
        .attrs
        .iter()
        .any(|attr| attr.path.is_ident("macro_export"))
}

fn read_file(path: &Path) -> Option<String> {
    let mut buf = String::new();
    std::fs::File::open(path)
        .ok()?
        .read_to_string(&mut buf)
        .ok()?;
    Some(buf)
}

// Debug toolkits

fn debug_str_items(items: &[syn::Item]) -> String {
    // let x = 5i32;
    // let y = x.to_string();
    //HIGHLY TODO
    let mut builder = simple_string_builder::Builder::new();
    builder.append("len=");
    // builder.append(items.len());
    builder.append(items.len().to_string());
    builder.append(" ");
    // result += &*items.len().to_string();
    for it in items {
        builder.append(" / ");
        builder.append(debug_str_item(it));
    }
    builder.try_to_string().unwrap()
    // let mut result = String::new();

    // result += "len=";
}

fn debug_str_item(it: &syn::Item) -> String {
    let refstr: &str = match it {
        syn::Item::ExternCrate(_e) => {
            // eprintln!("{:?}", e); //TODO-> too hacky
            "extern_crate"
        },
        syn::Item::Use(_e) => {
            // eprintln!("{:?}", e); //TODO-> too hacky
            "use"
        },
        syn::Item::Fn(_e) => {
            // eprintln!("{:?}", e); //TODO-> too hacky
            "Fn"
        },
        syn::Item::Mod(e) => {
            e.ident.to_string();
            // eprintln!("{:?}", e); //TODO-> too hacky
            // return "Mod(";
            return format!("Mod ({})", e.ident.to_string());
        },
        _ => {
            // eprintln!("{:?}", it); //TODO-> too hacky
            "Others"
        }
    };
    String::from(refstr)
}

#[derive(PartialEq, Eq, Hash)]
pub enum BundlerConfig {
    RustEdition,
}


/*
The test cases below is also considered as documents and examples.
*/

#[cfg(test)]
mod expander_test {
    use std::collections::HashSet;
    use std::path::Path;
    use syn::{Expr, File, parse_str};
    use crate::{
        Expander, compute_required_direct_modules, ensure_trailing_newline,
        file_references_selected_crate, prune_unused_macro_arms,
        prune_unused_support_items, prune_unused_support_macros, render_support_code,
        rewrite_for_edition, strip_redundant_support, strip_tools_main_stub,
    };
    use syn::__private::ToTokens;
    use syn::visit_mut::VisitMut;



    #[test]
    fn test_create() {
        let mut expander = create_expander();
    }
    #[test]
    fn test_read_source_code() {
        let mut file = read_source_code();
    }

    fn create_expander() -> Expander<'static> {
        // TODO This path seems to be wrong
        let base_path: &Path = Path::new("tests/testdata/input/rust_codeforce_template")
            .parent()
            .expect("lib.src_path has no parent");
        let crate_name = "my_lib";
        let mut expander = crate::Expander::new(base_path, "", crate_name);
        expander
    }


    fn read_source_code () -> File {
        let src_path = "tests/testdata/input/rust_codeforce_template/src/main.rs";
        let syntax_tree =
            crate::read_file(Path::new(src_path)).expect("failed to read binary target source");
        let mut file = syn::parse_file(&syntax_tree).expect("failed to parse binary target source");
        file
    }

    #[test]
    fn test_rewrite_crate_qualified_path_only() {
        let mut expander = create_expander();
        let mut expr: Expr = parse_str("my_lib::tools::Scanner::new(&buf)").unwrap();
        expander.visit_expr_mut(&mut expr);
        assert_eq!(expr.into_token_stream().to_string(), "tools :: Scanner :: new (& buf)");
    }

    #[test]
    fn test_keep_single_segment_local_binding_named_like_crate() {
        let mut expander = create_expander();
        let mut expr: Expr = parse_str("my_lib.signed_pow(exp_p)").unwrap();
        expander.visit_expr_mut(&mut expr);
        assert_eq!(expr.into_token_stream().to_string(), "my_lib . signed_pow (exp_p)");
    }

    #[test]
    fn test_strip_tools_main_stub() {
        let mut file: File = parse_str(
            r#"
            pub fn solve() {
                crate::prepare!();
            }
            crate::main!();
            mod main_macros {
                #[macro_export]
                macro_rules! main { () => {} }
            }
            "#,
        )
        .unwrap();
        strip_tools_main_stub(&mut file);
        let code = file.into_token_stream().to_string();
        assert!(!code.contains("pub fn solve"));
        assert!(!code.contains("crate :: main !"));
        assert!(code.contains("mod main_macros"));
    }

    #[test]
    fn test_ensure_trailing_newline_preserves_multiline_binary_source() {
        let source = "pub fn solve() {\n    cp::prepare!();\n}\n\ncp::main!();".to_string();
        let code = ensure_trailing_newline(source);
        assert_eq!(code, "pub fn solve() {\n    cp::prepare!();\n}\n\ncp::main!();\n");
    }

    #[test]
    fn test_strip_redundant_support_removes_docs_and_tests() {
        let mut file: File = parse_str(
            r#"
            #[doc = "top"]
            pub mod tools {
                #[doc = "kept item without docs"]
                pub fn keep() {}

                #[cfg(test)]
                mod tests {
                    #[test]
                    fn helper() {}
                }

                impl X {
                    #[doc = "internal doc"]
                    fn keep_impl() {}

                    #[cfg(test)]
                    fn drop_impl() {}
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        let code = file.into_token_stream().to_string();
        assert!(!code.contains("doc"));
        assert!(!code.contains("mod tests"));
        assert!(!code.contains("drop_impl"));
        assert!(code.contains("keep"));
        assert!(code.contains("keep_impl"));
    }

    #[test]
    fn test_render_support_code_strips_macro_body_doc_and_allow_attrs() {
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                mod main_macros {
                    #[macro_export]
                    macro_rules! prepare {
                        () => {
                            #[allow(unused_macros)]
                            #[doc = "helper"]
                            macro_rules! pp { () => {} }
                        }
                    }
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        let code = render_support_code(file, "cp", false);
        assert!(!code.contains("# [doc"));
        assert!(!code.contains("# [allow"));
        assert!(code.contains("macro_rules ! pp"));
    }

    #[test]
    fn test_render_support_code_compacts_whitespace_without_breaking_parse() {
        let file: File = parse_str(
            r#"
            pub mod tools {
                #[inline]
                pub fn compact() -> usize {
                    1usize.leading_zeros() as usize
                }

                #[macro_export]
                macro_rules! m {
                    () => {
                        $crate::tools::compact()
                    };
                }
            }
            "#,
        )
        .unwrap();
        let code = render_support_code(file, "cp", true);
        assert!(code.contains("extern crate self as cp;"));
        assert!(code.contains("#[inline]"));
        assert!(code.contains("1usize .leading_zeros"));
        syn::parse_file(&code).expect("compacted support should still parse");
    }

    #[test]
    fn test_rewrite_for_edition_2024_marks_extern_c_unsafe() {
        let code = rewrite_for_edition(
            "extern \"C\" { fn puts(_: *const i8); }\nunsafe extern \"C\" { fn abort(); }\n"
                .to_string(),
            Some("2024"),
        );
        assert_eq!(
            code,
            "unsafe extern \"C\" { fn puts(_: *const i8); }\nunsafe extern \"C\" { fn abort(); }\n"
        );
    }

    #[test]
    fn test_compute_required_direct_modules_prunes_nested_children_recursively() {
        let file: File = parse_str(
            r#"
            pub use self::scanner::*;
            mod scanner {
                pub struct Scanner;
            }
            mod random_generator {
                pub struct RandomSpec;
            }
            mod main {
                mod main_macros {
                    #[macro_export]
                    macro_rules! prepare {
                        () => {
                            let _ = $crate::tools::Scanner;
                        };
                    }
                }
            }
            "#,
        )
        .unwrap();
        let entry_refs = HashSet::from([String::from("prepare")]);
        let plan = compute_required_direct_modules(
            Path::new("."),
            "tools",
            &[String::from("tools")],
            &file.items,
            &entry_refs,
        );
        assert!(plan.required_children.contains("main"));
        assert!(plan.required_children.contains("scanner"));
        assert!(!plan.required_children.contains("random_generator"));
        assert_eq!(
            plan.child_entry_refs.get("main"),
            Some(&HashSet::from([String::from("prepare")]))
        );
    }

    #[test]
    fn test_render_support_code_adds_crate_alias_when_needed() {
        let file: File = parse_str("pub mod tools { pub fn helper() {} }").unwrap();
        let code = render_support_code(file, "cp", true);
        assert!(code.starts_with("extern crate self as cp ;"));
    }

    #[test]
    fn test_prune_unused_support_items_prunes_dead_items_but_keeps_item_macros() {
        let binary_source = parse_str::<File>(
            r#"
            pub fn solve() {
                cp::prepare!();
                let _ = cp::tools::FastOutput::stdout();
                sc!(n: u8);
            }
            cp::main!();
            "#,
        )
        .unwrap()
        .into_token_stream()
        .to_string();
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                pub use self::fastio::{FastInput, FastOutput};
                pub use self::scanner::*;

                mod fastio {
                    use std::{fs::File, io::{Read, Write}, os::fd::FromRawFd};

                    pub struct FastInput;
                    impl FastInput {
                        pub fn stdin() {
                            let _ = File::from_raw_fd(0);
                        }
                    }

                    pub struct FastOutput;
                    impl FastOutput {
                        pub fn stdout() -> Self { Self }
                    }
                }

                mod main {
                    mod main_macros {
                        #[macro_export]
                        macro_rules! prepare {
                            () => {
                                let _ = $crate::tools::Scanner::new("");
                                use $crate::tools::{Byte1, Usize1};
                            };
                        }
                        #[macro_export]
                        macro_rules! main { () => {} }
                    }
                }

                mod scanner {
                    pub trait IterScan: Sized {
                        type Output;
                        fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output>;
                    }

                    pub struct Scanner<'a> { s: &'a str }
                    impl<'a> Scanner<'a> {
                        pub fn new(s: &'a str) -> Self { Self { s } }
                    }

                    macro_rules! impl_iter_scan {
                        ($($T:ty)*) => {
                            $(impl IterScan for $T {
                                type Output = $T;
                                fn scan<'a, I: Iterator<Item = &'a str>>(_: &mut I) -> Option<Self::Output> { None }
                            })*
                        };
                    }
                    impl_iter_scan!(u8 usize);

                    pub fn read_stdin_all_unchecked() -> String { String::new() }

                    pub enum Usize1 {}
                    impl IterScan for Usize1 {
                        type Output = usize;
                        fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output> {
                            <usize as IterScan>::scan(iter)
                        }
                    }

                    pub enum Byte1 {}
                    impl IterScan for Byte1 {
                        type Output = u8;
                        fn scan<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<Self::Output> {
                            <u8 as IterScan>::scan(iter)
                        }
                    }

                    pub struct Dead;
                    impl Dead {
                        pub fn noop() {}
                    }
                }

                mod dead {
                    pub struct Never;
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        prune_unused_support_items(&mut file, "cp", &binary_source);
        let code = render_support_code(file, "cp", true);
        assert!(code.contains("impl_iter_scan !"));
        assert!(code.contains("impl_iter_scan ! (u8 usize)"));
        assert!(!code.contains("FromRawFd"));
        assert!(!code.contains("mod dead"));
        assert!(!code.contains("struct Dead"));
    }

    #[test]
    fn test_prune_unused_support_items_prunes_unreachable_inherent_methods() {
        let binary_source = parse_str::<File>(
            r#"
            pub fn solve() {
                let _ = cp::tools::Out::keep();
            }
            "#,
        )
        .unwrap()
        .into_token_stream()
        .to_string();
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                pub use self::fastio::{DropMe, Out};
                mod fastio {
                    pub struct Out;
                    pub struct DropMe;
                    impl Out {
                        pub fn keep() -> Self {
                            Self::helper()
                        }

                        fn helper() -> Self {
                            Self
                        }

                        pub fn drop() -> Self {
                            Self
                        }
                    }
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        prune_unused_support_items(&mut file, "cp", &binary_source);
        let code = render_support_code(file, "cp", true);
        assert!(code.contains("pub use self :: fastio :: { Out }"));
        assert!(!code.contains("DropMe"));
        assert!(code.contains("pub fn keep"));
        assert!(code.contains("fn helper"));
        assert!(!code.contains("pub fn drop"));
    }

    #[test]
    fn test_prune_unused_support_items_keeps_tools_items_referenced_from_other_roots() {
        let binary_source = parse_str::<File>(
            r#"
            pub fn solve() {
                cp::data_structure::run();
            }
            "#,
        )
        .unwrap()
        .into_token_stream()
        .to_string();
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                pub use self::comparator::Less;

                mod comparator {
                    pub struct Less;
                    pub struct Unused;
                }

                mod dead {
                    pub struct Never;
                }
            }

            pub mod graph {
                use crate::tools::comparator::Less;

                pub fn build(_: Less) {}
            }

            pub mod data_structure {
                pub fn run() {
                    crate::graph::build(crate::tools::comparator::Less);
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        prune_unused_support_items(&mut file, "cp", &binary_source);
        let code = render_support_code(file, "cp", true);
        assert!(code.contains("struct Less"));
        assert!(!code.contains("struct Unused"));
        assert!(!code.contains("mod dead"));
        assert!(code.contains("use crate::tools::comparator::Less;"));
    }

    #[test]
    fn test_prune_unused_support_macros_drops_unreachable_named_macros() {
        let binary_source = parse_str::<File>(
            r#"
            pub fn solve() {
                cp::keep!();
            }
            "#,
        )
        .unwrap()
        .into_token_stream()
        .to_string();
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                mod scanner {
                    macro_rules! helper {
                        () => {};
                    }

                    helper!();

                    #[macro_export]
                    macro_rules! keep {
                        () => {
                            helper!();
                        };
                    }

                    #[macro_export]
                    macro_rules! dropme {
                        () => {};
                    }
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        prune_unused_support_macros(&mut file, &binary_source);
        let code = render_support_code(file, "cp", true);
        assert!(code.contains("macro_rules ! helper"));
        assert!(code.contains("macro_rules ! keep"));
        assert!(!code.contains("macro_rules ! dropme"));
    }

    #[test]
    fn test_prune_unused_macro_arms_keeps_only_reachable_arms() {
        let binary_source = parse_str::<File>(
            r#"
            pub fn solve() {
                cp::prepare!();
            }
            cp::main!(large_stack);
            "#,
        )
        .unwrap()
        .into_token_stream()
        .to_string();
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                mod main {
                    mod main_macros {
                        macro_rules! helper_output { () => {} }
                        macro_rules! helper_normal { () => {} }
                        macro_rules! helper_interactive { () => {} }

                        #[macro_export]
                        macro_rules! prepare {
                            (@ output ($dol:tt)) => { helper_output!(); };
                            (@ normal ($dol:tt)) => { helper_normal!(); };
                            (@ interactive ($dol:tt)) => { helper_interactive!(); };
                            () => {
                                $crate::prepare!(@ output ($));
                                $crate::prepare!(@ normal ($));
                            };
                            (?) => {
                                $crate::prepare!(@ output ($));
                                $crate::prepare!(@ interactive ($));
                            };
                        }

                        #[macro_export]
                        macro_rules! main {
                            () => {};
                            (avx2) => {};
                            (large_stack) => {};
                        }
                    }
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        prune_unused_support_macros(&mut file, &binary_source);
        prune_unused_macro_arms(&mut file, &binary_source);
        prune_unused_support_macros(&mut file, &binary_source);
        let code = render_support_code(file, "cp", true);
        assert!(code.contains("@ output"));
        assert!(code.contains("@ normal"));
        assert!(!code.contains("@ interactive"));
        assert!(!code.contains("(?)"));
        assert!(!code.contains("(avx2)"));
        assert!(!code.contains("helper_interactive"));
        assert!(code.contains("(large_stack)"));
    }

    #[test]
    fn test_prune_unused_macro_arms_prunes_scan_entry_arms() {
        let binary_source = parse_str::<File>(
            r#"
            pub fn solve() {
                cp::prepare!();
                sc!(n: usize);
            }
            "#,
        )
        .unwrap()
        .into_token_stream()
        .to_string();
        let mut file: File = parse_str(
            r#"
            pub mod tools {
                mod main {
                    mod main_macros {
                        #[macro_export]
                        macro_rules! prepare {
                            () => {
                                let mut __scanner = ();
                                macro_rules! sc {
                                    ($($t:tt)*) => {
                                        $crate::scan!(__scanner, $($t)*)
                                    };
                                }
                            };
                        }
                    }
                }

                mod scanner {
                    #[macro_export]
                    macro_rules! scan_value {
                        (src = $src:expr, $($t:tt)*) => { ($src, stringify!($($t)*)) };
                        (iter = $iter:expr, $($t:tt)*) => { ($iter, stringify!($($t)*)) };
                        ($scanner:expr, $($t:tt)*) => { ($scanner, stringify!($($t)*)) };
                    }

                    #[macro_export]
                    macro_rules! scan {
                        (src = $src:expr, $($t:tt)*) => { $crate::scan_value!(src = $src, $($t)*) };
                        (iter = $iter:expr, $($t:tt)*) => { $crate::scan_value!(iter = $iter, $($t)*) };
                        ($scanner:expr, $($t:tt)*) => { $crate::scan_value!($scanner, $($t)*) };
                    }
                }
            }
            "#,
        )
        .unwrap();
        strip_redundant_support(&mut file);
        prune_unused_macro_arms(&mut file, &binary_source);
        let code = render_support_code(file, "cp", true);
        assert!(!code.contains("(src ="));
        assert!(!code.contains("(iter ="));
        assert!(code.contains("($ scanner : expr , $ ($ t : tt) *)"));
    }

    #[test]
    fn test_compute_required_direct_modules_keeps_iter_print_for_pp_macro() {
        let file: File = parse_str(
            r#"
            mod main {
                mod main_macros {
                    #[macro_export]
                    macro_rules! prepare {
                        (@ output ($dol:tt)) => {
                            macro_rules! pp {
                                ($dol($dol t:tt)*) => {
                                    $dol crate::iter_print!($dol($dol t)*)
                                }
                            }
                        };
                        () => {
                            $crate::prepare!(@ output ($));
                        };
                    }
                }
            }
            mod iter_print {
                #[macro_export]
                macro_rules! iter_print {
                    ($($t:tt)*) => {};
                }
            }
            "#,
        )
        .unwrap();
        let entry_refs = HashSet::from([String::from("prepare")]);
        let plan = compute_required_direct_modules(
            Path::new("."),
            "tools",
            &[String::from("tools")],
            &file.items,
            &entry_refs,
        );
        assert!(plan.required_children.contains("main"));
        assert!(plan.required_children.contains("iter_print"));
    }

    #[test]
    fn test_compute_required_direct_modules_propagates_cross_root_entry_refs() {
        let file: File = parse_str(
            r#"
            pub mod algebra {
                pub use self::monoid::Monoid;
                mod monoid {
                    pub trait Monoid {}
                }
                mod dead {
                    pub trait Dead {}
                }
            }

            pub mod data_structure {
                pub struct HashCounter;

                impl HashCounter {
                    pub fn new<T: crate::algebra::Monoid>() -> Self {
                        Self
                    }
                }
            }
            "#,
        )
        .unwrap();
        let entry_refs = HashSet::from([String::from("data_structure")]);
        let plan = compute_required_direct_modules(
            Path::new("."),
            "",
            &[],
            &file.items,
            &entry_refs,
        );
        assert!(plan.required_children.contains("data_structure"));
        assert!(plan.required_children.contains("algebra"));
        assert_eq!(
            plan.child_entry_refs.get("algebra"),
            Some(&HashSet::from([String::from("Monoid")]))
        );
    }

    #[test]
    fn test_compute_required_direct_modules_resolves_glob_reexports_via_pub_use_names() {
        let file: File = parse_str(
            r#"
            mod algebra {
                pub use self::operations::*;
                mod operations {
                    pub use self::min_operation_impl::MinOperation;
                    mod min_operation_impl {
                        pub struct MinOperation;
                    }
                }
            }

            mod data_structure {
                fn build(_: crate::algebra::MinOperation) {}
            }
            "#,
        )
        .unwrap();
        let entry_refs = HashSet::from([String::from("data_structure")]);
        let plan = compute_required_direct_modules(
            Path::new("."),
            "",
            &[],
            &file.items,
            &entry_refs,
        );
        assert!(plan.required_children.contains("algebra"));
        assert_eq!(
            plan.child_entry_refs.get("algebra"),
            Some(&HashSet::from([String::from("MinOperation")]))
        );
    }

    #[test]
    fn test_file_references_selected_crate_detects_cp_paths() {
        let file: File = parse_str(
            r#"
            pub fn solve() {
                cp::prepare!();
                let _ = cp::tools::Scanner::new("1");
            }
            "#,
        )
        .unwrap();
        assert!(file_references_selected_crate(&file, "cp"));
    }
}
