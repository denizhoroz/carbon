//! Extension grammars: tree-sitter grammars compiled to WebAssembly, run by
//! tree-sitter's own wasmtime (the `wasm` feature) and handed to the kit's
//! highlighter through `LanguageRegistry` parser factories.
//!
//! Compiling a grammar takes ~25 ms (Rust grammar, release build, measured
//! 2026-10-23); it starts on its own thread when the grammar is registered,
//! and a parser asked for earlier waits for it. Every parser gets a fresh
//! `WasmStore` (~8 ms) with the compiled language. Parsing is ~1.5x slower than a native grammar on a
//! full parse, about the same on the small reparse after an edit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use anyhow::{Context as _, anyhow};
use carbon_core::extension::{GrammarDef, Loaded, Registry};
use gpui_kit::SharedString;
use gpui_kit::component::highlighter::{GrammarConfig, LanguageRegistry};
use tree_sitter::wasmtime::Engine;
use tree_sitter::{Language, Parser, WasmStore};

/// The kit's language for plain text: no colours.
pub const PLAIN: &str = "text";

/// Stack of the threads that compile grammars.
const COMPILE_STACK: usize = 16 * 1024 * 1024;

/// One wasmtime engine for every grammar (compiled code is tied to it).
static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::default);

/// Grammars registered with the kit, by name, and the `.wasm` each came
/// from (a reload re-registers only those that changed).
static REGISTERED: LazyLock<Mutex<HashMap<String, PathBuf>>> = LazyLock::new(Mutex::default);

/// Registers the active extensions' grammars with the kit and removes the
/// ones no longer offered.
pub fn sync(registry: &Registry) {
    let wanted: Vec<(&Loaded, &GrammarDef)> = registry.grammars();
    let mut registered = REGISTERED.lock().unwrap();
    let kit = LanguageRegistry::singleton();
    registered.retain(|name, _| {
        let keep = wanted.iter().any(|(_, grammar)| &grammar.name == name);
        if !keep {
            kit.unregister(name);
        }
        keep
    });
    for (loaded, grammar) in wanted {
        let wasm = match loaded.file(&grammar.wasm) {
            Ok(path) => path,
            Err(err) => {
                eprintln!("grammar {}: {err}", grammar.name);
                continue;
            }
        };
        if registered.get(&grammar.name) == Some(&wasm) {
            continue;
        }
        match register(loaded, grammar, &wasm) {
            Ok(()) => {
                registered.insert(grammar.name.clone(), wasm);
            }
            Err(err) => eprintln!("grammar {} ({}): {err:#}", grammar.name, loaded.def.id),
        }
    }
}

fn read_query(loaded: &Loaded, path: Option<&str>) -> anyhow::Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };
    let file = loaded.file(path).map_err(|err| anyhow!(err))?;
    std::fs::read_to_string(&file).with_context(|| file.display().to_string())
}

fn register(loaded: &Loaded, grammar: &GrammarDef, wasm: &Path) -> anyhow::Result<()> {
    let config = GrammarConfig {
        name: grammar.name.clone().into(),
        // The factory supplies it (compiled on first use).
        language: None,
        injection_languages: Vec::new(),
        highlights: read_query(loaded, Some(&grammar.highlights))?.into(),
        injections: read_query(loaded, grammar.injections.as_deref())?.into(),
        locals: read_query(loaded, grammar.locals.as_deref())?.into(),
    };
    let kit = LanguageRegistry::singleton();
    kit.register(&grammar.name, &config);
    let name = grammar.name.clone();
    let compiled: Arc<OnceLock<Result<Language, String>>> = Arc::default();
    // Compile off the UI thread right away (wasmtime's compiler recurses
    // deeply in debug builds); a parser asked for before it is done waits.
    {
        let (name, wasm, compiled) = (name.clone(), wasm.to_path_buf(), compiled.clone());
        std::thread::Builder::new()
            .name(format!("grammar-{name}"))
            .stack_size(COMPILE_STACK)
            .spawn(move || {
                compiled.get_or_init(|| compile(&name, &wasm).map_err(|err| format!("{err:#}")));
            })?;
    }
    kit.register_parser_factory(
        &grammar.name,
        Arc::new(move || {
            let language = compiled
                .wait()
                .clone()
                .map_err(|err| anyhow!("grammar {name}: {err}"))?;
            let store = WasmStore::new(&ENGINE).map_err(|err| anyhow!("{}", err.message))?;
            let mut parser = Parser::new();
            parser.set_wasm_store(store)?;
            Ok((parser, language))
        }),
    );
    Ok(())
}

/// Compiles a grammar's `.wasm`; it exports `tree_sitter_<name>`.
fn compile(name: &str, wasm: &Path) -> anyhow::Result<Language> {
    let bytes = std::fs::read(wasm).with_context(|| wasm.display().to_string())?;
    let mut store = WasmStore::new(&ENGINE).map_err(|err| anyhow!("{}", err.message))?;
    store
        .load_language(name, &bytes)
        .map_err(|err| anyhow!("{}: {}", wasm.display(), err.message))
}

/// The kit language for a file: its extension language's grammar, else
/// JSON for `.json` files (the kit always has that grammar), else plain text.
pub fn highlight_language(registry: &Registry, path: &Path) -> SharedString {
    if let Some((_, language)) = registry.language_for_path(path) {
        if let Some(grammar) = &language.grammar
            && REGISTERED.lock().unwrap().contains_key(grammar)
        {
            return grammar.clone().into();
        }
        return PLAIN.into();
    }
    let json = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ["json", "jsonc", "json5"].contains(&ext.to_ascii_lowercase().as_str()));
    if json { "json" } else { PLAIN }.into()
}
