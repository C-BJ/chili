use chili_ast::{
    ast, compiler_info,
    path::{resolve_relative_path, try_resolve_relative_path, RelativeTo},
    workspace::{ModuleInfo, Workspace},
};
use chili_error::SyntaxError;
use chili_parse::{spawn_parser, ParserCache, ParserResult};
use chili_span::Span;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{mpsc::channel, Arc, Mutex},
};
use ustr::{ustr, Ustr, UstrMap};

#[derive(Debug, Clone, Copy)]
pub struct AstGenerationStats {
    pub total_lines: u32,
}

pub type AstGenerationResult = Option<(Vec<ast::Ast>, AstGenerationStats)>;

pub fn generate_ast(workspace: &mut Workspace) -> AstGenerationResult {
    let mut asts: Vec<ast::Ast> = vec![];

    let root_file_path =
        try_resolve_relative_path(&workspace.build_options.source_file, RelativeTo::Cwd, None)
            .map_err(|diag| workspace.diagnostics.push(diag))
            .ok()?;

    let stats = generate_ast_inner(workspace, root_file_path, &mut asts);

    // Add all module_infos to the workspace
    for ast in asts.iter_mut() {
        ast.module_id = workspace.add_module_info(ast.module_info);

        if ast.module_info.name == compiler_info::root_module_name() {
            workspace.root_module_id = ast.module_id;
        }
    }

    // Apply module ids to top level expressions, and check for duplicate globals
    for ast in asts.iter_mut() {
        let mut defined_symbols = UstrMap::<Span>::default();

        ast.bindings.iter_mut().for_each(|binding| {
            binding.module_id = ast.module_id;
            for pat in binding.pattern.iter() {
                check_duplicate_global_symbol(
                    workspace,
                    &mut defined_symbols,
                    pat.symbol,
                    pat.span,
                );
            }
        });
    }

    Some((asts, stats))
}

fn generate_ast_inner(
    workspace: &mut Workspace,
    root_file_path: PathBuf,
    asts: &mut Vec<ast::Ast>,
) -> AstGenerationStats {
    let (tx, rx) = channel::<Box<ParserResult>>();

    let cache = Arc::new(Mutex::new(ParserCache {
        root_file: resolve_relative_path(&workspace.build_options.source_file, RelativeTo::Cwd)
            .unwrap(),
        root_dir: workspace.root_dir.clone(),
        std_dir: workspace.std_dir.clone(),
        diagnostics: workspace.diagnostics.clone(),
        parsed_modules: HashSet::<ModuleInfo>::new(),
        total_lines: 0,
    }));

    let root_module_info = ModuleInfo::new(
        compiler_info::root_module_name(),
        ustr(&root_file_path.to_str().unwrap().to_string()),
    );

    spawn_parser(
        tx.clone(),
        Arc::clone(&cache),
        compiler_info::std_module_info(),
    );

    spawn_parser(tx, Arc::clone(&cache), root_module_info);

    for result in rx.iter() {
        match *result {
            ParserResult::NewAst(ast) => asts.push(ast),
            ParserResult::AlreadyParsed => (),
            ParserResult::Failed(diag) => cache.lock().unwrap().diagnostics.push(diag),
        }
    }

    let cache = Arc::try_unwrap(cache).unwrap().into_inner().unwrap();

    workspace.diagnostics = cache.diagnostics;

    AstGenerationStats {
        total_lines: cache.total_lines,
    }
}

fn check_duplicate_global_symbol(
    workspace: &mut Workspace,
    defined_symbols: &mut UstrMap<Span>,
    symbol: Ustr,
    span: Span,
) {
    if let Some(already_defined_span) = defined_symbols.get(&symbol) {
        workspace.diagnostics.push(SyntaxError::duplicate_symbol(
            *already_defined_span,
            span,
            symbol,
        ));
    } else {
        defined_symbols.insert(symbol, span);
    }
}
