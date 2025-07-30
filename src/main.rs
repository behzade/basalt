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
mod lexer;
mod parser;
mod token;

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
    /// Parse a file and print the AST
    Parse {
        path: String,
    },
    // Parse a file and collect all the imported modules
    Resolve {
        path: String,
    },
    /// Type-check and generate HIR
    Hir {
        path: String,
    },
    /// Generate MIR from HIR
    Mir {
        path: String,
    },
    /// Compile to WebAssembly (.wasm file)
    Build {
        path: String,
        #[arg(short, long)]
        output: Option<String>,
    },
}

impl Action {
    /// Returns the target pipeline stage for this action.
    fn target_stage(&self) -> CompilerStage {
        match self {
            Action::Parse { .. } => CompilerStage::Parse,
            Action::Resolve { .. } => CompilerStage::Resolve,
            Action::Hir { .. } => CompilerStage::Hir,
            Action::Mir { .. } => CompilerStage::Mir,
            Action::Build { .. } => CompilerStage::Build,
        }
    }

    /// Returns the input file path for this action.
    fn path(&self) -> &str {
        match self {
            Action::Parse { path }
            | Action::Resolve { path }
            | Action::Hir { path }
            | Action::Mir { path }
            | Action::Build { path, .. } => path,
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
        println!("\n✅ Pipeline finished successfully.");
    }

    Ok(())
}
