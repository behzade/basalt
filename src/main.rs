use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::prelude::*;
use clap::Parser as ClapParser;
use std::fs;
use std::io::{self, Read};

// --- Module Declarations ---
mod ast;
mod ast_owned;
mod compiler;
mod hir;
mod hir_validation;
mod interpreter;
mod lexer;
mod parser;
mod token;
mod type_unifier;
mod typechecker;

use crate::compiler::{Compiler, CompilerStage};
use crate::{
    hir::Item,
    lexer::lexer,
    parser::file_parser,
    token::{SimpleSpan, Token},
};

// --- Command-Line Interface ---

#[derive(ClapParser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand)]
enum Action {
    Ast {
        path: String,
    },
    Hir {
        path: String,
    },
    Mir {
        path: String,
    },
    Build {
        path: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    Run {
        path: String,
    },
}

impl Action {
    /// Returns the target pipeline stage for this action.
    fn target_stage(&self) -> CompilerStage {
        match self {
            Action::Ast { .. } => CompilerStage::Parse,
            Action::Hir { .. } => CompilerStage::Hir,
            Action::Mir { .. } => CompilerStage::Mir,
            Action::Build { .. } => CompilerStage::Build,
            // For run, we need HIR available; we'll invoke interpreter after run_until
            Action::Run { .. } => CompilerStage::Hir,
        }
    }

    /// Returns the input file path for this action.
    fn path(&self) -> &str {
        match self {
            Action::Ast { path }
            | Action::Hir { path }
            | Action::Mir { path }
            | Action::Build { path, .. }
            | Action::Run { path } => path,
        }
    }
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let target_stage = cli.action.target_stage();
    let path = cli.action.path();
    let output_path = if let Action::Build { output, .. } = &cli.action {
        output.clone()
    } else {
        None
    };

    let mut compiler = Compiler::new(path.to_string(), output_path);

    if let Err(e) = compiler.run_until(target_stage) {
        eprintln!("\n❌ Compilation failed: {}", e);
        // Use a non-zero exit code to indicate failure to shell scripts
        std::process::exit(1);
    } else {
        // Only print for explicit actions, not for Run which shares the HIR stage
        match &cli.action {
            Action::Ast { .. } => {
                let json_output = serde_json::to_string_pretty(&compiler.workspace.ast).unwrap();
                println!("{}", json_output);
            }
            Action::Hir { .. } => {
                let json_output = serde_json::to_string_pretty(&compiler.workspace.hir).unwrap();
                println!("{}", json_output);
            }
            _ => {}
        }

        if matches!(cli.action, Action::Run { .. }) {
            compiler
                .run_interpreter()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            if let Some(val) = compiler.workspace.last_run_result.as_ref() {
                let code = crate::interpreter::value_to_exit_code(val);
                std::process::exit(code);
            }
        }
    }

    Ok(())
}
